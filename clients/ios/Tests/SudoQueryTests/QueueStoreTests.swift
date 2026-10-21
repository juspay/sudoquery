import XCTest
@testable import SudoQuery

final class QueueStoreTests: XCTestCase {
    private let time = FakeTime()
    private let log = RecordingLog()
    private var root: URL!

    override func setUp() {
        super.setUp()
        root = makeTempDirectory()
    }

    override func tearDown() {
        removeDirectory(root)
        super.tearDown()
    }

    private func store(maxEvents: Int = 3, maxBytes: Int64 = QueueStore.maxBatchBytes, maxQueueBytes: Int64 = QueueStore.maxQueueBytes) throws -> QueueStore {
        let store = QueueStore(root: root, maxEventsPerBatch: maxEvents, time: time, log: log, maxBytesPerBatch: maxBytes, maxQueueBytes: maxQueueBytes)
        try store.recover()
        return store
    }

    private func event(_ n: Int) -> [UInt8] {
        Array(#"{"n":\#(n)}"#.utf8)
    }

    private func numbers(_ store: QueueStore, _ file: BatchFile) -> [Int] {
        (store.read(file) ?? []).compactMap { ((try? JSONSerialization.jsonObject(with: $0)) as? [String: Any])?["n"] as? Int }
    }

    private var stagingNames: [String] {
        (try? FileManager.default.contentsOfDirectory(atPath: root.appendingPathComponent("staging").path)) ?? []
    }

    private func writeReady(_ name: String, _ text: String) throws {
        try Data(text.utf8).write(to: root.appendingPathComponent("ready").appendingPathComponent(name))
    }

    func testABatchIsCommittedWhenItReachesTheEventLimit() throws {
        let store = try store(maxEvents: 3)
        XCTAssertFalse(try store.append(event(1)))
        XCTAssertFalse(try store.append(event(2)))
        XCTAssertTrue(try store.append(event(3)))
        XCTAssertFalse(try store.append(event(4)))

        let ready = store.readyBatches()
        XCTAssertEqual(ready.count, 1)
        XCTAssertEqual(ready.first.map { numbers(store, $0) }, [1, 2, 3])
        XCTAssertEqual(stagingNames.count, 1)
        store.closeOpenBatch()
    }

    func testABatchIsCommittedBeforeItWouldExceedTheByteLimit() throws {
        let store = try store(maxEvents: 100, maxBytes: 20)
        _ = try store.append(event(1)) // 8 bytes with the newline
        _ = try store.append(event(2)) // 16
        XCTAssertTrue(try store.append(event(3))) // would be 24: commits the first two, starts a new batch
        store.commit()
        XCTAssertEqual(store.readyBatches().map { numbers(store, $0) }, [[1, 2], [3]])
    }

    func testCommitReturnsFalseWhenNothingWasAppended() throws {
        let store = try store()
        XCTAssertFalse(store.commit())
        _ = try store.append(event(1))
        XCTAssertTrue(store.commit())
        XCTAssertFalse(store.commit())
    }

    func testRecoverCommitsBatchesAPreviousProcessLeftOpenAndKeepsTheSequenceGoing() throws {
        let first = try store(maxEvents: 10)
        _ = try first.append(event(1))
        _ = try first.append(event(2))
        // The process dies here: the batch is still in staging/. Add an empty and a foreign file too.
        first.closeOpenBatch()
        let staging = root.appendingPathComponent("staging")
        FileManager.default.createFile(atPath: staging.appendingPathComponent("0000000099-1.ndjson").path, contents: Data())
        try Data("x".utf8).write(to: staging.appendingPathComponent("notes.txt"))

        let second = try store(maxEvents: 10)
        XCTAssertEqual(second.readyBatches().map { numbers(second, $0) }, [[1, 2]])
        XCTAssertEqual(stagingNames, [])

        _ = try second.append(event(3))
        second.commit()
        XCTAssertEqual(second.readyBatches().map { numbers(second, $0) }, [[1, 2], [3]])
        XCTAssertEqual(second.readyBatches().map(\.seq), [1, 2])
    }

    func testLinesCutShortByACrashAreSkipped() throws {
        let store = try store()
        let name = QueueStore.fileName(seq: 1, createdMillis: T0)
        try writeReady(name, "{\"n\":1}\n{\"n\":2}\n{\"n\":3, \"cut")
        let file = try XCTUnwrap(store.readyBatches().first)
        XCTAssertEqual(numbers(store, file), [1, 2])
        XCTAssertEqual(log.warnings.count, 1)
        XCTAssertTrue(log.warnings.first?.contains("Skipped 1") ?? false)
    }

    func testReadReturnsNilForABatchThatIsGone() throws {
        let store = try store()
        let url = root.appendingPathComponent("ready/0000000001-1.ndjson")
        XCTAssertNil(store.read(BatchFile(url: url, name: url.lastPathComponent, seq: 1, createdMillis: 1)))
        XCTAssertEqual(log.warnings, [])
    }

    func testReadReturnsTheEventsExactlyAsWritten() throws {
        let store = try store(maxEvents: 2)
        let first = Array(#"{"b":1,"a":"/x"}"#.utf8)
        let second = Array(#"{"z":[1.0,null]}"#.utf8)
        _ = try store.append(first)
        _ = try store.append(second)
        let file = try XCTUnwrap(store.readyBatches().first)
        XCTAssertEqual(store.read(file), [Data(first), Data(second)])
        XCTAssertEqual(file.name, "0000000001-\(T0).ndjson")
    }

    func testBatchesAreOrderedBySequenceNotByCreationTime() throws {
        let store = try store(maxEvents: 1)
        time.now = T0 + 10_000
        _ = try store.append(event(1))
        time.now = T0 // the clock moved back
        _ = try store.append(event(2))
        XCTAssertEqual(store.readyBatches().map { numbers(store, $0) }, [[1], [2]])
    }

    func testExpiredBatchesAreDroppedAndFutureDatedOnesKept() throws {
        let store = try store()
        try writeReady(QueueStore.fileName(seq: 1, createdMillis: T0 - QueueStore.maxAgeMillis - 1), "{\"n\":1}\n")
        try writeReady(QueueStore.fileName(seq: 2, createdMillis: T0 - 1000), "{\"n\":2}\n")
        try writeReady(QueueStore.fileName(seq: 3, createdMillis: T0 + 86_400_000), "{\"n\":3}\n")

        store.enforceLimits()

        XCTAssertEqual(store.readyBatches().map { numbers(store, $0) }, [[2], [3]])
        XCTAssertEqual(log.warnings.count, 1)
        XCTAssertTrue(log.warnings.first?.contains("older than") ?? false)
    }

    func testAnOversizedQueueDropsItsOldestBatchesDownTo80Percent() throws {
        let store = try store(maxEvents: 1, maxQueueBytes: 100)
        for _ in 0..<10 {
            _ = try store.append(Array(#"{"pad":"xxxxxxxxx"}"#.utf8)) // 10 batches of 20 bytes = 200
        }

        store.enforceLimits()

        XCTAssertEqual(store.readyBatches().map(\.seq), [7, 8, 9, 10]) // 80 bytes <= 80 % of 100
    }

    func testNamesAreParsedStrictly() {
        XCTAssertEqual(QueueStore.fileName(seq: 42, createdMillis: 7), "0000000042-7.ndjson")
        XCTAssertEqual(QueueStore.parseName("0000000042-7.ndjson")?.seq, 42)
        XCTAssertEqual(QueueStore.parseName("0000000042-7.ndjson")?.createdMillis, 7)
        for bad in ["notes.txt", "1-2-3.ndjson", "-1.ndjson", "1-.ndjson", "a-1.ndjson", "1-1.ndjson.tmp", "1234567890123456789-1.ndjson", "١-1.ndjson"] {
            XCTAssertNil(QueueStore.parseName(bad), bad)
        }
    }

    func testADeletedStagingDirectoryIsCreatedAgain() throws {
        let store = try store(maxEvents: 1)
        try FileManager.default.removeItem(at: root.appendingPathComponent("staging"))
        XCTAssertTrue(try store.append(event(1)))
        XCTAssertEqual(store.readyBatches().map { numbers(store, $0) }, [[1]])
    }

    func testTheQueueCanBeExcludedFromBackup() throws {
        try FileIO.excludeFromBackup(root)
        XCTAssertEqual(try root.resourceValues(forKeys: [.isExcludedFromBackupKey]).isExcludedFromBackup, true)
    }
}
