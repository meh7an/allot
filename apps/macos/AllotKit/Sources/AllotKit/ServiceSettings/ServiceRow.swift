public enum ServiceButton: Sendable, Equatable {
    case stop
    case start

    /// The disabled, busy Start that follows a press.
    case starting
}

/// The status row at the top of the Service page (DESIGN.md section 4, Service).
public struct ServiceRow: Sendable, Equatable {
    public static let stoppedByYou =
        "Stopped. Heavy commands run without leases until you start it."

    public static let starting = "Starting…"

    /// The state in a sentence.
    public let text: String

    /// The button beside it.
    public let button: ServiceButton

    /// Whether Open Logs shows too.
    public let opensLogs: Bool

    /// - Parameters:
    ///   - status: The app's latest poll.
    ///   - session: The settings session's status, which knows the stop record.
    ///   - version: allotd's version, when it is known.
    ///   - port: The port allotd listens on, or this panel dials.
    public static func from(
        _ status: ServiceStatus,
        session: SessionStatus,
        version: String?,
        port: UInt32
    ) -> ServiceRow {
        switch status {
        case .running:
            let text = version.map { "Running · version \($0) · port \(port)" }
                ?? "Running · port \(port)"
            return ServiceRow(text: text, button: .stop, opensLogs: false)
        case .incompatible:
            return ServiceRow(text: ServiceBanner.incompatibleText, button: .stop, opensLogs: true)
        case .starting:
            return ServiceRow(text: starting, button: .starting, opensLogs: false)
        case .stopped where session == .stopped:
            return ServiceRow(text: stoppedByYou, button: .start, opensLogs: false)
        case .stopped:
            return ServiceRow(
                text: "Not answering on port \(port).", button: .start, opensLogs: true)
        }
    }

    public var buttonText: String {
        switch button {
        case .stop: "Stop Service…"
        case .start: "Start Service"
        case .starting: Self.starting
        }
    }
}
