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

/// The state the panel and the menu bar item render: the latest snapshot, the panel's mode,
/// and the Kill and Remove of each card.
@MainActor
@Observable
public final class PanelModel {
    public private(set) var snapshot: PanelSnapshot
    public private(set) var menuBar: MenuBarStatus
    public private(set) var mode = PanelMode.lightDismiss

    /// `tray.json`, loaded at launch; later milestones read and write it.
    public var settings: TraySettings

    /// Sends Kill and Remove; without one, presses only change the cards.
    @ObservationIgnored public var stopper: (any JobStopper)?

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
    public func apply(_ result: PollResult, lastKnown: UiState?) {
        status = result.status
        shown = result.state ?? lastKnown
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
            nowMs: clock.nowMs)
        // Observation reports every assignment, so equal values are skipped.
        if next != snapshot {
            snapshot = next
            let status = MenuBarStatus.from(next)
            if status != menuBar {
                menuBar = status
            }
        }
        onRefresh?()
    }

    /// A press of Kill, or of Kill? within its countdown.
    ///
    /// - Returns: The request to allotd when the press sends one, for the tests to await.
    @discardableResult
    public func pressKill(_ lease: Int64) -> Task<Void, Never>? {
        frozenStops = [:]
        let press = stops.pressKill(lease)
        refresh()
        return press == .send ? send(lease, .kill) : nil
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
