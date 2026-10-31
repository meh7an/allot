/// The menu bar icon's state, one template image each.
public enum MenuBarIcon: String, Sendable, CaseIterable {
    case idle
    case busy
    case benchmark
    case queued
    case down

    /// The image set in the app's asset catalog, written by `assets/brand/build.py`.
    public var assetName: String {
        "MenuBar" + rawValue.prefix(1).uppercased() + rawValue.dropFirst()
    }
}

/// What the menu bar item shows for one snapshot: the icon and the summary its tooltip and
/// accessibility label repeat.
public struct MenuBarStatus: Sendable, Equatable {
    public static let appName = "Allot"

    public let icon: MenuBarIcon
    public let summary: String

    public var tooltip: String {
        "\(Self.appName) · \(summary)"
    }

    /// Down while allotd is not serving state; Benchmark while a benchmark drains, waits for
    /// quiet or runs alone; Queued while anything else waits; Busy while a job runs; Idle
    /// otherwise. The same order as the Windows tray icon.
    public static func from(_ snapshot: PanelSnapshot) -> MenuBarStatus {
        let isWaiting = snapshot.groups.contains { group in group.jobs.contains(where: \.isQueued) }
        let icon: MenuBarIcon = if snapshot.isLastKnown {
            .down
        } else if snapshot.benchmark != nil {
            .benchmark
        } else if isWaiting {
            .queued
        } else if snapshot.header.glyph == .busy {
            .busy
        } else {
            .idle
        }
        return MenuBarStatus(icon: icon, summary: snapshot.header.summary)
    }
}
