import XCTest
@testable import SudoQuery

final class SudoQueryFacadeTests: XCTestCase {
    private let log = RecordingLog()
    private let time = FakeTime()
    private var server: LocalHTTPServer!
    private var root: URL!

    override func setUpWithError() throws {
        try super.setUpWithError()
        server = try LocalHTTPServer()
        root = makeTempDirectory()
        SudoQuery.resetForTests(environment: testEnvironment(root: root, log: log, time: time), log: log)
    }

    override func tearDown() {
        SudoQuery.resetForTests()
        server.close()
        removeDirectory(root)
        super.tearDown()
    }

    private func config(batchSize: Int = 2, endpoint: String? = nil, orgId: String = "acme-k3x9qa") throws -> SudoQueryConfig {
        try SudoQueryConfig(orgId: orgId, workspaceId: "ios-demo1", endpoint: endpoint ?? server.url().absoluteString, batchSize: batchSize, flushInterval: 0)
    }

    private func events(_ request: LocalHTTPServer.Request) -> [[String: Any]] {
        ((try? JSONSerialization.jsonObject(with: request.body)) as? [String: Any])?["events"] as? [[String: Any]] ?? []
    }

    func testCallsBeforeInitializeAreIgnoredWithoutCrashing() {
        SudoQuery.track("early", properties: ["k": 1])
        SudoQuery.setUser("u1")
        SudoQuery.removeUser()
        SudoQuery.flush()
        SudoQuery.reset()
        XCTAssertFalse(SudoQuery.isInitialized)
        XCTAssertNil(SudoQuery.user)
        XCTAssertEqual(log.warnings, ["SudoQuery.track was called before SudoQuery.initialize; such calls are ignored"])
    }

    func testSuperPropertiesWorkBeforeInitializeAndAreCopies() {
        SudoQuery.setSuperProperty("plan", "pro")
        SudoQuery.setSuperProperty("tags", ["a", "b"])
        XCTAssertEqual(canonicalJSON(SudoQuery.superProperties), #"{"plan":"pro","tags":["a","b"]}"#)

        var copy = SudoQuery.superProperties
        copy["plan"] = "free"
        XCTAssertEqual(SudoQuery.superProperties["plan"] as? String, "pro")

        SudoQuery.clearSuperProperties()
        XCTAssertTrue(SudoQuery.superProperties.isEmpty)
    }

    func testTheFirstInitializeWins() throws {
        SudoQuery.initialize(try config())
        let first = SudoQuery.clientForTests
        SudoQuery.initialize(try config(endpoint: "https://other.example.com/v1/events/batch"))
        XCTAssertTrue(SudoQuery.isInitialized)
        XCTAssertTrue(first === SudoQuery.clientForTests)
    }

    func testIdsThatArentSlugsWarnButStillStart() throws {
        SudoQuery.initialize(try config(orgId: "Acme"))
        XCTAssertTrue(SudoQuery.isInitialized)
        XCTAssertEqual(log.warnings.count, 1)
        XCTAssertTrue(log.warnings.first?.contains("slug shape") ?? false)
    }

    func testTrackedEventsReachTheCollectorWithIdentityAndSuperProperties() throws {
        server.enqueue(code: 200, body: #"{"filtered":0,"collected":2,"total":2}"#)
        SudoQuery.initialize(try config(batchSize: 2))
        SudoQuery.setSuperProperty("app_version", "1.0.0")
        SudoQuery.setUser("u1")
        XCTAssertEqual(SudoQuery.user, "u1")

        SudoQuery.track("checkout_started", properties: ["cart_value": 499.5, "app_version": "override"])
        SudoQuery.track("checkout_completed")

        let request = try XCTUnwrap(server.takeRequest(timeout: 10), "no request reached the server")
        XCTAssertEqual(request.header("x-org-id"), "acme-k3x9qa")
        XCTAssertEqual(request.header("x-project-id"), "ios-demo1")
        XCTAssertTrue(request.header("User-Agent")?.hasPrefix("sudoquery-ios/") ?? false)

        let events = events(request)
        XCTAssertEqual(events.map { text($0, "name") }, ["checkout_started", "checkout_completed"])
        XCTAssertEqual(events.map { text($0, "actor_id") }, ["u1", "u1"])
        XCTAssertEqual(text(events[0], "anon_id"), text(events[1], "anon_id"))
        XCTAssertEqual(text(events[0], "session_id"), text(events[1], "session_id"))
        // Super properties first, the event's own keys over them.
        XCTAssertTrue(request.bodyText.contains(#""properties":{"app_version":"override","cart_value":499.5}"#), request.bodyText)
        XCTAssertTrue(request.bodyText.contains(#""properties":{"app_version":"1.0.0"}"#), request.bodyText)
        XCTAssertTrue(request.bodyText.contains(#""system_properties":{"geo":null,"timezone":"#), request.bodyText)
    }

    func testResetClearsTheUserAndSuperProperties() throws {
        SudoQuery.initialize(try config())
        SudoQuery.setUser("u1")
        SudoQuery.setSuperProperty("plan", "pro")
        SudoQuery.reset()
        XCTAssertNil(SudoQuery.user)
        XCTAssertTrue(SudoQuery.superProperties.isEmpty)
        XCTAssertTrue(try XCTUnwrap(SudoQuery.clientForTests).awaitIdle())
    }

    func testTheTimestampAndThePropertiesAreTakenOnTheCallersThread() throws {
        SudoQuery.initialize(try config(batchSize: 1))
        let items = NSMutableArray(array: ["a"])
        SudoQuery.track("copied", properties: ["items": items])
        items.add("b")
        time.now = T0 + 60_000

        let request = try XCTUnwrap(server.takeRequest(timeout: 10))
        let event = try XCTUnwrap(events(request).first)
        XCTAssertEqual(text(event, "occured_at"), "2025-10-09T08:53:20.000Z")
        XCTAssertEqual(canonicalJSON(event["properties"] as Any), #"{"items":["a"]}"#)
    }

    func testEventsWaitingForTheWriterAreCapped() throws {
        SudoQuery.initialize(try config(batchSize: 500))
        let client = try XCTUnwrap(SudoQuery.clientForTests)
        let release = DispatchSemaphore(value: 0)
        client.onWriterQueueForTests { release.wait() }

        for index in 0...SudoQueryClient.maxQueuedTracks {
            SudoQuery.track("burst", properties: ["index": index])
        }
        release.signal()
        SudoQuery.flush()
        XCTAssertTrue(client.awaitIdle())

        XCTAssertEqual(log.warnings.filter { $0.contains("More than 1000 events") }.count, 1)
        var sent = 0
        while let request = server.takeRequest(timeout: 1) {
            sent += events(request).count
        }
        XCTAssertEqual(sent, SudoQueryClient.maxQueuedTracks)
    }
}

func testEnvironment(root: URL, log: RecordingLog, time: FakeTime? = nil) -> ClientEnvironment {
    var environment = ClientEnvironment()
    environment.storageRoot = { root }
    environment.makeLog = { _ in log }
    environment.observesLifecycle = false
    if let time { environment.time = time }
    return environment
}
