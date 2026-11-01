import AllotKit
import AppKit
import UserNotifications
import os

/// Posts the app's notifications through Notification Center and reports their buttons. The
/// tag is the request identifier, so a newer notification with the same tag replaces the shown
/// one, and a withdrawn tag removes it.
@MainActor
final class Notifier: NSObject, UNUserNotificationCenterDelegate {
    private static let log = Logger(subsystem: "tech.mehran.allot", category: "notifications")
    private nonisolated static let targetKey = "target"
    private static let thread = "allot"

    /// A button, or `nil` for a click on the notification itself, with the row it points at.
    var onAction: ((NotificationButton?, String?) -> Void)?

    /// Set before the app finishes launching, so a click that launched the app is delivered.
    func install() {
        let center = UNUserNotificationCenter.current()
        center.delegate = self
        center.setNotificationCategories(Set(Category.allCases.map(\.category)))
    }

    /// Called whenever the permission has been read, so the menu and the first-run card can
    /// say when notifications are off.
    var onPermission: ((NotificationPermission) -> Void)?

    /// Asks once; macOS remembers the answer, and later calls return it without a prompt.
    func requestPermission() {
        Task {
            do {
                let granted = try await UNUserNotificationCenter.current()
                    .requestAuthorization(options: [.alert, .sound])
                Self.log.notice("notifications allowed: \(granted)")
            } catch {
                let reason = String(describing: error)
                Self.log.error("notification permission: \(reason, privacy: .public)")
            }
            await refreshPermission()
        }
    }

    /// Reads the permission again: it changes in System Settings at any time.
    func refreshPermission() async {
        let permission: NotificationPermission = switch await Self.authorization() {
        case .denied: .off
        case .notDetermined: .notAsked
        default: .allowed
        }
        onPermission?(permission)
    }

    /// System Settings › Notifications › Allot.
    static func openSettings() {
        let id = Bundle.main.bundleIdentifier ?? "tech.mehran.allot"
        let page = "x-apple.systempreferences:com.apple.Notifications-Settings.extension?id=\(id)"
        if let url = URL(string: page) {
            NSWorkspace.shared.open(url)
        }
    }

    func apply(_ update: NotificationUpdate) {
        if !update.withdraw.isEmpty {
            let center = UNUserNotificationCenter.current()
            center.removeDeliveredNotifications(withIdentifiers: update.withdraw)
            center.removePendingNotificationRequests(withIdentifiers: update.withdraw)
        }
        show(update.show)
    }

    func show(_ messages: [NotificationMessage]) {
        guard !messages.isEmpty else {
            return
        }
        let requests = messages.map(Self.request)
        Task {
            let center = UNUserNotificationCenter.current()
            // Posting before permission fails, so a first notification asks for it.
            var status = await Self.authorization()
            if status == .notDetermined {
                _ = try? await center.requestAuthorization(options: [.alert, .sound])
                status = await Self.authorization()
            }
            // Notice level, so the log keeps it: macOS drops info messages.
            if status == .denied {
                let count = requests.count
                Self.log.notice(
                    "\(count) not shown: off for Allot in System Settings › Notifications")
                onPermission?(.off)
                return
            }
            for request in requests {
                do {
                    try await center.add(request)
                    Self.log.notice("notified \(request.identifier, privacy: .public)")
                } catch {
                    let tag = request.identifier
                    let reason = String(describing: error)
                    Self.log.error("notify \(tag, privacy: .public): \(reason, privacy: .public)")
                }
            }
        }
    }

    // The settings object is not Sendable, so it is read where it is made and only its
    // status crosses back to the main actor.
    private nonisolated static func authorization() async -> UNAuthorizationStatus {
        await UNUserNotificationCenter.current().notificationSettings().authorizationStatus
    }

    nonisolated func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        willPresent notification: UNNotification
    ) async -> UNNotificationPresentationOptions {
        [.banner, .list]
    }

    nonisolated func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        didReceive response: UNNotificationResponse
    ) async {
        let action = response.actionIdentifier
        let target = response.notification.request.content.userInfo[Self.targetKey] as? String
        await handle(action, target: target)
    }

    private func handle(_ action: String, target: String?) {
        Self.log.info("notification action \(action, privacy: .public)")
        let target = target?.isEmpty == false ? target : nil
        switch action {
        case UNNotificationDismissActionIdentifier:
            return
        case UNNotificationDefaultActionIdentifier:
            onAction?(nil, target)
        default:
            if let button = NotificationButton(rawValue: action) {
                onAction?(button, target)
            }
        }
    }

    private static func request(_ message: NotificationMessage) -> UNNotificationRequest {
        let content = UNMutableNotificationContent()
        content.title = message.title
        content.body = message.body
        content.threadIdentifier = thread
        content.categoryIdentifier = Category.of(message.buttons).rawValue
        content.userInfo = [targetKey: message.target ?? ""]
        return UNNotificationRequest(identifier: message.tag, content: content, trigger: nil)
    }

    /// One category per set of buttons the notifications use.
    private enum Category: String, CaseIterable {
        case service = "allot.service"
        case show = "allot.show"
        case plain = "allot.plain"

        static func of(_ buttons: [NotificationButton]) -> Category {
            if buttons.contains(.startService) {
                return .service
            }
            return buttons.contains(.show) ? .show : .plain
        }

        var category: UNNotificationCategory {
            let actions: [UNNotificationAction] = switch self {
            case .service: [
                action(.startService, "Start Service"), action(.openLogs, "Open Logs"),
            ]
            case .show: [action(.show, "Show")]
            case .plain: []
            }
            return UNNotificationCategory(
                identifier: rawValue, actions: actions, intentIdentifiers: [])
        }

        private func action(_ button: NotificationButton, _ title: String)
            -> UNNotificationAction
        {
            UNNotificationAction(identifier: button.rawValue, title: title, options: [])
        }
    }
}
