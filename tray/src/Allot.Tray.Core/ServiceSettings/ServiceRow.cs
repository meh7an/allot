using System.Globalization;

using Allot.Tray.Core.Daemon;
using Allot.Tray.Core.ViewModels;

namespace Allot.Tray.Core.ServiceSettings;

public enum ServiceButton
{
    Stop,
    Start,

    /// <summary>
    /// The disabled, busy Start that follows a press.
    /// </summary>
    Starting,
}

/// <summary>
/// The status row at the top of the Service page (DESIGN.md section 4, Service).
/// </summary>
/// <param name="Text">The state in a sentence.</param>
/// <param name="Button">The button beside it.</param>
/// <param name="OpenLogs">Whether Open logs shows too.</param>
public sealed record ServiceRow(string Text, ServiceButton Button, bool OpenLogs)
{
    public const string StoppedByYou =
        "Stopped. Heavy commands run without leases until you start it.";

    public const string Starting = "Starting…";

    /// <param name="status">The tray's latest poll.</param>
    /// <param name="session">The settings session's status, which knows the stop record.</param>
    /// <param name="version">allotd's version, when it is known.</param>
    /// <param name="port">The port allotd listens on, or this panel dials.</param>
    public static ServiceRow From(
        ServiceStatus status,
        SessionStatus session,
        string? version,
        uint port)
    {
        var portText = port.ToString(CultureInfo.InvariantCulture);
        return status switch
        {
            ServiceStatus.Running => new ServiceRow(
                version is null
                    ? $"Running · port {portText}"
                    : $"Running · version {version} · port {portText}",
                ServiceButton.Stop,
                false),
            ServiceStatus.Incompatible =>
                new ServiceRow(ServiceBanner.IncompatibleText, ServiceButton.Stop, true),
            ServiceStatus.Starting => new ServiceRow(Starting, ServiceButton.Starting, false),
            _ when session == SessionStatus.Stopped =>
                new ServiceRow(StoppedByYou, ServiceButton.Start, false),
            _ => new ServiceRow($"Not answering on port {portText}.", ServiceButton.Start, true),
        };
    }

    public string ButtonText => Button switch
    {
        ServiceButton.Stop => "Stop service",
        ServiceButton.Start => "Start service",
        _ => Starting,
    };
}
