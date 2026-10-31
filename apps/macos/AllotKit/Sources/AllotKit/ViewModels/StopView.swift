public enum StopKind: Sendable, Equatable {
    /// Kill a running job's process tree, after the two-step confirm.
    case kill

    /// Remove a queued request, without a confirm because no work is lost.
    case remove

    /// Not a stop: the job's session released it, and its card holds as Finished.
    case finished

    /// Not a stop of this panel: `allot kill` or another Allot app killed it.
    case killedOutside

    /// Not a stop: allotd ended the lease (idle session, quick-lease overrun, holder gone).
    case endedByAllotd
}

public enum StopPhase: Sendable, Equatable {
    /// The red Kill? button is up and counting down.
    case confirming

    /// The request is on its way to allotd.
    case sending

    /// The lease ended; the card holds Stopped, Removed or how it ended, then collapses.
    case ended

    /// allotd refused or did not answer; the card says why in red.
    case failed
}

/// What this panel is doing to one job, as its card shows it (the prototype's per-job
/// `killPhase` and `removePhase`).
public struct StopView: Sendable, Equatable {
    /// Kill for a running job, Remove for a queued one, or how a job ended outside this panel.
    public let kind: StopKind

    public let phase: StopPhase

    /// While confirming, the countdown from 4 to 1.
    public let secondsLeft: Int

    /// Ended: a note for the Stopped line, or empty; allotd's reason for
    /// ``StopKind/endedByAllotd``. Failed: why not.
    public let message: String

    /// Ended: when allotd ended the lease, in the daemon's clock.
    public let endedAtMs: Int64?

    public init(
        kind: StopKind,
        phase: StopPhase,
        secondsLeft: Int = 0,
        message: String = "",
        endedAtMs: Int64? = nil
    ) {
        self.kind = kind
        self.phase = phase
        self.secondsLeft = secondsLeft
        self.message = message
        self.endedAtMs = endedAtMs
    }
}
