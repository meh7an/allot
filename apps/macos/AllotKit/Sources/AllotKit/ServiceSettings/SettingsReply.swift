import AllotContract

/// Why a settings route did not answer as asked.
public enum SettingsFailureKind: String, Sendable, Equatable {
    /// 409: `config.toml` changed since its revision was read.
    case conflict

    /// 422: nothing written. With problems, the values that were refused (or the file's own
    /// problems); without, the file is not TOML or not UTF-8.
    case invalid

    /// 423: another program holds `config.toml`.
    case held

    /// 503: allotd has no machine sample or no config file yet.
    case unavailable

    /// No token file, or allotd refused the token after one fresh read.
    case token

    /// Nothing answered in time.
    case unreachable

    /// 404: the running allotd is older than Settings and has no settings routes.
    case outdated

    /// Anything else, such as a 500 or an answer that does not decode.
    case failed
}

/// A refused or unanswered settings request.
public struct SettingsFailure: Sendable, Equatable {
    public let kind: SettingsFailureKind

    /// allotd's sentence, or what went wrong on the way.
    public let message: String

    public let problems: [FieldError]

    public init(
        kind: SettingsFailureKind,
        message: String,
        problems: [FieldError] = []
    ) {
        self.kind = kind
        self.message = message
        self.problems = problems
    }
}

/// The answer of a settings route: the decoded value, or why there is none.
public enum SettingsReply<Value: Sendable>: Sendable {
    case value(Value)
    case failure(SettingsFailure)
}

extension SettingsReply: Equatable where Value: Equatable {}

/// The settings routes the session uses: ``DaemonClient``, or a fake in the tests.
public protocol SettingsClient: Sendable {
    /// `GET /settings`.
    ///
    /// - Throws: Only `CancellationError`.
    func settings() async throws -> SettingsReply<SettingsState>

    /// `PATCH /settings`: writes the changes together or not at all.
    ///
    /// - Throws: Only `CancellationError`.
    func changeSettings(
        revision: String,
        changes: [Change]
    ) async throws -> SettingsReply<SettingsState>

    /// `POST /settings/preview`.
    ///
    /// - Throws: Only `CancellationError`.
    func previewSettings(_ changes: [Change]) async throws -> SettingsReply<FitPreview>

    /// `POST /classify`.
    ///
    /// - Throws: Only `CancellationError`.
    func classify(
        line: String,
        shell: Shell,
        changes: [Change]
    ) async throws -> SettingsReply<CommandTest>
}
