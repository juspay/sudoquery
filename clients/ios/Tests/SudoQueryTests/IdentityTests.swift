import XCTest
@testable import SudoQuery

final class IdentityTests: XCTestCase {
    private let timeout: Int64 = 30 * 60 * 1000
    private var ids = 0
    private var root: URL!

    override func setUp() {
        super.setUp()
        root = makeTempDirectory()
    }

    override func tearDown() {
        removeDirectory(root)
        super.tearDown()
    }

    private func newId() -> String {
        ids += 1
        return "id-\(ids)"
    }

    func testTheFirstEventStartsASession() {
        let state = IdentityState(anonId: "anon").touched(at: T0, timeoutMillis: timeout, fixedSessionId: nil, newId: newId)
        XCTAssertEqual(state.sessionId, "id-1")
        XCTAssertEqual(state.lastActivityMillis, T0)
    }

    func testASessionContinuesWithinTheTimeoutAndEndsAfterIt() {
        let started = IdentityState(anonId: "anon").touched(at: T0, timeoutMillis: timeout, fixedSessionId: nil, newId: newId)
        let continued = started.touched(at: T0 + timeout, timeoutMillis: timeout, fixedSessionId: nil, newId: newId)
        XCTAssertEqual(continued.sessionId, started.sessionId)

        let renewed = continued.touched(at: T0 + 2 * timeout + 1, timeoutMillis: timeout, fixedSessionId: nil, newId: newId)
        XCTAssertEqual(renewed.sessionId, "id-2")
    }

    func testAClockMovingBackwardsKeepsTheSessionAndTimesItFromTheNewClock() {
        let started = IdentityState(anonId: "anon").touched(at: T0, timeoutMillis: timeout, fixedSessionId: nil, newId: newId)
        let back = started.touched(at: T0 - 3_600_000, timeoutMillis: timeout, fixedSessionId: nil, newId: newId)
        XCTAssertEqual(back.sessionId, started.sessionId)
        XCTAssertEqual(back.lastActivityMillis, T0 - 3_600_000)
    }

    func testAConfiguredSessionIdIsAlwaysUsed() {
        let state = IdentityState(anonId: "anon", sessionId: "old", lastActivityMillis: 0)
            .touched(at: T0, timeoutMillis: timeout, fixedSessionId: "fixed", newId: newId)
        XCTAssertEqual(state.sessionId, "fixed")
        XCTAssertEqual(ids, 0)
    }

    func testAnExtremeLastActivityStartsANewSessionWithoutTrapping() {
        let state = IdentityState(anonId: "anon", sessionId: "old", lastActivityMillis: .min)
            .touched(at: T0, timeoutMillis: timeout, fixedSessionId: nil, newId: newId)
        XCTAssertEqual(state.sessionId, "id-1")
    }

    func testTheStoreRoundTripsAndReplacesTheFileAtomically() {
        let file = root.appendingPathComponent("v1/identity.json")
        let store = IdentityStore(url: file, log: RecordingLog())
        XCTAssertEqual(store.load(), .missing)

        let state = IdentityState(anonId: "anon", actorId: "user", sessionId: "session", lastActivityMillis: T0)
        XCTAssertTrue(store.save(state))
        XCTAssertEqual(store.load(), .loaded(state))
        XCTAssertFalse(FileManager.default.fileExists(atPath: root.appendingPathComponent("v1/identity.json.tmp").path))

        var loggedOut = state
        loggedOut.actorId = nil
        store.save(loggedOut)
        XCTAssertEqual(IdentityStore(url: file, log: RecordingLog()).load(), .loaded(loggedOut))
    }

    func testAnInvalidFileIsReportedAsCorruptWithAWarning() throws {
        let file = root.appendingPathComponent("identity.json")
        try Data("{not json".utf8).write(to: file)
        let log = RecordingLog()
        XCTAssertEqual(IdentityStore(url: file, log: log).load(), .corrupt)
        XCTAssertEqual(log.warnings.count, 1)
    }

    func testAFileThatCantBeReadIsReportedAsUnavailable() throws {
        // Like a protected file before the first unlock after a reboot.
        let file = root.appendingPathComponent("identity.json")
        let store = IdentityStore(url: file, log: RecordingLog())
        store.save(IdentityState(anonId: "anon"))
        try FileManager.default.setAttributes([.posixPermissions: 0o000], ofItemAtPath: file.path)
        defer { try? FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: file.path) }
        XCTAssertEqual(store.load(), .unavailable)
    }

    func testASaveThatFailsReportsIt() throws {
        let readOnly = root.appendingPathComponent("locked")
        try FileManager.default.createDirectory(at: readOnly, withIntermediateDirectories: true)
        try FileManager.default.setAttributes([.posixPermissions: 0o500], ofItemAtPath: readOnly.path)
        defer { try? FileManager.default.setAttributes([.posixPermissions: 0o700], ofItemAtPath: readOnly.path) }
        let log = RecordingLog()
        XCTAssertFalse(IdentityStore(url: readOnly.appendingPathComponent("identity.json"), log: log).save(IdentityState(anonId: "anon")))
        XCTAssertEqual(log.warnings.count, 1)
    }
}
