import AllotKit
import Foundation
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

    /// Asks once; macOS remembers the answer, and later calls return it without a prompt.
    func requestPermission() {
        Task {
            do {
                let granted = try await UNUserNotificationCenter.current()
                    .requestAuthorization(options: [.alert, .sound])
                Self.log.info("notifications allowed: \(granted)")
            } catch {
                let reason = String(describing: error)
                Self.log.error("notification permission: \(reason, privacy: .public)")
            }
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
            if await center.notificationSettings().authorizationStatus == .notDetermined {
                _ = try? await center.requestAuthorization(options: [.alert, .sound])
            }
            for request in requests {
                do {
                    try await center.add(request)
                    Self.log.info("notified \(request.identifier, privacy: .public)")
                } catch {
                    let tag = request.identifier
                    let reason = String(describing: error)
                    Self.log.error("notify \(tag, privacy: .public): \(reason, privacy: .public)")
                }
            }
        }
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
