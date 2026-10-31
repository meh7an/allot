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

/// The state the panel and the menu bar item render: the latest snapshot, the Resources card,
/// the service-down banner, the panel's mode, the Kill and Remove of each card, and the
/// settings and login item behind the menus and the first-run card.
@MainActor
@Observable
public final class PanelModel {
    public private(set) var snapshot: PanelSnapshot
    public private(set) var menuBar: MenuBarStatus
    public private(set) var mode = PanelMode.lightDismiss
    public private(set) var resources = ResourcesView.hidden

    /// The banner while the service is down, else `nil`.
    public private(set) var banner: ServiceBanner?

    /// `tray.json`, loaded at launch; changed through ``updateSettings(_:)``.
    public private(set) var settings: TraySettings

    /// The login item as last read; ``refreshLogin()`` reads it again.
    public private(set) var login = LoginItemStatus.off

    /// The row a notification's Show asked for (`job:<lease>` or `recent`), until the panel
    /// has scrolled to it; empty for the top.
    public var scrollTarget: String?

    /// Sends Kill and Remove; without one, presses only change the cards.
    @ObservationIgnored public var stopper: (any JobStopper)?

    /// The app's login item; without one, Launch at login stays off.
    @ObservationIgnored public var loginItem: (any LoginItem)? {
        didSet {
            refreshLogin()
        }
    }

    /// Called after every change of ``settings``, so the caller can save them.
    @ObservationIgnored public var onSettingsChanged: ((TraySettings) -> Void)?

    /// Called right before a Kill goes to allotd, so the notification rules know it is ours.
    @ObservationIgnored public var onKillSent: ((Int64) -> Void)?

    /// The volume name mounted at a drive's mount point, for the drive rows.
    @ObservationIgnored public var volumeName: (String) -> String? = { _ in nil }

    /// Called after every rebuild of the snapshot, so the caller can time the next one.
    @ObservationIgnored public var onRefresh: (() -> Void)?

    /// Called when allotd has answered a Kill or Remove, so the caller can poll at once.
    @ObservationIgnored public var onStopAnswered: ((Int64, StopKind, StopResult) -> Void)?

    /// Debug scenarios: stops shown as they are until the first press.
    @ObservationIgnored public var frozenStops: [Int64: StopView] = [:]

    private let stops: JobStops
    private let clock: StateClock
    @ObservationIgnored private var status = ServiceStatus.starting
    @ObservationIgnored private var shown: UiState?
    @ObservationIgnored private var lastKnown: UiState?
    @ObservationIgnored private var startProblem: String?

    /// Until the first poll answers, the service is assumed to be starting.
    ///
    /// - Parameter now: The local clock that ticks times and stops; the tests pass their own.
    public init(
        settings: TraySettings = TraySettings(),
        now: @escaping () -> ContinuousClock.Instant = { .now }
    ) {
        let snapshot = PanelSnapshot.from(status: .starting, state: nil)
        self.snapshot = snapshot
        menuBar = MenuBarStatus.from(snapshot)
        self.settings = settings
        stops = JobStops(now: now)
        clock = StateClock(now: now)
    }

    /// The time until a card changes by itself (a countdown second or a hold ending), or `nil`.
    public var nextStopChange: Duration? {
        stops.nextChange
    }

    /// Shows a poll; while allotd is not running, the jobs come from `lastKnown`.
    ///
    /// - Parameter startProblem: Why the last Start Service failed, for the banner.
    public func apply(_ result: PollResult, lastKnown: UiState?, startProblem: String? = nil) {
        status = result.status
        shown = result.state ?? lastKnown
        self.lastKnown = lastKnown
        self.startProblem = startProblem
        clock.received(result.state)
        if let live = result.state {
            stops.observe(live)
        } else {
            stops.cancelConfirms()
        }
        refresh()
    }

    /// Rebuilds the snapshot for the current moment, so times tick between polls.
    public func refresh() {
        stops.expire()
        let jobs = status == .running ? shown.map(stops.merge) : nil
        let next = PanelSnapshot.from(
            status: status,
            state: shown,
            jobs: jobs,
            stops: frozenStops.isEmpty ? stops.views : frozenStops,
            nowMs: clock.nowMs,
            firstRun: FirstRunState(isDone: settings.firstRunDone, login: login))
        // Observation reports every assignment, so equal values are skipped.
        if next != snapshot {
            snapshot = next
            let status = MenuBarStatus.from(next)
            if status != menuBar {
                menuBar = status
            }
        }
        let card = ResourcesView.from(
            status: status, state: shown, toggles: settings.meters,
            expanded: settings.resourcesExpanded, volumeName: volumeName)
        if card != resources {
            resources = card
        }
        let banner = ServiceBanner.from(
            status: status, lastKnown: lastKnown, startProblem: startProblem)
        if banner != self.banner {
            self.banner = banner
        }
        onRefresh?()
    }

    /// Changes `tray.json`'s settings, shows the change and hands them to be saved.
    public func updateSettings(_ change: (inout TraySettings) -> Void) {
        var next = settings
        change(&next)
        guard next != settings else {
            return
        }
        settings = next
        refresh()
        onSettingsChanged?(next)
    }

    /// The Resources card's header: expanded shows the rows, collapsed the summary.
    public func toggleResources() {
        updateSettings { $0.resourcesExpanded.toggle() }
    }

    /// Reads the login item again: System Settings can change it at any time.
    public func refreshLogin() {
        let status = loginItem?.status ?? .off
        if status != login {
            login = status
            refresh()
        }
    }

    /// The first-run card's switch and the menus' Launch at Login.
    ///
    /// - Throws: Why macOS refused the change; the switch shows the login item as it is.
    public func setLaunchAtLogin(_ on: Bool) throws {
        defer {
            refreshLogin()
        }
        try loginItem?.set(on)
    }

    /// The first-run card's Done.
    public func finishFirstRun() {
        updateSettings { $0.firstRunDone = true }
    }

    /// A press of Kill, or of Kill? within its countdown.
    ///
    /// - Returns: The request to allotd when the press sends one, for the tests to await.
    @discardableResult
    public func pressKill(_ lease: Int64) -> Task<Void, Never>? {
        frozenStops = [:]
        let press = stops.pressKill(lease)
        refresh()
        guard press == .send else {
            return nil
        }
        onKillSent?(lease)
        return send(lease, .kill)
    }

    /// A press of Remove on a queued request.
    ///
    /// - Returns: The request to allotd when the press sends one, for the tests to await.
    @discardableResult
    public func pressRemove(_ lease: Int64) -> Task<Void, Never>? {
        frozenStops = [:]
        let sends = stops.pressRemove(lease)
        refresh()
        return sends ? send(lease, .remove) : nil
    }

    /// Whether a Kill? confirm is up, which Esc cancels before it closes the panel.
    public var isConfirming: Bool {
        stops.isConfirming
    }

    /// Esc, the panel losing focus or closing: every Kill? goes back to Kill.
    ///
    /// - Returns: True when any confirm was cancelled.
    @discardableResult
    public func cancelConfirms() -> Bool {
        guard stops.cancelConfirms() else {
            return false
        }
        refresh()
        return true
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

    private func send(_ lease: Int64, _ kind: StopKind) -> Task<Void, Never>? {
        guard let stopper else {
            return nil
        }
        return Task { [weak self] in
            let answer: StopResult?
            if kind == .kill {
                answer = try? await stopper.kill(lease)
            } else {
                answer = try? await stopper.remove(lease)
            }
            // Only an app quitting cancels a stop.
            guard let self, let answer else {
                return
            }
            stops.finished(lease, answer)
            refresh()
            onStopAnswered?(lease, kind, answer)
        }
    }
}
