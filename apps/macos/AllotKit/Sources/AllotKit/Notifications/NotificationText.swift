import AllotContract

/// The wording of each notification (handoff section 9), as the Windows tray app's toasts.
public enum NotificationText {
    /// The one Service stopped notification, withdrawn once the service is back.
    public static let serviceTag = "service"

    /// The Recently finished card's row key.
    public static let recentTarget = "recent"

    /// An active or queued card's row key.
    public static func jobTarget(_ lease: Int64) -> String {
        "job:\(lease)"
    }

    public static func serviceStopped(sessions: Int) -> NotificationMessage {
        NotificationMessage(
            kind: .serviceStopped,
            tag: serviceTag,
            title: "Service stopped",
            body: "Heavy commands now run unchecked. \(connected(sessions))",
            buttons: [.startService, .openLogs],
            target: nil)
    }

    /// - Parameters:
    ///   - job: The job past 1.5 times its estimate.
    ///   - session: Its session, whose folder stands in when the lease has none.
    ///   - nowMs: The state's time.
    public static func runningLong(
        _ job: ActiveJob,
        session: UiSession?,
        nowMs: Int64
    ) -> NotificationMessage {
        let lease = job.lease
        let elapsed = max(0, (nowMs - (lease.grantedAtMs ?? lease.requestedAtMs)) / 1000)
        let times = "\(Format.duration(elapsed)) so far against a "
            + "~\(Format.duration(Int64(lease.profile.estSecs))) estimate"
        return NotificationMessage(
            kind: .runningLong,
            tag: "long-\(lease.id)",
            title: "\(lease.label) is running long",
            body: joined(times, session: lease.holder.session, cwd: lease.cwd, live: session),
            buttons: [.show],
            target: jobTarget(lease.id))
    }

    /// - Parameters:
    ///   - run: The finished benchmark run.
    ///   - noise: Its background load.
    public static func benchmarkFinished(
        _ run: RecentRun,
        noise: NoiseReport
    ) -> NotificationMessage {
        let forced = noise.quietAtStart ? "" : "not quiet at start · "
        let avg = Format.oneDecimal(Double(noise.avgCpuPct))
        let peak = Format.oneDecimal(Double(noise.peakCpuPct))
        return NotificationMessage(
            kind: .benchmarkFinished,
            tag: "bench-\(run.leaseId.map(String.init) ?? "")",
            title: "Benchmark finished in \(Format.duration(ranSecs(run)))",
            body: "Background noise avg \(avg)%, peak \(peak)% CPU · \(forced)\(run.command)",
            buttons: [],
            target: run.leaseId.map(jobTarget))
    }

    /// - Parameters:
    ///   - run: The finished run.
    ///   - session: Its session, whose folder stands in when the run has none.
    ///   - tag: The tag of this merge window.
    public static func jobFinished(
        _ run: RecentRun,
        session: UiSession?,
        tag: String
    ) -> NotificationMessage {
        NotificationMessage(
            kind: .jobFinished,
            tag: tag,
            title: "\(run.command) finished",
            body: joined(
                Format.duration(ranSecs(run)), session: run.session ?? "", cwd: run.cwd,
                live: session),
            buttons: [.show],
            target: recentTarget)
    }

    /// Several finishes within the merge window: `3 jobs finished` with the first two
    /// commands.
    ///
    /// - Parameters:
    ///   - runs: The window's runs, oldest first; at least two.
    ///   - tag: The tag of this merge window.
    public static func jobsFinished(_ runs: [RecentRun], tag: String) -> NotificationMessage {
        let commands = runs.count == 2
            ? "\(runs[0].command) and \(runs[1].command)"
            : "\(runs[0].command), \(runs[1].command) and \(runs.count - 2) more"
        return NotificationMessage(
            kind: .jobFinished,
            tag: tag,
            title: "\(runs.count) jobs finished",
            body: commands,
            buttons: [.show],
            target: recentTarget)
    }

    public static func killedOutsidePanel(_ notice: Notice) -> NotificationMessage {
        NotificationMessage(
            kind: .killedOutsidePanel,
            tag: "killed-\(notice.leaseId)",
            title: "\(notice.command) was stopped",
            body: "Killed from the command line · Claude was notified",
            buttons: [],
            target: nil)
    }

    private static func connected(_ sessions: Int) -> String {
        switch sessions {
        case 0: "No Claude sessions are connected."
        case 1: "1 Claude session is connected."
        default: "\(sessions) Claude sessions are connected."
        }
    }

    private static func ranSecs(_ run: RecentRun) -> Int64 {
        max(0, (run.endedAtMs - run.startedAtMs) / 1000)
    }

    // <first> · <session> · <project>, leaving out what is unknown. The job's own folder comes
    // first, as the notification is about that job.
    private static func joined(
        _ first: String,
        session: String,
        cwd: String,
        live: UiSession?
    ) -> String {
        var parts = [first]
        if !session.isEmpty {
            parts.append(Format.shortId(session))
        }
        let project = Format.project(cwd.isEmpty ? live?.cwd : cwd)
        if !project.isEmpty {
            parts.append(project)
        }
        return parts.joined(separator: " · ")
    }
}
