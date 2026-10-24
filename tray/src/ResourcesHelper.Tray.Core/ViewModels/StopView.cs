namespace ResourcesHelper.Tray.Core.ViewModels;

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
}

public enum StopPhase
{
    /// <summary>
    /// The red Kill? button is up and counting down.
    /// </summary>
    Confirming,

    /// <summary>
    /// The request is on its way to rhd.
    /// </summary>
    Sending,

    /// <summary>
    /// rhd ended the lease; the card holds Stopped or Removed, then collapses.
    /// </summary>
    Ended,

    /// <summary>
    /// rhd refused or did not answer; the card says why in red.
    /// </summary>
    Failed,
}

/// <summary>
/// What this flyout is doing to one job, as its card shows it (the prototype's per-job
/// <c>killPhase</c> and <c>removePhase</c>).
/// </summary>
/// <param name="Kind">Kill for a running job, Remove for a queued one.</param>
/// <param name="Phase">Where the stop stands.</param>
/// <param name="SecondsLeft">While confirming, the countdown from 4 to 1.</param>
/// <param name="Message">Ended: a note for the Stopped line, or empty. Failed: why not.</param>
/// <param name="EndedAtMs">Ended: when rhd ended the lease, in the daemon's clock.</param>
public sealed record StopView(
    StopKind Kind,
    StopPhase Phase,
    int SecondsLeft = 0,
    string Message = "",
    long? EndedAtMs = null);
