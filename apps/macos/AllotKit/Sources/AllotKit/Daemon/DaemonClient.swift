import AllotContract
import Foundation
import os

/// HTTP client for the daemon routes the app uses. It never starts allotd by itself; that is
/// the user's Start service.
public actor DaemonClient: JobStopper, SettingsClient {
    private static let log = Logger(subsystem: "tech.mehran.allot", category: "daemon")

    private static let requestTimeout: TimeInterval = 3

    // allotd answers a kill after up to three sweeps over the tree, each waiting out the grace
    // period of the processes that ignore SIGTERM.
    private static let stopTimeout: TimeInterval = 10

    private var base: URL
    private let tokenFile: URL
    private let transport: any HTTPTransport
    private var token: String?

    /// - Parameters:
    ///   - port: allotd's loopback port.
    ///   - tokenFile: The bearer token file, read on first use and after a 401.
    ///   - transport: Sends the requests; the tests pass a fake.
    public init(port: UInt16, tokenFile: URL, transport: any HTTPTransport) {
        base = Self.url(port: port)
        self.tokenFile = tokenFile
        self.transport = transport
    }

    /// Sends every later request to `port`, after a restart moved allotd there.
    public func usePort(_ port: UInt16) {
        base = Self.url(port: port)
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

    /// `POST /shutdown` from Stop service: allotd writes the stop record, so no session starts
    /// it again, then accepts and stops. Start service needs nothing of the kind, since every
    /// start of allotd clears the record.
    ///
    /// - Parameter record: Whether allotd writes the stop record; a restart sends no body, so
    ///   no record is written.
    /// - Returns: `nil` when allotd accepted, else why not.
    /// - Throws: Only `CancellationError`.
    public func shutdown(record: Bool = true) async throws -> String? {
        do {
            let body = record ? Self.encode(ShutdownBody(by: .panel)) : nil
            guard let (_, response) = try await sendWithToken("POST", "shutdown", body: body)
            else {
                return "no token in \(tokenFile.path)"
            }
            return response.statusCode == 202
                ? nil : "POST /shutdown answered \(response.statusCode)"
        } catch let error as URLError {
            try Self.rethrowCancellation(error)
            return error.localizedDescription
        }
    }

    /// `GET /settings`: every setting with its spec, value and the file's revision.
    ///
    /// - Throws: Only `CancellationError`.
    public func settings() async throws -> SettingsReply<SettingsState> {
        try await settingsRoute("GET", "settings", body: nil, as: SettingsState.self)
    }

    /// `PATCH /settings` with the revision the changes were made against: writes the changes
    /// together or not at all. The generated ``SettingsPatch`` has no initializer outside its
    /// module, so the session sends the pieces.
    ///
    /// - Returns: The new state.
    /// - Throws: Only `CancellationError`.
    public func changeSettings(
        revision: String,
        changes: [Change]
    ) async throws -> SettingsReply<SettingsState> {
        try await settingsRoute(
            "PATCH", "settings",
            body: Self.encode(PatchBody(revision: revision, changes: changes)),
            as: SettingsState.self)
    }

    /// `POST /settings/preview`: what fits with unsaved changes on top of the file.
    ///
    /// - Throws: Only `CancellationError`.
    public func previewSettings(_ changes: [Change]) async throws -> SettingsReply<FitPreview> {
        try await settingsRoute(
            "POST", "settings/preview", body: Self.encode(PreviewBody(changes: changes)),
            as: FitPreview.self)
    }

    /// `POST /classify`: how the hook treats a line, the shell it is written for, under the
    /// file's rules or unsaved changes to them.
    ///
    /// - Throws: Only `CancellationError`.
    public func classify(
        line: String,
        shell: Shell,
        changes: [Change]
    ) async throws -> SettingsReply<CommandTest> {
        let body = ClassifyBody(line: line, shell: shell, changes: changes)
        return try await settingsRoute(
            "POST", "classify", body: Self.encode(body), as: CommandTest.self)
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

    private func settingsRoute<Value: Decodable & Sendable>(
        _ method: String,
        _ path: String,
        body: Data?,
        as type: Value.Type
    ) async throws -> SettingsReply<Value> {
        if body == nil && method != "GET" {
            return .failure(
                SettingsFailure(kind: .failed, message: "the request could not be encoded"))
        }
        do {
            guard let (data, response) = try await sendWithToken(method, path, body: body) else {
                return .failure(
                    SettingsFailure(kind: .token, message: "no token in \(tokenFile.path)"))
            }
            switch response.statusCode {
            case 200:
                do {
                    return .value(try JSONDecoder().decode(type, from: data))
                } catch {
                    // The decoder's dump names fields and paths; the log keeps it, the banner
                    // gets a sentence.
                    let dump = String(describing: error)
                    Self.log.error("settings answer: \(dump, privacy: .public)")
                    return .failure(SettingsFailure(
                        kind: .failed, message: "the service's answer could not be read"))
                }
            case 401:
                return .failure(
                    SettingsFailure(kind: .token, message: "the service refused the token"))
            default:
                return .failure(Self.refusal(response.statusCode, data))
            }
        } catch let error as URLError {
            try Self.rethrowCancellation(error)
            return .failure(
                SettingsFailure(kind: .unreachable, message: error.localizedDescription))
        }
    }

    /// A refused settings request. Settings routes refuse with a `SettingsRefusal`; a body that
    /// is not one, such as a 500's plain text, keeps its text as the message.
    private static func refusal(_ status: Int, _ data: Data) -> SettingsFailure {
        let kind: SettingsFailureKind = switch status {
        case 409: .conflict
        case 422: .invalid
        case 423: .held
        case 503: .unavailable
        // An allotd that serves /ui/state but not /settings predates Settings.
        case 404: .outdated
        default: .failed
        }
        if let refusal = try? JSONDecoder().decode(SettingsRefusal.self, from: data) {
            return SettingsFailure(
                kind: kind,
                message: refusal.message,
                problems: refusal.problems)
        }
        let text = String(decoding: data, as: UTF8.self)
            .trimmingCharacters(in: .whitespacesAndNewlines)
        return SettingsFailure(
            kind: kind, message: text.isEmpty ? "the service answered \(status)" : text)
    }

    private static func url(port: UInt16) -> URL {
        // A literal URL with a numeric port always parses.
        URL(string: "http://127.0.0.1:\(port)/")!
    }

    private static func encode<Body: Encodable>(_ body: Body) -> Data? {
        try? JSONEncoder().encode(body)
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
        timeout: TimeInterval = DaemonClient.requestTimeout,
        body: Data? = nil
    ) async throws -> (Data, HTTPURLResponse)? {
        guard let token = currentToken() else {
            return nil
        }
        let (data, response) = try await send(
            request(method, path, token: token, timeout: timeout, body: body))
        guard response.statusCode == 401 else {
            return (data, response)
        }

        // allotd writes a new token when its token file was deleted, and this client may still
        // hold the old one.
        self.token = nil
        guard let fresh = currentToken() else {
            return nil
        }
        return try await send(
            request(method, path, token: fresh, timeout: timeout, body: body))
    }

    private func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        try Task.checkCancellation()
        return try await transport.send(request)
    }

    private func request(
        _ method: String,
        _ path: String,
        token: String?,
        timeout: TimeInterval = DaemonClient.requestTimeout,
        body: Data? = nil
    ) -> URLRequest {
        var request = URLRequest(
            url: base.appendingPathComponent(path),
            cachePolicy: .reloadIgnoringLocalCacheData,
            timeoutInterval: timeout)
        request.httpMethod = method
        if let token {
            request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        }
        if let body {
            request.httpBody = body
            request.setValue("application/json", forHTTPHeaderField: "Content-Type")
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

// The generated request types have no initializers outside their module, so the bodies the app
// builds are encoded from these, which write the same JSON.

private struct ShutdownBody: Encodable {
    let by: StoppedBy
}

private struct PatchBody: Encodable {
    let revision: String
    let changes: [Change]
}

private struct PreviewBody: Encodable {
    let changes: [Change]
}

private struct ClassifyBody: Encodable {
    let line: String
    let shell: Shell
    let changes: [Change]
}
