using Allot.Tray.Core.TrayIcon;
using Allot.Tray.Diagnostics;
using Allot.Tray.Interop;

using Microsoft.UI.Dispatching;

namespace Allot.Tray.TrayIcon;

/// <summary>
/// The tray icon driven by live data: its state and tooltip follow each poll, a left click
/// toggles the flyout, and the right-click menu is handoff section 8.
/// </summary>
internal sealed class TrayController : IDisposable
{
    private const uint OpenId = 1;
    private const uint ServiceId = 2;
    private const uint LaunchAtLogonId = 3;
    private const uint OpenLogsId = 4;
    private const uint QuitId = 5;
    private const uint SettingsId = 6;

    private readonly NotifyIcon _icon;
    private readonly DispatcherQueue _dispatcher = DispatcherQueue.GetForCurrentThread();
    private readonly ServiceControl _service;
    private readonly string _logDirectory;
    private readonly Action _toggleFlyout;
    private readonly Action _openFlyout;
    private readonly Action _quit;
    private readonly Func<bool> _launchAtLogon;
    private readonly Action<bool> _setLaunchAtLogon;
    private readonly Action _openSettings;
    private readonly Func<string?> _settingsBlocked;
    private TrayStatus _status = new(TrayIconState.Down, "Starting service…");

    public TrayController(
        ServiceControl service,
        string logDirectory,
        Action toggleFlyout,
        Action openFlyout,
        Action quit,
        Func<bool> launchAtLogon,
        Action<bool> setLaunchAtLogon,
        Action openSettings,
        Func<string?> settingsBlocked)
    {
        _service = service;
        _logDirectory = logDirectory;
        _toggleFlyout = toggleFlyout;
        _openFlyout = openFlyout;
        _quit = quit;
        _launchAtLogon = launchAtLogon;
        _setLaunchAtLogon = setLaunchAtLogon;
        _openSettings = openSettings;
        _settingsBlocked = settingsBlocked;

        _icon = new NotifyIcon(_status.Icon, _status.Tooltip);
        _icon.Diagnostic += message => AppLog.Write($"tray: {message}");
        _icon.Selected += OnSelected;
        _icon.ContextMenuRequested += OnContextMenuRequested;
        _icon.ThemeChanged += UseMenuTheme;
        UseMenuTheme(_icon.Theme);
        _icon.Show();
    }

    public void Dispose() => _icon.Dispose();

    /// <summary>
    /// The icon file the tray shows now, as a 256 px PNG for a toast logo.
    /// </summary>
    public string LogoFileName => TrayIconAsset.LogoFileName(_status.Icon, _icon.Theme);

    /// <summary>
    /// Takes the latest poll; called on the UI thread.
    /// </summary>
    /// <param name="status">What the icon shows for the latest poll.</param>
    public void Update(TrayStatus status)
    {
        if (status == _status)
        {
            return;
        }

        _status = status;
        _icon.SetState(status.Icon);
        _icon.SetTooltip(status.Tooltip);
    }

    private static void UseMenuTheme(TaskbarTheme theme)
    {
        if (!NativeMenu.UseTheme(theme))
        {
            AppLog.Write("tray: no dark menu support; menus stay light");
        }
    }

    // The click arrives inside the icon's window procedure; the flyout work runs after it.
    private void OnSelected(TrayCallback callback)
    {
        Measure.Clicked();
        _ = _dispatcher.TryEnqueue(() => _toggleFlyout());
    }

    private void OnContextMenuRequested(TrayCallback callback)
    {
        var id = NativeMenu.Show(_icon.WindowHandle, BuildMenu(), callback.X, callback.Y);
        if (id != 0)
        {
            _ = _dispatcher.TryEnqueue(() => Run(id));
        }
    }

    // Built at every open, so the check follows a change made in Task Manager.
    private List<NativeMenuItem> BuildMenu()
    {
        var service = ServiceMenu.ItemFor(_service.Status);
        var blocked = _settingsBlocked();
        return
        [
            new(_status.Summary) { Enabled = false },
            NativeMenuItem.Separator,
            new(blocked ?? "Settings", SettingsId) { Enabled = blocked is null },
            new("Open Allot", OpenId) { IsDefault = true },
            new(service.Text, ServiceId) { Enabled = service.Enabled },
            new("Launch at logon", LaunchAtLogonId) { Checked = _launchAtLogon() },
            new("Open logs", OpenLogsId),
            NativeMenuItem.Separator,
            new("Quit", QuitId),
        ];
    }

    private void Run(uint id)
    {
        switch (id)
        {
            case OpenId:
                _openFlyout();
                break;
            case SettingsId:
                _openSettings();
                break;
            case ServiceId:
                RunServiceItem();
                break;
            case LaunchAtLogonId:
                _setLaunchAtLogon(!_launchAtLogon());
                break;
            case OpenLogsId:
                LogFolder.Open(_logDirectory);
                break;
            case QuitId:
                _quit();
                break;
        }
    }

    /// <summary>
    /// Stop service (asking first while jobs run or wait) or Start service, as the poll allows;
    /// also the flyout's ⋯ › Stop service or Start service.
    /// </summary>
    public void RunServiceItem()
    {
        switch (ServiceMenu.ItemFor(_service.Status).Action)
        {
            case ServiceAction.Start:
                _service.Start();
                break;
            case ServiceAction.Stop:
                if (ConfirmStop(_icon.WindowHandle, _service))
                {
                    _ = _service.StopAsync();
                }

                break;
            case ServiceAction.None:
                break;
        }
    }

    /// <summary>
    /// Asks before a stop while jobs run or wait, in a message box over <paramref name="owner"/>.
    /// </summary>
    /// <returns>Whether to stop.</returns>
    public static bool ConfirmStop(nint owner, ServiceControl service)
    {
        if (service.StopConfirmation is not { } text)
        {
            return true;
        }

        var answer = NativeMethods.MessageBox(
            owner,
            text,
            ServiceMenu.ConfirmTitle,
            NativeMethods.MbYesNo
                | NativeMethods.MbIconWarning
                | NativeMethods.MbDefButton2
                | NativeMethods.MbSetForeground);
        return answer == NativeMethods.IdYes;
    }
}
