import Foundation
#if canImport(Darwin)
import Darwin
#endif
@testable import SudoQuery

/// A loopback HTTP/1.1 server for tests, in the role of Android's MockWebServer: it answers each
/// request with the next queued response (200 `{}` once none is queued) and records the request.
/// The xctest hosts allow plain HTTP, so no ATS exception is needed.
final class LocalHTTPServer: @unchecked Sendable {
    struct Request: Sendable {
        let method: String
        let path: String
        let headers: [(name: String, value: String)]
        let body: Data

        func header(_ name: String) -> String? {
            headers.first { $0.name.lowercased() == name.lowercased() }?.value
        }

        var bodyText: String {
            String(decoding: body, as: UTF8.self)
        }
    }

    struct Response: Sendable {
        var code = 200
        var headers: [(name: String, value: String)] = []
        var body = "{}"
    }

    let port: UInt16
    private let socketFD: Int32
    private let state = Locked((requests: [Request](), untaken: [Request](), responses: [Response](), closed: false))
    private let arrivals = DispatchSemaphore(value: 0)
    private let acceptQueue = DispatchQueue(label: "LocalHTTPServer.accept")

    init() throws {
        let fd = socket(AF_INET, SOCK_STREAM, 0)
        guard fd >= 0 else { throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO) }
        var yes: Int32 = 1
        setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &yes, socklen_t(MemoryLayout<Int32>.size))

        var address = sockaddr_in()
        address.sin_len = UInt8(MemoryLayout<sockaddr_in>.size)
        address.sin_family = sa_family_t(AF_INET)
        address.sin_port = 0
        address.sin_addr.s_addr = inet_addr("127.0.0.1")
        let bound = withUnsafePointer(to: address) { pointer in
            pointer.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                bind(fd, $0, socklen_t(MemoryLayout<sockaddr_in>.size))
            }
        }
        guard bound == 0, listen(fd, 16) == 0 else {
            let code = errno
            Darwin.close(fd)
            throw POSIXError(POSIXErrorCode(rawValue: code) ?? .EIO)
        }

        var actual = sockaddr_in()
        var length = socklen_t(MemoryLayout<sockaddr_in>.size)
        _ = withUnsafeMutablePointer(to: &actual) { pointer in
            pointer.withMemoryRebound(to: sockaddr.self, capacity: 1) { getsockname(fd, $0, &length) }
        }
        socketFD = fd
        port = UInt16(bigEndian: actual.sin_port)
        acceptQueue.async { [self] in acceptLoop() }
    }

    deinit {
        close()
    }

    func url(_ path: String = "/v1/events/batch") -> URL {
        URL(string: "http://127.0.0.1:\(port)\(path)")!
    }

    func enqueue(_ response: Response) {
        state.withLock { $0.responses.append(response) }
    }

    func enqueue(code: Int, body: String = "{}", headers: [(name: String, value: String)] = []) {
        enqueue(Response(code: code, headers: headers, body: body))
    }

    var requestCount: Int {
        state.current.requests.count
    }

    /// The next request not taken yet, waiting up to `timeout` for it to arrive.
    func takeRequest(timeout: TimeInterval = 5) -> Request? {
        guard arrivals.wait(timeout: .now() + timeout) == .success else { return nil }
        return state.withLock { state in
            state.untaken.isEmpty ? nil : state.untaken.removeFirst()
        }
    }

    /// Stops listening at once, so later connections are refused.
    func close() {
        let first = state.withLock { state -> Bool in
            defer { state.closed = true }
            return !state.closed
        }
        guard first else { return }
        shutdown(socketFD, SHUT_RDWR)
        Darwin.close(socketFD)
    }

    private func acceptLoop() {
        while !state.current.closed {
            var descriptor = pollfd(fd: socketFD, events: Int16(POLLIN), revents: 0)
            guard poll(&descriptor, 1, 100) > 0, !state.current.closed else { continue }
            let client = accept(socketFD, nil, nil)
            if client >= 0 { serve(client) }
        }
    }

    private func serve(_ client: Int32) {
        defer { Darwin.close(client) }
        var yes: Int32 = 1
        setsockopt(client, SOL_SOCKET, SO_NOSIGPIPE, &yes, socklen_t(MemoryLayout<Int32>.size))
        var timeout = timeval(tv_sec: 5, tv_usec: 0)
        setsockopt(client, SOL_SOCKET, SO_RCVTIMEO, &timeout, socklen_t(MemoryLayout<timeval>.size))

        var buffer = Data()
        var chunk = [UInt8](repeating: 0, count: 4096)
        let separator = Data("\r\n\r\n".utf8)
        var headerEnd = buffer.range(of: separator)
        while headerEnd == nil {
            let count = read(client, &chunk, chunk.count)
            if count <= 0 { return }
            buffer.append(contentsOf: chunk[0..<count])
            headerEnd = buffer.range(of: separator)
        }
        guard let end = headerEnd else { return }

        var lines = String(decoding: buffer[buffer.startIndex..<end.lowerBound], as: UTF8.self).components(separatedBy: "\r\n")
        let requestLine = lines.removeFirst().split(separator: " ")
        guard requestLine.count >= 2 else { return }
        let headers = lines.compactMap { line -> (name: String, value: String)? in
            guard let colon = line.firstIndex(of: ":") else { return nil }
            return (String(line[..<colon]), line[line.index(after: colon)...].trimmingCharacters(in: .whitespaces))
        }
        let length = headers.first { $0.name.lowercased() == "content-length" }.flatMap { Int($0.value) } ?? 0
        var body = Data(buffer[end.upperBound...])
        while body.count < length {
            let count = read(client, &chunk, chunk.count)
            if count <= 0 { break }
            body.append(contentsOf: chunk[0..<count])
        }

        let request = Request(method: String(requestLine[0]), path: String(requestLine[1]), headers: headers, body: body)
        let response = state.withLock { state -> Response in
            state.requests.append(request)
            state.untaken.append(request)
            return state.responses.isEmpty ? Response() : state.responses.removeFirst()
        }

        var text = "HTTP/1.1 \(response.code) \(Self.reason(response.code))\r\n"
        text += "Content-Length: \(response.body.utf8.count)\r\n"
        text += "Connection: close\r\n"
        for (name, value) in response.headers {
            text += "\(name): \(value)\r\n"
        }
        text += "\r\n"
        text += response.body
        let bytes = Array(text.utf8)
        _ = bytes.withUnsafeBytes { write(client, $0.baseAddress, $0.count) }
        arrivals.signal()
    }

    private static func reason(_ code: Int) -> String {
        switch code {
        case 200: return "OK"
        case 202: return "Accepted"
        case 204: return "No Content"
        case 301: return "Moved Permanently"
        case 307: return "Temporary Redirect"
        case 400: return "Bad Request"
        case 413: return "Payload Too Large"
        case 429: return "Too Many Requests"
        case 500: return "Internal Server Error"
        case 502: return "Bad Gateway"
        default: return "Status"
        }
    }
}
