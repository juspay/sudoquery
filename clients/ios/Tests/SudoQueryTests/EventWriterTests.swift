import XCTest
@testable import SudoQuery

final class EventWriterTests: XCTestCase {
    private static let timeout: Int64 = 30 * 60 * 1000
    private static let uuidV4 = "^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$"

    private let time = FakeTime()
    private let log = RecordingLog()
    private let uploads = FakeUploads()
    private var root: URL!
    private var store: QueueStore!

    override func setUp() {
        super.setUp()
        root = makeTempDirectory()
    }

    override func tearDown() {
        store?.closeOpenBatch()
        removeDirectory(root)
        super.tearDown()
    }

    private let storedUsers = Locked<[String?]>([])

    private var identityURL: URL {
        root.appendingPathComponent("identity.json")
    }

    private func writer(batchSize: Int = 50, maxBytes: Int64 = QueueStore.maxBatchBytes, fixedSessionId: String? = nil, start: Bool = true) -> EventWriter {
        store = QueueStore(root: root, maxEventsPerBatch: batchSize, time: time, log: log, maxBytesPerBatch: maxBytes)
        let writer = EventWriter(
            encode: { CanonicalEvent.encode($0) },
            settings: WriterSettings(orgId: "acme-k3x9qa", workspaceId: "ios-demo1", source: "ios", fixedSessionId: fixedSessionId, sessionTimeoutMillis: Self.timeout),
            store: store,
            identityStore: IdentityStore(url: identityURL, log: log),
            uploads: uploads,
            log: log,
            onStoredUser: { [storedUsers] user in storedUsers.withLock { $0.append(user) } }
        )
        if start { _ = writer.start() }
        return writer
    }

    private func storedIdentity() -> IdentityState? {
        guard case .loaded(let state) = IdentityStore(url: identityURL, log: RecordingLog()).load() else { return nil }
        return state
    }

    /// Makes the identity file unreadable, as it is before the first unlock after a reboot.
    private func lockIdentity(_ locked: Bool) throws {
        try FileManager.default.setAttributes([.posixPermissions: locked ? 0o000 : 0o600], ofItemAtPath: identityURL.path)
    }

    private func track(_ name: String, at timestamp: Int64? = nil, properties: [(String, String)] = []) -> Command {
        .track(name: name, timestampMillis: timestamp ?? time.now, properties: JSONObject(properties.map { ($0.0, JSONValue.string($0.1)) }))
    }

    private func sentEvents() -> [[String: Any]] {
        store.readyBatches().flatMap { ndjsonObjects($0.url) }
    }

    private func matches(_ value: String?, _ pattern: String) -> Bool {
        value?.range(of: pattern, options: .regularExpression) != nil
    }

    func testATrackedEventCarriesTheIdentityConfigAndTimestamp() throws {
        let writer = writer()
        writer.handle(track("button_click", at: T0, properties: [("page", "/home")]))
        writer.handle(.flush)

        let event = try XCTUnwrap(sentEvents().first)
        XCTAssertEqual(sentEvents().count, 1)
        XCTAssertEqual(text(event, "envelop_version"), "1.0")
        XCTAssertEqual(text(event, "name"), "button_click")
        XCTAssertEqual(text(event, "org_id"), "acme-k3x9qa")
        XCTAssertEqual(text(event, "workspace_id"), "ios-demo1")
        XCTAssertEqual(text(event, "source"), "ios")
        XCTAssertEqual(text(event, "occured_at"), "2025-10-09T08:53:20.000Z")
        XCTAssertEqual(canonicalJSON(event["properties"] as Any), #"{"page":"/home"}"#)
        XCTAssertTrue(event["actor_id"] is NSNull)
        XCTAssertTrue(matches(text(event, "id"), Self.uuidV4))
        XCTAssertTrue(matches(text(event, "anon_id"), Self.uuidV4))
        XCTAssertTrue(matches(text(event, "session_id"), Self.uuidV4))
    }

    func testCommandsAreAppliedInTheOrderTheyWereMade() {
        let writer = writer()
        writer.handle(.setUser("u1"))
        writer.handle(track("a"))
        writer.handle(.setUser(nil))
        writer.handle(track("b"))
        writer.handle(.flush)

        XCTAssertEqual(sentEvents().map { text($0, "actor_id") }, ["u1", nil])
    }

    func testResetGivesLaterEventsANewAnonymousIdAndSession() {
        let writer = writer()
        writer.handle(.setUser("u1"))
        writer.handle(track("a"))
        writer.handle(.reset)
        writer.handle(track("b"))
        writer.handle(.flush)

        let events = sentEvents()
        XCTAssertEqual(events.count, 2)
        XCTAssertNotEqual(text(events[0], "anon_id"), text(events[1], "anon_id"))
        XCTAssertNotEqual(text(events[0], "session_id"), text(events[1], "session_id"))
        XCTAssertNil(text(events[1], "actor_id"))
    }

    func testIdentityAndSessionSurviveARestart() {
        let first = writer()
        first.handle(.setUser("u1"))
        first.handle(track("a"))
        first.handle(.background(done: nil))

        time.now += 60_000
        let second = writer(start: false)
        XCTAssertEqual(second.start(), "u1")
        second.handle(track("b"))
        second.handle(.flush)

        let events = sentEvents()
        XCTAssertEqual(events.count, 2)
        XCTAssertEqual(text(events[0], "anon_id"), text(events[1], "anon_id"))
        XCTAssertEqual(text(events[0], "session_id"), text(events[1], "session_id"))
        XCTAssertEqual(text(events[1], "actor_id"), "u1")
    }

    func testASessionEndsAfterTheTimeoutWithoutEvents() {
        let writer = writer()
        writer.handle(track("a", at: T0))
        writer.handle(track("b", at: T0 + Self.timeout))
        writer.handle(track("c", at: T0 + 2 * Self.timeout + 1))
        writer.handle(.flush)

        let sessions = sentEvents().map { text($0, "session_id") }
        XCTAssertEqual(sessions.count, 3)
        XCTAssertEqual(sessions[0], sessions[1])
        XCTAssertNotEqual(sessions[1], sessions[2])
    }

    func testAConfiguredSessionIdIsUsedForEveryEvent() {
        let writer = writer(fixedSessionId: "checkout-42")
        writer.handle(track("a", at: T0))
        writer.handle(track("b", at: T0 + 3 * Self.timeout))
        writer.handle(.flush)
        XCTAssertEqual(sentEvents().map { text($0, "session_id") }, ["checkout-42", "checkout-42"])
    }

    func testAFullBatchIsCommittedAndAnUploadRequested() {
        let writer = writer(batchSize: 2)
        let before = uploads.requests
        writer.handle(track("a"))
        writer.handle(track("b"))
        XCTAssertEqual(store.readyBatches().count, 1)
        XCTAssertEqual(uploads.requests, before + 1)
    }

    func testAnOversizedEventIsDropped() {
        let writer = writer()
        writer.handle(track("huge", properties: [("blob", String(repeating: "x", count: EventWriter.maxEventBytes))]))
        writer.handle(.flush)
        XCTAssertEqual(sentEvents().count, 0)
        XCTAssertEqual(log.warnings.count, 1)
        XCTAssertTrue(log.warnings.first?.contains("huge") ?? false)
    }

    func testTicksLeaveTheOpenBatchAloneWhileUploadsBackOff() {
        let writer = writer()
        writer.handle(track("a"))
        uploads.isBackingOff = true
        writer.handle(.tick)
        XCTAssertEqual(store.readyBatches().count, 0)

        uploads.isBackingOff = false
        writer.handle(.tick)
        XCTAssertEqual(store.readyBatches().count, 1)
    }

    func testGoingToTheBackgroundCommitsResetsTheBackoffAndWaitsForTheUploader() {
        let writer = writer()
        writer.handle(track("a"))
        uploads.isBackingOff = true
        let before = uploads.requests
        let done = Locked(false)
        writer.handle(.background(done: { done.withLock { $0 = true } }))
        XCTAssertEqual(store.readyBatches().count, 1)
        XCTAssertEqual(uploads.resets, 1) // one try before iOS suspends the app
        XCTAssertEqual(uploads.requests, before + 1)
        XCTAssertEqual(uploads.idleWaits, 1)
        XCTAssertTrue(done.current)
    }

    func testABatchCommittedBeforeAFailedAppendIsStillUploaded() throws {
        let writer = writer(maxBytes: 500) // room for one event
        writer.handle(track("a"))
        // The next batch file can't be created: a directory is in the way.
        let blocker = root.appendingPathComponent("staging").appendingPathComponent(QueueStore.fileName(seq: 2, createdMillis: T0))
        try FileManager.default.createDirectory(at: blocker, withIntermediateDirectories: true)
        let before = uploads.requests

        writer.handle(track("b"))

        XCTAssertEqual(store.readyBatches().count, 1)
        XCTAssertEqual(uploads.requests, before + 1)
        XCTAssertTrue(log.warnings.last?.contains("Could not store event \"b\"") ?? false)
    }

    func testAStoredIdentityThatCantBeReadYetIsNeverReplacedAndIsUsedOnceItCanBe() throws {
        let first = writer()
        first.handle(.setUser("u1"))
        first.handle(track("a"))
        first.handle(.flush)
        let stored = try XCTUnwrap(storedIdentity())

        try lockIdentity(true)
        let second = writer(start: false)
        XCTAssertNil(second.start())
        XCTAssertTrue(log.warnings.last?.contains("can't be read yet") ?? false)
        second.handle(.tick)
        second.handle(.background(done: nil))
        try lockIdentity(false)
        XCTAssertEqual(storedIdentity(), stored) // not overwritten by the temporary identity

        // After the unlock, the next command switches to the stored identity.
        second.handle(track("b"))
        second.handle(.flush)
        let event = try XCTUnwrap(sentEvents().last)
        XCTAssertEqual(text(event, "anon_id"), stored.anonId)
        XCTAssertEqual(text(event, "actor_id"), "u1")
        XCTAssertEqual(storedUsers.current, ["u1"])
    }

    func testAUserSetWhileTheStoredIdentityCantBeReadIsKept() throws {
        let first = writer()
        first.handle(.setUser("u1"))
        first.handle(.flush)
        let stored = try XCTUnwrap(storedIdentity())

        try lockIdentity(true)
        let second = writer(start: false)
        _ = second.start()
        second.handle(.setUser("u2"))
        try lockIdentity(false)
        second.handle(track("b"))
        second.handle(.flush)

        let event = try XCTUnwrap(sentEvents().last)
        XCTAssertEqual(text(event, "anon_id"), stored.anonId)
        XCTAssertEqual(text(event, "actor_id"), "u2")
        XCTAssertEqual(storedIdentity()?.actorId, "u2")
        XCTAssertEqual(storedUsers.current, [])
    }

    func testAResetWhileTheStoredIdentityCantBeReadReplacesIt() throws {
        let first = writer()
        first.handle(.setUser("u1"))
        first.handle(.flush)
        let stored = try XCTUnwrap(storedIdentity())

        try lockIdentity(true)
        let second = writer(start: false)
        _ = second.start()
        second.handle(.reset)
        try lockIdentity(false)
        second.handle(track("b"))
        second.handle(.flush)

        let event = try XCTUnwrap(sentEvents().last)
        XCTAssertNotEqual(text(event, "anon_id"), stored.anonId)
        XCTAssertNil(text(event, "actor_id"))
        XCTAssertEqual(storedIdentity()?.anonId, text(event, "anon_id"))
    }

    func testComingToTheForegroundResetsTheBackoffAndAsksForAnUpload() {
        let writer = writer()
        let before = uploads.requests
        writer.handle(.foreground)
        XCTAssertEqual(uploads.resets, 1)
        XCTAssertEqual(uploads.requests, before + 1)
    }

    func testABarrierCompletesInOrder() {
        let writer = writer()
        let done = Locked(false)
        writer.handle(.barrier { done.withLock { $0 = true } })
        XCTAssertTrue(done.current)
    }

    func testStartRequestsAnUploadOfWhatThePreviousRunLeft() {
        let before = uploads.requests
        _ = writer()
        XCTAssertEqual(uploads.requests, before + 1)
    }
}
