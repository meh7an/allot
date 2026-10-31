import AllotContract

public enum ServiceAction: Sendable, Equatable {
    case stop
    case start

    /// allotd is starting; the item is shown disabled.
    case none
}

/// The menu bar item's service entry: Stop Service while allotd answers, Start Service while it
/// is down.
public struct ServiceMenuItem: Sendable, Equatable {
    public let title: String
    public let action: ServiceAction

    public var isEnabled: Bool {
        action != .none
    }
}

/// The Windows tray menu's service item, in macOS title case.
public enum ServiceMenu {
    public static let confirmTitle = "Stop Allot?"

    public static func item(for status: ServiceStatus) -> ServiceMenuItem {
        switch status {
        case .running, .incompatible:
            ServiceMenuItem(title: "Stop Service…", action: .stop)
        case .starting:
            ServiceMenuItem(title: "Starting Service…", action: .none)
        case .stopped:
            ServiceMenuItem(title: "Start Service", action: .start)
        }
    }

    /// The confirmation shown before stopping allotd, or `nil` when no lease is running or
    /// waiting. Every Claude session's MCP server starts allotd again on its next call, so the
    /// text says the stop lasts only until then.
    public static func stopConfirmation(_ state: UiState?) -> String? {
        let running = state?.active.count ?? 0
        let waiting = state?.queue.count ?? 0
        guard running > 0 || waiting > 0 else {
            return nil
        }
        var parts: [String] = []
        if running > 0 {
            parts.append(running == 1 ? "1 job is running" : "\(running) jobs are running")
        }
        if waiting > 0 {
            parts.append(
                waiting == 1 ? "1 request is waiting" : "\(waiting) requests are waiting")
        }
        return "\(parts.joined(separator: " and ")).\n\n"
            + "Heavy commands run unchecked while the service is stopped. A Claude session "
            + "starts it again the next time it asks for resources."
    }
}
