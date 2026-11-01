/// Whether macOS lets the app show notifications, as System Settings › Notifications has it.
public enum NotificationPermission: Sendable, Equatable {
    case allowed

    /// Turned off for the app, or refused when the app asked; only System Settings turns it on.
    case off

    /// The app has not asked yet; the first notification asks.
    case notAsked

    public var isOff: Bool {
        self == .off
    }

    /// The line the Notifications menu and the first-run card show while notifications are off.
    public static let offHint = "Turned off in System Settings"
}
