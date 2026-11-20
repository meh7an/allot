using System.ComponentModel;
using System.Diagnostics;

using Allot.Tray.Core.Daemon;
using Allot.Tray.Core.ServiceSettings;
using Allot.Tray.Core.TrayIcon;

using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;

namespace Allot.Tray.Settings;

/// <summary>
/// The Service page's cards that hold no key: the status row with Stop or Start service, and
/// Files with the path of <c>config.toml</c>.
/// </summary>
internal static class ServiceCards
{
    /// <summary>
    /// The status row (DESIGN.md section 4, Service), following every poll.
    /// </summary>
    /// <param name="service">Start and Stop service, and the latest poll.</param>
    /// <param name="context">The session, which knows the stop record, and the dialogs.</param>
    /// <param name="port">The port this panel dials.</param>
    /// <param name="openLogs">Opens the log folder.</param>
    public static UIElement Status(
        ServiceControl service,
        PageContext context,
        Func<uint> port,
        Action openLogs)
    {
        var text = SettingsUi.Text(string.Empty, "BodyTextBlockStyle");
        text.VerticalAlignment = VerticalAlignment.Center;
        var action = new Button();
        var logs = SettingsUi.Button("Open logs", openLogs);
        var buttons = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 8,
            Children = { action, logs },
        };
        var grid = new Grid { ColumnSpacing = 12 };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = SettingsUi.Star });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        Grid.SetColumn(buttons, 1);
        grid.Children.Add(text);
        grid.Children.Add(buttons);
        AutomationProperties.SetLiveSetting(text, AutomationLiveSetting.Polite);

        var row = new ServiceRow(string.Empty, ServiceButton.Starting, false);
        action.Click += (_, _) =>
        {
            switch (row.Button)
            {
                case ServiceButton.Stop:
                    _ = StopAsync(service, context);
                    break;
                case ServiceButton.Start:
                    service.Start();
                    break;
                case ServiceButton.Starting:
                    break;
            }
        };

        void Update()
        {
            var listening = context.Session.ListeningPort ?? port();
            row = ServiceRow.From(
                service.Status,
                context.Session.Status,
                service.Version,
                listening);
            SettingsUi.SetLive(text, row.Text);
            action.Content = row.ButtonText;
            action.IsEnabled = row.Button != ServiceButton.Starting;
            action.Style = row.Button == ServiceButton.Start
                ? SettingsUi.Style("AccentButtonStyle")
                : null;
            logs.Visibility = SettingsUi.Shows(row.OpenLogs);
        }

        service.Changed += Update;
        context.Session.PropertyChanged += (_, _) => Update();
        Update();
        return Group("Service", grid);
    }

    /// <summary>
    /// Files: the config file's path with Open file and Show in File Explorer.
    /// </summary>
    public static UIElement Files(DataDirectory data)
    {
        var path = data.ConfigFile;
        var label = SettingsUi.Text("Config file", "BodyTextBlockStyle");
        var location = SettingsUi.Text(path, "SettingsHelpTextStyle");
        location.IsTextSelectionEnabled = true;
        var buttons = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 8,
            Margin = new Thickness(0, 8, 0, 0),
            Children =
            {
                SettingsUi.Button("Open file", () => OpenFile(path)),
                SettingsUi.Button("Show in File Explorer", () => ShowInFolder(path)),
            },
        };
        var panel = new StackPanel { Children = { label, location, buttons } };
        return Group("Files", panel);
    }

    /// <summary>
    /// Opens <c>config.toml</c> in the program Windows has for it, or Notepad without one.
    /// </summary>
    public static void OpenFile(string path)
    {
        if (!File.Exists(path))
        {
            AppLog.Write($"settings: {path} does not exist; showing its folder");
            ShowInFolder(path);
            return;
        }

        if (!Start(new ProcessStartInfo(path) { UseShellExecute = true }))
        {
            _ = Start(new ProcessStartInfo("notepad.exe") { ArgumentList = { path } });
        }
    }

    private static StackPanel Group(string heading, UIElement content) =>
        new() { Children = { SettingsUi.Heading(heading), SettingsUi.Card([content]) } };

    private static void ShowInFolder(string path)
    {
        var start = File.Exists(path)
            ? new ProcessStartInfo("explorer.exe") { ArgumentList = { $"/select,{path}" } }
            : new ProcessStartInfo("explorer.exe")
            {
                ArgumentList = { Path.GetDirectoryName(path) ?? path },
            };
        _ = Start(start);
    }

    private static bool Start(ProcessStartInfo start)
    {
        try
        {
            using var process = Process.Start(start);
            return true;
        }
        catch (Exception error) when (error is Win32Exception or InvalidOperationException)
        {
            AppLog.Write($"settings: starting {start.FileName} failed: {error.Message}");
            return false;
        }
    }

    private static async Task StopAsync(ServiceControl service, PageContext context)
    {
        if (service.StopConfirmation is { } body)
        {
            var confirmation = new Confirmation(ServiceMenu.ConfirmTitle, body, "Stop service");
            if (!await context.Confirm(confirmation).ConfigureAwait(true))
            {
                return;
            }
        }

        await service.StopAsync().ConfigureAwait(true);
    }
}
