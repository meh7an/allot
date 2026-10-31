import AllotContract
import Foundation

public enum BannerButton: Sendable, Equatable {
    /// No button: starting allotd again would not help.
    case none

    /// The accent Start Service button.
    case start

    /// The muted, disabled Starting… that follows a press.
    case starting
}

/// The banner at the top of the body while the service is down, and the caption above the
/// last known cards (handoff section 6).
public struct ServiceBanner: Sendable, Equatable {
    public static let stoppedText = "Service stopped · commands run unchecked"
    public static let incompatibleText = "Service needs an update · run allot install"

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
    ///   - timeZone: The time zone the caption's time is shown in.
    ///   - locale: The locale that formats the caption's time.
    /// - Returns: `nil` while the service runs.
    public static func from(
        status: ServiceStatus,
        lastKnown: UiState?,
        startProblem: String?,
        timeZone: TimeZone = .current,
        locale: Locale = .current
    ) -> ServiceBanner? {
        let caption = lastKnown.map {
            "Last known · \(localTime($0.generatedAtMs, timeZone: timeZone, locale: locale))"
        } ?? ""
        switch status {
        case .running:
            return nil
        case .starting:
            return ServiceBanner(text: stoppedText, problem: "", button: .starting,
                                 caption: caption)
        case .incompatible:
            return ServiceBanner(text: incompatibleText, problem: "", button: .none,
                                 caption: caption)
        case .stopped:
            return ServiceBanner(text: stoppedText, problem: startProblem ?? "", button: .start,
                                 caption: caption)
        }
    }

    /// A daemon time as the user's short time of day, for example `6:52 AM`.
    public static func localTime(_ ms: Int64, timeZone: TimeZone, locale: Locale) -> String {
        var style = Date.FormatStyle(date: .omitted, time: .shortened)
        style.timeZone = timeZone
        style.locale = locale
        return Date(timeIntervalSince1970: Double(ms) / 1000).formatted(style)
    }
}
