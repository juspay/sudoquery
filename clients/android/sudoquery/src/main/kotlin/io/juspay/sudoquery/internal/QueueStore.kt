package io.juspay.sudoquery.internal

import kotlinx.serialization.SerializationException
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonElement
import java.io.File
import java.io.FileOutputStream
import java.io.IOException
import java.util.Locale

/**
 * The on-disk event queue, ported from the file design of hyper-sdk-android's
 * `LogSessioniserExp` / `LogPusherExp`.
 *
 * Serialized events are appended as NDJSON lines to one open batch file in `staging/`. When the
 * batch is full (or flushed) it is committed: closed and renamed into `ready/`, after which it is
 * never modified. The uploader sends `ready/` files oldest first and deletes each once delivered.
 *
 * Files are named `<seq>-<createdMillis>.ndjson`. Order comes from the sequence number, so a
 * wall-clock change can't reorder the queue; the creation time is only used for expiry.
 *
 * [recover], [append], [commit] and [enforceLimits] belong to the writer coroutine.
 * [readyBatches], [read] and [delete] are also called by the uploader: committed files never
 * change and deleting one twice is harmless.
 */
internal class QueueStore(
    root: File,
    private val maxEventsPerBatch: Int,
    private val clock: Clock,
    private val logger: Logger,
    private val maxBytesPerBatch: Long = MAX_BATCH_BYTES,
    private val maxQueueBytes: Long = MAX_QUEUE_BYTES,
    private val maxAgeMillis: Long = MAX_AGE_MILLIS,
) {
    private val stagingDir = File(root, "staging")
    private val readyDir = File(root, "ready")
    private var nextSeq = 1L
    private var open: OpenBatch? = null

    private class OpenBatch(val file: File, val stream: FileOutputStream) {
        var events = 0
        var bytes = 0L
    }

    /** Commits any batch a previous process left open, and picks the next sequence number. Call once, first. */
    fun recover() {
        stagingDir.mkdirs()
        readyDir.mkdirs()
        var recovered = 0
        for (file in stagingDir.listFiles().orEmpty()) {
            if (parseName(file.name) == null || file.length() == 0L) {
                file.delete()
            } else {
                moveToReady(file)
                recovered++
            }
        }
        if (recovered > 0) logger.debug("Recovered $recovered unsent batch(es) from the previous run")
        nextSeq = (readyDir.listFiles().orEmpty().mapNotNull { parseName(it.name)?.seq }.maxOrNull() ?: 0) + 1
    }

    /**
     * Appends one serialized event (without a trailing newline) with a single unbuffered write, so it
     * reaches the file system even if the process dies right after. Returns true if a batch was
     * committed, i.e. there is something new to upload.
     */
    fun append(event: ByteArray): Boolean {
        var committed = false
        val current = open
        if (current != null && current.bytes + event.size + 1 > maxBytesPerBatch) committed = commit()

        val batch = open ?: openBatch()
        val record = event.copyOf(event.size + 1)
        record[event.size] = '\n'.code.toByte()
        try {
            batch.stream.write(record)
        } catch (e: IOException) {
            // Keep the events already written; the reader skips the partial line.
            commit()
            throw e
        }
        batch.events++
        batch.bytes += record.size

        if (batch.events >= maxEventsPerBatch) committed = commit() || committed
        return committed
    }

    /** Closes the open batch and moves it to `ready/`. Returns true if it held any events. */
    fun commit(): Boolean {
        val batch = open ?: return false
        open = null
        try {
            batch.stream.close()
        } catch (e: IOException) {
            logger.warn("Could not close ${batch.file.name}", e)
        }
        if (batch.events == 0) {
            batch.file.delete()
            return false
        }
        moveToReady(batch.file)
        return true
    }

    /** Committed batches, oldest first. */
    fun readyBatches(): List<File> =
        readyDir.listFiles().orEmpty()
            .mapNotNull { file -> parseName(file.name)?.let { file to it.seq } }
            .sortedBy { it.second }
            .map { it.first }

    /**
     * The events of a committed batch, or null if the file is gone. Lines that aren't valid JSON,
     * such as a write cut short by a crash, are skipped.
     */
    fun read(file: File): List<JsonElement>? {
        val text = try {
            file.readText(Charsets.UTF_8)
        } catch (e: IOException) {
            if (file.exists()) logger.warn("Could not read ${file.name}", e)
            return null
        }
        val events = ArrayList<JsonElement>()
        var skipped = 0
        for (line in text.lineSequence()) {
            if (line.isBlank()) continue
            try {
                events += Json.parseToJsonElement(line)
            } catch (e: SerializationException) {
                skipped++
            }
        }
        if (skipped > 0) logger.warn("Skipped $skipped unreadable event(s) in ${file.name}")
        return events
    }

    fun delete(file: File) {
        if (!file.delete() && file.exists()) logger.warn("Could not delete ${file.name}")
    }

    /**
     * Drops batches older than the maximum age, then the oldest batches while the queue is over its
     * size limit, down to 80 % of it. Unlike hyper-sdk's `checkFolderLimit`, a file's size is taken
     * before it is deleted.
     */
    fun enforceLimits() {
        val now = clock.nowMillis()
        var expired = 0
        val kept = ArrayList<Pair<File, Long>>()
        for (file in readyBatches()) {
            val created = parseName(file.name)?.createdMillis ?: continue
            // A batch dated in the future (the clock moved back) counts as fresh.
            if (now - created > maxAgeMillis) {
                if (file.delete()) expired++
            } else {
                kept += file to file.length()
            }
        }

        var total = kept.sumOf { it.second }
        var trimmed = 0
        if (total > maxQueueBytes) {
            val target = maxQueueBytes / 10 * 8
            for ((file, size) in kept) {
                if (total <= target) break
                if (file.delete()) {
                    total -= size
                    trimmed++
                }
            }
        }

        if (expired > 0) logger.warn("Dropped $expired unsent batch(es) older than ${maxAgeMillis / MILLIS_PER_HOUR} h")
        if (trimmed > 0) logger.warn("Dropped the $trimmed oldest unsent batch(es) to keep the queue under $maxQueueBytes bytes")
    }

    private fun openBatch(): OpenBatch {
        stagingDir.mkdirs()
        val file = File(stagingDir, fileName(nextSeq++, maxOf(0, clock.nowMillis())))
        return OpenBatch(file, FileOutputStream(file, true)).also { open = it }
    }

    private fun moveToReady(file: File) {
        readyDir.mkdirs()
        if (!file.renameTo(File(readyDir, file.name))) logger.warn("Could not move ${file.name} to the upload queue")
    }

    internal class BatchName(val seq: Long, val createdMillis: Long)

    internal companion object {
        private const val MILLIS_PER_HOUR = 60L * 60 * 1000
        const val MAX_BATCH_BYTES: Long = 512L * 1024
        const val MAX_QUEUE_BYTES: Long = 10L * 1024 * 1024
        const val MAX_AGE_MILLIS: Long = 7L * 24 * MILLIS_PER_HOUR
        private val NAME = Regex("""(\d{1,18})-(\d{1,18})\.ndjson""")

        fun fileName(seq: Long, createdMillis: Long): String =
            String.format(Locale.ROOT, "%010d-%d.ndjson", seq, createdMillis)

        fun parseName(name: String): BatchName? {
            val match = NAME.matchEntire(name) ?: return null
            return BatchName(match.groupValues[1].toLong(), match.groupValues[2].toLong())
        }
    }
}
