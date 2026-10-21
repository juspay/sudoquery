#if canImport(UIKit) && !os(watchOS)
import UIKit

/// Background-task calls, so tests can stand in for `UIApplication`.
protocol BackgroundTasks: Sendable {
    func begin(name: String, expiration: @escaping @Sendable () -> Void) -> UIBackgroundTaskIdentifier?
    func end(_ identifier: UIBackgroundTaskIdentifier)
}

struct UIApplicationBackgroundTasks: BackgroundTasks {
    let application: UIApplication

    /// The app's `UIApplication`, or nil in an app extension, where there is none. Looked up by
    /// name because `UIApplication.shared` doesn't compile for extensions.
    static func current() -> UIApplicationBackgroundTasks? {
        guard !Bundle.main.bundlePath.hasSuffix(".appex"),
              let application = (UIApplication.self as AnyObject).value(forKey: "sharedApplication") as? UIApplication
        else { return nil }
        return UIApplicationBackgroundTasks(application: application)
    }

    func begin(name: String, expiration: @escaping @Sendable () -> Void) -> UIBackgroundTaskIdentifier? {
        let identifier = application.beginBackgroundTask(withName: name, expirationHandler: expiration)
        return identifier == .invalid ? nil : identifier
    }

    func end(_ identifier: UIBackgroundTaskIdentifier) {
        application.endBackgroundTask(identifier)
    }
}

/// Reports the app moving between foreground and background. Going to the background starts a
/// background task, so the batch committed then can still be uploaded, and ends it when the
/// uploader is idle or iOS runs out of time. hyper-sdk-ios flushed on timers only.
///
/// The UIKit calls used here are nonisolated, so nothing needs the main actor.
final class AppLifecycleObserver: @unchecked Sendable {
    static let taskName = "SudoQuery flush"

    private let center: NotificationCenter
    private let tasks: @Sendable () -> (any BackgroundTasks)?
    private let onBackground: @Sendable (_ done: @escaping @Sendable () -> Void) -> Void
    private let onForeground: @Sendable () -> Void
    private let current = Locked<TaskSlot?>(nil)
    private let tokens = Locked<[any NSObjectProtocol]>([])

    init(
        center: NotificationCenter = .default,
        tasks: @escaping @Sendable () -> (any BackgroundTasks)? = { UIApplicationBackgroundTasks.current() },
        onBackground: @escaping @Sendable (_ done: @escaping @Sendable () -> Void) -> Void,
        onForeground: @escaping @Sendable () -> Void
    ) {
        self.center = center
        self.tasks = tasks
        self.onBackground = onBackground
        self.onForeground = onForeground
    }

    func register() {
        let background = center.addObserver(forName: UIApplication.didEnterBackgroundNotification, object: nil, queue: nil) { [weak self] _ in
            self?.didEnterBackground()
        }
        let foreground = center.addObserver(forName: UIApplication.willEnterForegroundNotification, object: nil, queue: nil) { [weak self] _ in
            self?.onForeground()
        }
        tokens.withLock { $0 = [background, foreground] }
    }

    func unregister() {
        let registered = tokens.withLock { tokens in
            defer { tokens = [] }
            return tokens
        }
        for token in registered {
            center.removeObserver(token)
        }
    }

    private func didEnterBackground() {
        let provider = tasks()
        let slot = TaskSlot(provider: provider)
        // A task from an earlier trip to the background that hasn't finished is ended now.
        let previous = current.withLock { current in
            defer { current = slot }
            return current
        }
        previous?.finish()

        if let provider, let identifier = provider.begin(name: Self.taskName, expiration: { slot.finish() }) {
            slot.started(identifier)
        }
        onBackground { slot.finish() }
    }
}

/// One background task, ended exactly once: by the flush finishing, by expiry, or by the next
/// trip to the background. If it finishes before `begin` has returned, it is ended right away.
private final class TaskSlot: Sendable {
    private let provider: (any BackgroundTasks)?
    private let state = Locked<(identifier: UIBackgroundTaskIdentifier?, finished: Bool)>((nil, false))

    init(provider: (any BackgroundTasks)?) {
        self.provider = provider
    }

    func started(_ identifier: UIBackgroundTaskIdentifier) {
        let endNow = state.withLock { state -> Bool in
            if state.finished { return true }
            state.identifier = identifier
            return false
        }
        if endNow { provider?.end(identifier) }
    }

    func finish() {
        let identifier = state.withLock { state -> UIBackgroundTaskIdentifier? in
            if state.finished { return nil }
            state.finished = true
            defer { state.identifier = nil }
            return state.identifier
        }
        if let identifier { provider?.end(identifier) }
    }
}
#endif
