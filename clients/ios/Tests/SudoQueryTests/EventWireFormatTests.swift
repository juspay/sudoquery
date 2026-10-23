import XCTest
@testable import SudoQuery

final class EventWireFormatTests: XCTestCase {
    private let input = EventInput(
        id: "3b241101-e2bb-4255-8caf-4136c566a962",
        name: "button_click",
        orgId: "acme-k3x9qa",
        workspaceId: "ios-demo1",
        sessionId: "9f1c2d3e-4b5a-4c6d-8e7f-0a1b2c3d4e5f",
        anonId: "6e8bc430-9c3a-4d6b-9f3e-2a1b0c9d8e7f",
        actorId: nil,
        source: "ios",
        occurredAt: "2026-10-14T09:30:12.345Z",
        properties: JSONObject([
            ("app_version", .string("1.0.0")),
            ("button_id", .string("submit")),
            ("page", .string("/checkout")),
        ])
    )

    func testAnEventV1EncodesExactlyLikeTheTSClientsEvent() throws {
        // All 14 keys, in the TS order, with explicit nulls and an unescaped "/".
        XCTAssertEqual(String(decoding: EventV1.encode(input), as: UTF8.self), try golden("event"))
    }

    func testABatchBodyMatchesTheCollectorsBatchShape() throws {
        let body = BatchBody.build(
            events: [Data(EventV1.encode(input))],
            systemProperties: SystemProperties(geo: nil, timezone: "Asia/Kolkata")
        )
        XCTAssertEqual(String(decoding: body, as: UTF8.self), try golden("batch"))
    }

    private struct TestEventV2: WireEvent {
        let eventId: String
        let eventName: String
        let anonId: String

        init(_ input: EventInput) {
            eventId = input.id
            eventName = input.name
            anonId = input.anonId
        }

        func json() -> JSONObject {
            JSONObject([
                ("envelop_version", .string("2.0")),
                ("event_id", .string(eventId)),
                ("event_name", .string(eventName)),
                ("anon_id", .string(anonId)),
            ])
        }
    }

    func testThePipelineSendsWhateverEventTypeItIsGiven() throws {
        // What changing the CanonicalEvent alias does: the same writer, store and uploader, another event type.
        let root = makeTempDirectory()
        defer { removeDirectory(root) }
        let time = FakeTime()
        let log = RecordingLog()
        let store = QueueStore(root: root, maxEventsPerBatch: 10, time: time, log: log)
        let writer = EventWriter(
            encode: { TestEventV2.encode($0) },
            settings: WriterSettings(orgId: "acme-k3x9qa", workspaceId: "ios-demo1", source: "ios", fixedSessionId: nil, sessionTimeoutMillis: 30 * 60 * 1000),
            store: store,
            identityStore: IdentityStore(url: root.appendingPathComponent("identity.json"), log: log),
            uploads: FakeUploads(),
            log: log
        )
        let transport = FakeTransport()
        let uploader = Uploader(store: store, transport: transport, time: time, scheduler: ManualScheduler(time: time), log: log, timezone: { "UTC" })

        _ = writer.start()
        writer.handle(.track(name: "signup", timestampMillis: T0, properties: JSONObject()))
        writer.handle(.flush)
        uploader.drainAndWait()

        let body = try XCTUnwrap(transport.bodyStrings.first)
        let pattern = #"^\{"events":\[\{"envelop_version":"2\.0","event_id":"[0-9a-f-]{36}","event_name":"signup","anon_id":"[0-9a-f-]{36}"\}\],"system_properties":\{"geo":null,"timezone":"UTC"\}\}$"#
        XCTAssertNotNil(body.range(of: pattern, options: .regularExpression), body)
    }

    private func golden(_ name: String) throws -> String {
        let url = try XCTUnwrap(Bundle.module.url(forResource: name, withExtension: "json", subdirectory: "golden"), "missing golden/\(name).json")
        return try String(contentsOf: url, encoding: .utf8).trimmingCharacters(in: .whitespacesAndNewlines)
    }
}
