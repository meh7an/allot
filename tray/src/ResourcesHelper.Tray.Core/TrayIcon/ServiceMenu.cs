using ResourcesHelper.Tray.Core.Contract;
using ResourcesHelper.Tray.Core.Daemon;

namespace ResourcesHelper.Tray.Core.TrayIcon;

public enum ServiceAction
{
    Stop,
    Start,

    /// <summary>
    /// rhd is starting; the item is shown disabled.
    /// </summary>
    None,
}

/// <param name="Text">The menu text.</param>
/// <param name="Action">What choosing it does.</param>
public sealed record ServiceMenuItem(string Text, ServiceAction Action)
{
    public bool Enabled => Action != ServiceAction.None;
}

/// <summary>
/// The tray menu's service item (handoff section 8): Stop service while rhd answers, Start
/// service while it is down.
/// </summary>
public static class ServiceMenu
{
    public const string ConfirmTitle = "Stop Resources Helper?";

    public static ServiceMenuItem ItemFor(ServiceStatus status) => status switch
    {
        ServiceStatus.Running or ServiceStatus.Incompatible =>
            new ServiceMenuItem("Stop service…", ServiceAction.Stop),
        ServiceStatus.Starting => new ServiceMenuItem("Starting service…", ServiceAction.None),
        ServiceStatus.Stopped => new ServiceMenuItem("Start service", ServiceAction.Start),
        _ => throw new ArgumentOutOfRangeException(nameof(status)),
    };

    /// <summary>
    /// The confirmation shown before stopping rhd, or <c>null</c> when no lease is running or
    /// waiting. Every Claude session's MCP server starts rhd again on its next call, so the text
    /// says the stop lasts only until then.
    /// </summary>
    public static string? StopConfirmation(UiState? state)
    {
        var running = state?.Active.Count ?? 0;
        var waiting = state?.Queue.Count ?? 0;
        if (running == 0 && waiting == 0)
        {
            return null;
        }

        var parts = new List<string>(2);
        if (running > 0)
        {
            parts.Add(running == 1 ? "1 job is running" : $"{running} jobs are running");
        }

        if (waiting > 0)
        {
            parts.Add(waiting == 1 ? "1 request is waiting" : $"{waiting} requests are waiting");
        }

        return $"{string.Join(" and ", parts)}.\n\n"
            + "Heavy commands run unchecked while the service is stopped. A Claude session starts "
            + "it again the next time it asks for resources.";
    }
}
