import Foundation

/// Configuration for `SudoQuery.initialize(_:)`.
///
/// ```swift
/// let config = try SudoQueryConfig(
///     orgId: "acme-store-k3x9qa",
///     workspaceId: "acme-store-app-9t2r4m",
///     endpoint: "https://collector.example.com/v1/events/batch"
/// )
/// ```
public struct SudoQueryConfig: Sendable {
    public static let defaultSource = "ios"
    public static let defaultBatchSize = 50
    public static let maxBatchSize = 500
    public static let defaultFlushInterval: TimeInterval = 30
    public static let defaultSessionTimeout: TimeInterval = 30 * 60

    let orgId: String
    let workspaceId: String
    let endpoint: String
    let endpointURL: URL
    let token: String?
    let headers: [String: String]
    let source: String?
    let sessionId: String?
    let batchSize: Int
    let flushIntervalMillis: Int64
    let sessionTimeoutMillis: Int64
    let debugLogging: Bool

    /// - Parameters:
    ///   - orgId: The organization id (a server-generated slug such as `acme-store-k3x9qa`), sent as
    ///     `x-org-id` and in every event.
    ///   - workspaceId: The project's id (a server-generated slug): the dashboard's Project ID, which
    ///     its switcher copies as the workspace ID. Sent as `x-workspace-id` and in every event as
    ///     `workspace_id`.
    ///   - endpoint: The full URL of the collector's batch route, e.g.
    ///     `https://collector.example.com/v1/events/batch`.
    ///   - token: Sent as `Authorization: Bearer <token>`, for a proxy or gateway in front of the collector.
    ///   - headers: Extra headers for every request. They can replace `Content-Type` and `User-Agent`,
    ///     but not `x-org-id`, `x-workspace-id` or the bearer token.
    ///   - source: The `source` of every event.
    ///   - sessionId: Use this session id for every event instead of SDK-managed sessions.
    ///   - batchSize: Events per batch file and per request, 1 to 500.
    ///   - flushInterval: Seconds between sends of pending events even if a batch isn't full; 0 turns
    ///     the timer off.
    ///   - sessionTimeout: A new session starts when no event was tracked for this many seconds.
    ///   - debugLogging: Log debug details to the unified log (subsystem `io.juspay.sudoquery`).
    ///     Warnings and errors are always logged.
    /// - Throws: `SudoQueryConfigError` if an id is blank, the endpoint isn't an http(s) URL, a
    ///   value is out of range, or a header contains a line break.
    public init(
        orgId: String,
        workspaceId: String,
        endpoint: String,
        token: String? = nil,
        headers: [String: String] = [:],
        source: String? = SudoQueryConfig.defaultSource,
        sessionId: String? = nil,
        batchSize: Int = SudoQueryConfig.defaultBatchSize,
        flushInterval: TimeInterval = SudoQueryConfig.defaultFlushInterval,
        sessionTimeout: TimeInterval = SudoQueryConfig.defaultSessionTimeout,
        debugLogging: Bool = false
    ) throws {
        guard !orgId.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw SudoQueryConfigError(.blankOrgId, "orgId must not be blank")
        }
        guard !workspaceId.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw SudoQueryConfigError(.blankWorkspaceId, "workspaceId must not be blank")
        }
        guard let url = Self.parseURL(endpoint) else {
            throw SudoQueryConfigError(.invalidEndpoint, "endpoint is not a valid URL: \(endpoint)")
        }
        guard let scheme = url.scheme?.lowercased(), scheme == "https" || scheme == "http",
              let host = url.host, !host.isEmpty else {
            throw SudoQueryConfigError(.invalidEndpoint, "endpoint must be an http(s) URL: \(endpoint)")
        }
        guard (1...Self.maxBatchSize).contains(batchSize) else {
            throw SudoQueryConfigError(.batchSizeOutOfRange, "batchSize must be between 1 and \(Self.maxBatchSize), was \(batchSize)")
        }
        guard flushInterval.isFinite, flushInterval >= 0 else {
            throw SudoQueryConfigError(.invalidFlushInterval, "flushInterval must be a non-negative number of seconds, was \(flushInterval)")
        }
        guard sessionTimeout.isFinite, sessionTimeout > 0 else {
            throw SudoQueryConfigError(.invalidSessionTimeout, "sessionTimeout must be a positive number of seconds, was \(sessionTimeout)")
        }
        for (name, value) in headers {
            if name.isEmpty || Self.hasLineBreak(name) || Self.hasLineBreak(value) {
                throw SudoQueryConfigError(.invalidHeader, "header \"\(name)\" must have a name and no line breaks")
            }
        }

        self.orgId = orgId
        self.workspaceId = workspaceId
        self.endpoint = endpoint
        self.endpointURL = url
        self.token = token
        self.headers = headers
        self.source = source
        self.sessionId = sessionId
        self.batchSize = batchSize
        self.flushIntervalMillis = millis(flushInterval)
        self.sessionTimeoutMillis = millis(sessionTimeout)
        self.debugLogging = debugLogging
    }

    /// Strict on every OS version: from iOS 17, `URL(string:)` percent-encodes invalid characters
    /// instead of failing.
    private static func parseURL(_ text: String) -> URL? {
        if #available(iOS 17, macOS 14, tvOS 17, watchOS 10, *) {
            return URL(string: text, encodingInvalidCharacters: false)
        }
        return URL(string: text)
    }

    private static func hasLineBreak(_ text: String) -> Bool {
        text.unicodeScalars.contains { $0 == "\r" || $0 == "\n" }
    }
}

/// Why a `SudoQueryConfig` could not be created. From Objective-C it arrives as an `NSError` in
/// the `io.juspay.sudoquery.config` domain, with a `SQSudoQueryConfigErrorCode` code.
public struct SudoQueryConfigError: Error, Equatable, CustomStringConvertible, LocalizedError, CustomNSError {
    public let code: SudoQueryConfigErrorCode
    public let message: String

    init(_ code: SudoQueryConfigErrorCode, _ message: String) {
        self.code = code
        self.message = message
    }

    public var description: String {
        message
    }

    public var errorDescription: String? {
        message
    }

    public static var errorDomain: String {
        "io.juspay.sudoquery.config"
    }

    public var errorCode: Int {
        code.rawValue
    }

    public var errorUserInfo: [String: Any] {
        [NSLocalizedDescriptionKey: message]
    }
}

@objc(SQSudoQueryConfigErrorCode)
public enum SudoQueryConfigErrorCode: Int, Sendable {
    case blankOrgId = 1
    case blankWorkspaceId = 2
    case invalidEndpoint = 3
    case batchSizeOutOfRange = 4
    case invalidFlushInterval = 5
    case invalidSessionTimeout = 6
    case invalidHeader = 7
}
