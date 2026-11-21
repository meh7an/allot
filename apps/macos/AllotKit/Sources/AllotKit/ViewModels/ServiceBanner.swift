import AllotContract
import Foundation

public enum BannerButton: Sendable, Equatable {
    /// No button: starting allotd again would not help.
    case none

    /// The accent Start Service button.
    case start

    /// The muted, disabled Starting… that follows a press.
    case starting

    /// The accent Update Service, which installs the binaries the app carries.
    case update

    /// Update Service while `~/.allot/bin` has no allotd.
    case install

    /// The muted, disabled Updating… while the bundled `allot install` runs.
    case updating
}

public enum BannerTone: Sendable, Equatable {
    /// The service is down.
    case danger

    /// The service runs, and the app carries a newer one.
    case notice
}

/// The banner at the top of the body while the service is down or the app carries a newer
/// service, and the caption above the last known cards (handoff section 6).
public struct ServiceBanner: Sendable, Equatable {
    public static let stoppedText = "Service stopped · commands run unchecked"
    public static let incompatibleText = "Service needs an update · run allot install"
    public static let missingText = "Service not installed"
    public static let unreadableText = "Service version unknown"

    public let tone: BannerTone

    /// The semibold banner line.
    public let text: String

    /// Why the last Start Service failed, or empty.
    public let problem: String

    public let button: BannerButton

    /// For example `Last known · 6:52 AM`; empty without a last known state, which also means
    /// no cards.
    public let caption: String

    /// - Parameters:
    ///   - status: The service status of the latest poll.
    ///   - lastKnown: The last state allotd served, if any.
    ///   - startProblem: Why the last Start Service failed, if it did.
    ///   - update: The newer service the app carries, if it carries one.
    ///   - timeZone: The time zone the caption's time is shown in.
    ///   - locale: The locale that formats the caption's time.
    /// - Returns: `nil` while the service runs and the app carries no newer one.
    public static func from(
        status: ServiceStatus,
        lastKnown: UiState?,
        startProblem: String?,
        update: ServiceUpdate? = nil,
        timeZone: TimeZone = .current,
        locale: Locale = .current
    ) -> ServiceBanner? {
        let caption = lastKnown.map {
            "Last known · \(localTime($0.generatedAtMs, timeZone: timeZone, locale: locale))"
        } ?? ""
        // Updating also starts the service, so it takes the place of Start Service.
        if let update, status != .starting || update.installing {
            return ServiceBanner(
                tone: status == .running ? .notice : .danger,
                text: text(of: update),
                problem: update.problem ?? "",
                button: button(of: update),
                caption: status == .running ? "" : caption)
        }
        switch status {
        case .running:
            return nil
        case .starting:
            return ServiceBanner(tone: .danger, text: stoppedText, problem: "",
                                 button: .starting, caption: caption)
        case .incompatible:
            return ServiceBanner(tone: .danger, text: incompatibleText, problem: "",
                                 button: .none, caption: caption)
        case .stopped:
            return ServiceBanner(tone: .danger, text: stoppedText, problem: startProblem ?? "",
                                 button: .start, caption: caption)
        }
    }

    private static func text(of update: ServiceUpdate) -> String {
        switch update.installed {
        case .version(let version):
            "Service \(version) is older than this app"
        case .missing:
            missingText
        case .unreadable:
            unreadableText
        }
    }

    private static func button(of update: ServiceUpdate) -> BannerButton {
        if update.installing {
            return .updating
        }
        return update.installed == .missing ? .install : .update
    }

    /// A daemon time as the user's short time of day, for example `6:52 AM`.
    public static func localTime(_ ms: Int64, timeZone: TimeZone, locale: Locale) -> String {
        var style = Date.FormatStyle(date: .omitted, time: .shortened)
        style.timeZone = timeZone
        style.locale = locale
        return Date(timeIntervalSince1970: Double(ms) / 1000).formatted(style)
    }
}
