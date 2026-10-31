import AllotContract
import Foundation

/// HTTP client for the daemon routes the app uses. It never starts allotd by itself; that is
/// the user's Start service.
public actor DaemonClient {
    private static let requestTimeout: TimeInterval = 3

    private let base: URL
    private let tokenFile: URL
    private let transport: any HTTPTransport
    private var token: String?

    /// - Parameters:
    ///   - port: allotd's loopback port.
    ///   - tokenFile: The bearer token file, read on first use and after a 401.
    ///   - transport: Sends the requests; the tests pass a fake.
    public init(port: UInt16, tokenFile: URL, transport: any HTTPTransport) {
        // A literal URL with a numeric port always parses.
        base = URL(string: "http://127.0.0.1:\(port)/")!
        self.tokenFile = tokenFile
        self.transport = transport
    }

    /// A client for the local install: the port from `config.toml`, the token file beside it.
    public static func forInstall(_ data: DataDirectory) -> DaemonClient {
        DaemonClient(
            port: DaemonConfig.readPort(data),
            tokenFile: data.tokenFile,
            transport: URLSessionTransport())
    }

    /// `GET /ui/state`, mapped to a service status.
    ///
    /// - Throws: Only `CancellationError`.
    public func poll() async throws -> PollResult {
        do {
            guard let (data, response) = try await sendWithToken("GET", "ui/state") else {
                return PollResult(status: .stopped, detail: "no token in \(tokenFile.path)")
            }
            switch response.statusCode {
            case 200:
                return Self.decode(data)
            case 404:
                return try await withoutUiState()
            case 503:
                return PollResult(status: .starting, detail: "the daemon has no first sample yet")
            case 401:
                return PollResult(status: .incompatible, detail: "the daemon refused the token")
            default:
                return PollResult(
                    status: .incompatible,
                    detail: "GET /ui/state answered \(response.statusCode)")
            }
        } catch let error as URLError {
            try Self.rethrowCancellation(error)
            return PollResult(status: .stopped, detail: error.localizedDescription)
        }
    }

    /// `GET /health`, which needs no token; `nil` when nothing healthy answers.
    ///
    /// - Throws: Only `CancellationError`.
    public func health() async throws -> HealthResponse? {
        do {
            let (data, response) = try await send(request("GET", "health", token: nil))
            guard response.statusCode == 200 else {
                return nil
            }
            return try? JSONDecoder().decode(HealthResponse.self, from: data)
        } catch let error as URLError {
            try Self.rethrowCancellation(error)
            return nil
        }
    }

    private static func decode(_ data: Data) -> PollResult {
        do {
            let state = try JSONDecoder().decode(UiState.self, from: data)
            return PollResult(status: .running, state: state, version: state.service.version)
        } catch {
            return PollResult(status: .incompatible, detail: String(describing: error))
        }
    }

    // URLSession reports a cancelled task as a URLError, which must not read as a stopped
    // service.
    private static func rethrowCancellation(_ error: URLError) throws {
        if error.code == .cancelled || Task.isCancelled {
            throw CancellationError()
        }
    }

    // allotd v0.1 has no /ui/state, and answers 404 for it.
    private func withoutUiState() async throws -> PollResult {
        guard let health = try await health() else {
            return PollResult(status: .stopped, detail: "GET /ui/state answered 404")
        }
        return PollResult(
            status: .incompatible,
            version: health.version,
            detail: "allotd \(health.version) has no /ui/state")
    }

    /// - Returns: `nil` when there is no token to send.
    private func sendWithToken(
        _ method: String,
        _ path: String
    ) async throws -> (Data, HTTPURLResponse)? {
        guard let token = currentToken() else {
            return nil
        }
        let (data, response) = try await send(request(method, path, token: token))
        guard response.statusCode == 401 else {
            return (data, response)
        }

        // allotd writes a new token when its token file was deleted, and this client may still
        // hold the old one.
        self.token = nil
        guard let fresh = currentToken() else {
            return nil
        }
        return try await send(request(method, path, token: fresh))
    }

    private func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        try Task.checkCancellation()
        return try await transport.send(request)
    }

    private func request(_ method: String, _ path: String, token: String?) -> URLRequest {
        var request = URLRequest(
            url: base.appendingPathComponent(path),
            cachePolicy: .reloadIgnoringLocalCacheData,
            timeoutInterval: Self.requestTimeout)
        request.httpMethod = method
        if let token {
            request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        }
        return request
    }

    private func currentToken() -> String? {
        if let token {
            return token
        }
        let read = (try? String(contentsOf: tokenFile, encoding: .utf8))?
            .trimmingCharacters(in: .whitespacesAndNewlines)
        token = read?.isEmpty == false ? read : nil
        return token
    }
}
