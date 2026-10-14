package io.juspay.sudoquery.internal

import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.IOException
import java.net.ConnectException
import kotlin.random.Random

class UploaderTest {
    @get:Rule
    val temp = TemporaryFolder()

    private val clock = FakeClock()
    private val logger = RecordingLogger()
    private val transport = FakeTransport()
    private val store by lazy {
        QueueStore(temp.newFolder("queue"), maxEventsPerBatch = 2, clock = clock, logger = logger).also { it.recover() }
    }
    private val uploader by lazy { Uploader(store, transport, clock, logger, timezone = { "Asia/Kolkata" }, random = Random(1)) }

    /** Commits [count] batches of two events each, numbered from 1. */
    private fun queueBatches(count: Int) {
        repeat(count * 2) { store.append("""{"n":${it + 1}}""".toByteArray()) }
    }

    private fun sentNumbers(request: Int) = transport.events(request).map { it.getValue("n").jsonPrimitive.content.toInt() }

    @Test
    fun `sends batches oldest first and deletes them`() {
        queueBatches(2)
        uploader.drain()

        assertEquals(listOf(1, 2), sentNumbers(0))
        assertEquals(listOf(3, 4), sentNumbers(1))
        assertEquals(0, store.readyBatches().size)
        assertFalse(uploader.isBackingOff)
    }

    @Test
    fun `the batch carries the timezone at batch level`() {
        queueBatches(1)
        uploader.drain()
        val body = Json.parseToJsonElement(transport.bodies.single()).jsonObject
        assertEquals("""{"geo":null,"timezone":"Asia/Kolkata"}""", body.getValue("system_properties").toString())
    }

    @Test
    fun `any 2xx counts as delivered`() {
        queueBatches(2)
        transport.reply(202)
        transport.reply(204, body = "")
        uploader.drain()
        assertEquals(0, store.readyBatches().size)
    }

    @Test
    fun `a rejected batch is dropped and the queue moves on`() {
        for (code in listOf(400, 413, 422)) {
            queueBatches(2)
            transport.reply(code, body = """{"error":"missing field `anon_id`"}""")
            uploader.drain()
            assertEquals("HTTP $code", 0, store.readyBatches().size)
        }
        assertEquals(6, transport.bodies.size)
        assertTrue(logger.errors.first().contains("missing field `anon_id`"))
        assertFalse(uploader.isBackingOff)
    }

    @Test
    fun `other failures keep the batch, stop the queue and back off`() {
        for (code in listOf(301, 401, 403, 404, 408, 429, 500, 502, 503)) {
            uploader.resetBackoff()
            queueBatches(1)
            val before = transport.bodies.size
            transport.reply(code)
            uploader.drain()

            assertEquals("HTTP $code", before + 1, transport.bodies.size)
            assertTrue("HTTP $code", uploader.isBackingOff)
            uploader.resetBackoff()
            uploader.drain() // delivered on the next attempt
            assertEquals("HTTP $code", 0, store.readyBatches().size)
        }
    }

    @Test
    fun `a network error keeps the batch and stops the queue`() {
        queueBatches(2)
        transport.failWith(ConnectException("Connection refused"))
        uploader.drain()

        assertEquals(1, transport.bodies.size)
        assertEquals(2, store.readyBatches().size)
        assertTrue(uploader.isBackingOff)
        assertTrue(logger.warnings.single().contains("Could not reach the collector"))
    }

    @Test
    fun `a resend is byte-for-byte the same request`() {
        queueBatches(1)
        transport.reply(502)
        uploader.drain()
        uploader.resetBackoff()
        uploader.drain()
        assertEquals(2, transport.bodies.size)
        assertEquals(transport.bodies[0], transport.bodies[1])
    }

    @Test
    fun `backoff doubles from 5 s up to 5 min, half of it randomized`() {
        queueBatches(1)
        val delays = (1..10).map {
            transport.failWith(IOException("offline"))
            clock.elapsed = 0
            uploader.drain()
            waitTime()
        }
        val ceilings = listOf(5_000L, 10_000, 20_000, 40_000, 80_000, 160_000, 300_000, 300_000, 300_000, 300_000)
        delays.zip(ceilings).forEach { (delay, ceiling) -> assertTrue("$delay vs $ceiling", delay in ceiling / 2..ceiling) }
    }

    @Test
    fun `Retry-After is honoured`() {
        queueBatches(1)
        transport.reply(429, retryAfterSeconds = 120)
        uploader.drain()
        assertTrue(waitTime() >= 120_000)
    }

    @Test
    fun `an empty batch file is deleted without a request`() {
        store.append("""{"n":1}""".toByteArray())
        store.commit()
        val file = store.readyBatches().single()
        file.writeText("")
        uploader.drain()
        assertEquals(0, transport.bodies.size)
        assertEquals(0, store.readyBatches().size)
    }

    @OptIn(ExperimentalCoroutinesApi::class)
    @Test
    fun `run retries by itself once the backoff ends`() = runTest {
        val virtualClock = object : Clock {
            override fun nowMillis() = T0
            override fun elapsedMillis() = testScheduler.currentTime
        }
        val uploader = Uploader(store, transport, virtualClock, logger, timezone = { "UTC" }, random = Random(1))
        backgroundScope.launch { uploader.run() }

        queueBatches(1)
        transport.reply(503)
        uploader.requestUpload()
        runCurrent()
        assertEquals(1, transport.bodies.size)
        assertEquals(1, store.readyBatches().size)

        uploader.requestUpload() // ignored while backing off
        runCurrent()
        assertEquals(1, transport.bodies.size)

        advanceTimeBy(5_001)
        runCurrent()
        assertEquals(2, transport.bodies.size)
        assertEquals(0, store.readyBatches().size)
    }

    @OptIn(ExperimentalCoroutinesApi::class)
    @Test
    fun `resetBackoff lets a request through at once`() = runTest {
        val virtualClock = object : Clock {
            override fun nowMillis() = T0
            override fun elapsedMillis() = testScheduler.currentTime
        }
        val uploader = Uploader(store, transport, virtualClock, logger, timezone = { "UTC" }, random = Random(1))
        backgroundScope.launch { uploader.run() }

        queueBatches(1)
        transport.reply(503)
        uploader.requestUpload()
        runCurrent()
        uploader.resetBackoff()
        uploader.requestUpload()
        runCurrent()
        assertEquals(2, transport.bodies.size)
        assertEquals(0, store.readyBatches().size)
    }

    /** How long the uploader will wait before retrying, read through its public backoff check. */
    private fun waitTime(): Long {
        val start = clock.elapsed
        var low = 0L
        var high = 2 * 60 * 60 * 1000L
        while (low < high) {
            val mid = (low + high) / 2
            clock.elapsed = start + mid
            if (uploader.isBackingOff) low = mid + 1 else high = mid
        }
        clock.elapsed = start
        return low
    }
}
