using System.Globalization;

using Allot.Tray.Core.Contract;
using Allot.Tray.Core.Daemon;
using Allot.Tray.Core.ServiceSettings;

namespace Allot.Tray.Core.ViewModels;

/// <summary>
/// What the flyout's update banner and the Service page's update card say about
/// <c>UiState.update</c> and offer.
/// </summary>
/// <param name="Title">The state in a line, or empty while there is nothing to show.</param>
/// <param name="Detail">Why a check, an update or a press failed, or empty.</param>
/// <param name="Checked">When the last check finished, or empty; the Service page only.</param>
/// <param name="UpdateText">The Update button's text, or empty without the button.</param>
/// <param name="CanUpdate">Whether Update is enabled.</param>
/// <param name="CheckText">The Check now button's text, or empty without the button.</param>
/// <param name="CanCheck">Whether Check now is enabled.</param>
/// <param name="InFlyout">The flyout shows a banner too: a release is out, installing or not
/// installed.</param>
/// <param name="Failed">The last update did not finish.</param>
public sealed record UpdateView(
    string Title,
    string Detail,
    string Checked,
    string UpdateText,
    bool CanUpdate,
    string CheckText,
    bool CanCheck,
    bool InFlyout,
    bool Failed)
{
    private const string Updating = "Updating…";

    /// <summary>
    /// Nothing to show. The flyout model holds this instead of <c>null</c>, as it holds
    /// <see cref="ServiceBanner.Hidden"/>.
    /// </summary>
    public static UpdateView Hidden { get; } =
        new("", "", "", "", false, "", false, false, false);

    /// <param name="status">The service status of the latest poll; only a running service has an
    /// update to show.</param>
    /// <param name="update">What allotd says, or <c>null</c> from one that does not check.</param>
    /// <param name="press">What a press of Update or Check now is doing.</param>
    /// <param name="now">The moment, to tell a check of today from an older one.</param>
    /// <param name="zone">The time zone the check's time is shown in.</param>
    /// <param name="culture">The culture that formats the check's time.</param>
    public static UpdateView From(
        ServiceStatus status,
        UpdateStatus? update,
        UpdatePress press,
        DateTimeOffset now,
        TimeZoneInfo zone,
        CultureInfo culture)
    {
        if (status != ServiceStatus.Running || update is null)
        {
            return Hidden;
        }

        var view = Of(update, press) with
        {
            Checked = LastChecked(update.CheckedAtMs, now, zone, culture),
        };
        return press.Problem is { } problem
            ? view with { Detail = SettingsBanners.Sentence(problem) }
            : view;
    }

    private static UpdateView Of(UpdateStatus update, UpdatePress press)
    {
        var checking = press.Pending == UpdateAction.Checking
            || update.State == UpdateState.Checking;
        var idle = press.Pending == UpdateAction.None;
        var check = Hidden with
        {
            CheckText = checking ? "Checking…" : "Check now",
            CanCheck = idle && !checking,
        };
        var starting = press.Pending == UpdateAction.Starting;
        var offer = check with
        {
            Title = $"Allot {update.Latest} is available",
            UpdateText = starting ? Updating : "Update",
            CanUpdate = idle && !checking,
            InFlyout = true,
        };
        var reason = SettingsBanners.Sentence(update.Reason);
        return update.State switch
        {
            UpdateState.Available => offer,
            UpdateState.Checking when IsNewer(update.Latest, update.Installed) => offer,
            UpdateState.Checking => check with { Title = "Checking for updates…" },
            UpdateState.Installing => Hidden with
            {
                Title = $"Updating to {update.Latest}…",
                UpdateText = Updating,
                InFlyout = true,
            },
            UpdateState.InstallFailed => offer with
            {
                Title = $"The update to {update.Latest} did not finish",
                Detail = reason,
                UpdateText = starting ? Updating : "Try again",
                Failed = true,
            },
            UpdateState.CheckFailed => check with
            {
                Title = "Could not check for updates",
                Detail = reason,
            },
            UpdateState.UpToDate => check with { Title = $"Allot {update.Installed} is up to date" },
            UpdateState.Unchecked => check with { Title = "Not checked for updates yet" },
            UpdateState.Disabled => Hidden with { Title = "Update checks are off" },
            _ => Hidden,
        };
    }

    private static string LastChecked(
        long? checkedAtMs,
        DateTimeOffset now,
        TimeZoneInfo zone,
        CultureInfo culture)
    {
        if (checkedAtMs is not { } ms)
        {
            return "";
        }

        var then = TimeZoneInfo.ConvertTime(DateTimeOffset.FromUnixTimeMilliseconds(ms), zone);
        var today = TimeZoneInfo.ConvertTime(now, zone);
        var format = then.Date == today.Date ? "t" : "g";
        return $"Last checked {then.DateTime.ToString(format, culture)}";
    }

    // Release numbers alone: only stable releases are offered, so a suffix never decides.
    private static bool IsNewer(string? latest, string installed) =>
        Release(latest) is { } offered && Release(installed) is { } current && offered > current;

    private static Version? Release(string? text) =>
        Version.TryParse(text?.Split('-', '+')[0], out var version) ? version : null;
}
