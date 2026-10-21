import Foundation

struct HTTPResult: Sendable, Equatable {
    let code: Int
    /// `Retry-After` in seconds, when the server sent one.
    let retryAfterSeconds: Int64?
    /// The start of the response body, for logging.
    let body: String?
}

protocol Transport: Sendable {
    /// Posts one batch. `completion` is called once, on any queue, with the response, or with the
    /// error that kept one from arriving.
    func post(_ body: Data, completion: @escaping @Sendable (Result<HTTPResult, any Error>) -> Void)
}

/// Request headers, with the TS client's precedence: custom headers can replace `Content-Type` and
/// `User-Agent`, but `x-org-id`, `x-project-id` and the bearer token always win. Names are compared
/// case-insensitively. (hyper-sdk-ios merged headers through a dictionary, so which of two
/// duplicates won depended on the dictionary's order.)
func requestHeaders(
    orgId: String,
    projectId: String,
    token: String?,
    custom: [String: String],
    userAgent: String
) -> [(name: String, value: String)] {
    var headers: [(name: String, value: String)] = []
    func put(_ name: String, _ value: String) {
        headers.removeAll { $0.name.lowercased() == name.lowercased() }
        headers.append((name, value))
    }
    put("Content-Type", "application/json")
    put("User-Agent", userAgent)
    for (name, value) in custom.sorted(by: { $0.key < $1.key }) {
        put(name, value)
    }
    put("x-org-id", orgId)
    put("x-project-id", projectId)
    if let token, !token.isEmpty {
        put("Authorization", "Bearer \(token)")
    }
    return headers
}

/// `sudoquery-ios/<version> (iOS <version>)`.
func defaultUserAgent() -> String {
    let version = ProcessInfo.processInfo.operatingSystemVersion
    var osVersion = "\(version.majorVersion).\(version.minorVersion)"
    if version.patchVersion != 0 {
        osVersion += ".\(version.patchVersion)"
    }
    #if targetEnvironment(macCatalyst)
    let platform = "Mac Catalyst"
    #elseif os(iOS)
    let platform = "iOS"
    #elseif os(macOS)
    let platform = "macOS"
    #elseif os(tvOS)
    let platform = "tvOS"
    #elseif os(visionOS)
    let platform = "visionOS"
    #else
    let platform = "Apple"
    #endif
    return "sudoquery-ios/\(SudoQuery.version) (\(platform) \(osVersion))"
}

/// `Retry-After` as whole seconds. The HTTP-date form is ignored, as in the Android SDK.
func parseRetryAfter(_ value: String?) -> Int64? {
    guard let value else { return nil }
    return Int64(value.trimmingCharacters(in: .whitespaces))
}

/// `Transport` over one shared `URLSession`, ported from Airborne's `AJPNetworkClient` (shared
/// session, header merging, JSON body, redirect interception).
///
/// Unlike hyper-sdk-ios's log uploads, one session is reused for every batch, so connections are
/// kept alive; cookies, caches and stored credentials are off; timeouts are explicit; and a
/// transport error is reported as an error instead of an empty `URLResponse()`.
final class URLSessionTransport: Transport {
    static let maxBodyBytes = 2048

    private let session: URLSession
    private let endpoint: URL
    private let headers: [(name: String, value: String)]
    /// A task created on an invalidated session raises an Objective-C exception.
    private let invalidated = Locked(false)

    init(
        endpoint: URL,
        headers: [(name: String, value: String)],
        configuration: URLSessionConfiguration = URLSessionTransport.configuration()
    ) {
        self.endpoint = endpoint
        self.headers = headers
        session = URLSession(configuration: configuration, delegate: RedirectBlocker(), delegateQueue: nil)
    }

    static func configuration() -> URLSessionConfiguration {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.urlCache = nil
        configuration.requestCachePolicy = .reloadIgnoringLocalCacheData
        configuration.httpCookieStorage = nil
        configuration.httpShouldSetCookies = false
        configuration.httpCookieAcceptPolicy = .never
        configuration.urlCredentialStorage = nil
        // 30 s without data fails a request, like Android's read timeout. The total time is
        // capped loosely: a 512 KiB batch on a slow cellular link needs minutes, and a tighter cap
        // would fail it on every retry and hold up the batches behind it.
        configuration.timeoutIntervalForRequest = 30
        configuration.timeoutIntervalForResource = 10 * 60
        configuration.httpMaximumConnectionsPerHost = 1
        configuration.waitsForConnectivity = false
        return configuration
    }

    func post(_ body: Data, completion: @escaping @Sendable (Result<HTTPResult, any Error>) -> Void) {
        var request = URLRequest(url: endpoint)
        request.httpMethod = "POST"
        request.httpBody = body
        for (name, value) in headers {
            request.setValue(value, forHTTPHeaderField: name)
        }
        let task = invalidated.withLock { invalidated -> URLSessionDataTask? in
            guard !invalidated else { return nil }
            return session.dataTask(with: request) { data, response, error in
                if let error {
                    completion(.failure(error))
                    return
                }
                guard let http = response as? HTTPURLResponse else {
                    completion(.failure(URLError(.badServerResponse)))
                    return
                }
                let text = data.map { String(decoding: $0.prefix(Self.maxBodyBytes), as: UTF8.self) }
                let retryAfter = parseRetryAfter(http.value(forHTTPHeaderField: "Retry-After"))
                completion(.success(HTTPResult(code: http.statusCode, retryAfterSeconds: retryAfter, body: text)))
            }
        }
        guard let task else {
            completion(.failure(URLError(.cancelled)))
            return
        }
        task.resume()
    }

    /// Cancels requests in flight and releases the session. Later posts fail with `.cancelled`.
    func invalidate() {
        invalidated.withLock { invalidated in
            invalidated = true
            session.invalidateAndCancel()
        }
    }
}

/// Refuses redirects: following one would send the batch again, somewhere else. The 3xx response
/// comes back instead and the batch is kept for a retry, as for any misconfigured endpoint.
final class RedirectBlocker: NSObject, URLSessionTaskDelegate, Sendable {
    func urlSession(
        _ session: URLSession,
        task: URLSessionTask,
        willPerformHTTPRedirection response: HTTPURLResponse,
        newRequest request: URLRequest,
        completionHandler: @escaping @Sendable (URLRequest?) -> Void
    ) {
        completionHandler(nil)
    }
}
