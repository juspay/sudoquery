import ObjectiveC
import XCTest
@testable import SudoQuery

/// The Objective-C surface, called through the Objective-C runtime where it matters (bridging of
/// `NSDictionary`, `NSNumber`, nil). The Sample app's ObjCSmokeTest.m compile-checks the header.
final class ObjCBridgeTests: XCTestCase {
    private let log = RecordingLog()
    private var server: LocalHTTPServer!
    private var root: URL!

    override func setUpWithError() throws {
        try super.setUpWithError()
        server = try LocalHTTPServer()
        root = makeTempDirectory()
        SudoQuery.resetForTests(environment: testEnvironment(root: root, log: log), log: log)
    }

    override func tearDown() {
        SudoQuery.resetForTests()
        server.close()
        removeDirectory(root)
        super.tearDown()
    }

    private var sdk: AnyObject {
        SQSudoQuery.self as AnyObject
    }

    private func config(endpoint: String? = nil) -> SQSudoQueryConfig {
        let config = SQSudoQueryConfig(orgId: "acme-k3x9qa", projectId: "ios-demo1", endpoint: endpoint ?? server.url().absoluteString)
        config.batchSize = 1
        config.flushInterval = 0
        return config
    }

    func testTheClassesHaveTheirObjectiveCNames() {
        XCTAssertEqual(NSStringFromClass(SQSudoQuery.self), "SQSudoQuery")
        XCTAssertEqual(NSStringFromClass(SQSudoQueryConfig.self), "SQSudoQueryConfig")
        XCTAssertNotNil(NSClassFromString("SQSudoQuery"))
        XCTAssertNotNil(NSClassFromString("SQSudoQueryConfig"))
    }

    func testEverySelectorIsThere() {
        let classSelectors = [
            "initializeWithConfig:error:", "isInitialized", "track:", "track:properties:", "setUser:", "removeUser", "user",
            "setSuperProperty:value:", "superProperties", "clearSuperProperties", "flush", "reset", "sdkVersion",
        ]
        for name in classSelectors {
            XCTAssertNotNil(class_getClassMethod(SQSudoQuery.self, NSSelectorFromString(name)), name)
        }
        let configSelectors = [
            "initWithOrgId:projectId:endpoint:", "orgId", "projectId", "endpoint", "token", "setToken:", "headers", "setHeaders:",
            "source", "setSource:", "sessionId", "setSessionId:", "batchSize", "setBatchSize:", "flushInterval", "setFlushInterval:",
            "sessionTimeout", "setSessionTimeout:", "debugLogging", "setDebugLogging:",
        ]
        for name in configSelectors {
            XCTAssertTrue(SQSudoQueryConfig.instancesRespond(to: NSSelectorFromString(name)), name)
        }
        XCTAssertNotNil(class_getClassMethod(SQSudoQueryConfig.self, NSSelectorFromString("errorDomain")))
        XCTAssertEqual(SQSudoQuery.version(), 0) // still NSObject's +version
    }

    func testObjectiveCDefaultsMatchSwift() {
        let config = SQSudoQueryConfig(orgId: "acme-k3x9qa", projectId: "ios-demo1", endpoint: "https://c.example.com/v1/events/batch")
        XCTAssertEqual(config.source, SudoQueryConfig.defaultSource)
        XCTAssertEqual(config.batchSize, SudoQueryConfig.defaultBatchSize)
        XCTAssertEqual(config.flushInterval, SudoQueryConfig.defaultFlushInterval)
        XCTAssertEqual(config.sessionTimeout, SudoQueryConfig.defaultSessionTimeout)
        XCTAssertNil(config.token)
        XCTAssertNil(config.sessionId)
        XCTAssertFalse(config.debugLogging)
        XCTAssertEqual(SQSudoQueryConfig.errorDomain, "io.juspay.sudoquery.config")
        XCTAssertEqual(SQSudoQuery.sdkVersion, SudoQuery.version)
    }

    func testAnInvalidConfigFailsWithAnNSErrorAndDoesNotStart() {
        XCTAssertThrowsError(try SQSudoQuery.initialize(with: config(endpoint: "ftp://nope"))) { error in
            let nsError = error as NSError
            XCTAssertEqual(nsError.domain, SQSudoQueryConfig.errorDomain)
            XCTAssertEqual(nsError.code, SudoQueryConfigErrorCode.invalidEndpoint.rawValue)
        }
        XCTAssertFalse(SQSudoQuery.isInitialized)
    }

    func testTrackPropertiesFromObjectiveCKeepTheirTypes() throws {
        try SQSudoQuery.initialize(with: config())
        XCTAssertTrue(SQSudoQuery.isInitialized)

        let properties: NSDictionary = [
            NSNumber(value: 1): "numeric key",
            "b": NSNumber(value: true),
            "i": NSNumber(value: 1),
            "d": NSNumber(value: 0.5),
            "f": NSNumber(value: Float(0.1)),
            "dec": NSDecimalNumber(string: "10.25"),
            "n": NSNull(),
        ]
        _ = sdk.perform(NSSelectorFromString("track:properties:"), with: "objc_event", with: properties)

        let request = try XCTUnwrap(server.takeRequest(timeout: 10))
        XCTAssertTrue(
            request.bodyText.contains(#""name":"objc_event""#) && request.bodyText.contains(#""properties":{"1":"numeric key","b":true,"d":0.5,"dec":10.25,"f":0.1,"i":1,"n":null}"#),
            request.bodyText
        )
    }

    func testNilArgumentsFromObjectiveC() throws {
        try SQSudoQuery.initialize(with: config())

        _ = sdk.perform(NSSelectorFromString("setUser:"), with: "u1")
        XCTAssertEqual(SQSudoQuery.user, "u1")
        _ = sdk.perform(NSSelectorFromString("setUser:"), with: nil)
        XCTAssertNil(SQSudoQuery.user)

        _ = sdk.perform(NSSelectorFromString("setSuperProperty:value:"), with: "k", with: nil)
        XCTAssertTrue(SQSudoQuery.superProperties["k"] is NSNull)

        _ = sdk.perform(NSSelectorFromString("track:properties:"), with: "no_properties", with: nil)
        SQSudoQuery.flush()
        XCTAssertNotNil(server.takeRequest(timeout: 10))
    }

    func testHeadersThatArentStringsAreReportedInsteadOfCrashing() {
        let config = config()
        // What Objective-C can pass for an NSDictionary<NSString *, NSString *> *, e.g. from JSON.
        let headers: NSDictionary = ["X-Version": NSNumber(value: 42)]
        _ = config.perform(NSSelectorFromString("setHeaders:"), with: headers)
        XCTAssertThrowsError(try SQSudoQuery.initialize(with: config)) { error in
            XCTAssertEqual((error as NSError).code, SudoQueryConfigErrorCode.invalidHeader.rawValue)
        }

        config.headers = ["X-Version": "42"]
        XCTAssertNoThrow(try SQSudoQuery.initialize(with: config))
    }

    func testSuperPropertyBooleansComeBackAsBooleans() {
        SQSudoQuery.setSuperProperty("flag", value: true)
        let value = SQSudoQuery.superProperties["flag"] as AnyObject
        XCTAssertEqual(CFGetTypeID(value), CFBooleanGetTypeID())
        SQSudoQuery.clearSuperProperties()
    }
}
