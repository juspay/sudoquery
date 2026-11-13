package io.juspay.sudoquery.internal

import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

class QueueStoreTest {
    @get:Rule
    val temp = TemporaryFolder()

    private val clock = FakeClock()
    private val logger = RecordingLogger()
    private val root by lazy { temp.newFolder("queue") }

    private fun store(
        maxEvents: Int = 3,
        maxBytes: Long = QueueStore.MAX_BATCH_BYTES,
        maxQueueBytes: Long = QueueStore.MAX_QUEUE_BYTES,
    ) = QueueStore(root, maxEvents, clock, logger, maxBytes, maxQueueBytes).also { it.recover() }

    private fun event(n: Int) = """{"n":$n}""".toByteArray()

    private fun QueueStore.numbers(file: File) = read(file)!!.map { it.jsonObject.getValue("n").jsonPrimitive.content.toInt() }

    @Test
    fun `a batch is committed when it reaches the event limit`() {
        val store = store(maxEvents = 3)
        assertFalse(store.append(event(1)))
        assertFalse(store.append(event(2)))
        assertTrue(store.append(event(3)))
        assertFalse(store.append(event(4)))

        val ready = store.readyBatches()
        assertEquals(1, ready.size)
        assertEquals(listOf(1, 2, 3), store.numbers(ready.single()))
        assertEquals(1, File(root, "staging").listFiles()!!.size)
    }

    @Test
    fun `a batch is committed before it would exceed the byte limit`() {
        val store = store(maxEvents = 100, maxBytes = 20)
        store.append(event(1)) // 8 bytes with the newline
        store.append(event(2)) // 16
        assertTrue(store.append(event(3))) // would be 24: commits the first two, starts a new batch
        store.commit()
        assertEquals(listOf(listOf(1, 2), listOf(3)), store.readyBatches().map { store.numbers(it) })
    }

    @Test
    fun `commit returns false when nothing was appended`() {
        val store = store()
        assertFalse(store.commit())
        store.append(event(1))
        assertTrue(store.commit())
        assertFalse(store.commit())
    }

    @Test
    fun `recover commits batches a previous process left open and keeps the sequence going`() {
        val first = store(maxEvents = 10)
        first.append(event(1))
        first.append(event(2))
        // The process dies here: the batch is still in staging/. Add an empty and a foreign file too.
        File(root, "staging/0000000099-1.ndjson").createNewFile()
        File(root, "staging/notes.txt").writeText("x")

        val second = store(maxEvents = 10)
        val ready = second.readyBatches()
        assertEquals(listOf(listOf(1, 2)), ready.map { second.numbers(it) })
        assertEquals(0, File(root, "staging").listFiles()!!.size)

        second.append(event(3))
        second.commit()
        assertEquals(listOf(listOf(1, 2), listOf(3)), second.readyBatches().map { second.numbers(it) })
    }

    @Test
    fun `lines cut short by a crash are skipped`() {
        val store = store()
        val file = File(root, "ready/${QueueStore.fileName(1, T0)}")
        file.writeText("{\"n\":1}\n{\"n\":2}\n{\"n\":3, \"cut")
        assertEquals(listOf(1, 2), store.numbers(file))
        assertTrue(logger.warnings.single().contains("Skipped 1"))
    }

    @Test
    fun `read returns null for a batch that is gone`() {
        assertNull(store().read(File(root, "ready/missing.ndjson")))
    }

    @Test
    fun `batches are ordered by sequence, not by creation time`() {
        val store = store(maxEvents = 1)
        clock.now = T0 + 10_000
        store.append(event(1))
        clock.now = T0 // the clock moved back
        store.append(event(2))
        assertEquals(listOf(listOf(1), listOf(2)), store.readyBatches().map { store.numbers(it) })
    }

    @Test
    fun `expired batches are dropped and future-dated ones kept`() {
        val store = store()
        val ready = File(root, "ready")
        File(ready, QueueStore.fileName(1, T0 - QueueStore.MAX_AGE_MILLIS - 1)).writeText("{\"n\":1}\n")
        File(ready, QueueStore.fileName(2, T0 - 1000)).writeText("{\"n\":2}\n")
        File(ready, QueueStore.fileName(3, T0 + 86_400_000)).writeText("{\"n\":3}\n")

        store.enforceLimits()

        assertEquals(listOf(listOf(2), listOf(3)), store.readyBatches().map { store.numbers(it) })
        assertTrue(logger.warnings.single().contains("older than"))
    }

    @Test
    fun `an oversized queue drops its oldest batches down to 80 percent`() {
        val store = store(maxEvents = 1, maxQueueBytes = 100)
        repeat(10) { store.append("""{"pad":"${"x".repeat(9)}"}""".toByteArray()) } // 10 batches of 20 bytes = 200

        store.enforceLimits()

        val remaining = store.readyBatches()
        assertEquals(4, remaining.size) // 80 bytes <= 80 % of 100
        assertEquals(listOf(7L, 8L, 9L, 10L), remaining.map { QueueStore.parseName(it.name)!!.seq })
    }
}
