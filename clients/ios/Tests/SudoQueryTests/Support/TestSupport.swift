import Foundation
import XCTest
@testable import SudoQuery

/// 2025-10-09T08:53:20Z, a fixed "now" for tests.
let T0: Int64 = 1_760_000_000_000

final class FakeTime: TimeSource {
    private let state = Locked((now: T0, elapsed: Int64(0)))

    var now: Int64 {
        get { state.current.now }
        set { state.withLock { $0.now = newValue } }
    }

    var elapsed: Int64 {
        get { state.current.elapsed }
        set { state.withLock { $0.elapsed = newValue } }
    }

    func nowMillis() -> Int64 { now }

    func elapsedMillis() -> Int64 { elapsed }
}

final class RecordingLog: SDKLog {
    private let state = Locked((warnings: [String](), errors: [String]()))

    var warnings: [String] { state.current.warnings }
    var errors: [String] { state.current.errors }

    func debug(_ message: String) {}

    func warn(_ message: String, error: (any Error)?) {
        state.withLock { $0.warnings.append(message) }
    }

    func error(_ message: String, error: (any Error)?) {
        state.withLock { $0.errors.append(message) }
    }
}

final class FakeUploads: UploadControl {
    private let state = Locked((requests: 0, resets: 0, backingOff: false, idleWaits: 0))

    var requests: Int { state.current.requests }
    var resets: Int { state.current.resets }
    var idleWaits: Int { state.current.idleWaits }

    var isBackingOff: Bool {
        get { state.current.backingOff }
        set { state.withLock { $0.backingOff = newValue } }
    }

    func requestUpload() {
        state.withLock { $0.requests += 1 }
    }

    func resetBackoff() {
        state.withLock {
            $0.resets += 1
            $0.backingOff = false
        }
    }

    func whenIdle(_ done: @escaping @Sendable () -> Void) {
        state.withLock { $0.idleWaits += 1 }
        done()
    }
}

/// Replies with the queued outcomes in order, then with 200. Records every body it was given.
final class FakeTransport: Transport {
    private enum Outcome {
        case reply(HTTPResult)
        case fail(any Error)
    }

    private let state = Locked((bodies: [Data](), outcomes: [Outcome]()))

    var bodies: [Data] { state.current.bodies }
    var bodyStrings: [String] { bodies.map { String(decoding: $0, as: UTF8.self) } }

    func reply(_ code: Int, body: String = "{}", retryAfterSeconds: Int64? = nil) {
        state.withLock { $0.outcomes.append(.reply(HTTPResult(code: code, retryAfterSeconds: retryAfterSeconds, body: body))) }
    }

    func failWith(_ error: any Error) {
        state.withLock { $0.outcomes.append(.fail(error)) }
    }

    func post(_ body: Data, completion: @escaping @Sendable (Result<HTTPResult, any Error>) -> Void) {
        let outcome = state.withLock { state -> Outcome in
            state.bodies.append(body)
            guard !state.outcomes.isEmpty else {
                return .reply(HTTPResult(code: 200, retryAfterSeconds: nil, body: #"{"filtered":0,"collected":1,"total":1}"#))
            }
            return state.outcomes.removeFirst()
        }
        switch outcome {
        case .reply(let result): completion(.success(result))
        case .fail(let error): completion(.failure(error))
        }
    }

    /// The events of the `index`th request.
    func events(_ index: Int) -> [[String: Any]] {
        let body = bodies[index]
        let object = (try? JSONSerialization.jsonObject(with: body)) as? [String: Any]
        return object?["events"] as? [[String: Any]] ?? []
    }
}

/// Keeps every request in flight until the test answers it with `release`.
final class HoldingTransport: Transport {
    private let held = Locked<[@Sendable (Result<HTTPResult, any Error>) -> Void]>([])

    var heldCount: Int { held.current.count }

    func post(_ body: Data, completion: @escaping @Sendable (Result<HTTPResult, any Error>) -> Void) {
        held.withLock { $0.append(completion) }
    }

    func release(code: Int = 200) {
        let next = held.withLock { $0.isEmpty ? nil : $0.removeFirst() }
        next?(.success(HTTPResult(code: code, retryAfterSeconds: nil, body: "{}")))
    }
}

/// Polls `condition` until it holds or `timeout` passes.
func eventually(timeout: TimeInterval = 5, _ condition: () -> Bool) -> Bool {
    let deadline = Date().addingTimeInterval(timeout)
    while !condition() {
        if Date() > deadline { return false }
        Thread.sleep(forTimeInterval: 0.01)
    }
    return true
}

/// Runs scheduled work only when a test moves time forward with `advance(to:)`.
final class ManualScheduler: WorkScheduler {
    private struct Entry {
        let id: Int
        let dueMillis: Int64
        let queue: DispatchQueue
        let work: @Sendable () -> Void
    }

    private let time: FakeTime
    private let state = Locked((nextId: 0, entries: [Entry]()))

    init(time: FakeTime) {
        self.time = time
    }

    var pendingCount: Int { state.current.entries.count }

    func schedule(afterMillis delayMillis: Int64, on queue: DispatchQueue, _ work: @escaping @Sendable () -> Void) -> any Cancellable {
        let due = time.elapsed + delayMillis
        let id = state.withLock { state -> Int in
            state.nextId += 1
            state.entries.append(Entry(id: state.nextId, dueMillis: due, queue: queue, work: work))
            return state.nextId
        }
        return ManualCancellable { [state] in
            state.withLock { $0.entries.removeAll { $0.id == id } }
        }
    }

    /// Sets the elapsed time and runs the work that came due, each on its queue.
    func advance(to elapsed: Int64) {
        time.elapsed = elapsed
        let due = state.withLock { state -> [Entry] in
            let due = state.entries.filter { $0.dueMillis <= elapsed }
            state.entries.removeAll { $0.dueMillis <= elapsed }
            return due
        }
        for entry in due {
            entry.queue.async { entry.work() }
        }
    }
}

struct ManualCancellable: Cancellable {
    let onCancel: @Sendable () -> Void

    func cancel() {
        onCancel()
    }
}

/// A seeded generator, so backoff jitter is the same on every run.
struct SplitMix64: RandomNumberGenerator {
    private var state: UInt64

    init(seed: UInt64) {
        state = seed
    }

    mutating func next() -> UInt64 {
        state &+= 0x9E37_79B9_7F4A_7C15
        var z = state
        z = (z ^ (z >> 30)) &* 0xBF58_476D_1CE4_E5B9
        z = (z ^ (z >> 27)) &* 0x94D0_49BB_1331_11EB
        return z ^ (z >> 31)
    }
}

func seededRandom(_ seed: UInt64 = 1) -> @Sendable (ClosedRange<Int64>) -> Int64 {
    let generator = Locked(SplitMix64(seed: seed))
    return { range in generator.withLock { Int64.random(in: range, using: &$0) } }
}

/// A fresh empty directory, removed by `removeTempDirectories()`.
func makeTempDirectory(_ name: String = "sudoquery") -> URL {
    let url = FileManager.default.temporaryDirectory
        .appendingPathComponent("\(name)-\(UUID().uuidString)", isDirectory: true)
    try? FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
    return url
}

func removeDirectory(_ url: URL) {
    try? FileManager.default.removeItem(at: url)
}

/// The objects in an NDJSON file.
func ndjsonObjects(_ url: URL) -> [[String: Any]] {
    guard let data = try? Data(contentsOf: url) else { return [] }
    return data.split(separator: UInt8(ascii: "\n")).compactMap {
        (try? JSONSerialization.jsonObject(with: Data($0))) as? [String: Any]
    }
}

/// `value` as compact JSON with sorted keys and unescaped slashes, for comparing parsed JSON.
func canonicalJSON(_ value: Any) -> String {
    guard let data = try? JSONSerialization.data(
        withJSONObject: value,
        options: [.sortedKeys, .withoutEscapingSlashes, .fragmentsAllowed]
    ) else { return "<not JSON>" }
    return String(decoding: data, as: UTF8.self)
}

/// A string field, or nil when it is JSON null or missing.
func text(_ object: [String: Any], _ key: String) -> String? {
    object[key] as? String
}

extension Uploader {
    /// Sends every ready batch now and waits until the uploader is idle again.
    func drainAndWait(timeout: TimeInterval = 5, file: StaticString = #filePath, line: UInt = #line) {
        let done = DispatchSemaphore(value: 0)
        drainNow { done.signal() }
        XCTAssertEqual(done.wait(timeout: .now() + timeout), .success, "the uploader did not finish", file: file, line: line)
    }

    /// Asks for an upload, as the writer does, and waits until the uploader is idle again.
    func requestAndWait(timeout: TimeInterval = 5, file: StaticString = #filePath, line: UInt = #line) {
        let done = DispatchSemaphore(value: 0)
        requestUpload()
        whenIdle { done.signal() }
        XCTAssertEqual(done.wait(timeout: .now() + timeout), .success, "the uploader did not finish", file: file, line: line)
    }

    /// Waits until the uploader is idle.
    func waitUntilIdle(timeout: TimeInterval = 5, file: StaticString = #filePath, line: UInt = #line) {
        let done = DispatchSemaphore(value: 0)
        whenIdle { done.signal() }
        XCTAssertEqual(done.wait(timeout: .now() + timeout), .success, "the uploader did not finish", file: file, line: line)
    }
}
