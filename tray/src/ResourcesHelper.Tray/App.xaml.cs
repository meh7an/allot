using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.Windows.AppLifecycle;

using ResourcesHelper.Tray.Flyout;
using ResourcesHelper.Tray.Spike;

namespace ResourcesHelper.Tray;

public sealed partial class App : Application
{
    private DispatcherQueue? _dispatcher;
    private FlyoutWindow? _flyout;

    public App()
    {
        InitializeComponent();

        // A tray app has no window most of the time; it ends only on an explicit quit.
        DispatcherShutdownMode = DispatcherShutdownMode.OnExplicitShutdown;
    }

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        _dispatcher = DispatcherQueue.GetForCurrentThread();
        AppInstance.GetCurrent().Activated += OnRedirectedActivation;
        UnhandledException += (_, e) => SpikeLog.Write($"unhandled: {e.Exception}");
        SpikeLog.Write("launched");
    }

    // A second launch means the user wants to see the flyout; it is also the toggle. The event
    // arrives on a background thread.
    private void OnRedirectedActivation(object? sender, AppActivationArguments args)
    {
        SpikeLog.Write($"redirected activation: {args.Kind}");
        _ = _dispatcher?.TryEnqueue(ToggleFlyout);
    }

    private void ToggleFlyout()
    {
        try
        {
            if (_flyout is null)
            {
                _flyout = new FlyoutWindow();
                _flyout.SetContent(
                    new SpikeFlyoutContent(_flyout), SpikeFlyoutContent.SpikeHeight);
            }

            _flyout.Toggle();
        }
        catch (Exception e)
        {
            SpikeLog.Write($"toggle failed: {e}");
            throw;
        }
    }
}
