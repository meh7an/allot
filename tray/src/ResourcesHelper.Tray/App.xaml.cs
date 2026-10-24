using System.Diagnostics.CodeAnalysis;

using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.Windows.AppLifecycle;

using ResourcesHelper.Tray.Core.Contract;
using ResourcesHelper.Tray.Core.Daemon;
using ResourcesHelper.Tray.Core.Logging;
using ResourcesHelper.Tray.Core.TrayIcon;
using ResourcesHelper.Tray.Core.ViewModels;
using ResourcesHelper.Tray.Flyout;
using ResourcesHelper.Tray.Notifications;
using ResourcesHelper.Tray.TrayIcon;

namespace ResourcesHelper.Tray;

[SuppressMessage(
    "Design",
    "CA1001",
    Justification = "The app lives as long as the process; Quit disposes what it owns.")]
public sealed partial class App : Application
{
    private readonly CancellationTokenSource _stopping = new();
    private readonly FlyoutModel _model = new();
    private readonly StateClock _clock = new(TimeProvider.System);
    private DispatcherQueue? _dispatcher;
    private DispatcherQueueTimer? _ticker;
    private ServiceStatus _status = ServiceStatus.Starting;
    private UiState? _state;
    private bool _toasts = true;
    private DataDirectory? _data;
    private DaemonClient? _client;
    private DaemonPoller? _poller;
    private FlyoutWindow? _flyout;
    private TrayController? _tray;

    public App()
    {
        InitializeComponent();

        // A tray app has no window most of the time; it ends only on an explicit quit.
        DispatcherShutdownMode = DispatcherShutdownMode.OnExplicitShutdown;
    }

    /// <summary>
    /// Removes the tray icon and ends the app; the service keeps running.
    /// </summary>
    public void Quit()
    {
        AppLog.Write("quit");
        _stopping.Cancel();
        _tray?.Dispose();
        _client?.Dispose();
        if (_toasts)
        {
            Toasts.Unregister();
        }

        Exit();
    }

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        _dispatcher = DispatcherQueue.GetForCurrentThread();
        _data = DataDirectory.FromEnvironment();
        AppLog.Initialize(TrayLog.In(_data.LogDirectory));
        UnhandledException += (_, e) => AppLog.Write($"unhandled: {e.Exception}");
        AppLog.Write($"started, data in {_data.Root}");
        AppInstance.GetCurrent().Activated += OnRedirectedActivation;

#if DEBUG
        // A scenario instance runs beside the real one, which owns the toast registration.
        _toasts = DebugScenario.Name is null;
#endif
        if (_toasts)
        {
            Toasts.Register(_dispatcher, OnToastAction);
        }

        _client = DaemonClient.ForInstall(_data);
        _poller = new DaemonPoller(_client, new ServiceLauncher(_data), TimeProvider.System);

        _flyout = new FlyoutWindow();
        _flyout.Diagnostic += message => AppLog.Write($"flyout: {message}");
        _flyout.OpenChanged += open => _poller.SetFlyoutOpen(open);
        _flyout.OpenChanged += Tick;
        var shell = new FlyoutShell(_flyout, _model, OpenLogs);
        _flyout.SetContent(shell, shell);
        _flyout.Prewarm();

        // Times tick every second while the flyout is open, between polls too.
        _ticker = _dispatcher.CreateTimer();
        _ticker.Interval = TimeSpan.FromSeconds(1);
        _ticker.Tick += (_, _) => Show();

        _tray = new TrayController(
            _poller, _data.LogDirectory, ToggleFlyout, OpenFlyout, Quit);
#if DEBUG
        if (DebugScenario.Name is { } scenario)
        {
            shell.AddScenarios(Core.Samples.Handoff.ScenarioNames, ShowScenario);
            ShowScenario(scenario);
            return;
        }
#endif
        _poller.Updated += (_, result) => _dispatcher.TryEnqueue(() => Apply(result));
        _ = PollAsync(_poller);

        // A toast clicked while the app was not running starts it with that activation.
        if (Toasts.ActionOf(AppInstance.GetCurrent().GetActivatedEventArgs()) is { } action)
        {
            OnToastAction(action);
        }
    }

    private async Task PollAsync(DaemonPoller poller)
    {
        try
        {
            await poller.RunAsync(_stopping.Token).ConfigureAwait(false);
        }
        catch (OperationCanceledException)
        {
            // Quit.
        }
    }

    private void Apply(PollResult result) =>
        Apply(result.Status, result.State, result.State ?? _poller?.LastKnown);

    /// <param name="status">The service status of the poll.</param>
    /// <param name="live">The state rhd served, or <c>null</c> while it is not running.</param>
    /// <param name="shown">The state to show: the live one, or the last known one.</param>
    private void Apply(ServiceStatus status, UiState? live, UiState? shown)
    {
        _status = status;
        _state = shown;
        _clock.Received(live);
        var snapshot = Show();
        _tray?.Update(status, live, TrayStatus.From(snapshot));
    }

    private FlyoutSnapshot Show()
    {
        var snapshot = FlyoutSnapshot.From(_status, _state, nowMs: _clock.NowMs);
        _model.Apply(snapshot);
        return snapshot;
    }

    private void Tick(bool open)
    {
        if (open)
        {
            _ = Show();
            _ticker?.Start();
        }
        else
        {
            _ticker?.Stop();
        }
    }

#if DEBUG
    private void ShowScenario(string name)
    {
        AppLog.Write($"showing scenario {name}");
        var (status, state) = Core.Samples.Handoff.Scenario(name);
        Apply(status, status == ServiceStatus.Running ? state : null, state);
    }
#endif

    // A second launch means the user wants to see the flyout. The event arrives on a background
    // thread.
    private void OnRedirectedActivation(object? sender, AppActivationArguments args)
    {
        var action = Toasts.ActionOf(args);
        _ = _dispatcher?.TryEnqueue(() =>
        {
            if (action is null)
            {
                OpenFlyout();
            }
            else
            {
                OnToastAction(action);
            }
        });
    }

    private void OnToastAction(string action)
    {
        switch (action)
        {
            case Toasts.OpenLogs:
                OpenLogs();
                break;
            case Toasts.StartService:
                if (_poller?.StartService() is { } problem)
                {
                    AppLog.Write($"start service failed: {problem}");
                }

                break;
            default:
                OpenFlyout();
                break;
        }
    }

    private void OpenLogs()
    {
        if (_data is not null)
        {
            LogFolder.Open(_data.LogDirectory);
        }
    }

    private void ToggleFlyout() => _flyout?.Toggle();

    private void OpenFlyout() => _flyout?.Open();
}
