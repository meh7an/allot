/// How long to wait before the next poll. Live data ticks at 1 Hz while the panel is open; the
/// closed panel only drives the menu bar icon. While allotd is unreachable the wait doubles,
/// capped lower while the panel is open, so a service started elsewhere shows up quickly.
public struct PollSchedule: Sendable {
    public static let openInterval = Duration.seconds(1)
    public static let closedInterval = Duration.seconds(5)
    public static let startingInterval = Duration.milliseconds(500)
    public static let firstRetry = Duration.seconds(1)
    public static let openRetryCap = Duration.seconds(5)
    public static let closedRetryCap = Duration.seconds(30)

    private var failures = 0

    public init() {}

    /// - Parameters:
    ///   - panelOpen: Whether the panel is showing.
    ///   - status: The status the poll just returned.
    public mutating func next(panelOpen: Bool, status: ServiceStatus) -> Duration {
        switch status {
        case .running:
            failures = 0
            return panelOpen ? Self.openInterval : Self.closedInterval
        case .starting:
            failures = 0
            return Self.startingInterval
        case .stopped, .incompatible:
            let cap = panelOpen ? Self.openRetryCap : Self.closedRetryCap
            let retry = Self.firstRetry * (1 << min(failures, 16))
            failures += 1
            return min(retry, cap)
        }
    }

    /// Opening the panel or starting the service retries at once and from the shortest wait.
    public mutating func reset() {
        failures = 0
    }
}
