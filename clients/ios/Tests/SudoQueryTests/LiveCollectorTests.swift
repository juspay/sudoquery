import XCTest
@testable import SudoQuery

/// Sends events through the real writer, store, uploader and URLSession transport to a running
/// collector. Skipped unless a collector URL is given:
///
/// ```
/// SUDOQUERY_LIVE_COLLECTOR_URL=http://localhost:3001/v1/events/batch swift test --filter LiveCollectorTests
/// ```
///
/// With xcodebuild, pass it as `TEST_RUNNER_SUDOQUERY_LIVE_COLLECTOR_URL`. The collector must be able
/// to publish to the `events.acme-k3x9qa.ios-demo1` topic.
final class LiveCollectorTests: XCTestCase {
    private static let orgId = "acme-k3x9qa"
    private static let workspaceId = "ios-demo1"

    func testEventsAreAcceptedByARealCollector() throws {
        let url = ProcessInfo.processInfo.environment["SUDOQUERY_LIVE_COLLECTOR_URL"] ?? ""
        try XCTSkipIf(url.isEmpty, "set SUDOQUERY_LIVE_COLLECTOR_URL to run")
        let endpoint = try XCTUnwrap(URL(string: url))

        let root = makeTempDirectory()
        defer { removeDirectory(root) }
        let time = SystemTime()
        let log = RecordingLog()
        let store = QueueStore(root: root, maxEventsPerBatch: 2, time: time, log: log)
        let headers = requestHeaders(orgId: Self.orgId, workspaceId: Self.workspaceId, token: nil, custom: [:], userAgent: "sudoquery-ios/live-test")
        let transport = URLSessionTransport(endpoint: endpoint, headers: headers)
        defer { transport.invalidate() }
        let uploader = Uploader(store: store, transport: transport, time: time, scheduler: DispatchWorkScheduler(), log: log)
        defer { uploader.shutdown() }
        let writer = EventWriter(
            encode: { CanonicalEvent.encode($0) },
            settings: WriterSettings(orgId: Self.orgId, workspaceId: Self.workspaceId, source: "ios", fixedSessionId: nil, sessionTimeoutMillis: 30 * 60 * 1000),
            store: store,
            identityStore: IdentityStore(url: root.appendingPathComponent("identity.json"), log: log),
            uploads: uploader,
            log: log
        )

        _ = writer.start()
        writer.handle(.setUser("live-test-user"))
        for index in 0..<3 {
            writer.handle(.track(name: "live_test_event", timestampMillis: time.nowMillis(), properties: JSONObject([("index", .number(String(index)))])))
        }
        writer.handle(.flush)
        uploader.drainAndWait(timeout: 30)

        XCTAssertEqual(log.errors, [], "collector errors")
        XCTAssertEqual(log.warnings, [], "warnings")
        XCTAssertEqual(store.readyBatches().count, 0)
        XCTAssertFalse(uploader.isBackingOff)
    }
}
