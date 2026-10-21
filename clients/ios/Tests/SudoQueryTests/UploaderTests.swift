import XCTest
@testable import SudoQuery

final class UploaderTests: XCTestCase {
    private let time = FakeTime()
    private let log = RecordingLog()
    private let transport = FakeTransport()
    private var scheduler: ManualScheduler!
    private var root: URL!
    private var store: QueueStore!
    private var uploader: Uploader!

    override func setUpWithError() throws {
        try super.setUpWithError()
        root = makeTempDirectory()
        scheduler = ManualScheduler(time: time)
        store = QueueStore(root: root, maxEventsPerBatch: 2, time: time, log: log)
        try store.recover()
        uploader = Uploader(store: store, transport: transport, time: time, scheduler: scheduler, log: log, timezone: { "Asia/Kolkata" }, random: seededRandom())
    }

    override func tearDown() {
        uploader.shutdown()
        store.closeOpenBatch()
        removeDirectory(root)
        super.tearDown()
    }

    /// Commits `count` batches of two events each, numbered from 1.
    private func queueBatches(_ count: Int) throws {
        let first = (store.readyBatches().count) * 2 + 1
        for n in first..<(first + count * 2) {
            _ = try store.append(Array(#"{"n":\#(n)}"#.utf8))
        }
    }

    private func sentNumbers(_ request: Int) -> [Int] {
        transport.events(request).compactMap { $0["n"] as? Int }
    }

    func testSendsBatchesOldestFirstAndDeletesThem() throws {
        try queueBatches(2)
        uploader.drainAndWait()

        XCTAssertEqual(sentNumbers(0), [1, 2])
        XCTAssertEqual(sentNumbers(1), [3, 4])
        XCTAssertEqual(store.readyBatches().count, 0)
        XCTAssertFalse(uploader.isBackingOff)
    }

    func testTheBatchCarriesTheTimezoneAtBatchLevel() throws {
        try queueBatches(1)
        uploader.drainAndWait()
        let body = try XCTUnwrap(transport.bodyStrings.first)
        XCTAssertTrue(body.hasSuffix(#"],"system_properties":{"geo":null,"timezone":"Asia/Kolkata"}}"#), body)
    }

    func testAny2xxCountsAsDelivered() throws {
        try queueBatches(2)
        transport.reply(202)
        transport.reply(204, body: "")
        uploader.drainAndWait()
        XCTAssertEqual(store.readyBatches().count, 0)
        XCTAssertEqual(transport.bodies.count, 2)
    }

    func testARejectedBatchIsDroppedAndTheQueueMovesOn() throws {
        for code in [400, 413, 422] {
            try queueBatches(2)
            transport.reply(code, body: #"{"error":"missing field `anon_id`"}"#)
            uploader.drainAndWait()
            XCTAssertEqual(store.readyBatches().count, 0, "HTTP \(code)")
        }
        XCTAssertEqual(transport.bodies.count, 6)
        XCTAssertEqual(log.errors.count, 3)
        XCTAssertTrue(log.errors.first?.contains("missing field `anon_id`") ?? false)
        XCTAssertFalse(uploader.isBackingOff)
    }

    func testARejectionLogsAtMost300CharactersOfAPlainTextBody() throws {
        try queueBatches(1)
        transport.reply(413, body: String(repeating: "y", count: 1_000))
        uploader.drainAndWait()
        let error = try XCTUnwrap(log.errors.first)
        XCTAssertTrue(error.hasSuffix(": " + String(repeating: "y", count: 300)), error)
    }

    func testOtherFailuresKeepTheBatchStopTheQueueAndBackOff() throws {
        for code in [301, 401, 403, 404, 408, 429, 500, 502, 503] {
            uploader.resetBackoff()
            try queueBatches(1)
            let before = transport.bodies.count
            transport.reply(code)
            uploader.drainAndWait()

            XCTAssertEqual(transport.bodies.count, before + 1, "HTTP \(code)")
            XCTAssertTrue(uploader.isBackingOff, "HTTP \(code)")
            uploader.resetBackoff()
            uploader.drainAndWait() // delivered on the next attempt
            XCTAssertEqual(store.readyBatches().count, 0, "HTTP \(code)")
        }
    }

    func testANetworkErrorKeepsTheBatchAndStopsTheQueue() throws {
        try queueBatches(2)
        transport.failWith(URLError(.cannotConnectToHost))
        uploader.drainAndWait()

        XCTAssertEqual(transport.bodies.count, 1)
        XCTAssertEqual(store.readyBatches().count, 2)
        XCTAssertTrue(uploader.isBackingOff)
        XCTAssertEqual(log.warnings.count, 1)
        XCTAssertTrue(log.warnings.first?.contains("Could not reach the collector") ?? false)
    }

    func testAResendIsByteForByteTheSameRequest() throws {
        try queueBatches(1)
        transport.reply(502)
        uploader.drainAndWait()
        uploader.resetBackoff()
        uploader.drainAndWait()
        XCTAssertEqual(transport.bodies.count, 2)
        XCTAssertEqual(transport.bodies.first, transport.bodies.last)
    }

    func testBackoffDoublesFrom5sUpTo5MinutesHalfOfItRandomized() throws {
        try queueBatches(1)
        var delays: [Int64] = []
        for _ in 1...10 {
            transport.failWith(URLError(.notConnectedToInternet))
            time.elapsed = 0
            uploader.drainAndWait()
            delays.append(waitTime())
        }
        let ceilings: [Int64] = [5_000, 10_000, 20_000, 40_000, 80_000, 160_000, 300_000, 300_000, 300_000, 300_000]
        for (delay, ceiling) in zip(delays, ceilings) {
            XCTAssertTrue((ceiling / 2...ceiling).contains(delay), "\(delay) vs \(ceiling)")
        }
        XCTAssertEqual(log.warnings.count, 1) // later failures in the streak are debug only
    }

    func testRetryAfterIsHonouredUpToAnHour() throws {
        try queueBatches(1)
        transport.reply(429, retryAfterSeconds: 120)
        uploader.drainAndWait()
        XCTAssertGreaterThanOrEqual(waitTime(), 120_000)

        uploader.resetBackoff()
        transport.reply(503, retryAfterSeconds: 7_200)
        uploader.drainAndWait()
        XCTAssertEqual(waitTime(), 3_600_000)
    }

    func testAnEmptyBatchFileIsDeletedWithoutARequest() throws {
        _ = try store.append(Array(#"{"n":1}"#.utf8))
        store.commit()
        let file = try XCTUnwrap(store.readyBatches().first)
        try Data().write(to: file.url)
        uploader.drainAndWait()
        XCTAssertEqual(transport.bodies.count, 0)
        XCTAssertEqual(store.readyBatches().count, 0)
    }

    func testRetriesByItselfOnceTheBackoffEnds() throws {
        try queueBatches(1)
        transport.reply(503)
        uploader.requestAndWait()
        XCTAssertEqual(transport.bodies.count, 1)
        XCTAssertEqual(store.readyBatches().count, 1)

        uploader.requestAndWait() // ignored while backing off
        XCTAssertEqual(transport.bodies.count, 1)
        XCTAssertEqual(scheduler.pendingCount, 1)

        let retryAt = waitTime()
        scheduler.advance(to: retryAt - 1) // not yet
        uploader.waitUntilIdle()
        XCTAssertEqual(transport.bodies.count, 1)

        scheduler.advance(to: retryAt)
        uploader.waitUntilIdle()
        XCTAssertEqual(transport.bodies.count, 2)
        XCTAssertEqual(store.readyBatches().count, 0)
    }

    func testResetBackoffLetsARequestThroughAtOnce() throws {
        try queueBatches(1)
        transport.reply(503)
        uploader.requestAndWait()
        uploader.resetBackoff()
        uploader.requestAndWait()
        XCTAssertEqual(transport.bodies.count, 2)
        XCTAssertEqual(store.readyBatches().count, 0)
    }

    func testWhenIdleWaitsForTheDrainAndReturnsWhileBackingOff() throws {
        let fired = Locked(0)
        uploader.whenIdle { fired.withLock { $0 += 1 } }
        uploader.waitUntilIdle()
        XCTAssertEqual(fired.current, 1) // nothing to do: idle at once

        try queueBatches(2)
        transport.reply(500)
        uploader.requestAndWait() // stops at the first batch and backs off, which counts as idle
        XCTAssertEqual(transport.bodies.count, 1)
        XCTAssertTrue(uploader.isBackingOff)
    }

    func testWhenIdleWaitsForTheRequestInFlight() throws {
        let holding = HoldingTransport()
        let uploader = Uploader(store: store, transport: holding, time: time, scheduler: scheduler, log: log, random: seededRandom())
        defer { uploader.shutdown() }
        try queueBatches(1)

        let done = DispatchSemaphore(value: 0)
        uploader.requestUpload()
        uploader.whenIdle { done.signal() }
        XCTAssertTrue(eventually { holding.heldCount == 1 })
        XCTAssertEqual(done.wait(timeout: .now() + 0.3), .timedOut) // the batch hasn't been answered yet

        holding.release()
        XCTAssertEqual(done.wait(timeout: .now() + 5), .success)
        XCTAssertEqual(store.readyBatches().count, 0)
    }

    func testANewFailureAfterAResetReplacesTheWakeUp() throws {
        try queueBatches(1)
        transport.reply(503)
        uploader.requestAndWait()
        let firstRetry = waitTime()
        XCTAssertEqual(scheduler.pendingCount, 1)

        uploader.resetBackoff()
        transport.reply(503, retryAfterSeconds: 600)
        uploader.requestAndWait()
        XCTAssertEqual(scheduler.pendingCount, 1) // the earlier wake-up was cancelled
        XCTAssertEqual(waitTime(), 600_000)

        scheduler.advance(to: firstRetry)
        uploader.waitUntilIdle()
        XCTAssertEqual(transport.bodies.count, 2)

        scheduler.advance(to: 600_000)
        uploader.waitUntilIdle()
        XCTAssertEqual(transport.bodies.count, 3)
        XCTAssertEqual(store.readyBatches().count, 0)
    }

    func testNothingIsSentAfterShutdown() throws {
        try queueBatches(1)
        transport.reply(503)
        uploader.requestAndWait()
        uploader.shutdown()

        scheduler.advance(to: 3_600_000)
        uploader.requestAndWait()
        XCTAssertEqual(transport.bodies.count, 1)
        XCTAssertEqual(store.readyBatches().count, 1)
    }

    func testRequestsDuringADrainAreSentInOneMoreDrain() throws {
        try queueBatches(1)
        uploader.requestUpload()
        try queueBatches(1)
        uploader.requestUpload()
        uploader.requestUpload()
        uploader.waitUntilIdle()
        XCTAssertEqual(transport.bodies.count, 2)
        XCTAssertEqual(store.readyBatches().count, 0)
    }

    /// How long the uploader will wait before retrying, read through its public backoff check.
    private func waitTime() -> Int64 {
        let start = time.elapsed
        var low: Int64 = 0
        var high: Int64 = 2 * 60 * 60 * 1000
        while low < high {
            let mid = (low + high) / 2
            time.elapsed = start + mid
            if uploader.isBackingOff { low = mid + 1 } else { high = mid }
        }
        time.elapsed = start
        return low
    }
}
