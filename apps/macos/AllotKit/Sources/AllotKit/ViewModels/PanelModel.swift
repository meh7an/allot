import AllotContract
import Observation

/// How the panel behaves toward other windows.
public enum PanelMode: Sendable, Equatable {
    /// Closes on a click elsewhere, an app switch or Esc; sits above app windows.
    case lightDismiss

    /// Stays until closed from the menu bar; a normal window that others can cover.
    case stayOpen

    /// Stay open, floating above app windows on every Space and draggable by its header.
    case pinned

    public var isStayOpen: Bool {
        self != .lightDismiss
    }

    public var isPinned: Bool {
        self == .pinned
    }
}

/// The state the panel and the menu bar item render: the latest snapshot and the panel's mode.
@MainActor
@Observable
public final class PanelModel {
    public private(set) var snapshot: PanelSnapshot
    public private(set) var menuBar: MenuBarStatus
    public private(set) var mode = PanelMode.lightDismiss

    /// `tray.json`, loaded at launch; later milestones read and write it.
    public var settings: TraySettings

    /// Until the first poll answers, the service is assumed to be starting.
    public init(settings: TraySettings = TraySettings()) {
        let snapshot = PanelSnapshot.from(status: .starting, state: nil)
        self.snapshot = snapshot
        menuBar = MenuBarStatus.from(snapshot)
        self.settings = settings
    }

    /// Shows a poll; while allotd is not running, the jobs come from `lastKnown`.
    public func apply(_ result: PollResult, lastKnown: UiState?) {
        let next = PanelSnapshot.from(status: result.status, state: result.state ?? lastKnown)
        // Observation reports every assignment, so equal snapshots are skipped.
        guard next != snapshot else {
            return
        }
        snapshot = next
        let status = MenuBarStatus.from(next)
        if status != menuBar {
            menuBar = status
        }
    }

    /// Opening from the menu bar always starts in light-dismiss mode.
    public func resetForOpen() {
        mode = .lightDismiss
    }

    /// Stay open on, or off; turning it off also unpins.
    public func toggleStayOpen() {
        mode = mode.isStayOpen ? .lightDismiss : .stayOpen
    }

    /// Pin exists only while Stay open is on.
    public func togglePinned() {
        switch mode {
        case .lightDismiss: break
        case .stayOpen: mode = .pinned
        case .pinned: mode = .stayOpen
        }
    }
}
