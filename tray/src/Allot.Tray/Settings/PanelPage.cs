using Allot.Tray.Core.ServiceSettings;
using Allot.Tray.Core.Settings;

using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;

namespace Allot.Tray.Settings;

/// <summary>
/// What the Panel page changes outside the window: <c>tray.json</c>, the autostart entry and
/// Windows' notification settings. It works whatever state allotd is in.
/// </summary>
internal sealed record PanelHost(
    Func<TraySettings> Settings,
    Action<Action<TraySettings>> Change,
    Func<bool> ReadLaunchAtLogon,
    Action<bool> SetLaunchAtLogon,
    Func<bool> ReadNotificationsOff,
    Action OpenNotificationSettings);

/// <summary>
/// The Panel page (DESIGN.md section 4, Panel): Meters, Notifications and Startup, saved at once
/// to the tray's own store, and its own Reset page.
/// </summary>
internal sealed class PanelPage
{
    public const string Title = "Panel";

    private const string StartupHelp = "Start Allot when you sign in.";

    private readonly PanelHost _host;
    private readonly Func<Confirmation, Task<bool>> _confirm;
    private readonly List<Action> _syncs = [];
    private readonly InfoBar _notificationsOff;
    private readonly Button _reset;
    private readonly TextBlock _nothing;
    private bool _syncing;

    public PanelPage(PanelHost host, Func<Confirmation, Task<bool>> confirm)
    {
        _host = host;
        _confirm = confirm;
        var panel = new StackPanel { Padding = new Thickness(32, 16, 32, 32), MaxWidth = 1000 };
        var heading = SettingsUi.Text(Title, "TitleTextBlockStyle");
        AutomationProperties.SetHeadingLevel(heading, AutomationHeadingLevel.Level1);
        panel.Children.Add(heading);
        panel.Children.Add(SettingsUi.Text(
            "How the flyout looks and when it tells you something. These work while the service "
                + "is stopped.",
            "BodyTextBlockStyle"));

        panel.Children.Add(SettingsUi.Heading("Meters"));
        var meters = new List<UIElement>
        {
            SettingsUi.Text(PanelToggles.MetersHelp, "SettingsHelpTextStyle"),
        };
        meters.AddRange(PanelToggles.Meters.Select(Row));
        panel.Children.Add(SettingsUi.Card(meters));

        panel.Children.Add(SettingsUi.Heading("Notifications"));
        _notificationsOff = new InfoBar
        {
            Severity = InfoBarSeverity.Warning,
            Message = "Notifications for Allot are off in Windows Settings.",
            IsClosable = false,
            ActionButton = SettingsUi.Button(
                "Open notification settings",
                host.OpenNotificationSettings),
            Margin = new Thickness(0, 0, 0, 8),
        };
        panel.Children.Add(_notificationsOff);
        panel.Children.Add(SettingsUi.Card(PanelToggles.Notifications.Select(Row)));

        panel.Children.Add(SettingsUi.Heading("Startup"));
        panel.Children.Add(SettingsUi.Card([LaunchAtLogonRow()]));

        panel.Children.Add(SettingsUi.Heading("Reset"));
        _reset = SettingsUi.Button("Reset page…", () => _ = ResetAsync());
        _reset.HorizontalAlignment = HorizontalAlignment.Left;
        _nothing = SettingsUi.Text(SettingsText.NothingToReset, "SettingsHelpTextStyle");
        _nothing.Margin = new Thickness(0, 8, 0, 0);
        panel.Children.Add(_reset);
        panel.Children.Add(_nothing);

        Root = new ScrollViewer { Content = panel };
    }

    public ScrollViewer Root { get; }

    /// <summary>
    /// Reads the stores again: on every show, since the autostart entry and Windows' notification
    /// setting can change elsewhere.
    /// </summary>
    public void Sync()
    {
        _syncing = true;
        foreach (var sync in _syncs)
        {
            sync();
        }

        _syncing = false;
        _notificationsOff.IsOpen = _host.ReadNotificationsOff();
        var changed = PanelToggles.Changed(_host.Settings()).Count > 0;
        _reset.IsEnabled = changed;
        _nothing.Visibility = SettingsUi.Shows(!changed);
    }

    private SettingRow Row(PanelToggle toggle)
    {
        var control = new ToggleSwitch();
        var row = new SettingRow(toggle.Label, toggle.Help, null, control)
        {
            Indented = toggle.Parent is not null,
        };
        control.Toggled += (_, _) =>
        {
            if (!_syncing)
            {
                var on = control.IsOn;
                _host.Change(settings => toggle.Set(settings, on));
                Sync();
            }
        };
        _syncs.Add(() =>
        {
            var settings = _host.Settings();
            control.IsOn = toggle.Get(settings);
            control.IsEnabled = toggle.EnabledIn(settings);
        });
        return row;
    }

    private SettingRow LaunchAtLogonRow()
    {
        var control = new ToggleSwitch();
        var row = new SettingRow("Launch at logon", StartupHelp, null, control);
        control.Toggled += (_, _) =>
        {
            if (!_syncing)
            {
                _host.SetLaunchAtLogon(control.IsOn);
                Sync();
            }
        };
        _syncs.Add(() => control.IsOn = _host.ReadLaunchAtLogon());
        return row;
    }

    private async Task ResetAsync()
    {
        var settings = _host.Settings();
        var changed = PanelToggles.Changed(settings)
            .Select(toggle => $"{toggle.Label}: {SettingsText.OnOff(toggle.Default)}")
            .ToList();
        if (changed.Count == 0)
        {
            return;
        }

        if (await _confirm(SettingsText.ResetPage(Title, changed)).ConfigureAwait(true))
        {
            _host.Change(PanelToggles.Reset);
            Sync();
        }
    }
}
