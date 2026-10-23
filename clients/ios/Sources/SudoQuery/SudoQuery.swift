import Foundation

/// The SudoQuery iOS SDK. Call `initialize(_:)` once, typically in
/// `application(_:didFinishLaunchingWithOptions:)` or your `App`'s `init`, then `track` events
/// from anywhere.
///
/// Every method is thread-safe, returns quickly and never throws: events are written to disk and
/// sent to the collector on background queues. Events survive app restarts and are retried until
/// the collector accepts them (at-least-once delivery).
public enum SudoQuery {
    private static let state = Locked(FacadeState())
    private static let superPropertyStore = SuperProperties()

    struct FacadeState {
        var client: SudoQueryClient?
        var log: any SDKLog = OSLogLog(debugEnabled: false)
        var environment = ClientEnvironment()
        var warnedNotInitialized = false
    }

    /// Starts the SDK. Only the first call has an effect; later calls are ignored, as in the TS
    /// client. Returns at once: storage is set up on a background queue.
    public static func initialize(_ config: SudoQueryConfig) {
        state.withLock { state in
            if state.client != nil {
                state.log.debug("initialize called again; keeping the first configuration")
                return
            }
            let log = state.environment.makeLog(config.debugLogging)
            warnIfNotSlug("orgId", config.orgId, log)
            warnIfNotSlug("workspaceId", config.workspaceId, log)
            state.log = log
            state.client = SudoQueryClient.start(config: config, log: log, environment: state.environment)
        }
    }

    /// True once `initialize(_:)` has started the SDK.
    public static var isInitialized: Bool {
        state.withLock { $0.client != nil }
    }

    /// Records an event. `properties` are copied and sent as a JSON object, merged over the super
    /// properties (keys given here win). Nested dictionaries and arrays are supported; `Date`s are
    /// sent as ISO 8601 strings, and NaN or infinite numbers become null. Events tracked before
    /// `initialize(_:)` are dropped with a warning.
    public static func track(_ name: String, properties: [String: Any] = [:]) {
        record(name) { JSONConversion.object(from: properties, log: $0) }
    }

    /// `track` for dictionaries from Objective-C, whose keys may be numbers.
    static func track(_ name: String, anyKeyedProperties properties: [AnyHashable: Any]?) {
        record(name) { JSONConversion.object(fromAnyKeys: properties ?? [:], log: $0) }
    }

    /// Sets the user id sent as `actor_id` with every later event. It is kept across app restarts.
    public static func setUser(_ userId: String) {
        clientOrWarn("setUser")?.client.setUser(userId)
    }

    /// Stops sending a user id. The anonymous id and session are kept; see `reset()` for a full logout.
    public static func removeUser() {
        clientOrWarn("removeUser")?.client.setUser(nil)
    }

    /// The current user id, or nil.
    public static var user: String? {
        state.withLock { $0.client }?.currentUser()
    }

    /// Adds a property to every later event. Kept in memory only, and usable before `initialize(_:)`.
    public static func setSuperProperty(_ key: String, _ value: Any?) {
        let log = state.withLock { $0.log }
        superPropertyStore.set(key, JSONConversion.value(value, log: log))
    }

    /// A copy of the super properties. Nested objects come back as dictionaries, and null as `NSNull`.
    public static var superProperties: [String: Any] {
        JSONConversion.plain(superPropertyStore.snapshot())
    }

    public static func clearSuperProperties() {
        superPropertyStore.clear()
    }

    /// Commits the events tracked so far into a batch and starts sending it. Returns at once; if
    /// uploads are failing, the batch waits for the next retry.
    public static func flush() {
        clientOrWarn("flush")?.client.flush()
    }

    /// For logout: later events get a new anonymous id and session, and no user id. Super
    /// properties are cleared. Events already tracked are still sent unchanged.
    public static func reset() {
        superPropertyStore.clear()
        clientOrWarn("reset")?.client.reset()
    }

    static var clientForTests: SudoQueryClient? {
        state.withLock { $0.client }
    }

    /// Stops the SDK and forgets its state, so a test can initialize again with `environment`.
    static func resetForTests(environment: ClientEnvironment = ClientEnvironment(), log: any SDKLog = OSLogLog(debugEnabled: false)) {
        let client = state.withLock { state -> SudoQueryClient? in
            defer { state = FacadeState(client: nil, log: log, environment: environment, warnedNotInitialized: false) }
            return state.client
        }
        client?.shutdown()
        superPropertyStore.clear()
    }

    private static func record(_ name: String, _ properties: (any SDKLog) -> JSONObject) {
        // The event's time is when the app called track, not when the writer gets to it.
        let timestamp = state.withLock { $0.environment.time }.nowMillis()
        guard let active = clientOrWarn("track") else { return }
        let merged = superPropertyStore.mergeUnder(properties(active.log))
        active.client.track(name, timestampMillis: timestamp, properties: merged)
    }

    private static func clientOrWarn(_ operation: String) -> (client: SudoQueryClient, log: any SDKLog)? {
        let (client, log, warn) = state.withLock { state -> (SudoQueryClient?, any SDKLog, Bool) in
            guard state.client == nil, !state.warnedNotInitialized else {
                return (state.client, state.log, false)
            }
            state.warnedNotInitialized = true
            return (nil, state.log, true)
        }
        if warn {
            log.warn("SudoQuery.\(operation) was called before SudoQuery.initialize; such calls are ignored")
        }
        guard let client else { return nil }
        return (client, log)
    }

    /// Org and workspace ids are server-generated slugs (e.g. "acme-store-k3x9qa"). Warn, never
    /// throw, when one deviates, so the misconfiguration is visible without breaking the app.
    private static func warnIfNotSlug(_ field: String, _ value: String, _ log: any SDKLog) {
        guard !isSlug(value) else { return }
        log.warn(
            "config option \"\(field)\" value \"\(value)\" does not match the expected server-generated slug shape "
                + "/^[a-z][a-z0-9-]{4,28}[a-z0-9]$/ (e.g. \"acme-store-k3x9qa\"); the collector may reject events carrying it."
        )
    }

    /// `^[a-z][a-z0-9-]{4,28}[a-z0-9]$`, the collector's `is_valid_slug`.
    static func isSlug(_ value: String) -> Bool {
        let bytes = Array(value.utf8)
        guard (6...30).contains(bytes.count), let first = bytes.first, let last = bytes.last else { return false }
        func lower(_ byte: UInt8) -> Bool { (UInt8(ascii: "a")...UInt8(ascii: "z")).contains(byte) }
        func digit(_ byte: UInt8) -> Bool { (UInt8(ascii: "0")...UInt8(ascii: "9")).contains(byte) }
        return lower(first)
            && (lower(last) || digit(last))
            && bytes.allSatisfy { lower($0) || digit($0) || $0 == UInt8(ascii: "-") }
    }
}
