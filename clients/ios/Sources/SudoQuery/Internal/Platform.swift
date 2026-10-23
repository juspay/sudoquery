import Foundation
import os.log

/// A value guarded by a lock, for the little state that more than one queue touches.
final class Locked<Value>: @unchecked Sendable {
    private let lock = NSLock()
    private var value: Value

    init(_ value: Value) {
        self.value = value
    }

    func withLock<Result>(_ body: (inout Value) throws -> Result) rethrows -> Result {
        lock.lock()
        defer { lock.unlock() }
        return try body(&value)
    }

    var current: Value {
        withLock { $0 }
    }
}

protocol TimeSource: Sendable {
    /// Wall-clock milliseconds since 1970, for event timestamps, file ages and sessions.
    func nowMillis() -> Int64

    /// Monotonic milliseconds that ignore clock changes, for retry backoff.
    func elapsedMillis() -> Int64
}

struct SystemTime: TimeSource {
    func nowMillis() -> Int64 {
        millis(Date().timeIntervalSince1970)
    }

    // The same clock as `DispatchQueue.asyncAfter`, so a backoff wake-up and `isBackingOff` agree.
    // (`ProcessInfo.systemUptime` would need a privacy-manifest reason.)
    func elapsedMillis() -> Int64 {
        Int64(clamping: DispatchTime.now().uptimeNanoseconds / 1_000_000)
    }
}

/// Seconds to whole milliseconds, rounded down. Saturates instead of trapping on huge or NaN input.
func millis(_ seconds: TimeInterval) -> Int64 {
    let value = (seconds * 1000).rounded(.down)
    if value.isNaN { return 0 }
    if value >= Double(Int64.max) { return .max }
    if value < Double(Int64.min) { return .min }
    return Int64(value)
}

protocol Cancellable: Sendable {
    func cancel()
}

protocol WorkScheduler: Sendable {
    /// Runs `work` on `queue` after `delayMillis`, unless it is cancelled first.
    func schedule(afterMillis delayMillis: Int64, on queue: DispatchQueue, _ work: @escaping @Sendable () -> Void) -> any Cancellable
}

struct DispatchWorkScheduler: WorkScheduler {
    func schedule(afterMillis delayMillis: Int64, on queue: DispatchQueue, _ work: @escaping @Sendable () -> Void) -> any Cancellable {
        let item = ScheduledWork(work)
        queue.asyncAfter(deadline: .now() + .milliseconds(Int(clamping: max(0, delayMillis)))) { item.run() }
        return item
    }
}

private final class ScheduledWork: Cancellable, @unchecked Sendable {
    private let state: Locked<(@Sendable () -> Void)?>

    init(_ work: @escaping @Sendable () -> Void) {
        state = Locked(work)
    }

    func run() {
        let work = state.withLock { work -> (@Sendable () -> Void)? in
            defer { work = nil }
            return work
        }
        work?()
    }

    func cancel() {
        state.withLock { $0 = nil }
    }
}

protocol SDKLog: Sendable {
    func debug(_ message: String)
    func warn(_ message: String, error: (any Error)?)
    func error(_ message: String, error: (any Error)?)
}

extension SDKLog {
    func warn(_ message: String) {
        warn(message, error: nil)
    }

    func error(_ message: String) {
        error(message, error: nil)
    }
}

/// Logs to the unified log under the `io.juspay.sudoquery` subsystem. Debug messages appear only
/// with `debugLogging`; warnings and errors always do. Debug messages are logged at the default
/// level, not `.debug`, so that once switched on they show up in Console and `log show` like
/// Android's logcat lines do.
struct OSLogLog: SDKLog {
    let debugEnabled: Bool

    private static let log = OSLog(subsystem: "io.juspay.sudoquery", category: "SudoQuery")

    func debug(_ message: String) {
        guard debugEnabled else { return }
        os_log("%{public}@", log: Self.log, type: .default, message)
    }

    func warn(_ message: String, error: (any Error)?) {
        os_log("%{public}@", log: Self.log, type: .default, describe(message, error))
    }

    func error(_ message: String, error: (any Error)?) {
        os_log("%{public}@", log: Self.log, type: .error, describe(message, error))
    }

    private func describe(_ message: String, _ error: (any Error)?) -> String {
        guard let error else { return message }
        return "\(message): \(describeError(error))"
    }
}

/// An error as one line, e.g. `NSURLErrorDomain -1004: Could not connect to the server.`
func describeError(_ error: any Error) -> String {
    let nsError = error as NSError
    return "\(nsError.domain) \(nsError.code): \(nsError.localizedDescription)"
}

enum StorageLocation {
    /// `Library/Application Support/sudoquery/v1`, created if needed, with `sudoquery` excluded
    /// from backup. Application Support is never purged by the OS (unlike Caches). On macOS and Mac
    /// Catalyst it can be shared by every app of a user, so the bundle id goes in front of `sudoquery`.
    static func defaultRoot() throws -> URL {
        var base = try FileManager.default.url(
            for: .applicationSupportDirectory,
            in: .userDomainMask,
            appropriateFor: nil,
            create: true
        )
        #if os(macOS) || targetEnvironment(macCatalyst)
        base.appendPathComponent(Bundle.main.bundleIdentifier ?? ProcessInfo.processInfo.processName, isDirectory: true)
        #endif
        let folder = base.appendingPathComponent("sudoquery", isDirectory: true)
        let root = folder.appendingPathComponent("v1", isDirectory: true)
        try FileIO.ensureDirectory(root)
        try FileIO.excludeFromBackup(folder)
        return root
    }
}
