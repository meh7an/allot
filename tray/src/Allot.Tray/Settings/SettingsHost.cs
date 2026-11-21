using Allot.Tray.Core.Settings;
using Allot.Tray.Interop;

using Microsoft.UI;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;

using Windows.Graphics;

namespace Allot.Tray.Settings;

/// <summary>
/// Owns the one Settings window (DESIGN.md section 10): built on the
/// first open, then hidden on close and shown again, since rebuilding it leaks native memory. It
/// owns its presenter, remembers its restored rect in <c>tray.json</c>, keeps its minimum size per
/// DPI, and opening it while open brings it to the front.
/// </summary>
/// <param name="services">What the window works with; made on the first open.</param>
/// <param name="placement">Reads the saved placement.</param>
/// <param name="savePlacement">Writes the placement when the window closes.</param>
internal sealed class SettingsHost(
    Func<SettingsServices> services,
    Func<WindowPlacement?> placement,
    Action<WindowPlacement> savePlacement)
{
    private const double DefaultWidth = 920;
    private const double DefaultHeight = 680;
    private const double MinimumWidth = 640;
    private const double MinimumHeight = 500;

    private SettingsWindow? _window;
    private WindowPlacement? _restored;
    private double _scale;

    /// <summary>
    /// Shows the window in front, on <paramref name="pageId"/> or the page shown last.
    /// </summary>
    public void Open(string? pageId = null)
    {
        if (_window is null)
        {
            _window = Create();
            Restore(_window);
        }

        _window.SelectPage(pageId);
        BringToFront(_window);
    }

    /// <summary>
    /// Sends what Settings has not saved yet, for Quit; gives up after
    /// <paramref name="timeout"/>.
    /// </summary>
    public Task SettleAsync(TimeSpan timeout) =>
        _window?.SettleAsync(timeout) ?? Task.CompletedTask;

    private static double ScaleOf(nint hwnd) => NativeMethods.GetDpiForWindow(hwnd) / 96.0;

    private static double MonitorScale(DisplayArea display)
    {
        var monitor = Win32Interop.GetMonitorFromDisplayId(display.DisplayId);
        return NativeMethods.GetDpiForMonitor(
            monitor,
            NativeMethods.MdtEffectiveDpi,
            out var dpi,
            out _) == 0
            ? dpi / 96.0
            : 1.0;
    }

    private static RectInt32 Clamp(
        PointInt32 origin,
        double width,
        double height,
        double scale,
        RectInt32 work)
    {
        var pixelsWide = Math.Min((int)Math.Round(width * scale), work.Width);
        var pixelsHigh = Math.Min((int)Math.Round(height * scale), work.Height);
        var x = Math.Clamp(origin.X, work.X, work.X + work.Width - pixelsWide);
        var y = Math.Clamp(origin.Y, work.Y, work.Y + work.Height - pixelsHigh);
        return new RectInt32(x, y, pixelsWide, pixelsHigh);
    }

    private SettingsWindow Create()
    {
        AppLog.Write("settings: building the window");
        var window = new SettingsWindow(services());
        window.AppWindow.Closing += OnClosing;
        window.AppWindow.Changed += OnChanged;
        if (window.Content is FrameworkElement root)
        {
            root.Loaded += (_, _) =>
            {
                _scale = root.XamlRoot.RasterizationScale;
                root.XamlRoot.Changed += (sender, _) =>
                {
                    if (sender.RasterizationScale != _scale)
                    {
                        _scale = sender.RasterizationScale;
                        ApplyMinimumSize(window);
                    }
                };
            };
        }

        return window;
    }

    // The saved rect on the display nearest to it, or the default size centred on the display
    // under the cursor, where the tray icon or flyout was clicked; sized for that display's scale.
    private void Restore(SettingsWindow window)
    {
        var saved = placement();
        DisplayArea display;
        PointInt32 origin;
        var (width, height) = (DefaultWidth, DefaultHeight);
        if (saved is null || saved.WidthDip <= 0 || saved.HeightDip <= 0)
        {
            _ = NativeMethods.GetCursorPos(out var cursor);
            display = DisplayArea.GetFromPoint(
                new PointInt32(cursor.X, cursor.Y),
                DisplayAreaFallback.Nearest);
            var scale = MonitorScale(display);
            var work = display.WorkArea;
            origin = new PointInt32(
                work.X + ((work.Width - (int)(width * scale)) / 2),
                work.Y + ((work.Height - (int)(height * scale)) / 2));
        }
        else
        {
            (width, height) = (saved.WidthDip, saved.HeightDip);
            origin = new PointInt32(saved.X, saved.Y);
            display = DisplayArea.GetFromRect(
                new RectInt32(saved.X, saved.Y, (int)width, (int)height),
                DisplayAreaFallback.Nearest);
        }

        var target = Clamp(origin, width, height, MonitorScale(display), display.WorkArea);
        window.AppWindow.MoveAndResize(target);

        // The window was made on whichever display Windows chose; a move to one with another
        // scale may have resized it a second time.
        var actual = window.AppWindow.Size;
        if (actual.Width != target.Width || actual.Height != target.Height)
        {
            window.AppWindow.Resize(new SizeInt32(target.Width, target.Height));
        }

        ApplyMinimumSize(window);
        if (saved?.Maximized == true)
        {
            window.Presenter.Maximize();
        }
    }

    // The presenter's minimum size is in physical pixels, so it is set again on a DPI change.
    private static void ApplyMinimumSize(SettingsWindow window)
    {
        var scale = ScaleOf(window.Hwnd);
        window.Presenter.PreferredMinimumWidth = (int)Math.Round(MinimumWidth * scale);
        window.Presenter.PreferredMinimumHeight = (int)Math.Round(MinimumHeight * scale);
    }

    private static void BringToFront(SettingsWindow window)
    {
        if (window.Presenter.State == OverlappedPresenterState.Minimized)
        {
            window.Presenter.Restore();
        }

        var shown = window.AppWindow.IsVisible;
        if (!shown)
        {
            window.AppWindow.Show();
            window.SetVisible(true);
        }

        window.Activate();
        if (!NativeMethods.SetForegroundWindow(window.Hwnd))
        {
            AppLog.Write("settings: SetForegroundWindow refused");
        }
    }

    private void OnChanged(AppWindow sender, AppWindowChangedEventArgs args)
    {
        if (_window is null
            || !(args.DidPositionChange || args.DidSizeChange)
            || _window.Presenter.State != OverlappedPresenterState.Restored)
        {
            return;
        }

        var scale = ScaleOf(_window.Hwnd);
        _restored = new WindowPlacement
        {
            X = sender.Position.X,
            Y = sender.Position.Y,
            WidthDip = sender.Size.Width / scale,
            HeightDip = sender.Size.Height / scale,
        };
    }

    // Closing hides the window instead; the tray app keeps running either way.
    private void OnClosing(AppWindow sender, AppWindowClosingEventArgs args)
    {
        args.Cancel = true;
        if (_restored is { } restored && _window is not null)
        {
            restored.Maximized = _window.Presenter.State == OverlappedPresenterState.Maximized;
            savePlacement(restored);
        }

        _window?.SetVisible(false);
        sender.Hide();
    }
}
