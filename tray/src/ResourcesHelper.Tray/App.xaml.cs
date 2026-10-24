using System.Diagnostics.CodeAnalysis;

using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.Windows.AppLifecycle;

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
    private DispatcherQueue? _dispatcher;
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
        Toasts.Unregister();
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

        Toasts.Register(_dispatcher, OnToastAction);

        _client = DaemonClient.ForInstall(_data);
        _poller = new DaemonPoller(_client, new ServiceLauncher(_data), TimeProvider.System);

        _flyout = new FlyoutWindow();
        _flyout.Diagnostic += message => AppLog.Write($"flyout: {message}");
        _flyout.OpenChanged += open => _poller.SetFlyoutOpen(open);
        var shell = new FlyoutShell(_flyout, _model, OpenLogs);
        _flyout.SetContent(shell, shell);
        _flyout.Prewarm();

        _tray = new TrayController(
            _poller, _data.LogDirectory, ToggleFlyout, OpenFlyout, Quit);
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

    private void Apply(PollResult result)
    {
        var state = result.State ?? _poller?.LastKnown;
        var snapshot = FlyoutSnapshot.From(result.Status, state);
        _model.Apply(snapshot);
        _tray?.Update(result.Status, result.State, TrayStatus.From(snapshot));
    }

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
