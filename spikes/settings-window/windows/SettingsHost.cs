using System.Diagnostics;

using Microsoft.UI;
using Microsoft.UI.Windowing;

using Windows.Graphics;

namespace SettingsSpike;

/// <summary>
/// What closing the Settings window does: destroy it (the next open builds a new one), or
/// cancel the close and hide it (the next open shows the same one).
/// </summary>
internal enum CloseModel
{
    Destroy,
    Hide,
}

/// <summary>
/// Owns at most one Settings window: opens it, brings an open one to the front instead of making
/// a second, saves and restores its placement, and counts the windows still alive after closing.
/// </summary>
internal sealed class SettingsHost
{
    public static readonly SizeDip DefaultSize = new(900, 640);

    public static readonly SizeDip MinimumSize = new(640, 480);

    private readonly List<WeakReference<SettingsWindow>> _created = [];
    private SettingsWindow? _window;
    private SavedPlacement? _lastNormal;
    private Stopwatch? _opening;
    private int _count;

    public CloseModel CloseModel { get; set; } = CloseModel.Destroy;

    /// <summary>
    /// Calls SetForegroundWindow after Activate; off tests Activate alone.
    /// </summary>
    public bool ForceForeground { get; set; } = true;

    public SettingsWindow? Current => _window;

    public Action? ShowLauncher { get; set; }

    public Action? Quit { get; set; }

    public SettingsWindow Open(string source)
    {
        var state = _window is null ? "new window" : _window.AppWindow.IsVisible ? "already open" : "hidden, reusing";
        SpikeLog.Write($"open from {source}: {state}");
        _opening = Stopwatch.StartNew();
        if (_window is null)
        {
            _window = Create();
            Restore(_window);
        }

        BringToFront(_window);
        return _window;
    }

    /// <summary>
    /// Closes the window for good, whatever the close model.
    /// </summary>
    public void Shutdown()
    {
        CloseModel = CloseModel.Destroy;
        _window?.Close();
    }

    /// <summary>
    /// Opens and closes the window <paramref name="times"/> times the way a user would (WM_CLOSE,
    /// as the title bar's X sends), then counts the windows still alive.
    /// </summary>
    public async Task<AliveCount> CycleAsync(int times)
    {
        for (var i = 0; i < times; i++)
        {
            await CycleOnceAsync($"cycle {i + 1}/{times}").ConfigureAwait(true);
        }

        return await CountAliveAsync().ConfigureAwait(true);
    }

    /// <summary>
    /// One open and close in its own method, so no state machine field keeps the last window.
    /// </summary>
    private async Task CycleOnceAsync(string source)
    {
        var window = Open(source);
        await window.ContentLoaded.ConfigureAwait(true);
        await Task.Delay(60).ConfigureAwait(true);
        _ = NativeMethods.PostMessage(window.Hwnd, NativeMethods.WmClose, 0, 0);
        await Task.Delay(120).ConfigureAwait(true);
    }

    /// <summary>
    /// WinUI releases a closed window's native side on later dispatcher ticks, so this collects a
    /// few times with the queue running in between.
    /// </summary>
    public static async Task CollectAsync()
    {
        for (var i = 0; i < 4; i++)
        {
            GC.Collect();
            GC.WaitForPendingFinalizers();
            GC.Collect();
            await Task.Delay(200).ConfigureAwait(true);
        }
    }

    public static long PrivateMb()
    {
        using var process = Process.GetCurrentProcess();
        return process.PrivateMemorySize64 / (1024 * 1024);
    }

    public async Task<AliveCount> CountAliveAsync()
    {
        await CollectAsync().ConfigureAwait(true);
        var alive = _created.Count(reference => reference.TryGetTarget(out _));
        using var process = Process.GetCurrentProcess();
        var count = new AliveCount(_created.Count, alive, _window is not null,
            process.PrivateMemorySize64 / (1024 * 1024), process.WorkingSet64 / (1024 * 1024));
        SpikeLog.Write($"alive: {count}");
        return count;
    }

    /// <summary>
    /// Called by the window when its content first loads; logs how long the open took.
    /// </summary>
    public void OnContentLoaded(SettingsWindow window)
    {
        if (_opening is { } opening)
        {
            SpikeLog.Write(
                $"#{window.Number} loaded {opening.ElapsedMilliseconds} ms after open, private {PrivateMb()} MB");
            _opening = null;
        }
    }

    public static double MonitorScale(DisplayArea display)
    {
        var monitor = Win32Interop.GetMonitorFromDisplayId(display.DisplayId);
        return NativeMethods.GetDpiForMonitor(monitor, NativeMethods.MdtEffectiveDpi, out var dpi, out _) == 0
            ? dpi / 96.0
            : 1.0;
    }

    private SettingsWindow Create()
    {
        var window = new SettingsWindow(++_count, this);
        _created.Add(new WeakReference<SettingsWindow>(window));
        window.AppWindow.Closing += OnClosing;
        window.AppWindow.Changed += OnChanged;
        window.Closed += OnClosed;
        return window;
    }

    /// <summary>
    /// Moves the hidden window to its saved place, or centres it on the monitor under the cursor
    /// (where the tray icon was clicked), then sizes it for that monitor's scale.
    /// </summary>
    private void Restore(SettingsWindow window)
    {
        var saved = PlacementStore.Load();
        DisplayArea display;
        PointInt32 origin;
        var size = DefaultSize;
        if (saved is null)
        {
            _ = NativeMethods.GetCursorPos(out var cursor);
            display = DisplayArea.GetFromPoint(new PointInt32(cursor.X, cursor.Y), DisplayAreaFallback.Nearest);
            var scale = MonitorScale(display);
            var work = display.WorkArea;
            origin = new PointInt32(
                work.X + (work.Width - (int)(size.Width * scale)) / 2,
                work.Y + (work.Height - (int)(size.Height * scale)) / 2);
        }
        else
        {
            size = new SizeDip(saved.WidthDip, saved.HeightDip);
            origin = new PointInt32(saved.X, saved.Y);
            display = DisplayArea.GetFromRect(
                new RectInt32(saved.X, saved.Y, (int)saved.WidthDip, (int)saved.HeightDip),
                DisplayAreaFallback.Nearest);
        }

        var monitorScale = MonitorScale(display);
        var target = Clamp(origin, size, monitorScale, display.WorkArea);
        window.AppWindow.MoveAndResize(target);

        // The window was created on whichever monitor Windows chose; check that the move did not
        // let a DPI change rescale the size a second time.
        var windowScale = NativeMethods.GetDpiForWindow(window.Hwnd) / 96.0;
        var actual = window.AppWindow.Size;
        SpikeLog.Write(
            $"restore {(saved is null ? "default" : "saved")}: display {display.DisplayId.Value} "
            + $"monitor scale {monitorScale:0.##}, window scale after move {windowScale:0.##}, "
            + $"asked {target.Width}x{target.Height} at {target.X},{target.Y}, got {actual.Width}x{actual.Height}");
        if (actual.Width != target.Width || actual.Height != target.Height)
        {
            SpikeLog.Write("restore: size changed by the move, resizing again");
            window.AppWindow.Resize(new SizeInt32(target.Width, target.Height));
        }

        window.ApplyMinimumSize();
        if (saved?.Maximized == true)
        {
            window.Presenter.Maximize();
        }
    }

    private static RectInt32 Clamp(PointInt32 origin, SizeDip size, double scale, RectInt32 work)
    {
        var width = Math.Min((int)Math.Round(size.Width * scale), work.Width);
        var height = Math.Min((int)Math.Round(size.Height * scale), work.Height);
        var x = Math.Clamp(origin.X, work.X, work.X + work.Width - width);
        var y = Math.Clamp(origin.Y, work.Y, work.Y + work.Height - height);
        return new RectInt32(x, y, width, height);
    }

    private void BringToFront(SettingsWindow window)
    {
        var app = window.AppWindow;
        if (window.Presenter.State == OverlappedPresenterState.Minimized)
        {
            window.Presenter.Restore();
        }

        if (!app.IsVisible)
        {
            app.Show();
        }

        window.Activate();
        var forced = ForceForeground ? NativeMethods.SetForegroundWindow(window.Hwnd).ToString() : "skipped";
        var now = NativeMethods.GetForegroundWindow() == window.Hwnd;
        SpikeLog.Write($"#{window.Number} to front: SetForegroundWindow {forced}, foreground now {now}");
        _ = window.DispatcherQueue.TryEnqueue(async () =>
        {
            await Task.Delay(250).ConfigureAwait(true);
            SpikeLog.Write($"#{window.Number} foreground 250 ms later: {NativeMethods.GetForegroundWindow() == window.Hwnd}");
        });
    }

    private void OnChanged(AppWindow sender, AppWindowChangedEventArgs args)
    {
        if (_window is null || sender.Id != _window.AppWindow.Id)
        {
            return;
        }

        if ((args.DidPositionChange || args.DidSizeChange)
            && _window.Presenter.State == OverlappedPresenterState.Restored)
        {
            var scale = NativeMethods.GetDpiForWindow(_window.Hwnd) / 96.0;
            _lastNormal = new SavedPlacement
            {
                X = sender.Position.X,
                Y = sender.Position.Y,
                WidthDip = sender.Size.Width / scale,
                HeightDip = sender.Size.Height / scale,
            };
        }

        _window.Refresh();
    }

    private void OnClosing(AppWindow sender, AppWindowClosingEventArgs args)
    {
        if (_lastNormal is { } normal)
        {
            normal.Maximized = _window?.Presenter.State == OverlappedPresenterState.Maximized;
            PlacementStore.Save(normal);
            SpikeLog.Write($"saved placement {normal}");
        }

        if (CloseModel == CloseModel.Hide)
        {
            args.Cancel = true;
            sender.Hide();
            SpikeLog.Write($"#{_window?.Number} closing: cancelled and hidden");
        }
        else
        {
            SpikeLog.Write($"#{_window?.Number} closing: destroying");
        }
    }

    private void OnClosed(object sender, Microsoft.UI.Xaml.WindowEventArgs args)
    {
        if (sender is not SettingsWindow window)
        {
            return;
        }

        window.AppWindow.Closing -= OnClosing;
        window.AppWindow.Changed -= OnChanged;
        window.Closed -= OnClosed;
        window.Detach();
        if (ReferenceEquals(window, _window))
        {
            _window = null;
            _lastNormal = null;
        }

        SpikeLog.Write($"#{window.Number} closed");
    }
}

internal readonly record struct SizeDip(double Width, double Height);

internal readonly record struct AliveCount(int Created, int Alive, bool OneOpen, long PrivateMb, long WorkingSetMb);
