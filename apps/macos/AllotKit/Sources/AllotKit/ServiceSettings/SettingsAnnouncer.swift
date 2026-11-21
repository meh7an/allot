import Foundation

// What a Settings window says to a screen reader by itself, without a focus change (Mehran at
// M7.1: errors and outcomes): a field's new error, a banner that appears, and the outcome of a
// save. Live sentences and What fits are not spoken; they are read with their group when focus
// reaches it. Each panel speaks the text through its own toolkit.

/// A setting with an error, with the sentence to speak.
public struct AnnouncedIssue: Sendable, Equatable {
    public let key: String

    /// "Headroom: Enter a number at least 0 and below 100."
    public let text: String

    public init(key: String, text: String) {
        self.key = key
        self.text = text
    }
}

/// What the announcer compares between two changes of the window.
public struct AnnouncerView: Sendable, Equatable {
    /// The shown page's banner messages.
    public let banners: [String]

    /// Each setting with an error.
    public let issues: [AnnouncedIssue]

    public let save: SaveStatus

    public init(
        banners: [String] = [],
        issues: [AnnouncedIssue] = [],
        save: SaveStatus = .saved
    ) {
        self.banners = banners
        self.issues = issues
        self.save = save
    }

    /// The announcer's view of a session and the banners of the page shown.
    ///
    /// - Parameter number: Formats a number for the user's locale.
    @MainActor
    public static func of(
        _ session: SettingsSession,
        banners: [SettingsBanner],
        number: (Double) -> String
    ) -> AnnouncerView {
        var issues: [AnnouncedIssue] = []
        for page in session.pages {
            for group in page.groups {
                for setting in group.settings {
                    if let issue = setting.issue {
                        issues.append(
                            AnnouncedIssue(
                                key: setting.key,
                                text: "\(setting.label): \(issueText(issue, number: number))"))
                    }
                }
            }
        }
        return AnnouncerView(
            banners: banners.map(\.message), issues: issues, save: session.saveStatus)
    }
}

/// Turns successive views into what to speak: each new or changed error once, each banner as it
/// appears, and "All changes saved" when a save ends with nothing left unsaved. The first view,
/// and the first after another page shows, only sets what is already on screen.
public final class SettingsAnnouncer {
    public static let savedAnnouncement = "All changes saved"

    private var banners: Set<String>?
    private var issues: [String: String] = [:]
    private var saving = false

    public init() {}

    /// What to speak for this view, as one text; empty when nothing spoken changed.
    public func update(_ view: AnnouncerView) -> String {
        var spoken: [String] = []
        if let shown = banners {
            spoken.append(contentsOf: view.banners.filter { !shown.contains($0) })
        }
        for issue in view.issues where issues[issue.key] != issue.text {
            spoken.append(issue.text)
        }
        if saving && view.save == .saved && spoken.isEmpty {
            spoken.append(Self.savedAnnouncement)
        }
        banners = Set(view.banners)
        issues = Dictionary(
            view.issues.map { ($0.key, $0.text) }, uniquingKeysWith: { _, last in last })
        saving = view.save == .saving
        return spoken.joined(separator: " ")
    }

    /// Another page shows: its banners are read with the page, not announced.
    public func pageShown() {
        banners = nil
    }
}
