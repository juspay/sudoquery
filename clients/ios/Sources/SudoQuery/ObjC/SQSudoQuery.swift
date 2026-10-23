import Foundation

/// The Objective-C entry point. It forwards to `SudoQuery`, which Swift code should call instead.
///
/// ```objc
/// @import SudoQuery;
///
/// SQSudoQueryConfig *config =
///     [[SQSudoQueryConfig alloc] initWithOrgId:@"acme-store-k3x9qa"
///                                  workspaceId:@"acme-store-app-9t2r4m"
///                                     endpoint:@"https://collector.example.com/v1/events/batch"];
/// NSError *error = nil;
/// if (![SQSudoQuery initializeWithConfig:config error:&error]) {
///     NSLog(@"SudoQuery: %@", error.localizedDescription);
/// }
/// [SQSudoQuery track:@"button_click" properties:@{@"button_id": @"submit"}];
/// ```
@objc(SQSudoQuery)
public final class SQSudoQuery: NSObject {
    @available(*, unavailable)
    override public init() {
        super.init()
    }

    /// Starts the SDK; see `SudoQuery.initialize(_:)`. Fails only if `config` is invalid, with an
    /// error in the `SQSudoQueryConfig.errorDomain` domain.
    @objc(initializeWithConfig:error:)
    public static func initialize(with config: SQSudoQueryConfig) throws {
        SudoQuery.initialize(try config.makeConfig())
    }

    @objc public static var isInitialized: Bool {
        SudoQuery.isInitialized
    }

    @objc(track:)
    public static func track(_ name: String) {
        SudoQuery.track(name)
    }

    /// Keys that aren't strings (e.g. `@1`) are sent as their description.
    @objc(track:properties:)
    public static func track(_ name: String, properties: [AnyHashable: Any]?) {
        SudoQuery.track(name, anyKeyedProperties: properties)
    }

    /// Passing nil removes the user.
    @objc(setUser:)
    public static func setUser(_ userId: String?) {
        if let userId {
            SudoQuery.setUser(userId)
        } else {
            SudoQuery.removeUser()
        }
    }

    @objc public static func removeUser() {
        SudoQuery.removeUser()
    }

    @objc public static var user: String? {
        SudoQuery.user
    }

    @objc(setSuperProperty:value:)
    public static func setSuperProperty(_ key: String, value: Any?) {
        SudoQuery.setSuperProperty(key, value)
    }

    @objc public static var superProperties: [String: Any] {
        SudoQuery.superProperties
    }

    @objc public static func clearSuperProperties() {
        SudoQuery.clearSuperProperties()
    }

    @objc public static func flush() {
        SudoQuery.flush()
    }

    @objc public static func reset() {
        SudoQuery.reset()
    }

    /// The SDK version. (NSObject already has a `+version` class method.)
    @objc public static var sdkVersion: String {
        SudoQuery.version
    }
}
