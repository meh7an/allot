namespace Allot.Tray.Core.Daemon;

/// <summary>
/// The update routes of allotd: <see cref="DaemonClient"/>, or a fake in tests. Each answers
/// <c>null</c> when allotd accepted, else why not. Only cancellation throws.
/// </summary>
public interface IUpdateClient
{
    /// <summary>
    /// <c>POST /update/check</c>: asks allotd to look for a newer release now.
    /// </summary>
    Task<string?> CheckForUpdateAsync(CancellationToken cancellationToken);

    /// <summary>
    /// <c>POST /update/start</c>: asks allotd to start <c>allot update</c>.
    /// </summary>
    Task<string?> StartUpdateAsync(CancellationToken cancellationToken);

    /// <summary>
    /// <c>POST /update/announced</c>: the notification for <paramref name="version"/> was raised,
    /// so no panel raises it again.
    /// </summary>
    Task<string?> UpdateAnnouncedAsync(string version, CancellationToken cancellationToken);
}
