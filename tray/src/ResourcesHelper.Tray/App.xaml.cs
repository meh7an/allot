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
    private readonly JobStops _stops = new(TimeProvider.System);
    private DispatcherQueue? _dispatcher;
    private DispatcherQueueTimer? _ticker;
    private DispatcherQueueTimer? _stopTimer;

    [SuppressMessage(
        "Performance",
        "CA1859",
        Justification = "Debug builds put the scenarios' fake daemon here.")]
    private IJobStopper? _stopper;
    private ServiceStatus _status = ServiceStatus.Starting;
    private UiState? _state;
#if DEBUG
    // A scenario instance runs beside the real one, which owns the toast registration.
    private readonly bool _toasts = DebugScenario.Name is null;
#else
    private readonly bool _toasts = true;
#endif
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

        if (_toasts)
        {
            Toasts.Register(_dispatcher, OnToastAction);
        }

        _client = DaemonClient.ForInstall(_data);
        _stopper = _client;
        _poller = new DaemonPoller(_client, new ServiceLauncher(_data), TimeProvider.System);

        _flyout = new FlyoutWindow();
        _flyout.Diagnostic += message => AppLog.Write($"flyout: {message}");
        _flyout.OpenChanged += open => _poller.SetFlyoutOpen(open);
        _flyout.OpenChanged += Tick;

        // Kill? goes back to Kill when focus leaves it, and so when the flyout does.
        _flyout.Activated += (_, args) =>
        {
            if (args.WindowActivationState == WindowActivationState.Deactivated
                && _stops.CancelConfirms())
            {
                _ = Show();
            }
        };
        var shell = new FlyoutShell(_flyout, _model, OpenLogs);
        shell.KillPressed += OnKillPressed;
        shell.RemovePressed += OnRemovePressed;
        shell.ConfirmCancelled += lease =>
        {
            if (_stops.Cancel(lease))
            {
                _ = Show();
            }
        };
        _flyout.SetContent(shell, shell);
        _flyout.Prewarm();

        // Times tick every second while the flyout is open, between polls too.
        _ticker = _dispatcher.CreateTimer();
        _ticker.Interval = TimeSpan.FromSeconds(1);
        _ticker.Tick += (_, _) => Show();

        // Countdowns and holds change cards on their own schedule, open or not.
        _stopTimer = _dispatcher.CreateTimer();
        _stopTimer.IsRepeating = false;
        _stopTimer.Tick += (_, _) => Show();

        _tray = new TrayController(
            _poller, _data.LogDirectory, ToggleFlyout, OpenFlyout, Quit);
#if DEBUG
        if (DebugScenario.Name is { } scenario)
        {
            _stopper = new DebugScenario.Stopper(() => _scenario, _clock, EndScenarioJob);
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
        if (live is null)
        {
            _ = _stops.CancelConfirms();
        }
        else
        {
            _stops.Observe(live);
        }

        var snapshot = Show();
        _tray?.Update(status, live, TrayStatus.From(snapshot));
    }

    private FlyoutSnapshot Show()
    {
        _ = _stops.Expire();
        var running = _status == ServiceStatus.Running && _state is not null;
        var state = running ? _stops.Merge(_state!) : _state;
        var stops = _stops.Views;
#if DEBUG
        if (_scenarioStops.Count > 0)
        {
            stops = _scenarioStops;
        }
#endif
        var snapshot = FlyoutSnapshot.From(_status, state, stops, _clock.NowMs);
        _model.Apply(snapshot);
        ScheduleStops();
        return snapshot;
    }

    private void ScheduleStops()
    {
        if (_stopTimer is null)
        {
            return;
        }

        _stopTimer.Stop();
        if (_stops.NextChange is { } due)
        {
            // A few milliseconds late, so the second or hold has really passed.
            _stopTimer.Interval = due + TimeSpan.FromMilliseconds(15);
            _stopTimer.Start();
        }
    }

    private void OnKillPressed(long lease)
    {
#if DEBUG
        ClearScenarioStops();
#endif
        if (_stops.PressKill(lease) == StopPress.Send)
        {
            _ = StopAsync(lease, StopKind.Kill);
        }

        _ = Show();
    }

    private void OnRemovePressed(long lease)
    {
#if DEBUG
        ClearScenarioStops();
#endif
        if (_stops.PressRemove(lease))
        {
            _ = StopAsync(lease, StopKind.Remove);
        }

        _ = Show();
    }

    // Runs on the UI thread: the continuation comes back to the dispatcher.
    private async Task StopAsync(long lease, StopKind kind)
    {
        if (_stopper is null)
        {
            return;
        }

        StopResult result;
        try
        {
            result = kind == StopKind.Kill
                ? await _stopper.KillAsync(lease, _stopping.Token).ConfigureAwait(true)
                : await _stopper.RemoveAsync(lease, _stopping.Token).ConfigureAwait(true);
        }
        catch (OperationCanceledException)
        {
            // Quit.
            return;
        }

        var verb = kind == StopKind.Kill ? "kill" : "remove";
        AppLog.Write(result.Ended
            ? $"{verb} of lease {lease}: ended{(result.Text.Length > 0 ? $", {result.Text}" : "")}"
            : $"{verb} of lease {lease} refused: {result.Text}");
        _stops.Finished(lease, result);
        _ = Show();
        _poller?.PollNow();
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
            if (_stops.CancelConfirms())
            {
                _ = Show();
            }
        }
    }

#if DEBUG
    private string _scenario = "";
    private IReadOnlyDictionary<long, StopView> _scenarioStops = new Dictionary<long, StopView>();

    private void ShowScenario(string name)
    {
        AppLog.Write($"showing scenario {name}");
        _scenario = name;
        _scenarioStops = Core.Samples.Handoff.Stops(name);
        var (status, state) = Core.Samples.Handoff.Scenario(name);
        Apply(status, status == ServiceStatus.Running ? state : null, state);
    }

    // A press in a scenario that starts in a Kill or Remove state hands the cards to the live
    // state machine.
    private void ClearScenarioStops() =>
        _scenarioStops = new Dictionary<long, StopView>();

    // The fake daemon ended a job: the scenario's state stops listing it, as rhd's would.
    private void EndScenarioJob(long lease)
    {
        if (_state is not { } state)
        {
            return;
        }

        var shown = state with
        {
            Active = state.Active.Where(job => job.Lease.Id != lease).ToList(),
            Queue = state.Queue.Where(job => job.Lease.Id != lease).ToList(),
        };
        Apply(_status, shown, shown);
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
