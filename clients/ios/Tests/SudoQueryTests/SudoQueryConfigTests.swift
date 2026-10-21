import XCTest
@testable import SudoQuery

final class SudoQueryConfigTests: XCTestCase {
    private let endpoint = "https://collector.example.com/v1/events/batch"

    func testDefaults() throws {
        let config = try SudoQueryConfig(orgId: "acme-k3x9qa", projectId: "ios-demo1", endpoint: endpoint)
        XCTAssertEqual(config.source, "ios")
        XCTAssertEqual(config.batchSize, 50)
        XCTAssertEqual(config.flushIntervalMillis, 30_000)
        XCTAssertEqual(config.sessionTimeoutMillis, 30 * 60 * 1000)
        XCTAssertNil(config.token)
        XCTAssertNil(config.sessionId)
        XCTAssertEqual(config.headers, [:])
        XCTAssertFalse(config.debugLogging)
        XCTAssertEqual(config.endpointURL.absoluteString, endpoint)
    }

    func testIntervalsAreSecondsAndZeroTurnsTheTimerOff() throws {
        let config = try SudoQueryConfig(orgId: "acme-k3x9qa", projectId: "ios-demo1", endpoint: endpoint, flushInterval: 0, sessionTimeout: 1.5)
        XCTAssertEqual(config.flushIntervalMillis, 0)
        XCTAssertEqual(config.sessionTimeoutMillis, 1_500)
    }

    func testInvalidValuesAreRejected() {
        assertRejected(.blankOrgId) { try SudoQueryConfig(orgId: " ", projectId: "ios-demo1", endpoint: endpoint) }
        assertRejected(.blankProjectId) { try SudoQueryConfig(orgId: "acme-k3x9qa", projectId: "", endpoint: endpoint) }
        assertRejected(.invalidEndpoint) { try SudoQueryConfig(orgId: "acme-k3x9qa", projectId: "ios-demo1", endpoint: "collector/v1/events/batch") }
        assertRejected(.invalidEndpoint) { try SudoQueryConfig(orgId: "acme-k3x9qa", projectId: "ios-demo1", endpoint: "ftp://c.example.com") }
        assertRejected(.invalidEndpoint) { try SudoQueryConfig(orgId: "acme-k3x9qa", projectId: "ios-demo1", endpoint: "https://") }
        assertRejected(.invalidEndpoint) { try SudoQueryConfig(orgId: "acme-k3x9qa", projectId: "ios-demo1", endpoint: "https://c.example.com/a b") }
        assertRejected(.batchSizeOutOfRange) { try SudoQueryConfig(orgId: "acme-k3x9qa", projectId: "ios-demo1", endpoint: self.endpoint, batchSize: 0) }
        assertRejected(.batchSizeOutOfRange) { try SudoQueryConfig(orgId: "acme-k3x9qa", projectId: "ios-demo1", endpoint: self.endpoint, batchSize: 501) }
        assertRejected(.invalidFlushInterval) { try SudoQueryConfig(orgId: "acme-k3x9qa", projectId: "ios-demo1", endpoint: self.endpoint, flushInterval: -1) }
        assertRejected(.invalidFlushInterval) { try SudoQueryConfig(orgId: "acme-k3x9qa", projectId: "ios-demo1", endpoint: self.endpoint, flushInterval: .nan) }
        assertRejected(.invalidSessionTimeout) { try SudoQueryConfig(orgId: "acme-k3x9qa", projectId: "ios-demo1", endpoint: self.endpoint, sessionTimeout: 0) }
        assertRejected(.invalidHeader) { try SudoQueryConfig(orgId: "acme-k3x9qa", projectId: "ios-demo1", endpoint: self.endpoint, headers: ["X-Bad": "a\r\nInjected: 1"]) }
        assertRejected(.invalidHeader) { try SudoQueryConfig(orgId: "acme-k3x9qa", projectId: "ios-demo1", endpoint: self.endpoint, headers: ["": "x"]) }
    }

    func testErrorsBridgeToNSError() {
        do {
            _ = try SudoQueryConfig(orgId: "acme-k3x9qa", projectId: "ios-demo1", endpoint: "ftp://c.example.com")
            XCTFail("expected an error")
        } catch {
            let nsError = error as NSError
            XCTAssertEqual(nsError.domain, "io.juspay.sudoquery.config")
            XCTAssertEqual(nsError.code, SudoQueryConfigErrorCode.invalidEndpoint.rawValue)
            XCTAssertEqual(nsError.localizedDescription, "endpoint must be an http(s) URL: ftp://c.example.com")
        }
    }

    func testSlugsAreCheckedLikeTheCollector() {
        for slug in ["acme-k3x9qa", "ios-demo1", "a2345z", String(repeating: "a", count: 30)] {
            XCTAssertTrue(SudoQuery.isSlug(slug), slug)
        }
        for bad in ["breez", "Acme-k3x9qa", "1cme-k3x9qa", "acme-k3x9q-", "acme_k3x9qa", "a2345", String(repeating: "a", count: 31), "acmé-k3x9qa"] {
            XCTAssertFalse(SudoQuery.isSlug(bad), bad)
        }
    }

    private func assertRejected(_ code: SudoQueryConfigErrorCode, file: StaticString = #filePath, line: UInt = #line, _ make: () throws -> SudoQueryConfig) {
        do {
            _ = try make()
            XCTFail("expected \(code)", file: file, line: line)
        } catch let error as SudoQueryConfigError {
            XCTAssertEqual(error.code, code, error.message, file: file, line: line)
        } catch {
            XCTFail("unexpected \(error)", file: file, line: line)
        }
    }
}
