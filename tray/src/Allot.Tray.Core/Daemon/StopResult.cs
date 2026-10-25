using Allot.Tray.Core.Contract;

namespace Allot.Tray.Core.Daemon;

/// <summary>
/// What allotd answered a Kill or Remove from the flyout.
/// </summary>
/// <param name="Ended">allotd ended the lease, and its session will be told.</param>
/// <param name="Text">Ended: a note for the Stopped line, or empty. Otherwise why not.</param>
/// <param name="EndedAtMs">Ended: when the lease ended, in the daemon's clock.</param>
public sealed record StopResult(bool Ended, string Text, long? EndedAtMs = null)
{
    /// <summary>
    /// A kill ends the lease even when part of the tree, or all of it, is left running; the note
    /// says so.
    /// </summary>
    public static StopResult Killed(KillResponse response)
    {
        var note = response.UntouchedReason is not null
            ? "nothing was running"
            : response.Failed.Count switch
            {
                0 => "",
                1 => "1 process still running",
                var count => $"{count} processes still running",
            };
        return new StopResult(true, note, response.Lease.EndedAtMs);
    }

    public static StopResult Removed(RemoveResponse response) =>
        new(true, "", response.Lease.EndedAtMs);

    public static StopResult Refused(string why) => new(false, why);
}

/// <summary>
/// Ends leases on the user's behalf: <see cref="DaemonClient"/>, or a fake for the Debug
/// scenarios. Only cancellation throws.
/// </summary>
public interface IJobStopper
{
    /// <summary>
    /// <c>POST /leases/{id}/kill</c>.
    /// </summary>
    Task<StopResult> KillAsync(long lease, CancellationToken cancellationToken);

    /// <summary>
    /// <c>POST /leases/{id}/remove</c>.
    /// </summary>
    Task<StopResult> RemoveAsync(long lease, CancellationToken cancellationToken);
}
