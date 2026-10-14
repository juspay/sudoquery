package io.juspay.sudoquery.internal

import io.juspay.sudoquery.BatchPayload
import io.juspay.sudoquery.SystemProperties
import io.juspay.sudoquery.WireJson
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.withTimeoutOrNull
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.jsonPrimitive
import java.io.File
import java.io.IOException
import java.util.TimeZone
import kotlin.random.Random

/** What the event writer needs from the uploader. */
internal interface UploadControl {
    /** Asks for queued batches to be sent. Cheap; requests made while one is pending collapse into it. */
    fun requestUpload()

    /** Forgets the retry backoff, e.g. when the app comes back to the foreground. */
    fun resetBackoff()

    /** True while uploads are failing and waiting to retry. */
    val isBackingOff: Boolean
}

/** How the SDK treats a collector response. */
internal enum class Delivery {
    /** Delivered: delete the batch. */
    SENT,

    /** The collector will never accept this batch: delete it and move on. */
    REJECTED,

    /** Keep the batch and try again later. */
    RETRY,
}

/**
 * 400 (bad headers or body), 413 (too large) and 422 can't succeed on a resend. Everything else
 * (redirects, 401/403/404 configuration problems, 408, 429, 5xx) may, so the batch is kept.
 */
internal fun classify(code: Int): Delivery = when (code) {
    in 200..299 -> Delivery.SENT
    400, 413, 422 -> Delivery.REJECTED
    else -> Delivery.RETRY
}

/**
 * Sends committed batches to the collector, one file per request, oldest first. Ported from
 * hyper-sdk-android's `LogPusherExp`, but any 2xx counts as delivered, rejected batches are
 * dropped instead of blocking the queue, and failures back off exponentially with jitter instead
 * of retrying forever at a fixed interval.
 *
 * Only one upload runs at a time: [run] is the single consumer of upload requests.
 */
internal class Uploader(
    private val store: QueueStore,
    private val transport: Transport,
    private val clock: Clock,
    private val logger: Logger,
    private val timezone: () -> String? = { TimeZone.getDefault().id },
    private val random: Random = Random.Default,
) : UploadControl {
    private val requests = Channel<Unit>(Channel.CONFLATED)

    @Volatile
    private var retryAtMillis = 0L

    @Volatile
    private var failures = 0

    override fun requestUpload() {
        requests.trySend(Unit)
    }

    override fun resetBackoff() {
        failures = 0
        retryAtMillis = 0
    }

    override val isBackingOff: Boolean
        get() = clock.elapsedMillis() < retryAtMillis

    /** Runs for the life of the SDK: waits for a request, or for a backoff to end, then drains the queue. */
    suspend fun run() {
        while (true) {
            val wait = retryAtMillis - clock.elapsedMillis()
            if (wait > 0) {
                // Sleep out the backoff. A request can wake us early, which only matters after resetBackoff().
                withTimeoutOrNull(wait) { requests.receive() }
                if (isBackingOff) continue
            } else {
                requests.receive()
            }
            try {
                drain()
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                logger.error("Upload failed unexpectedly", e)
            }
        }
    }

    /** Sends ready batches until the queue is empty or a batch has to be retried later. */
    internal fun drain() {
        val attempted = HashSet<String>()
        while (true) {
            val batches = store.readyBatches().filter { it.name !in attempted }
            if (batches.isEmpty()) return
            for (file in batches) {
                attempted += file.name
                // Stop at the first batch to retry, so the queue stays in order.
                if (!send(file)) return
            }
        }
    }

    /** Returns false if the batch has to be retried later. */
    private fun send(file: File): Boolean {
        val events = store.read(file) ?: return true
        if (events.isEmpty()) {
            store.delete(file)
            return true
        }
        val payload = BatchPayload(events, SystemProperties(geo = null, timezone = timezone()))
        val body = WireJson.encodeToString(BatchPayload.serializer(), payload).toByteArray(Charsets.UTF_8)

        val response = try {
            transport.post(body)
        } catch (e: IOException) {
            return retryLater("Could not reach the collector (${e.javaClass.simpleName}: ${e.message})", null)
        } catch (e: RuntimeException) {
            // E.g. a custom header value HttpURLConnection refuses.
            logger.error("Could not send events", e)
            return retryLater("Could not send events", null)
        }

        return when (classify(response.code)) {
            Delivery.SENT -> {
                store.delete(file)
                failures = 0
                logger.debug("Sent ${events.size} event(s)")
                true
            }
            Delivery.REJECTED -> {
                store.delete(file)
                logger.error(
                    "The collector rejected ${events.size} event(s) with HTTP ${response.code}, dropping them: " +
                        response.errorText(),
                )
                true
            }
            Delivery.RETRY -> retryLater("The collector answered HTTP ${response.code}", response.retryAfterSeconds)
        }
    }

    private fun retryLater(reason: String, retryAfterSeconds: Long?): Boolean {
        failures++
        val delay = backoffMillis(failures, retryAfterSeconds)
        retryAtMillis = clock.elapsedMillis() + delay
        val message = "$reason; events are kept and retried in ${(delay + 999) / 1000} s"
        if (failures == 1) logger.warn(message) else logger.debug(message)
        return false
    }

    /** 5 s doubling to 5 min, with half of it randomized so many devices don't retry in step. */
    private fun backoffMillis(failures: Int, retryAfterSeconds: Long?): Long {
        val exponential = (BASE_BACKOFF_MILLIS shl (failures - 1).coerceAtMost(16)).coerceAtMost(MAX_BACKOFF_MILLIS)
        val jittered = exponential / 2 + random.nextLong(exponential / 2 + 1)
        val requested = (retryAfterSeconds ?: 0).coerceIn(0, MAX_RETRY_AFTER_SECONDS) * 1000
        return maxOf(jittered, requested)
    }

    /** The collector's `{"error": "..."}` message, or the raw body (e.g. a 413 is plain text). */
    private fun HttpResult.errorText(): String {
        val text = body?.trim().orEmpty()
        val message = try {
            (Json.parseToJsonElement(text) as? JsonObject)?.get("error")?.jsonPrimitive?.contentOrNull
        } catch (e: Exception) {
            null
        }
        return (message ?: text).take(300)
    }

    internal companion object {
        const val BASE_BACKOFF_MILLIS: Long = 5_000
        const val MAX_BACKOFF_MILLIS: Long = 5 * 60 * 1000
        const val MAX_RETRY_AFTER_SECONDS: Long = 60 * 60
    }
}
