namespace Allot.Tray.Core.ViewModels;

public enum StopKind
{
    /// <summary>
    /// Kill a running job's process tree, after the two-step confirm.
    /// </summary>
    Kill,

    /// <summary>
    /// Remove a queued request, without a confirm because no work is lost.
    /// </summary>
    Remove,

    /// <summary>
    /// Not a stop: the job's session released it, and its card holds as Finished.
    /// </summary>
    Finished,

    /// <summary>
    /// Not a stop of this flyout: <c>allot kill</c> or another tray app killed it.
    /// </summary>
    KilledOutside,

    /// <summary>
    /// Not a stop: allotd ended the lease (idle session, quick-lease overrun, holder gone).
    /// </summary>
    EndedByRhd,
}

public enum StopPhase
{
    /// <summary>
    /// The red Kill? button is up and counting down.
    /// </summary>
    Confirming,

    /// <summary>
    /// The request is on its way to allotd.
    /// </summary>
    Sending,

    /// <summary>
    /// The lease ended; the card holds Stopped, Removed or how it ended, then collapses.
    /// </summary>
    Ended,

    /// <summary>
    /// allotd refused or did not answer; the card says why in red.
    /// </summary>
    Failed,
}

/// <summary>
/// What this flyout is doing to one job, as its card shows it (the prototype's per-job
/// <c>killPhase</c> and <c>removePhase</c>).
/// </summary>
/// <param name="Kind">Kill for a running job, Remove for a queued one, or how a job ended outside
/// this flyout.</param>
/// <param name="Phase">Where the stop stands.</param>
/// <param name="SecondsLeft">While confirming, the countdown from 4 to 1.</param>
/// <param name="Message">Ended: a note for the Stopped line, or empty; allotd's reason for
/// <see cref="StopKind.EndedByRhd"/>. Failed: why not.</param>
/// <param name="EndedAtMs">Ended: when allotd ended the lease, in the daemon's clock.</param>
public sealed record StopView(
    StopKind Kind,
    StopPhase Phase,
    int SecondsLeft = 0,
    string Message = "",
    long? EndedAtMs = null);
