import Foundation

public enum BannerSeverity: Sendable, Equatable {
    case informational
    case warning
    case error
}

/// The buttons a banner can carry.
public enum BannerAction: Sendable, Equatable {
    case startService
    case restartService
    case tryNow
    case openLogs
    case openFile
    case checkAgain
    case applyMine
    case keepFileValues
    case saveAgain

    /// The button's title, in title case as macOS buttons are.
    public var label: String {
        switch self {
        case .startService: "Start Service"
        case .restartService: "Restart Service"
        case .tryNow: "Try Now"
        case .openLogs: "Open Logs"
        case .openFile: "Open File"
        case .checkAgain: "Check Again"
        case .applyMine: "Apply My Change"
        case .keepFileValues: "Keep File's Values"
        case .saveAgain: "Try Again"
        }
    }
}

/// A lasting state at the top of a Settings page (DESIGN.md section 7).
public struct SettingsBanner: Sendable, Equatable {
    public let severity: BannerSeverity

    /// What happened and what it means.
    public let message: String

    /// Its buttons, the first one primary.
    public let actions: [BannerAction]
}

/// Which banners a page shows for the session's state. The Panel page shows none of allotd's:
/// it keeps working in every state.
@MainActor
public enum SettingsBanners {
    public static let panelPage = "panel"

    public static let servicePage = "service"

    private static let portKey = "daemon.port"

    /// - Parameters:
    ///   - pageId: The page shown.
    ///   - port: The port this panel reaches allotd on.
    ///   - number: Formats a number for the user's locale.
    public static func banners(
        _ session: SettingsSession,
        pageId: String,
        port: UInt16,
        number: (Double) -> String
    ) -> [SettingsBanner] {
        if pageId == panelPage {
            return []
        }
        if let unanswered = unanswered(session, port: port) {
            return [unanswered]
        }
        guard session.status == .ready else {
            return []
        }
        var banners: [SettingsBanner] = []
        if let fileError = session.fileError {
            banners.append(
                SettingsBanner(
                    severity: .error,
                    message: "config.toml has an error: \(sentence(fileError)) The service keeps "
                        + "using its last good settings. Fix the file to change settings here.",
                    actions: [.openFile, .checkAgain]))
        }
        if let conflict = session.conflict {
            banners.append(
                SettingsBanner(
                    severity: .warning,
                    message: "config.toml changed outside Settings. Showing the file's values. "
                        + notSaved(conflict, number: number),
                    actions: [.applyMine, .keepFileValues]))
        }
        if let problem = session.writeProblem {
            banners.append(
                SettingsBanner(
                    severity: .warning,
                    message: "Your changes are not saved yet. \(sentence(problem))",
                    actions: [.saveAgain]))
        }
        if pageId == servicePage, session.pendingRestart.contains(portKey),
            case .number(let newPort?)? = session.setting(portKey)?.saved
        {
            banners.append(
                SettingsBanner(
                    severity: .informational,
                    message: "Restart the service to use port \(portText(newPort)).",
                    actions: [.restartService]))
        }
        return banners
    }

    /// The line under Port while a new port waits for a restart, or `nil`.
    public static func restartNote(_ session: SettingsSession, key: String) -> String? {
        guard session.pendingRestart.contains(key), let listening = session.listeningPort else {
            return nil
        }
        return "Restart needed. The service still listens on \(listening)."
    }

    private static func unanswered(_ session: SettingsSession, port: UInt16) -> SettingsBanner? {
        switch session.status {
        case .stopped:
            SettingsBanner(
                severity: .informational,
                message: "The service is stopped. Start it to change these settings.",
                actions: [.startService])
        case .down:
            SettingsBanner(
                severity: .warning,
                message: "The service isn't answering on port \(port). Trying again every 5 s.",
                actions: [.tryNow, .openLogs])
        case .token:
            SettingsBanner(
                severity: .error,
                message: "The service refused this panel's key. Restart the service to make a "
                    + "new one.",
                actions: [.restartService])
        case .failed:
            SettingsBanner(
                severity: .error,
                message: "The service sent an answer Settings can't use. "
                    + sentence(session.detail),
                actions: [.tryNow, .openLogs])
        case .loading, .ready:
            nil
        }
    }

    // "Your change to Safety margin (15 %) was not saved."
    private static func notSaved(_ conflict: Conflict, number: (Double) -> String) -> String {
        let edits = conflict.edits.map { edit(of: $0, number: number) }
        return edits.count == 1
            ? "Your change to \(edits[0]) was not saved."
            : "Your changes to \(SettingsText.joined(edits)) were not saved."
    }

    private static func edit(of edit: ConflictEdit, number: (Double) -> String) -> String {
        guard let change = edit.changes.first else {
            return edit.setting.label
        }
        let value = SettingsText.changeValue(edit.setting, change, number: number)
        return "\(edit.setting.label) (\(value))"
    }

    // allotd's reasons come without a full stop; the banner's next sentence needs one.
    static func sentence(_ text: String?) -> String {
        let trimmed = (text ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty || trimmed.hasSuffix(".") ? trimmed : "\(trimmed)."
    }

    // A port is never grouped.
    private static func portText(_ port: Double) -> String {
        isWhole(port) && abs(port) < 1e15 ? String(Int64(port)) : String(port)
    }
}
