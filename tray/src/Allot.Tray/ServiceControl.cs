using Allot.Tray.Core.Contract;
using Allot.Tray.Core.Daemon;
using Allot.Tray.Core.TrayIcon;

namespace Allot.Tray;

/// <summary>
/// Start, Stop and Restart service, shared by the tray menu, the flyout's More options and the
/// Settings window. The caller asks before stopping, with <see cref="StopConfirmation"/>.
/// </summary>
/// <param name="poller">Polls allotd and launches it.</param>
/// <param name="data">The install, for the port a restart moves to.</param>
/// <param name="stopping">Told before a stop or restart, so its toast does not fire.</param>
internal sealed class ServiceControl(DaemonPoller poller, DataDirectory data, Action stopping)
{
    /// <summary>
    /// The latest poll's status, on the UI thread.
    /// </summary>
    public ServiceStatus Status { get; private set; } = ServiceStatus.Starting;

    /// <summary>
    /// The latest state allotd served, or <c>null</c> while it does not answer.
    /// </summary>
    public UiState? State { get; private set; }

    /// <summary>
    /// The version of the allotd that answered last.
    /// </summary>
    public string? Version { get; private set; }

    /// <summary>
    /// Why the last start failed, if it did.
    /// </summary>
    public string? StartProblem => poller.StartProblem;

    /// <summary>
    /// The port this panel dials; it follows a restart, not <c>config.toml</c>.
    /// </summary>
    public ushort Port => poller.Port;

    /// <summary>
    /// What the stop asks first, or <c>null</c> when nothing runs or waits.
    /// </summary>
    public string? StopConfirmation => ServiceMenu.StopConfirmation(State);

    /// <summary>
    /// Raised on the UI thread after <see cref="Update"/>.
    /// </summary>
    public event Action? Changed;

    /// <summary>
    /// Takes a poll; called on the UI thread.
    /// </summary>
    public void Update(PollResult result)
    {
        Status = result.Status;
        State = result.State;
        Version = result.State?.Service.Version ?? result.Version ?? Version;
        Changed?.Invoke();
    }

    public void Start()
    {
        if (poller.StartService() is { } problem)
        {
            AppLog.Write($"start service failed: {problem}");
        }
    }

    /// <summary>
    /// Stops allotd with the stop record, so it stays stopped until started again.
    /// </summary>
    /// <returns><c>null</c> when allotd accepted the stop, else why not.</returns>
    public async Task<string?> StopAsync()
    {
        AppLog.Write("stop service chosen");
        stopping();
        var problem = await poller.StopServiceAsync(CancellationToken.None).ConfigureAwait(true);
        if (problem is not null)
        {
            AppLog.Write($"stop service failed: {problem}");
        }

        return problem;
    }

    /// <summary>
    /// Stops allotd and starts it again on the port <c>config.toml</c> names now.
    /// </summary>
    /// <returns><c>null</c> when it was started again, else why not.</returns>
    public async Task<string?> RestartAsync()
    {
        AppLog.Write("restart service chosen");
        stopping();
        var port = DaemonConfig.ReadPort(data);
        var problem = await poller.RestartServiceAsync(port, CancellationToken.None)
            .ConfigureAwait(true);
        AppLog.Write(problem is null
            ? $"restarted the service on port {port}"
            : $"restart service failed: {problem}");
        return problem;
    }
}
