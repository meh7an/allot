import AllotContract

/// Decides from each poll which notifications to show (handoff section 9), as the Windows tray
/// app's `ToastRules`. Everything is kept in memory: the first live state after the app starts
/// is a baseline whose jobs, runs and notices never notify, so a restart repeats nothing, and
/// an event from while the app was not running is not shown. An event that comes while its
/// toggle is off or the panel is open counts as seen without a notification.
@MainActor
public final class NotificationRules {
    /// A job is running long at this multiple of its estimate.
    public static let longFactor = 1.5

    /// A service stop seen on a single poll is a restart (`allot install`, for one), not a
    /// stop.
    public static let stoppedPolls = 2

    /// Finishes within this time of a window's first notification join it.
    public static let mergeWindow = Duration.seconds(60)

    /// How long after the user chose Stop Service a stop counts as theirs.
    public static let expectedStopWindow = Duration.seconds(15)

    private static let silent: NotificationToggles = {
        var toggles = NotificationToggles()
        toggles.serviceStopped = false
        toggles.runningLong = false
        toggles.benchmarkFinished = false
        toggles.jobFinished = false
        toggles.killedOutsidePanel = false
        return toggles
    }()

    private let now: () -> ContinuousClock.Instant
    private var long: Set<Int64> = []
    private var runs: Set<RunKey> = []
    private var notices: Set<Int64> = []
    private var localKills: Set<Int64> = []
    private var merged: [RecentRun] = []
    private var mergeStart: ContinuousClock.Instant?
    private var mergeWindows = 0
    private var isBaselined = false
    private var wasRunning = false
    private var stoppedPollCount = 0
    private var isStopShown = false
    private var stopChosenAt: ContinuousClock.Instant?

    /// - Parameter now: The local clock of the merge and stop windows; the tests pass their own.
    public init(now: @escaping () -> ContinuousClock.Instant = { .now }) {
        self.now = now
    }

    /// The panel is about to kill this lease, so its notice is not a kill from outside. Called
    /// before the request, so a poll that sees the notice first cannot race it.
    public func noteLocalKill(_ lease: Int64) {
        localKills.insert(lease)
    }

    /// The user chose Stop Service, so the stop that follows does not notify.
    public func expectStop() {
        stopChosenAt = now()
    }

    /// - Parameters:
    ///   - status: The poll's service status.
    ///   - live: The state allotd served, or `nil` while it is not running.
    ///   - lastKnown: The newest state allotd served, for the session count after a stop.
    ///   - toggles: The More options › Notifications toggles.
    ///   - panelOpen: The panel is showing, so the user sees events already.
    public func observe(
        status: ServiceStatus,
        live: UiState?,
        lastKnown: UiState?,
        toggles: NotificationToggles,
        panelOpen: Bool
    ) -> NotificationUpdate {
        let allowed = panelOpen ? Self.silent : toggles
        var show: [NotificationMessage] = []
        var withdraw: [String] = []
        observeService(status, lastKnown, allowed, &show, &withdraw)
        if let live {
            if isBaselined {
                observeJobs(live, allowed, &show)
            } else {
                baseline(live)
            }
        }
        return show.isEmpty && withdraw.isEmpty
            ? .none : NotificationUpdate(show: show, withdraw: withdraw)
    }

    private func observeService(
        _ status: ServiceStatus,
        _ lastKnown: UiState?,
        _ allowed: NotificationToggles,
        _ show: inout [NotificationMessage],
        _ withdraw: inout [String]
    ) {
        if status == .running {
            wasRunning = true
            stoppedPollCount = 0
            if isStopShown {
                isStopShown = false
                withdraw.append(NotificationText.serviceTag)
            }
            return
        }

        // Starting and Incompatible change nothing: neither is a running service going away.
        guard status == .stopped, wasRunning else {
            return
        }
        stoppedPollCount += 1
        guard stoppedPollCount >= Self.stoppedPolls else {
            return
        }
        wasRunning = false
        let chosen = stopChosenAt.map { now() - $0 <= Self.expectedStopWindow } ?? false
        stopChosenAt = nil
        if !chosen && allowed.serviceStopped {
            show.append(NotificationText.serviceStopped(sessions: lastKnown?.sessions.count ?? 0))
            isStopShown = true
        }
    }

    private func baseline(_ live: UiState) {
        isBaselined = true
        for job in live.active where Self.isLong(job, nowMs: live.generatedAtMs) {
            long.insert(job.lease.id)
        }
        runs.formUnion(live.recent.map(RunKey.init))
        notices.formUnion(live.notices.map(\.leaseId))
    }

    private func observeJobs(
        _ live: UiState,
        _ allowed: NotificationToggles,
        _ show: inout [NotificationMessage]
    ) {
        var sessions: [String: UiSession] = [:]
        for session in live.sessions where sessions[session.session] == nil {
            sessions[session.session] = session
        }

        // Jobs, then runs and notices, each oldest first: allotd lists runs and notices newest
        // first.
        long.formIntersection(live.active.map(\.lease.id))
        for job in live.active where Self.isLong(job, nowMs: live.generatedAtMs) {
            if long.insert(job.lease.id).inserted && allowed.runningLong {
                show.append(NotificationText.runningLong(
                    job, session: sessions[job.lease.holder.session], nowMs: live.generatedAtMs))
            }
        }

        var finished: [RecentRun] = []
        for run in live.recent.reversed() {
            // A killed run has the kill's notification or the panel's own Stopped card.
            guard runs.insert(RunKey(run)).inserted,
                  run.endCause == nil || run.endCause == .released
            else {
                continue
            }
            if let noise = run.noise {
                if allowed.benchmarkFinished && run.leaseId != nil {
                    show.append(NotificationText.benchmarkFinished(run, noise: noise))
                }
            } else if allowed.jobFinished {
                finished.append(run)
            }
        }
        runs.formIntersection(live.recent.map(RunKey.init))
        if !finished.isEmpty {
            show.append(self.finished(finished, sessions))
        }

        for notice in live.notices.reversed() {
            if notices.insert(notice.leaseId).inserted,
               notice.cause == .killed,
               !localKills.contains(notice.leaseId),
               allowed.killedOutsidePanel {
                show.append(NotificationText.killedOutsidePanel(notice))
            }
        }
        notices.formIntersection(live.notices.map(\.leaseId))
    }

    // The first finish opens a window; later ones within it replace its notification with the
    // count.
    private func finished(
        _ runs: [RecentRun],
        _ sessions: [String: UiSession]
    ) -> NotificationMessage {
        let current = now()
        if merged.isEmpty || mergeStart.map({ current - $0 >= Self.mergeWindow }) ?? true {
            merged = []
            mergeStart = current
            mergeWindows += 1
        }
        merged += runs
        let tag = "done-\(mergeWindows)"
        if merged.count > 1 {
            return NotificationText.jobsFinished(merged, tag: tag)
        }
        let run = merged[0]
        return NotificationText.jobFinished(
            run, session: run.session.flatMap { sessions[$0] }, tag: tag)
    }

    private static func isLong(_ job: ActiveJob, nowMs: Int64) -> Bool {
        let lease = job.lease
        let elapsedMs = Double(nowMs - (lease.grantedAtMs ?? lease.requestedAtMs))
        let estSecs = Double(lease.profile.estSecs)
        return estSecs > 0 && elapsedMs >= longFactor * estSecs * 1000
    }

    // Runs from before v0.2 have no lease; their times tell them apart.
    private struct RunKey: Hashable {
        let lease: Int64?
        let startedAtMs: Int64
        let endedAtMs: Int64

        init(_ run: RecentRun) {
            lease = run.leaseId
            startedAtMs = run.startedAtMs
            endedAtMs = run.endedAtMs
        }
    }
}
