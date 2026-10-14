package io.juspay.sudoquery

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject

/**
 * The event class the SDK sends. To move to a new envelope, add its class to this file (with a
 * `constructor(input: EventInput)`) and point this alias at it; nothing else needs to change.
 * Batching, storage and upload only ever see serialized events.
 */
internal typealias CanonicalEvent = EventV1

/**
 * Everything the SDK knows about one tracked event, independent of the envelope version.
 * Each event class picks the fields it needs in its `constructor(input: EventInput)`.
 */
internal class EventInput(
    val id: String,
    val name: String,
    val orgId: String,
    val projectId: String,
    val sessionId: String?,
    val anonId: String,
    val actorId: String?,
    val source: String?,
    /** RFC 3339 in UTC with milliseconds, e.g. `2026-10-14T09:30:12.345Z`. */
    val occurredAt: String,
    val properties: JsonObject,
)

/**
 * Envelope "1.0". Mirrors `CollectorEvent` in crates/events-collector/src/collector_event.rs and
 * `Event` in clients/typescript/src/types.ts, field for field and in the same order, including
 * the `envelop_version` and `occured_at` spellings.
 */
@Serializable
internal data class EventV1(
    @SerialName("envelop_version") val envelopVersion: String = "1.0",
    @SerialName("id") val id: String,
    @SerialName("name") val name: String,
    @SerialName("org_id") val orgId: String,
    @SerialName("project_id") val projectId: String,
    @SerialName("session_id") val sessionId: String? = null,
    @SerialName("anon_id") val anonId: String,
    @SerialName("actor_id") val actorId: String? = null,
    @SerialName("source") val source: String? = null,
    @SerialName("occured_at") val occuredAt: String,
    @SerialName("properties") val properties: JsonObject? = null,
    @SerialName("correlation_id") val correlationId: String? = null,
    @SerialName("trace_id") val traceId: String? = null,
    @SerialName("system_properties") val systemProperties: SystemProperties? = null,
) {
    constructor(input: EventInput) : this(
        id = input.id,
        name = input.name,
        orgId = input.orgId,
        projectId = input.projectId,
        sessionId = input.sessionId,
        anonId = input.anonId,
        actorId = input.actorId,
        source = input.source,
        occuredAt = input.occurredAt,
        properties = input.properties,
    )
}

/** System properties understood by the collector. */
@Serializable
internal data class SystemProperties(
    @SerialName("geo") val geo: Geo? = null,
    @SerialName("timezone") val timezone: String? = null,
)

@Serializable
internal data class Geo(
    @SerialName("country") val country: String? = null,
)

/**
 * Body of `POST /v1/events/batch`. Events stay opaque JSON, so stored events of any envelope
 * version are sent exactly as they were written.
 */
@Serializable
internal data class BatchPayload(
    @SerialName("events") val events: List<JsonElement>,
    @SerialName("system_properties") val systemProperties: SystemProperties?,
)

/** JSON settings for everything on the wire: like `JSON.stringify` in the TS client, nulls and defaults are written out. */
internal val WireJson: Json = Json { encodeDefaults = true }
