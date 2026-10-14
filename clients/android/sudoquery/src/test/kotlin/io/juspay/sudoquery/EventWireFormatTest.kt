package io.juspay.sudoquery

import io.juspay.sudoquery.internal.EventWriter
import io.juspay.sudoquery.internal.FakeClock
import io.juspay.sudoquery.internal.FakeTransport
import io.juspay.sudoquery.internal.FakeUploads
import io.juspay.sudoquery.internal.IdentityStore
import io.juspay.sudoquery.internal.QueueStore
import io.juspay.sudoquery.internal.RecordingLogger
import io.juspay.sudoquery.internal.Uploader
import io.juspay.sudoquery.internal.WriterSettings
import io.juspay.sudoquery.internal.Command
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.serializer
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class EventWireFormatTest {
    @get:Rule
    val temp = TemporaryFolder()

    private val input = EventInput(
        id = "3b241101-e2bb-4255-8caf-4136c566a962",
        name = "button_click",
        orgId = "acme-k3x9qa",
        projectId = "android-demo1",
        sessionId = "9f1c2d3e-4b5a-4c6d-8e7f-0a1b2c3d4e5f",
        anonId = "6e8bc430-9c3a-4d6b-9f3e-2a1b0c9d8e7f",
        actorId = null,
        source = "android",
        occurredAt = "2026-10-14T09:30:12.345Z",
        properties = JsonObject(
            linkedMapOf(
                "app_version" to JsonPrimitive("1.0.0"),
                "button_id" to JsonPrimitive("submit"),
                "page" to JsonPrimitive("/checkout"),
            ),
        ),
    )

    @Test
    fun `an EventV1 serializes exactly like the TS client's event`() {
        // All 14 keys, in the TS order, with explicit nulls.
        assertEquals(golden("event.json"), WireJson.encodeToString(EventV1.serializer(), EventV1(input)))
    }

    @Test
    fun `a batch body matches the collector's Batch shape`() {
        val event = Json.parseToJsonElement(WireJson.encodeToString(EventV1.serializer(), EventV1(input)))
        val batch = BatchPayload(listOf(event), SystemProperties(geo = null, timezone = "Asia/Kolkata"))
        assertEquals(golden("batch.json"), WireJson.encodeToString(BatchPayload.serializer(), batch))
    }

    @Serializable
    private data class TestEventV2(
        @SerialName("envelop_version") val envelopVersion: String = "2.0",
        @SerialName("event_id") val eventId: String,
        @SerialName("event_name") val eventName: String,
        @SerialName("anon_id") val anonId: String,
    ) {
        constructor(input: EventInput) : this(eventId = input.id, eventName = input.name, anonId = input.anonId)
    }

    @Test
    fun `the pipeline sends whatever event class it is given`() {
        // What changing the CanonicalEvent alias does: the same writer, store and uploader, another event class.
        val clock = FakeClock()
        val logger = RecordingLogger()
        val store = QueueStore(temp.newFolder("queue"), maxEventsPerBatch = 10, clock = clock, logger = logger)
        val writer = EventWriter(
            serializer = serializer<TestEventV2>(),
            buildEvent = { TestEventV2(it) },
            settings = WriterSettings("acme-k3x9qa", "android-demo1", "android", null, 30 * 60 * 1000L),
            store = store,
            identityStore = IdentityStore(temp.root.resolve("identity.json"), logger),
            uploads = FakeUploads(),
            logger = logger,
        )
        val transport = FakeTransport()
        val uploader = Uploader(store, transport, clock, logger, timezone = { "UTC" })

        writer.start()
        writer.handle(Command.Track("signup", T_EVENT, JsonObject(emptyMap())))
        writer.handle(Command.Flush)
        uploader.drain()

        val event = transport.events(0).single()
        assertEquals(listOf("envelop_version", "event_id", "event_name", "anon_id"), event.keys.toList())
        assertEquals("2.0", event.getValue("envelop_version").jsonPrimitive.content)
        assertEquals("signup", event.getValue("event_name").jsonPrimitive.content)
    }

    private fun golden(name: String): String =
        checkNotNull(javaClass.classLoader?.getResource("golden/$name")) { "missing golden/$name" }.readText().trim()

    private companion object {
        const val T_EVENT = 1_760_000_000_000L
    }
}
