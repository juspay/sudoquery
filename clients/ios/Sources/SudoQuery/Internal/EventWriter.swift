import Foundation

/// Work for the event writer, handled strictly in the order the app made the calls.
enum Command: Sendable {
    case track(name: String, timestampMillis: Int64, properties: JSONObject)
    case setUser(String?)
    case reset
    case flush
    case tick
    /// The app went to the background. `done` runs once the batch committed here has been tried,
    /// or the uploader stopped at a failure.
    case background(done: (@Sendable () -> Void)?)
    case foreground
    /// Runs `done` once every earlier command has been handled.
    case barrier(@Sendable () -> Void)
}

struct WriterSettings: Sendable {
    let orgId: String
    let workspaceId: String
    let source: String?
    let fixedSessionId: String?
    let sessionTimeoutMillis: Int64
}

/// Turns commands into stored events. It runs on one serial queue, so `setUser` → `track` →
/// `removeUser` keeps its meaning, and identity is resolved here (after it was loaded from disk)
/// rather than on the app's thread. This is the role of hyper-sdk-ios's serial `trackerQueue` in
/// `HPJPLogsManager`, without its in-memory staging (`HPJPPreLogs`), whose copy-per-event and races
/// with the sessioniser lost or duplicated events.
///
/// Each event is encoded exactly once, so a resend carries the same `id` and timestamp. The event
/// type comes in through `encode`; the SDK passes `CanonicalEvent.encode`.
final class EventWriter: @unchecked Sendable {
    /// Larger events are dropped: they would crowd out whole batches.
    static let maxEventBytes = 64 * 1024

    private let encode: @Sendable (EventInput) -> [UInt8]
    private let settings: WriterSettings
    private let store: QueueStore
    private let identityStore: IdentityStore
    private let uploads: any UploadControl
    private let log: any SDKLog
    private let newId: @Sendable () -> String
    private let onStoredUser: @Sendable (String?) -> Void

    // Writer queue only.
    private var identity = IdentityState(anonId: "")
    private var identityDirty = false
    /// The stored identity exists but couldn't be read at start, e.g. because the app was launched
    /// in the background before the first unlock after a reboot. Until it can be read, a temporary
    /// identity is used and never saved over it.
    private var storedIdentityPending = false
    private var userChangedWhilePending = false
    private var resetWhilePending = false

    /// `onStoredUser` gets the stored user id when it can only be read after `start()`.
    init(
        encode: @escaping @Sendable (EventInput) -> [UInt8],
        settings: WriterSettings,
        store: QueueStore,
        identityStore: IdentityStore,
        uploads: any UploadControl,
        log: any SDKLog,
        newId: @escaping @Sendable () -> String = { UUID().uuidString.lowercased() },
        onStoredUser: @escaping @Sendable (String?) -> Void = { _ in }
    ) {
        self.encode = encode
        self.settings = settings
        self.store = store
        self.identityStore = identityStore
        self.uploads = uploads
        self.log = log
        self.newId = newId
        self.onStoredUser = onStoredUser
    }

    /// Loads the identity and recovers batches left by the previous run. Returns the stored user id.
    func start() -> String? {
        switch identityStore.load() {
        case .loaded(let stored):
            identity = stored
        case .unavailable:
            identity = IdentityState(anonId: newId())
            storedIdentityPending = true
            log.warn("The SDK identity can't be read yet (the device may not have been unlocked since it started); using a temporary one until it can")
        case .missing, .corrupt:
            identity = IdentityState(anonId: newId())
            saveIdentity()
        }
        do {
            try store.recover()
            store.enforceLimits()
        } catch {
            log.error("Could not recover the event queue", error: error)
        }
        uploads.requestUpload()
        return identity.actorId
    }

    func handle(_ command: Command) {
        if storedIdentityPending { adoptStoredIdentityIfReadable() }
        switch command {
        case let .track(name, timestampMillis, properties):
            track(name: name, timestampMillis: timestampMillis, properties: properties)
        case .setUser(let userId):
            if storedIdentityPending { userChangedWhilePending = true }
            var state = identity
            state.actorId = userId
            updateIdentity(state)
        case .reset:
            if storedIdentityPending { resetWhilePending = true }
            updateIdentity(IdentityState(anonId: newId()))
        case .flush:
            commitAndUpload()
        case .tick:
            // While uploads back off (e.g. offline), let the open batch fill up instead of
            // committing many small ones.
            if !uploads.isBackingOff { commitAndUpload() }
            saveIdentityIfDirty()
        case .background(let done):
            // iOS suspends the app soon after this, so try once now even if uploads are backing
            // off, as coming back to the foreground does.
            uploads.resetBackoff()
            commitAndUpload()
            saveIdentityIfDirty()
            if let done { uploads.whenIdle(done) }
        case .foreground:
            uploads.resetBackoff()
            uploads.requestUpload()
        case .barrier(let done):
            done()
        }
    }

    private func track(name: String, timestampMillis: Int64, properties: JSONObject) {
        let previousSession = identity.sessionId
        identity = identity.touched(
            at: timestampMillis,
            timeoutMillis: settings.sessionTimeoutMillis,
            fixedSessionId: settings.fixedSessionId,
            newId: newId
        )
        if identity.sessionId != previousSession {
            updateIdentity(identity)
        } else {
            identityDirty = true
        }

        let input = EventInput(
            id: newId(),
            name: name,
            orgId: settings.orgId,
            workspaceId: settings.workspaceId,
            sessionId: identity.sessionId,
            anonId: identity.anonId,
            actorId: identity.actorId,
            source: settings.source,
            occurredAt: formatIsoUtcMillis(timestampMillis),
            properties: properties
        )
        let line = encode(input)
        if line.count > Self.maxEventBytes {
            log.warn("Dropped event \"\(name)\": it is \(line.count) bytes, over the \(Self.maxEventBytes)-byte limit")
            return
        }
        do {
            if try store.append(line) {
                afterCommit()
            }
        } catch let failure as QueueStore.AppendFailure {
            log.warn("Could not store event \"\(name)\"; it was dropped", error: failure.cause)
            // The batch before it may still have been committed.
            if failure.committed { afterCommit() }
        } catch {
            log.warn("Could not store event \"\(name)\"; it was dropped", error: error)
        }
    }

    private func commitAndUpload() {
        if store.commit() { store.enforceLimits() }
        uploads.requestUpload()
    }

    private func afterCommit() {
        store.enforceLimits()
        uploads.requestUpload()
    }

    /// Switches from the temporary identity to the stored one once it can be read. The app's
    /// `setUser`, `removeUser` or `reset` from this run still apply, and a session already started
    /// keeps going.
    private func adoptStoredIdentityIfReadable() {
        let stored: IdentityState
        switch identityStore.load() {
        case .unavailable:
            return
        case .missing, .corrupt:
            storedIdentityPending = false
            saveIdentity()
            return
        case .loaded(let state):
            stored = state
        }
        storedIdentityPending = false
        if resetWhilePending {
            // The app logged out meanwhile: the new identity replaces the stored one.
            saveIdentity()
            return
        }
        var adopted = stored
        if userChangedWhilePending {
            adopted.actorId = identity.actorId
        } else {
            onStoredUser(stored.actorId)
        }
        if identity.sessionId != nil {
            adopted.sessionId = identity.sessionId
            adopted.lastActivityMillis = identity.lastActivityMillis
        }
        identity = adopted
        saveIdentity()
    }

    private func updateIdentity(_ state: IdentityState) {
        identity = state
        saveIdentity()
    }

    /// Saves the identity, or leaves it dirty to try again on the next tick: when the save fails,
    /// and while the stored identity can't be read, so it is never overwritten.
    private func saveIdentity() {
        identityDirty = storedIdentityPending || !identityStore.save(identity)
    }

    private func saveIdentityIfDirty() {
        if identityDirty { saveIdentity() }
    }
}
