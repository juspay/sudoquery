import Foundation

/// The event type the SDK sends. To move to a new envelope, add its type to this file (conforming
/// to `WireEvent`) and point this alias at it; nothing else needs to change. Batching, storage and
/// upload only ever see encoded events.
typealias CanonicalEvent = EventV1

/// Everything the SDK knows about one tracked event, independent of the envelope version. Each
/// event type picks the fields it needs in its `init(_:)`.
struct EventInput: Sendable {
    let id: String
    let name: String
    let orgId: String
    let workspaceId: String
    let sessionId: String?
    let anonId: String
    let actorId: String?
    let source: String?
    /// RFC 3339 in UTC with milliseconds, e.g. `2026-10-14T09:30:12.345Z`.
    let occurredAt: String
    let properties: JSONObject
}

protocol WireEvent {
    init(_ input: EventInput)

    /// The event as a JSON object, keys in wire order.
    func json() -> JSONObject
}

extension WireEvent {
    /// One NDJSON line, without the newline.
    static func encode(_ input: EventInput) -> [UInt8] {
        JSON.encode(.object(Self(input).json()))
    }
}

/// Envelope "1.0". Mirrors `CollectorEvent` in crates/events-collector/src/collector_event.rs and
/// `Event` in clients/typescript/src/types.ts field for field and in the same order, including the
/// `envelop_version` and `occured_at` spellings. The workspace id goes out as `project_id`, the
/// collector's name for it.
struct EventV1: WireEvent, Sendable {
    var envelopVersion = "1.0"
    var id: String
    var name: String
    var orgId: String
    var workspaceId: String
    var sessionId: String?
    var anonId: String
    var actorId: String?
    var source: String?
    var occuredAt: String
    var properties: JSONObject?
    var correlationId: String?
    var traceId: String?
    var systemProperties: SystemProperties?

    init(_ input: EventInput) {
        id = input.id
        name = input.name
        orgId = input.orgId
        workspaceId = input.workspaceId
        sessionId = input.sessionId
        anonId = input.anonId
        actorId = input.actorId
        source = input.source
        occuredAt = input.occurredAt
        properties = input.properties
    }

    func json() -> JSONObject {
        JSONObject([
            ("envelop_version", .string(envelopVersion)),
            ("id", .string(id)),
            ("name", .string(name)),
            ("org_id", .string(orgId)),
            ("project_id", .string(workspaceId)),
            ("session_id", string(sessionId)),
            ("anon_id", .string(anonId)),
            ("actor_id", string(actorId)),
            ("source", string(source)),
            ("occured_at", .string(occuredAt)),
            ("properties", properties.map(JSONValue.object) ?? .null),
            ("correlation_id", string(correlationId)),
            ("trace_id", string(traceId)),
            ("system_properties", systemProperties?.json() ?? .null),
        ])
    }
}

/// System properties understood by the collector.
struct SystemProperties: Sendable, Equatable {
    var geo: Geo?
    var timezone: String?

    func json() -> JSONValue {
        .object(JSONObject([
            ("geo", geo?.json() ?? .null),
            ("timezone", string(timezone)),
        ]))
    }
}

struct Geo: Sendable, Equatable {
    var country: String?

    func json() -> JSONValue {
        .object(JSONObject([("country", string(country))]))
    }
}

/// The body of `POST /v1/events/batch`.
enum BatchBody {
    /// `{"events":[…],"system_properties":{…}}`. The events are the stored NDJSON lines copied
    /// verbatim, so a resend is byte-for-byte the same and events of any envelope version are sent
    /// exactly as they were written.
    static func build(events: [Data], systemProperties: SystemProperties?) -> Data {
        var out: [UInt8] = []
        out.reserveCapacity(events.reduce(64) { $0 + $1.count + 1 })
        out.append(contentsOf: #"{"events":["#.utf8)
        for (index, event) in events.enumerated() {
            if index > 0 { out.append(UInt8(ascii: ",")) }
            out.append(contentsOf: event)
        }
        out.append(contentsOf: #"],"system_properties":"#.utf8)
        JSON.write(systemProperties?.json() ?? .null, to: &out)
        out.append(UInt8(ascii: "}"))
        return Data(out)
    }
}

private func string(_ value: String?) -> JSONValue {
    value.map(JSONValue.string) ?? .null
}
