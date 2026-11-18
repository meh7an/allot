using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.Windows.AppLifecycle;

namespace SettingsSpike;

/// <summary>
/// Lives like the tray: no main window, and it ends only on an explicit quit, so closing the
/// Settings window never ends the app.
/// </summary>
public sealed partial class App : Application
{
    private readonly SettingsHost _host = new();
    private DispatcherQueue? _dispatcher;
    private MainWindow? _launcher;

    public App()
    {
        InitializeComponent();
        DispatcherShutdownMode = DispatcherShutdownMode.OnExplicitShutdown;
    }

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        _dispatcher = DispatcherQueue.GetForCurrentThread();
        UnhandledException += (_, e) => SpikeLog.Write($"unhandled: {e.Exception}");
        SpikeLog.Write($"started {Environment.ProcessPath} (AOT: {!System.Runtime.CompilerServices.RuntimeFeature.IsDynamicCodeSupported})");
        AppInstance.GetCurrent().Activated += OnRedirectedActivation;
        _host.Quit = Quit;
        _host.ShowLauncher = () => _launcher?.ShowLauncher();

        if (SelfTest.Requested)
        {
            _ = RunSelfTest();
            return;
        }

        _launcher = new MainWindow(_host);
        _launcher.Activate();
    }

    private async Task RunSelfTest()
    {
        var passed = await SelfTest.RunAsync(_host).ConfigureAwait(true);
        Environment.ExitCode = passed ? 0 : 1;
        Quit();
    }

    private void Quit()
    {
        SpikeLog.Write("quit");
        _host.Shutdown();
        _launcher?.Close();
        Exit();
    }

    /// <summary>
    /// A second launch: runs on a pool thread, so the work goes to the UI thread.
    /// </summary>
    private void OnRedirectedActivation(object? sender, AppActivationArguments args)
    {
        var text = args.Data is Windows.ApplicationModel.Activation.ILaunchActivatedEventArgs launch
            ? launch.Arguments
            : "";
        _ = _dispatcher?.TryEnqueue(() =>
        {
            if (text.Contains("--launcher", StringComparison.Ordinal))
            {
                _launcher?.ShowLauncher();
            }
            else
            {
                _ = _host.Open(text.Contains("--no-grant", StringComparison.Ordinal)
                    ? "second launch, no grant"
                    : "second launch");
            }
        });
    }
}
