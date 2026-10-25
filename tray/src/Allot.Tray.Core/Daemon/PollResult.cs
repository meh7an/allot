using Allot.Tray.Core.Contract;

namespace Allot.Tray.Core.Daemon;

public enum ServiceStatus
{
    /// <summary>
    /// allotd answered with UI state.
    /// </summary>
    Running,

    /// <summary>
    /// allotd was just started, or answers but has no first sample yet.
    /// </summary>
    Starting,

    /// <summary>
    /// Nothing answers on the port, or no token exists yet.
    /// </summary>
    Stopped,

    /// <summary>
    /// allotd answers but cannot serve this tray app: an older build without <c>/ui/state</c>, a
    /// state shape the generated types reject, or a token it refuses. Starting it again does not
    /// help; reinstalling does.
    /// </summary>
    Incompatible,
}

/// <summary>
/// The outcome of one poll.
/// </summary>
/// <param name="Status">The service status.</param>
/// <param name="State">The UI state; only set while <see cref="ServiceStatus.Running"/>.</param>
/// <param name="Version">The daemon version, when it is known.</param>
/// <param name="Detail">Why the service is not running, for logs and tooltips.</param>
public sealed record PollResult(
    ServiceStatus Status,
    UiState? State = null,
    string? Version = null,
    string? Detail = null);
