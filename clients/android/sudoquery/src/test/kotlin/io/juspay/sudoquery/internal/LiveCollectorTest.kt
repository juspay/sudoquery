package io.juspay.sudoquery.internal

import io.juspay.sudoquery.CanonicalEvent
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.serializer
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assume.assumeTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File
import java.net.URL

/**
 * Sends events through the real writer, store, uploader and HTTP transport to a running collector.
 * Skipped unless a collector URL is given:
 *
 * ```
 * ./gradlew :sudoquery:testDebugUnitTest --tests '*LiveCollectorTest*' \
 *     -Psudoquery.liveCollectorUrl=http://localhost:3001/v1/events/batch
 * ```
 *
 * The collector must be able to publish to the `events.acme-k3x9qa.android-demo1` topic.
 */
class LiveCollectorTest {
    @get:Rule
    val temp = TemporaryFolder()

    @Test
    fun `events are accepted by a real collector`() {
        val url = System.getProperty("sudoquery.liveCollectorUrl").orEmpty()
        assumeTrue("set -Psudoquery.liveCollectorUrl to run", url.isNotEmpty())

        val clock = JvmClock
        val logger = RecordingLogger()
        val store = QueueStore(temp.newFolder("queue"), maxEventsPerBatch = 2, clock = clock, logger = logger)
        val headers = requestHeaders(ORG_ID, PROJECT_ID, token = null, custom = emptyMap(), userAgent = "sudoquery-android/live-test")
        val uploader = Uploader(store, HttpUrlConnectionTransport(URL(url), headers), clock, logger)
        val writer = EventWriter(
            serializer = serializer<CanonicalEvent>(),
            buildEvent = { CanonicalEvent(it) },
            settings = WriterSettings(ORG_ID, PROJECT_ID, "android", null, 30 * 60 * 1000L),
            store = store,
            identityStore = IdentityStore(File(temp.root, "identity.json"), logger),
            uploads = uploader,
            logger = logger,
        )

        writer.start()
        writer.handle(Command.SetUser("live-test-user"))
        repeat(3) { writer.handle(Command.Track("live_test_event", clock.nowMillis(), JsonObject(mapOf("index" to JsonPrimitive(it))))) }
        writer.handle(Command.Flush)
        uploader.drain()

        assertEquals("collector errors: ${logger.errors}", emptyList<String>(), logger.errors)
        assertEquals("warnings: ${logger.warnings}", emptyList<String>(), logger.warnings)
        assertEquals(0, store.readyBatches().size)
        assertFalse(uploader.isBackingOff)
    }

    private object JvmClock : Clock {
        override fun nowMillis(): Long = System.currentTimeMillis()

        override fun elapsedMillis(): Long = System.nanoTime() / 1_000_000
    }

    private companion object {
        const val ORG_ID = "acme-k3x9qa"
        const val PROJECT_ID = "android-demo1"
    }
}
