package io.juspay.sudoquery.internal

import io.juspay.sudoquery.CanonicalEvent
import kotlinx.coroutines.CompletableDeferred
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.serializer
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

class EventWriterTest {
    @get:Rule
    val temp = TemporaryFolder()

    private val clock = FakeClock()
    private val logger = RecordingLogger()
    private val uploads = FakeUploads()
    private val root by lazy { temp.newFolder("queue") }
    private lateinit var store: QueueStore

    private fun writer(batchSize: Int = 50, fixedSessionId: String? = null): EventWriter<CanonicalEvent> {
        store = QueueStore(root, batchSize, clock, logger)
        return EventWriter(
            serializer = serializer<CanonicalEvent>(),
            buildEvent = { CanonicalEvent(it) },
            settings = WriterSettings("acme-k3x9qa", "android-demo1", "android", fixedSessionId, TIMEOUT),
            store = store,
            identityStore = IdentityStore(File(root, "identity.json"), logger),
            uploads = uploads,
            logger = logger,
        ).also { it.start() }
    }

    private fun track(name: String, at: Long = clock.now, properties: Map<String, String> = emptyMap()) =
        Command.Track(name, at, JsonObject(properties.mapValues { JsonPrimitive(it.value) }))

    private fun sentEvents(): List<JsonObject> = store.readyBatches().flatMap { it.ndjsonLines() }

    private fun JsonObject.text(key: String): String? = getValue(key).let { if (it is JsonNull) null else it.jsonPrimitive.content }

    @Test
    fun `a tracked event carries the identity, config and timestamp`() {
        val writer = writer()
        writer.handle(track("button_click", at = T0, properties = mapOf("page" to "/home")))
        writer.handle(Command.Flush)

        val event = sentEvents().single()
        assertEquals("1.0", event.text("envelop_version"))
        assertEquals("button_click", event.text("name"))
        assertEquals("acme-k3x9qa", event.text("org_id"))
        assertEquals("android-demo1", event.text("project_id"))
        assertEquals("android", event.text("source"))
        assertEquals("2025-10-09T08:53:20.000Z", event.text("occured_at"))
        assertEquals("""{"page":"/home"}""", event.getValue("properties").toString())
        assertEquals(null, event.text("actor_id"))
        assertTrue(event.text("id")!!.matches(UUID_V4))
        assertTrue(event.text("anon_id")!!.matches(UUID_V4))
        assertTrue(event.text("session_id")!!.matches(UUID_V4))
    }

    @Test
    fun `commands are applied in the order they were made`() {
        val writer = writer()
        writer.handle(Command.SetUser("u1"))
        writer.handle(track("a"))
        writer.handle(Command.SetUser(null))
        writer.handle(track("b"))
        writer.handle(Command.Flush)

        assertEquals(listOf("u1", null), sentEvents().map { it.text("actor_id") })
    }

    @Test
    fun `reset gives later events a new anonymous id and session`() {
        val writer = writer()
        writer.handle(Command.SetUser("u1"))
        writer.handle(track("a"))
        writer.handle(Command.Reset)
        writer.handle(track("b"))
        writer.handle(Command.Flush)

        val (a, b) = sentEvents()
        assertNotEquals(a.text("anon_id"), b.text("anon_id"))
        assertNotEquals(a.text("session_id"), b.text("session_id"))
        assertEquals(null, b.text("actor_id"))
    }

    @Test
    fun `identity and session survive a restart`() {
        val first = writer()
        first.handle(Command.SetUser("u1"))
        first.handle(track("a"))
        first.handle(Command.Background)

        clock.now += 60_000
        val second = EventWriter(
            serializer<CanonicalEvent>(), { CanonicalEvent(it) },
            WriterSettings("acme-k3x9qa", "android-demo1", "android", null, TIMEOUT),
            store, IdentityStore(File(root, "identity.json"), logger), uploads, logger,
        )
        assertEquals("u1", second.start())
        second.handle(track("b"))
        second.handle(Command.Flush)

        val (a, b) = sentEvents()
        assertEquals(a.text("anon_id"), b.text("anon_id"))
        assertEquals(a.text("session_id"), b.text("session_id"))
        assertEquals("u1", b.text("actor_id"))
    }

    @Test
    fun `a session ends after the timeout without events`() {
        val writer = writer()
        writer.handle(track("a", at = T0))
        writer.handle(track("b", at = T0 + TIMEOUT))
        writer.handle(track("c", at = T0 + 2 * TIMEOUT + 1))
        writer.handle(Command.Flush)

        val sessions = sentEvents().map { it.text("session_id") }
        assertEquals(sessions[0], sessions[1])
        assertNotEquals(sessions[1], sessions[2])
    }

    @Test
    fun `a configured session id is used for every event`() {
        val writer = writer(fixedSessionId = "checkout-42")
        writer.handle(track("a", at = T0))
        writer.handle(track("b", at = T0 + 3 * TIMEOUT))
        writer.handle(Command.Flush)
        assertEquals(listOf("checkout-42", "checkout-42"), sentEvents().map { it.text("session_id") })
    }

    @Test
    fun `a full batch is committed and an upload requested`() {
        val writer = writer(batchSize = 2)
        val before = uploads.requests
        writer.handle(track("a"))
        writer.handle(track("b"))
        assertEquals(1, store.readyBatches().size)
        assertEquals(before + 1, uploads.requests)
    }

    @Test
    fun `an oversized event is dropped`() {
        val writer = writer()
        writer.handle(track("huge", properties = mapOf("blob" to "x".repeat(EventWriter.MAX_EVENT_BYTES))))
        writer.handle(Command.Flush)
        assertEquals(emptyList<JsonObject>(), sentEvents())
        assertTrue(logger.warnings.single().contains("huge"))
    }

    @Test
    fun `ticks leave the open batch alone while uploads back off`() {
        val writer = writer()
        writer.handle(track("a"))
        uploads.isBackingOff = true
        writer.handle(Command.Tick)
        assertEquals(0, store.readyBatches().size)

        uploads.isBackingOff = false
        writer.handle(Command.Tick)
        assertEquals(1, store.readyBatches().size)
    }

    @Test
    fun `coming to the foreground resets the backoff and asks for an upload`() {
        val writer = writer()
        val before = uploads.requests
        writer.handle(Command.Foreground)
        assertEquals(1, uploads.resets)
        assertEquals(before + 1, uploads.requests)
    }

    @Test
    fun `a barrier completes in order`() {
        val writer = writer()
        val done = CompletableDeferred<Unit>()
        writer.handle(Command.Barrier(done))
        assertTrue(done.isCompleted)
    }

    private companion object {
        const val TIMEOUT = 30 * 60 * 1000L
        val UUID_V4 = Regex("[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}")
    }
}
