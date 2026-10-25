using Allot.Tray.Core.Contract;
using Allot.Tray.Core.Daemon;
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

    private readonly NotifyIcon _icon;
    private readonly DispatcherQueue _dispatcher = DispatcherQueue.GetForCurrentThread();
    private readonly DaemonPoller _poller;
    private readonly string _logDirectory;
    private readonly Action _toggleFlyout;
    private readonly Action _openFlyout;
    private readonly Action _quit;
    private readonly Func<bool> _launchAtLogon;
    private readonly Action<bool> _setLaunchAtLogon;
    private readonly Action _stopping;
    private TrayStatus _status = new(TrayIconState.Down, "Starting service…");
    private ServiceStatus _service = ServiceStatus.Starting;
    private UiState? _state;

    public TrayController(
        DaemonPoller poller,
        string logDirectory,
        Action toggleFlyout,
        Action openFlyout,
        Action quit,
        Func<bool> launchAtLogon,
        Action<bool> setLaunchAtLogon,
        Action stopping)
    {
        _poller = poller;
        _logDirectory = logDirectory;
        _toggleFlyout = toggleFlyout;
        _openFlyout = openFlyout;
        _quit = quit;
        _launchAtLogon = launchAtLogon;
        _setLaunchAtLogon = setLaunchAtLogon;
        _stopping = stopping;

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
    /// <param name="service">The service status of the poll.</param>
    /// <param name="state">The served state, or <c>null</c> while allotd is not serving it.</param>
    /// <param name="status">What the icon shows for it.</param>
    public void Update(ServiceStatus service, UiState? state, TrayStatus status)
    {
        _service = service;
        _state = state;
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
        var service = ServiceMenu.ItemFor(_service);
        return
        [
            new(_status.Summary) { Enabled = false },
            NativeMenuItem.Separator,
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

    private void RunServiceItem()
    {
        switch (ServiceMenu.ItemFor(_service).Action)
        {
            case ServiceAction.Start:
                if (_poller.StartService() is { } problem)
                {
                    AppLog.Write($"start service failed: {problem}");
                }

                break;
            case ServiceAction.Stop:
                if (ConfirmStop())
                {
                    _ = StopAsync();
                }

                break;
            case ServiceAction.None:
                break;
        }
    }

    private bool ConfirmStop()
    {
        if (ServiceMenu.StopConfirmation(_state) is not { } text)
        {
            return true;
        }

        var answer = NativeMethods.MessageBox(
            _icon.WindowHandle,
            text,
            ServiceMenu.ConfirmTitle,
            NativeMethods.MbYesNo
                | NativeMethods.MbIconWarning
                | NativeMethods.MbDefButton2
                | NativeMethods.MbSetForeground);
        return answer == NativeMethods.IdYes;
    }

    private async Task StopAsync()
    {
        AppLog.Write("stop service chosen");
        _stopping();
        if (await _poller.StopServiceAsync(CancellationToken.None) is { } problem)
        {
            AppLog.Write($"stop service failed: {problem}");
        }
    }
}
