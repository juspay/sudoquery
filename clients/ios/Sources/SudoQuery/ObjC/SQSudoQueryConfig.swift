import Foundation

/// Objective-C configuration for `SQSudoQuery`; Swift code should use `SudoQueryConfig`. The
/// defaults and rules are the same. The values are read when the config is passed to
/// `+[SQSudoQuery initializeWithConfig:error:]`, which is when it is validated; later changes have
/// no effect. Configure it on one thread.
@objc(SQSudoQueryConfig)
public final class SQSudoQueryConfig: NSObject {
    /// The domain of the `NSError` that `initializeWithConfig:error:` reports. Its codes are
    /// `SQSudoQueryConfigErrorCode` values.
    @objc public static let errorDomain = SudoQueryConfigError.errorDomain

    @objc public let orgId: String
    @objc public let workspaceId: String
    @objc public let endpoint: String
    @objc public var token: String?
    /// Header names and values, both strings. Typed loosely because Objective-C can't rule out
    /// other values, which `initializeWithConfig:error:` then reports instead of crashing.
    @objc public var headers: [AnyHashable: Any] = [:]
    @objc public var source: String? = SudoQueryConfig.defaultSource
    @objc public var sessionId: String?
    @objc public var batchSize: Int = SudoQueryConfig.defaultBatchSize
    /// Seconds; 0 turns the timer off.
    @objc public var flushInterval: TimeInterval = SudoQueryConfig.defaultFlushInterval
    /// Seconds.
    @objc public var sessionTimeout: TimeInterval = SudoQueryConfig.defaultSessionTimeout
    @objc public var debugLogging = false

    @objc(initWithOrgId:workspaceId:endpoint:)
    public init(orgId: String, workspaceId: String, endpoint: String) {
        self.orgId = orgId
        self.workspaceId = workspaceId
        self.endpoint = endpoint
        super.init()
    }

    @available(*, unavailable)
    override public init() {
        orgId = ""
        workspaceId = ""
        endpoint = ""
        super.init()
    }

    func makeConfig() throws -> SudoQueryConfig {
        var stringHeaders: [String: String] = [:]
        for (key, value) in headers {
            guard let name = key.base as? String, let text = value as? String else {
                throw SudoQueryConfigError(.invalidHeader, "header \(key.base) must have a string name and a string value")
            }
            stringHeaders[name] = text
        }
        return try SudoQueryConfig(
            orgId: orgId,
            workspaceId: workspaceId,
            endpoint: endpoint,
            token: token,
            headers: stringHeaders,
            source: source,
            sessionId: sessionId,
            batchSize: batchSize,
            flushInterval: flushInterval,
            sessionTimeout: sessionTimeout,
            debugLogging: debugLogging
        )
    }
}
