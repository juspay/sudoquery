import XCTest
@testable import SudoQuery

final class HTTPTransportTests: XCTestCase {
    private var server: LocalHTTPServer!
    private var transports: [URLSessionTransport] = []

    override func setUpWithError() throws {
        try super.setUpWithError()
        server = try LocalHTTPServer()
    }

    override func tearDown() {
        transports.forEach { $0.invalidate() }
        server.close()
        super.tearDown()
    }

    private func transport(_ headers: [(name: String, value: String)] = [("Content-Type", "application/json")]) -> URLSessionTransport {
        let transport = URLSessionTransport(endpoint: server.url(), headers: headers)
        transports.append(transport)
        return transport
    }

    private func post(_ transport: URLSessionTransport, _ body: Data = Data()) -> Result<HTTPResult, any Error>? {
        let result = Locked<Result<HTTPResult, any Error>?>(nil)
        let done = DispatchSemaphore(value: 0)
        transport.post(body) { value in
            result.withLock { $0 = value }
            done.signal()
        }
        XCTAssertEqual(done.wait(timeout: .now() + 10), .success, "no response")
        return result.current
    }

    private func response(_ transport: URLSessionTransport, _ body: Data = Data()) throws -> HTTPResult {
        switch post(transport, body) {
        case .success(let result)?: return result
        case .failure(let error)?: throw error
        case nil: throw URLError(.timedOut)
        }
    }

    func testPostsTheBodyWithTheGivenHeaders() throws {
        server.enqueue(code: 200, body: #"{"filtered":0,"collected":1,"total":1}"#)
        let headers = requestHeaders(orgId: "acme-k3x9qa", workspaceId: "ios-demo1", token: "secret", custom: ["X-Api-Key": "k"], userAgent: defaultUserAgent())

        let result = try response(transport(headers), Data(#"{"events":[]}"#.utf8))

        XCTAssertEqual(result.code, 200)
        XCTAssertEqual(result.body, #"{"filtered":0,"collected":1,"total":1}"#)
        let request = try XCTUnwrap(server.takeRequest())
        XCTAssertEqual(request.method, "POST")
        XCTAssertEqual(request.path, "/v1/events/batch")
        XCTAssertEqual(request.bodyText, #"{"events":[]}"#)
        XCTAssertEqual(request.header("Content-Type"), "application/json")
        XCTAssertEqual(request.header("x-org-id"), "acme-k3x9qa")
        XCTAssertEqual(request.header("x-project-id"), "ios-demo1")
        XCTAssertEqual(request.header("Authorization"), "Bearer secret")
        XCTAssertEqual(request.header("X-Api-Key"), "k")
        let userAgent = try XCTUnwrap(request.header("User-Agent"))
        XCTAssertNotNil(userAgent.range(of: #"^sudoquery-ios/\d+\.\d+\.\d+ \((iOS|macOS|Mac Catalyst) \d+\.\d+(\.\d+)?\)$"#, options: .regularExpression), userAgent)
    }

    func testReturnsErrorResponsesWithTheirBodyAndRetryAfter() throws {
        server.enqueue(code: 400, body: #"{"error":"bad"}"#)
        server.enqueue(code: 429, headers: [("Retry-After", "30")])
        server.enqueue(code: 503, headers: [("Retry-After", "Wed, 21 Oct 2026 07:28:00 GMT")])

        let rejected = try response(transport())
        XCTAssertEqual(rejected.code, 400)
        XCTAssertEqual(rejected.body, #"{"error":"bad"}"#)
        XCTAssertNil(rejected.retryAfterSeconds)

        XCTAssertEqual(try response(transport()).retryAfterSeconds, 30)
        XCTAssertNil(try response(transport()).retryAfterSeconds) // an HTTP-date is ignored
        XCTAssertEqual(parseRetryAfter(" 30 "), 30)
    }

    func testDoesNotFollowRedirects() throws {
        server.enqueue(code: 307, headers: [("Location", server.url("/elsewhere").absoluteString)])
        XCTAssertEqual(try response(transport(), Data([1])).code, 307)
        XCTAssertEqual(server.requestCount, 1)
    }

    func testReportsAnErrorWhenTheCollectorIsUnreachable() {
        let unreachable = transport()
        server.close()
        guard case .failure? = post(unreachable, Data([1])) else {
            return XCTFail("expected a transport error")
        }
    }

    func testKeepsOnlyTheStartOfALongBody() throws {
        server.enqueue(code: 413, body: String(repeating: "x", count: 10_000))
        XCTAssertEqual(try response(transport()).body?.count, URLSessionTransport.maxBodyBytes)
    }

    func testDoesNotSendCookiesBack() throws {
        server.enqueue(code: 200, headers: [("Set-Cookie", "session=abc; Path=/")])
        let transport = transport()
        _ = try response(transport)
        _ = try response(transport)
        _ = try XCTUnwrap(server.takeRequest())
        XCTAssertNil(try XCTUnwrap(server.takeRequest()).header("Cookie"))
    }

    func testCustomHeadersCantReplaceTheIdsOrTheToken() {
        let headers = requestHeaders(
            orgId: "acme-k3x9qa",
            workspaceId: "ios-demo1",
            token: "secret",
            custom: [
                "content-type": "application/x-ndjson",
                "X-Org-Id": "other-org",
                "authorization": "Basic abc",
                "user-agent": "custom",
            ],
            userAgent: "sudoquery-ios/0.1.0"
        )
        XCTAssertEqual(headers.map(\.name), ["content-type", "user-agent", "x-org-id", "x-project-id", "Authorization"])
        XCTAssertEqual(headers.map(\.value), ["application/x-ndjson", "custom", "acme-k3x9qa", "ios-demo1", "Bearer secret"])
    }

    func testAnEmptyTokenIsNotSent() {
        let headers = requestHeaders(orgId: "acme-k3x9qa", workspaceId: "ios-demo1", token: "", custom: [:], userAgent: "ua")
        XCTAssertEqual(headers.map(\.name), ["Content-Type", "User-Agent", "x-org-id", "x-project-id"])
    }
}
