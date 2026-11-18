using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

using Windows.Graphics;

namespace SettingsSpike;

/// <summary>
/// The stand-in flyout; see MainWindow.xaml.
/// </summary>
public sealed partial class MainWindow : Window
{
    private const int LogLines = 14;

    private readonly SettingsHost _host;
    private readonly Queue<string> _lines = new();
    private readonly DispatcherQueue _dispatcher;

    internal MainWindow(SettingsHost host)
    {
        _host = host;
        InitializeComponent();
        _dispatcher = DispatcherQueue;

        AppWindow.IsShownInSwitchers = false;
        AppWindow.SetIcon(Path.Combine(AppContext.BaseDirectory, "Assets", "AppIcon.ico"));
        var hwnd = Microsoft.UI.Win32Interop.GetWindowFromWindowId(AppWindow.Id);
        var scale = NativeMethods.GetDpiForWindow(hwnd) / 96.0;
        AppWindow.Resize(new SizeInt32((int)(460 * scale), (int)(600 * scale)));

        LogPath.Text = $"Log: {SpikeLog.File}";
        SpikeLog.Written += OnLogWritten;
        Closed += (_, _) => SpikeLog.Written -= OnLogWritten;
    }

    public void ShowLauncher()
    {
        AppWindow.Show();
        Activate();
        _ = NativeMethods.SetForegroundWindow(Microsoft.UI.Win32Interop.GetWindowFromWindowId(AppWindow.Id));
    }

    private void OnLogWritten(string line) => _dispatcher.TryEnqueue(() =>
    {
        _lines.Enqueue(line);
        while (_lines.Count > LogLines)
        {
            _ = _lines.Dequeue();
        }

        LogText.Text = string.Join("\n", _lines);
    });

    private void OnSettingsClick(object sender, RoutedEventArgs e)
    {
        _ = _host.Open("flyout menu");
        if (HideLauncherSwitch.IsOn)
        {
            AppWindow.Hide();
        }
    }

    private void OnQuitClick(object sender, RoutedEventArgs e) => _host.Quit?.Invoke();

    private void OnCloseModelChanged(object sender, SelectionChangedEventArgs e)
    {
        _host.CloseModel = CloseModelChoice.SelectedIndex == 1 ? CloseModel.Hide : CloseModel.Destroy;
        SpikeLog.Write($"close model: {_host.CloseModel}");
        // A window hidden under the old model would otherwise linger.
        if (_host.CloseModel == CloseModel.Destroy && _host.Current is { AppWindow.IsVisible: false } hidden)
        {
            hidden.Close();
        }
    }

    private void OnForceForegroundToggled(object sender, RoutedEventArgs e)
    {
        _host.ForceForeground = ForceForegroundSwitch.IsOn;
        SpikeLog.Write($"force foreground: {_host.ForceForeground}");
    }

    private async void OnDelayedOpenClick(object sender, RoutedEventArgs e)
    {
        SpikeLog.Write("opening Settings in 5 s; click into another app now");
        await Task.Delay(5000).ConfigureAwait(true);
        _ = _host.Open("timer, 5 s later");
    }

    private async void OnCycleClick(object sender, RoutedEventArgs e) =>
        _ = await _host.CycleAsync(20).ConfigureAwait(true);

    private async void OnCountClick(object sender, RoutedEventArgs e) =>
        _ = await _host.CountAliveAsync().ConfigureAwait(true);

    private void OnForgetClick(object sender, RoutedEventArgs e)
    {
        PlacementStore.Forget();
        SpikeLog.Write("placement forgotten");
    }
}
