import Foundation

/// What a client needs from its surroundings. Tests replace parts of it.
struct ClientEnvironment: Sendable {
    var storageRoot: @Sendable () throws -> URL = { try StorageLocation.defaultRoot() }
    var time: any TimeSource = SystemTime()
    var scheduler: any WorkScheduler = DispatchWorkScheduler()
    var makeTransport: @Sendable (URL, [(name: String, value: String)]) -> any Transport = {
        URLSessionTransport(endpoint: $0, headers: $1)
    }
    var timezone: @Sendable () -> String? = { TimeZone.autoupdatingCurrent.identifier }
    var makeLog: @Sendable (_ debugEnabled: Bool) -> any SDKLog = { OSLogLog(debugEnabled: $0) }
    var observesLifecycle = true
}

/// One initialized SDK. App threads only enqueue commands; the writer queue handles them in order,
/// and the uploader does the network I/O on its own queue, so a slow network never blocks writes.
/// Nothing here runs on the main thread (hyper-sdk-ios did its log file I/O there).
final class SudoQueryClient: @unchecked Sendable {
    static let maxQueuedTracks = 1_000

    private let writerQueue = DispatchQueue(label: "io.juspay.sudoquery.writer", qos: .utility)
    private let log: any SDKLog
    private let backlog = Locked(Backlog())
    private let user = UserMirror()

    private struct Backlog {
        var queued = 0
        var overflowWarned = false
    }

    // Writer queue only.
    private var writer: EventWriter?
    private var store: QueueStore?
    private var uploader: Uploader?
    private var transport: (any Transport)?

    // Set while starting, read by shutdown().
    private var timer: (any DispatchSourceTimer)?
    private var lifecycle: AnyObject?

    private init(log: any SDKLog) {
        self.log = log
    }

    static func start(config: SudoQueryConfig, log: any SDKLog, environment: ClientEnvironment) -> SudoQueryClient {
        let client = SudoQueryClient(log: log)
        // Storage touches the disk, so it is set up on the writer queue, not on the app's thread.
        // Commands sent meanwhile wait behind it.
        client.writerQueue.async {
            client.setUp(config: config, environment: environment)
        }
        if config.flushIntervalMillis > 0 {
            client.startTimer(intervalMillis: config.flushIntervalMillis)
        }
        if environment.observesLifecycle {
            client.observeLifecycle()
        }
        return client
    }

    func track(_ name: String, timestampMillis: Int64, properties: JSONObject) {
        // Only events are capped: control commands such as setUser must never be dropped.
        let (accepted, warn) = backlog.withLock { state -> (Bool, Bool) in
            guard state.queued < Self.maxQueuedTracks else {
                defer { state.overflowWarned = true }
                return (false, !state.overflowWarned)
            }
            state.queued += 1
            return (true, false)
        }
        if warn {
            log.warn("More than \(Self.maxQueuedTracks) events are waiting to be stored; dropping new ones until they are")
        }
        if accepted {
            send(.track(name: name, timestampMillis: timestampMillis, properties: properties))
        }
    }

    func setUser(_ userId: String?) {
        user.set(userId)
        send(.setUser(userId))
    }

    func currentUser() -> String? {
        user.get()
    }

    func reset() {
        user.set(nil)
        send(.reset)
    }

    func flush() {
        send(.flush)
    }

    func onForeground() {
        send(.foreground)
    }

    func onBackground(done: @escaping @Sendable () -> Void) {
        send(.background(done: done))
    }

    /// Runs `work` on the writer queue after every command sent so far. For tests.
    func onWriterQueueForTests(_ work: @escaping @Sendable () -> Void) {
        send(.barrier(work))
    }

    /// Waits until every command sent so far has been handled and the uploader is idle. For tests;
    /// never call it on the writer queue.
    @discardableResult
    func awaitIdle(timeout: TimeInterval = 10) -> Bool {
        let done = DispatchSemaphore(value: 0)
        writerQueue.async {
            if let uploader = self.uploader {
                uploader.whenIdle { done.signal() }
            } else {
                done.signal()
            }
        }
        return done.wait(timeout: .now() + timeout) == .success
    }

    /// Stops the timer, the lifecycle observers and the network session. For tests: the SDK
    /// otherwise lives as long as the process.
    func shutdown() {
        timer?.cancel()
        #if canImport(UIKit) && !os(watchOS)
        (lifecycle as? AppLifecycleObserver)?.unregister()
        #endif
        writerQueue.sync {
            store?.closeOpenBatch()
            uploader?.shutdown()
            (transport as? URLSessionTransport)?.invalidate()
            writer = nil
        }
    }

    private func setUp(config: SudoQueryConfig, environment: ClientEnvironment) {
        let root: URL
        do {
            root = try environment.storageRoot()
        } catch {
            log.error("Could not set up the event store; events will be dropped", error: error)
            return
        }
        let store = QueueStore(root: root, maxEventsPerBatch: config.batchSize, time: environment.time, log: log)
        let headers = requestHeaders(
            orgId: config.orgId,
            projectId: config.projectId,
            token: config.token,
            custom: config.headers,
            userAgent: defaultUserAgent()
        )
        let transport = environment.makeTransport(config.endpointURL, headers)
        let uploader = Uploader(
            store: store,
            transport: transport,
            time: environment.time,
            scheduler: environment.scheduler,
            log: log,
            timezone: environment.timezone
        )
        let writer = EventWriter(
            // The only place that names the event type; see `CanonicalEvent` in Event.swift.
            encode: { CanonicalEvent.encode($0) },
            settings: WriterSettings(
                orgId: config.orgId,
                projectId: config.projectId,
                source: config.source,
                fixedSessionId: config.sessionId,
                sessionTimeoutMillis: config.sessionTimeoutMillis
            ),
            store: store,
            identityStore: IdentityStore(url: root.appendingPathComponent("identity.json"), log: log),
            uploads: uploader,
            log: log,
            onStoredUser: { [user] in user.loaded($0) }
        )
        self.store = store
        self.uploader = uploader
        self.transport = transport
        self.writer = writer
        user.loaded(writer.start())
    }

    private func send(_ command: Command) {
        writerQueue.async {
            self.handle(command)
        }
    }

    private func handle(_ command: Command) {
        if case .track = command {
            backlog.withLock { state in
                state.queued -= 1
                if state.queued < Self.maxQueuedTracks / 2 { state.overflowWarned = false }
            }
        }
        guard let writer else {
            // No storage: nothing is stored, but nobody waiting on a command is left hanging.
            switch command {
            case .barrier(let done): done()
            case .background(let done): done?()
            default: break
            }
            return
        }
        writer.handle(command)
    }

    /// A `DispatchSourceTimer` on the writer queue. hyper-sdk-ios used `NSTimer`s on the main run
    /// loop, which stop firing while the user scrolls.
    private func startTimer(intervalMillis: Int64) {
        let timer = DispatchSource.makeTimerSource(queue: writerQueue)
        let interval = DispatchTimeInterval.milliseconds(Int(clamping: intervalMillis))
        timer.schedule(deadline: .now() + interval, repeating: interval, leeway: .seconds(1))
        timer.setEventHandler { [weak self] in
            self?.handle(.tick)
        }
        timer.resume()
        self.timer = timer
    }

    private func observeLifecycle() {
        #if canImport(UIKit) && !os(watchOS)
        let observer = AppLifecycleObserver(
            onBackground: { [weak self] done in
                if let self {
                    self.onBackground(done: done)
                } else {
                    done()
                }
            },
            onForeground: { [weak self] in
                self?.onForeground()
            }
        )
        observer.register()
        lifecycle = observer
        #endif
    }
}

/// `SudoQuery.user` must reflect `setUser` at once, before the writer has loaded the stored user.
private final class UserMirror: Sendable {
    private let state = Locked<(value: String?, setByApp: Bool)>((nil, false))

    func set(_ userId: String?) {
        state.withLock { $0 = (userId, true) }
    }

    func loaded(_ userId: String?) {
        state.withLock { state in
            if !state.setByApp { state.value = userId }
        }
    }

    func get() -> String? {
        state.withLock { $0.value }
    }
}
