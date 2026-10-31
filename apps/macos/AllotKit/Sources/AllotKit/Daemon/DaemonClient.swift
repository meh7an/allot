import AllotContract
import Foundation

/// HTTP client for the daemon routes the app uses. It never starts allotd by itself; that is
/// the user's Start service.
public actor DaemonClient: JobStopper {
    private static let requestTimeout: TimeInterval = 3

    // allotd answers a kill after up to three sweeps over the tree, each waiting out the grace
    // period of the processes that ignore SIGTERM.
    private static let stopTimeout: TimeInterval = 10

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

    public func kill(_ lease: Int64) async throws -> StopResult {
        try await stop("leases/\(lease)/kill", as: KillResponse.self, ended: StopResult.killed)
    }

    public func remove(_ lease: Int64) async throws -> StopResult {
        try await stop(
            "leases/\(lease)/remove", as: RemoveResponse.self, ended: StopResult.removed)
    }

    /// The reason in an error body of the kill or remove route, for a card that already names
    /// the job: allotd's text starts with the lease.
    public static func conflictReason(_ body: String) -> String {
        let notKilled = " was not killed: "
        if let cut = body.range(of: notKilled) {
            return String(body[cut.upperBound...])
        }
        if body.contains(" has already ended") {
            return "the job has already ended"
        }
        return body.isEmpty ? "the service refused" : body
    }

    private func stop<Response: Decodable>(
        _ path: String,
        as type: Response.Type,
        ended: (Response) -> StopResult
    ) async throws -> StopResult {
        do {
            guard let (data, response) = try await sendWithToken(
                "POST", path, timeout: Self.stopTimeout)
            else {
                return .refused("the service's token file is missing")
            }
            guard response.statusCode == 200 else {
                let body = String(decoding: data, as: UTF8.self)
                    .trimmingCharacters(in: .whitespacesAndNewlines)
                let reason = switch response.statusCode {
                case 409: Self.conflictReason(body)
                case 404: "the job no longer exists"
                case 503: "the service has just started; try again"
                case 401: "the service refused the token"
                default: "the service answered \(response.statusCode)"
                }
                return .refused(reason)
            }
            guard let value = try? JSONDecoder().decode(type, from: data) else {
                return .refused("the service's answer could not be read")
            }
            return ended(value)
        } catch let error as URLError {
            try Self.rethrowCancellation(error)
            return .refused(error.code == .timedOut
                ? "the service did not answer within \(Int(Self.stopTimeout)) s"
                : "the service is not running")
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
        _ path: String,
        timeout: TimeInterval = DaemonClient.requestTimeout
    ) async throws -> (Data, HTTPURLResponse)? {
        guard let token = currentToken() else {
            return nil
        }
        let (data, response) = try await send(
            request(method, path, token: token, timeout: timeout))
        guard response.statusCode == 401 else {
            return (data, response)
        }

        // allotd writes a new token when its token file was deleted, and this client may still
        // hold the old one.
        self.token = nil
        guard let fresh = currentToken() else {
            return nil
        }
        return try await send(request(method, path, token: fresh, timeout: timeout))
    }

    private func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        try Task.checkCancellation()
        return try await transport.send(request)
    }

    private func request(
        _ method: String,
        _ path: String,
        token: String?,
        timeout: TimeInterval = DaemonClient.requestTimeout
    ) -> URLRequest {
        var request = URLRequest(
            url: base.appendingPathComponent(path),
            cachePolicy: .reloadIgnoringLocalCacheData,
            timeoutInterval: timeout)
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
