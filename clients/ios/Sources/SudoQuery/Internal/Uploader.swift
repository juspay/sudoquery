import Foundation

/// What the event writer needs from the uploader.
protocol UploadControl: AnyObject, Sendable {
    /// Asks for queued batches to be sent. Cheap; requests made while one is pending collapse into it.
    func requestUpload()

    /// Forgets the retry backoff, e.g. when the app comes back to the foreground.
    func resetBackoff()

    /// True while uploads are failing and waiting to retry.
    var isBackingOff: Bool { get }

    /// Calls `done` once the uploader has nothing it can send right now: every batch has been
    /// tried, or it is waiting out a backoff.
    func whenIdle(_ done: @escaping @Sendable () -> Void)
}

/// How the SDK treats a collector response.
enum Delivery: Equatable {
    /// Delivered: delete the batch.
    case sent
    /// The collector will never accept this batch: delete it and move on.
    case rejected
    /// Keep the batch and try again later.
    case retry
}

/// 400 (bad headers or body), 413 (too large) and 422 can't succeed on a resend. Everything else
/// (redirects, 401/403/404 configuration problems, 408, 429, 5xx) may, so the batch is kept.
func classify(_ code: Int) -> Delivery {
    switch code {
    case 200...299: return .sent
    case 400, 413, 422: return .rejected
    default: return .retry
    }
}

/// Sends committed batches to the collector, one file per request, oldest first. Ported from
/// hyper-sdk-ios's `HPJPLogPusher` (`processBatchLogs`), which deleted a batch after any response
/// whose status wasn't in an (empty by default) retry list, so 4xx, 5xx and offline all lost data,
/// and which retried back to back. Here any 2xx counts as delivered, rejected batches are dropped,
/// and every other failure keeps the batch and backs off exponentially with jitter.
///
/// It runs on its own serial queue and never blocks it: a request is a `URLSession` task, and its
/// result is handled back on the queue. Only one request is in flight at a time.
final class Uploader: UploadControl, @unchecked Sendable {
    static let baseBackoffMillis: Int64 = 5_000
    static let maxBackoffMillis: Int64 = 5 * 60 * 1000
    static let maxRetryAfterSeconds: Int64 = 60 * 60

    private let queue = DispatchQueue(label: "io.juspay.sudoquery.uploader", qos: .utility)
    private let store: QueueStore
    private let transport: any Transport
    private let time: any TimeSource
    private let scheduler: any WorkScheduler
    private let log: any SDKLog
    private let timezone: @Sendable () -> String?
    private let random: @Sendable (ClosedRange<Int64>) -> Int64

    /// Read by the writer queue too.
    private let backoff = Locked(Backoff())

    private struct Backoff {
        var failures = 0
        var retryAtMillis: Int64 = 0
    }

    // Uploader queue only.
    private var stopped = false
    private var pending = false
    private var draining = false
    private var attempted = Set<String>()
    private var pass: [BatchFile] = []
    private var passIndex = 0
    private var wake: (any Cancellable)?
    private var wakeAtMillis: Int64?
    private var waiters: [@Sendable () -> Void] = []

    init(
        store: QueueStore,
        transport: any Transport,
        time: any TimeSource,
        scheduler: any WorkScheduler,
        log: any SDKLog,
        timezone: @escaping @Sendable () -> String? = { TimeZone.autoupdatingCurrent.identifier },
        random: @escaping @Sendable (ClosedRange<Int64>) -> Int64 = { Int64.random(in: $0) }
    ) {
        self.store = store
        self.transport = transport
        self.time = time
        self.scheduler = scheduler
        self.log = log
        self.timezone = timezone
        self.random = random
    }

    func requestUpload() {
        queue.async {
            self.pending = true
            self.pump()
        }
    }

    func resetBackoff() {
        backoff.withLock { $0 = Backoff() }
    }

    var isBackingOff: Bool {
        time.elapsedMillis() < backoff.withLock { $0.retryAtMillis }
    }

    func whenIdle(_ done: @escaping @Sendable () -> Void) {
        queue.async {
            if self.isIdle {
                done()
            } else {
                self.waiters.append(done)
            }
        }
    }

    /// Sends every ready batch now, even while backing off, then calls `done`. For tests.
    func drainNow(_ done: @escaping @Sendable () -> Void) {
        queue.async {
            if !self.draining { self.startDrain() }
            if self.isIdle { done() } else { self.waiters.append(done) }
        }
    }

    /// Stops sending, for good, and cancels a pending backoff wake-up. For tests.
    func shutdown() {
        queue.sync {
            stopped = true
            wake?.cancel()
            wake = nil
            wakeAtMillis = nil
            fireWaiters()
        }
    }

    private var isIdle: Bool {
        stopped || (!draining && (!pending || isBackingOff))
    }

    /// Starts a drain if one is wanted and allowed; otherwise, when the backoff ends, wakes up to
    /// retry without being asked again.
    private func pump() {
        if stopped {
            fireWaiters()
            return
        }
        guard !draining else { return }
        let now = time.elapsedMillis()
        let retryAt = backoff.withLock { $0.retryAtMillis }
        if retryAt > now {
            if pending { scheduleWake(at: retryAt, now: now) }
            fireWaiters()
            return
        }
        guard pending else {
            fireWaiters()
            return
        }
        pending = false
        startDrain()
    }

    private func scheduleWake(at retryAt: Int64, now: Int64) {
        if wakeAtMillis == retryAt { return }
        wake?.cancel()
        wakeAtMillis = retryAt
        wake = scheduler.schedule(afterMillis: retryAt - now, on: queue) { [weak self] in
            guard let self else { return }
            self.wake = nil
            self.wakeAtMillis = nil
            self.pump()
        }
    }

    private func startDrain() {
        draining = true
        attempted = []
        pass = []
        passIndex = 0
        sendNext()
    }

    /// Sends the next ready batch. Each file is tried at most once per drain, and the queue is
    /// listed again after each pass so batches committed meanwhile go out too.
    private func sendNext() {
        while true {
            if stopped {
                finishDrain()
                return
            }
            if passIndex >= pass.count {
                pass = store.readyBatches().filter { !attempted.contains($0.name) }
                passIndex = 0
                if pass.isEmpty {
                    finishDrain()
                    return
                }
            }
            let file = pass[passIndex]
            passIndex += 1
            attempted.insert(file.name)
            guard let events = store.read(file) else { continue }
            if events.isEmpty {
                store.delete(file)
                continue
            }
            let body = BatchBody.build(events: events, systemProperties: SystemProperties(geo: nil, timezone: timezone()))
            transport.post(body) { result in
                self.queue.async { self.handle(result, file: file, count: events.count) }
            }
            return
        }
    }

    private func handle(_ result: Result<HTTPResult, any Error>, file: BatchFile, count: Int) {
        let response: HTTPResult
        switch result {
        case .failure(let error):
            retryLater("Could not reach the collector (\(describeError(error)))", retryAfterSeconds: nil)
            return
        case .success(let value):
            response = value
        }
        switch classify(response.code) {
        case .sent:
            store.delete(file)
            backoff.withLock { $0.failures = 0 }
            log.debug("Sent \(count) event(s)")
            sendNext()
        case .rejected:
            store.delete(file)
            log.error("The collector rejected \(count) event(s) with HTTP \(response.code), dropping them: \(errorText(response.body))")
            sendNext()
        case .retry:
            retryLater("The collector answered HTTP \(response.code)", retryAfterSeconds: response.retryAfterSeconds)
        }
    }

    /// Stops at the first batch to retry, so the queue stays in order, and backs off.
    private func retryLater(_ reason: String, retryAfterSeconds: Int64?) {
        let now = time.elapsedMillis()
        let (failures, delay) = backoff.withLock { state -> (Int, Int64) in
            state.failures += 1
            let delay = backoffMillis(failures: state.failures, retryAfterSeconds: retryAfterSeconds)
            state.retryAtMillis = now + delay
            return (state.failures, delay)
        }
        let message = "\(reason); events are kept and retried in \((delay + 999) / 1000) s"
        if failures == 1 {
            log.warn(message)
        } else {
            log.debug(message)
        }
        // Retry on our own once the backoff ends.
        pending = true
        finishDrain()
    }

    /// 5 s doubling to 5 min, with half of it randomized so many devices don't retry in step.
    private func backoffMillis(failures: Int, retryAfterSeconds: Int64?) -> Int64 {
        let shift = Int64(min(max(failures - 1, 0), 16))
        let exponential = min(Self.baseBackoffMillis << shift, Self.maxBackoffMillis)
        let jittered = exponential / 2 + random(0...(exponential / 2))
        let requested = min(max(retryAfterSeconds ?? 0, 0), Self.maxRetryAfterSeconds) * 1000
        return max(jittered, requested)
    }

    private func finishDrain() {
        draining = false
        pass = []
        passIndex = 0
        pump()
    }

    private func fireWaiters() {
        guard !waiters.isEmpty else { return }
        let ready = waiters
        waiters = []
        for done in ready {
            done()
        }
    }

    /// The collector's `{"error": "..."}` message, or the raw body (a 413 is plain text).
    private func errorText(_ body: String?) -> String {
        let text = (body ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        var message: String?
        if let data = text.data(using: .utf8),
           let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
           let error = object["error"] as? String {
            message = error
        }
        return String((message ?? text).prefix(300))
    }
}
