import AllotContract

public enum ServiceStatus: Sendable, Equatable {
    /// allotd answered with UI state.
    case running

    /// allotd answers but has no first sample yet.
    case starting

    /// Nothing answers on the port, or no token exists yet.
    case stopped

    /// allotd answers but cannot serve this app: an older build without `/ui/state`, a state
    /// shape the generated types reject, or a token it refuses. Starting it again does not help;
    /// reinstalling does.
    case incompatible
}

/// The outcome of one poll.
public struct PollResult: Sendable, Equatable {
    public let status: ServiceStatus

    /// The UI state; only set while ``ServiceStatus/running``.
    public let state: UiState?

    /// The daemon version, when it is known.
    public let version: String?

    /// Why the service is not running, for logs and the service-down view.
    public let detail: String?

    public init(
        status: ServiceStatus,
        state: UiState? = nil,
        version: String? = nil,
        detail: String? = nil
    ) {
        self.status = status
        self.state = state
        self.version = version
        self.detail = detail
    }
}
