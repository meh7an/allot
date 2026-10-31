/// The notifications of handoff section 9, each behind its own More options › Notifications
/// toggle.
public enum NotificationKind: Sendable, Equatable {
    case serviceStopped
    case runningLong
    case benchmarkFinished
    case jobFinished
    case killedOutsidePanel
}

public enum NotificationButton: String, Sendable, Equatable {
    case startService = "start-service"
    case openLogs = "open-logs"
    case show
}

/// One notification to show, in the app's words; the app turns it into a user notification.
public struct NotificationMessage: Sendable, Equatable {
    public let kind: NotificationKind

    /// Names the notification in Notification Center; showing another with the same tag
    /// replaces it.
    public let tag: String

    /// The first, bold line.
    public let title: String

    /// The second line.
    public let body: String

    /// The buttons, primary first.
    public let buttons: [NotificationButton]

    /// The panel row that Show or a click on the notification scrolls to (`job:<lease>` or
    /// `recent`), or `nil` for the top.
    public let target: String?
}

/// What one poll changes: notifications to show, and tags of shown ones that no longer apply.
public struct NotificationUpdate: Sendable, Equatable {
    public static let none = NotificationUpdate(show: [], withdraw: [])

    /// New notifications, oldest event first.
    public let show: [NotificationMessage]

    /// Tags to remove from Notification Center.
    public let withdraw: [String]
}
