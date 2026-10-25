using System.Diagnostics;
using System.Globalization;
using System.Runtime.InteropServices;

using Allot.Tray.Core.Flyout;
using Allot.Tray.Diagnostics;
using Allot.Tray.Interop;

using Microsoft.UI;
using Microsoft.UI.Composition;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Input;
using Microsoft.UI.System;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Hosting;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;

using Windows.Graphics;
using Windows.UI.ViewManagement;

using VirtualKey = Windows.System.VirtualKey;

namespace Allot.Tray.Flyout;

/// <summary>
/// The borderless acrylic popup next to the tray: no taskbar button, no Alt+Tab entry, light
/// dismiss, Stay open, Pin, and pinned dragging with corner snapping. Window plumbing only; the
/// content is supplied and decides the height.
/// </summary>
public sealed unsafe partial class FlyoutWindow : Window
{
    // A click on the tray icon deactivates the flyout 16-141 ms before its NIN_SELECT arrives, as
    // measured; about twice that still lets a deliberate reopen through.
    private static readonly TimeSpan ToggleDebounce = TimeSpan.FromMilliseconds(300);
    // Chosen over the handoff's 12 px content slide: the window itself rises out of
    // the taskbar and sinks back into it, as the shell's tray flyouts do. A flyout dragged away
    // from the tray corner, or reduced motion, fades instead.
    private static readonly TimeSpan SlideUpDuration = TimeSpan.FromMilliseconds(250);
    private static readonly TimeSpan SlideDownDuration = TimeSpan.FromMilliseconds(167);
    private static readonly TimeSpan FadeInDuration = TimeSpan.FromMilliseconds(167);
    private static readonly TimeSpan FadeOutDuration = TimeSpan.FromMilliseconds(100);
    private const nuint SubclassId = 1;
    // A larger gap between the visible frame and the client area is a transient state, not an
    // edge DWM keeps.
    private const int MaxClientShortfall = 8;
    // The handoff edges over the tints as COLORREFs: rgba(0,0,0,.09) over #F3F3F3, and
    // rgba(255,255,255,.08) over #242424.
    private const int LightEdge = 0x00DDDDDD;
    private const int DarkEdge = 0x00363636;

    // The subclass procedure is a static callback, so it finds the window here; there is only one.
    private static FlyoutWindow? s_current;

    private readonly FlyoutBehavior _behavior = new(ToggleDebounce);
    private readonly AcrylicBackdrop _backdrop = new();
    private readonly OverlappedPresenter _presenter = OverlappedPresenter.Create();
    private readonly UISettings _uiSettings = new();
    // Windows.UI.ViewManagement.AccessibilitySettings can't be activated in this unpackaged app
    // (ClassFactory cannot supply requested class); this is the App SDK's own.
    private readonly ThemeSettings _themeSettings;
    private readonly DispatcherQueue _dispatcher;
    private readonly nint _hwnd;
    private FrameworkElement? _root;
    private IFlyoutContent? _content;
    private FrameInsets _insets;
    private FrameInsets _dragInsets;
    // Effective px the client area is shorter than the visible frame.
    private double _clientShortfall;
    private bool _clientReported;
    private bool _framed;
    private bool _prewarming;
    private bool _dragging;
    private int _dragMoves;
    private WindowSlide? _slide;
    private bool _atTrayCorner = true;
    private PixelRect _dragStart;
    private NativeMethods.NativePoint _dragCursor;
    private int _generation;
    private bool _wasPinned;
    private Windows.Foundation.Rect _dragRegion;

    public FlyoutWindow()
    {
        _dispatcher = DispatcherQueue.GetForCurrentThread();
        _hwnd = WinRT.Interop.WindowNative.GetWindowHandle(this);

        _presenter.SetBorderAndTitleBar(hasBorder: true, hasTitleBar: false);
        _presenter.IsResizable = false;
        _presenter.IsMaximizable = false;
        _presenter.IsMinimizable = false;
        AppWindow.SetPresenter(_presenter);
        AppWindow.IsShownInSwitchers = false;
        AppWindow.Title = "Allot";
        AppWindow.Closing += OnClosing;

        SystemBackdrop = _backdrop;
        Activated += OnActivated;
        _themeSettings = ThemeSettings.CreateForWindowId(AppWindow.Id);
        _themeSettings.Changed += (_, _) => _dispatcher.TryEnqueue(ApplyHighContrast);
        ApplyHighContrast();

        s_current = this;
        if (!NativeMethods.SetWindowSubclass(_hwnd, &SubclassProc, SubclassId, 0))
        {
            Report("SetWindowSubclass failed; pinned dragging won't snap");
        }
    }

    /// <summary>
    /// Raised when the flyout starts to show or to hide.
    /// </summary>
    public event Action<bool>? OpenChanged;

    /// <summary>
    /// Raised as the flyout starts to show, before it measures its content: content that is not
    /// kept current while the flyout is closed brings itself up to date here.
    /// </summary>
    public event Action? Opening;

    /// <summary>
    /// Raised when Stay open or Pin changes, including the reset on a tray close.
    /// </summary>
    public event Action? ModesChanged;

    public event Action<string>? Diagnostic;

    public FlyoutBehavior Behavior => _behavior;

    public bool AnimationsEnabled => _uiSettings.AnimationsEnabled;

    /// <summary>
    /// True while <see cref="Opening"/> runs: what changes then is the flyout's first look, not a
    /// change to animate.
    /// </summary>
    public bool IsOpening { get; private set; }

    private static long Now => Environment.TickCount64;

    public void SetContent(FrameworkElement root, IFlyoutContent content)
    {
        _root = root;
        _content = content;
        Content = root;
        root.AddHandler(UIElement.KeyDownEvent, new KeyEventHandler(OnKeyDown), true);
        root.ActualThemeChanged += (sender, _) =>
        {
            _backdrop.SetTheme(sender.ActualTheme);
            ApplyBorderColor();
        };
        root.Loaded += (_, _) => _backdrop.SetTheme(root.ActualTheme);
        root.Loaded += (_, _) => root.XamlRoot.Changed += OnXamlRootChanged;
        content.DesiredHeightChanged += OnDesiredHeightChanged;
    }

    /// <summary>
    /// Builds the window and its XAML off screen without showing or activating it, so the first
    /// tray click doesn't wait for them (about 3 s in a Debug build, measured).
    /// </summary>
    public void Prewarm()
    {
        if (_root is null || _root.IsLoaded)
        {
            return;
        }

        _prewarming = true;
        _root.Loaded += OnPrewarmLoaded;
        AppWindow.Move(new PointInt32(-32000, -32000));
        AppWindow.Show(activateWindow: false);
    }

    /// <summary>
    /// A tray icon click.
    /// </summary>
    public void Toggle()
    {
        var result = _behavior.Toggle(Now);
        Report($"toggle -> {result}");
        switch (result)
        {
            case ToggleResult.Show:
                Show();
                break;
            case ToggleResult.Hide:
                ApplyModes();
                Hide();
                break;
            case ToggleResult.Ignore:
                break;
        }
    }

    /// <summary>
    /// A menu command, a toast or a second launch: opens the flyout if it is closed.
    /// </summary>
    public void Open()
    {
        if (_behavior.Open())
        {
            Show();
        }
    }

    public void SetStayOpen(bool on)
    {
        _behavior.SetStayOpen(on);
        ApplyModes();
    }

    public void SetPinned(bool on)
    {
        _behavior.SetPinned(on);
        ApplyModes();
    }

    public void SetMenuOpen(bool open)
    {
        _behavior.MenuOpen = open;

        // A click on another app closes the menu; if the flyout itself was not reactivated by
        // that click, it has lost activation while the dismiss was suppressed.
        if (!open && !ForegroundIsOurs() && _behavior.Deactivated(Now))
        {
            Hide();
        }
    }

    /// <summary>
    /// The empty header space, in effective pixels relative to the content root. While pinned
    /// it becomes a caption region, so Windows runs the drag natively. Sending
    /// WM_NCLBUTTONDOWN from a XAML pointer press does not work: XAML input arrives through a
    /// separate input window, so the move loop never sees the button go up and keeps
    /// following the cursor after the click.
    /// </summary>
    public void SetDragRegion(Windows.Foundation.Rect region)
    {
        _dragRegion = region;
        ApplyDragRegion();
    }

    private static PixelRect ToPixelRect(RectInt32 rect) =>
        new(rect.X, rect.Y, rect.Width, rect.Height);

    private static PixelRect ToPixelRect(NativeMethods.NativeRect rect) =>
        new(rect.Left, rect.Top, rect.Right - rect.Left, rect.Bottom - rect.Top);

    private static double ScaleOf(nint monitor)
    {
        var hr = NativeMethods.GetDpiForMonitor(
            monitor, NativeMethods.MdtEffectiveDpi, out var dpi, out _);
        return hr == 0 ? dpi / 96.0 : 1.0;
    }

    private static double ScaleOf(DisplayArea area) =>
        ScaleOf(Win32Interop.GetMonitorFromDisplayId(area.DisplayId));

    private static bool ForegroundIsOurs()
    {
        var foreground = NativeMethods.GetForegroundWindow();
        if (foreground == 0)
        {
            return false;
        }

        _ = NativeMethods.GetWindowThreadProcessId(foreground, out var processId);
        return processId == (uint)Environment.ProcessId;
    }

    [UnmanagedCallersOnly]
    private static nint SubclassProc(
        nint hwnd,
        uint message,
        nint wParam,
        nint lParam,
        nuint id,
        nuint referenceData)
    {
        // The caption region would otherwise offer the system menu (Move, Close).
        if (message is NativeMethods.WmNcRButtonDown or NativeMethods.WmNcRButtonUp
            && wParam == NativeMethods.HtCaption
            && s_current?._behavior.Pinned == true)
        {
            return 0;
        }

        // The rest of the chain, WinUI's caption handling included, sees every message first;
        // the flyout only adjusts what it proposed.
        var result = NativeMethods.DefSubclassProc(hwnd, message, wParam, lParam);

        // An exception must not unwind into native code, which would end the process.
        try
        {
            s_current?.AfterMessage(message, lParam);
        }
        catch (Exception e)
        {
            s_current?.Report($"subclass procedure failed: {e}");
        }

        return message == NativeMethods.WmMoving ? 1 : result;
    }

    private void AfterMessage(uint message, nint lParam)
    {
        if (!_behavior.Pinned)
        {
            return;
        }

        switch (message)
        {
            case NativeMethods.WmEnterSizeMove:
                _dragInsets = CurrentInsets();
                _dragStart = WindowBounds();
                _ = NativeMethods.GetCursorPos(out _dragCursor);
                _dragging = true;
                _dragMoves = 0;
                Report("pinned drag started");
                break;
            case NativeMethods.WmMoving:
                _dragMoves++;
                SnapWhileDragging((NativeMethods.NativeRect*)lParam);
                break;
            case NativeMethods.WmExitSizeMove:
                _dragging = false;
                var visible = VisibleBounds();
                _atTrayCorner = visible == TrayCorner();
                Report($"pinned drag ended at {visible.X},{visible.Y} after {_dragMoves} moves");
                // A height change that arrived during the drag waited for it to end.
                OnDesiredHeightChanged();
                break;
        }
    }

    // WM_MOVING proposes the window rectangle, invisible frame included, as the current position
    // plus the last mouse step. After a snap that position is the corner, so steps of a slow drag
    // would never get away from it; the unsnapped position comes from the cursor's whole travel
    // since the drag started instead.
    private void SnapWhileDragging(NativeMethods.NativeRect* proposed)
    {
        if (!NativeMethods.GetCursorPos(out var cursor))
        {
            return;
        }

        var size = ToPixelRect(*proposed);
        var unsnapped = size with
        {
            X = _dragStart.X + cursor.X - _dragCursor.X,
            Y = _dragStart.Y + cursor.Y - _dragCursor.Y,
        };
        var insets = _dragInsets;
        var visible = insets.Inside(unsnapped);
        var native = new NativeMethods.NativeRect
        {
            Left = visible.X,
            Top = visible.Y,
            Right = visible.Right,
            Bottom = visible.Bottom,
        };
        var monitor = NativeMethods.MonitorFromRect(native, NativeMethods.MonitorDefaultToNearest);
        var info = new NativeMethods.MonitorInfo { Size = (uint)sizeof(NativeMethods.MonitorInfo) };
        if (!NativeMethods.GetMonitorInfo(monitor, ref info))
        {
            return;
        }

        var snapped = FlyoutPlacement.SnapToCorner(
            visible, ToPixelRect(info.Work), ScaleOf(monitor));
        var window = insets.Around(snapped);
        proposed->Left = window.X;
        proposed->Top = window.Y;
        proposed->Right = window.Right;
        proposed->Bottom = window.Bottom;
    }

    private void OnPrewarmLoaded(object sender, RoutedEventArgs args)
    {
        _root!.Loaded -= OnPrewarmLoaded;
        _prewarming = false;
        if (!_behavior.IsOpen)
        {
            AppWindow.Hide();
        }

        Report("prewarmed");
        Measure.Startup("prewarmed");
    }

    private void ApplyDwmFrame()
    {
        _ = NativeMethods.DwmSetWindowAttribute(
            _hwnd,
            NativeMethods.DwmwaWindowCornerPreference,
            NativeMethods.DwmwcpRound,
            sizeof(int));
        ApplyBorderColor();
    }

    /// <summary>
    /// DWM draws the flyout's 1 px edge: an edge drawn by the content sat inside DWM's
    /// rounded clip and its own 1 px frame, which left a light pixel at the top corners and a
    /// sliver of backdrop at the bottom ones. The border colour takes no alpha, so it is the
    /// handoff edge blended over the tint; high contrast keeps the system's frame colour.
    /// </summary>
    private void ApplyHighContrast()
    {
        _backdrop.SetHighContrast(_themeSettings.HighContrast);
        ApplyBorderColor();
    }

    private void ApplyBorderColor()
    {
        var color = _themeSettings.HighContrast
            ? NativeMethods.DwmwaColorDefault
            : _root?.ActualTheme == ElementTheme.Dark ? DarkEdge : LightEdge;
        _ = NativeMethods.DwmSetWindowAttribute(
            _hwnd, NativeMethods.DwmwaBorderColor, color, sizeof(int));
    }

    private void Show()
    {
        Measure.Showing();
        var generation = ++_generation;
        if (!_framed)
        {
            ApplyDwmFrame();
            _framed = true;
        }

        IsOpening = true;
        try
        {
            Opening?.Invoke();
        }
        finally
        {
            IsOpening = false;
        }

        // Content may have changed while hidden; measure it before sizing the window.
        _root?.UpdateLayout();
        var target = PlaceAtTrayCorner();
        _presenter.IsAlwaysOnTop = false;
        var slide = AnimationsEnabled;
        SetContentOpacity(slide ? 1 : 0);
        if (slide)
        {
            MoveVisible(BehindTaskbar(target));
        }

        AppWindow.Show();
        Activate();
        if (!NativeMethods.SetForegroundWindow(_hwnd))
        {
            Report("SetForegroundWindow refused");
        }

        if (slide)
        {
            StartSlide(new WindowSlide(
                BehindTaskbar(target),
                target,
                Stopwatch.GetTimestamp(),
                generation,
                SlideUpDuration,
                CubicBezier.Decelerate,
                Closes: false));
        }
        else
        {
            FadeIn();
        }

        OpenChanged?.Invoke(true);
        _content?.FocusFirst();
    }

    private void Hide()
    {
        var generation = ++_generation;
        OpenChanged?.Invoke(false);
        if (_root is null)
        {
            AppWindow.Hide();
            return;
        }

        if (AnimationsEnabled && _atTrayCorner)
        {
            var visible = VisibleBounds();
            StartSlide(new WindowSlide(
                visible,
                BehindTaskbar(visible),
                Stopwatch.GetTimestamp(),
                generation,
                SlideDownDuration,
                CubicBezier.Accelerate,
                Closes: true));
            return;
        }

        var visual = ElementCompositionPreview.GetElementVisual(_root);
        var compositor = visual.Compositor;
        var fade = compositor.CreateScalarKeyFrameAnimation();
        fade.InsertKeyFrame(1, 0, compositor.CreateLinearEasingFunction());
        fade.Duration = FadeOutDuration;

        Measure.Animation("close-fade", FadeOutDuration);
        var batch = compositor.CreateScopedBatch(CompositionBatchTypes.Animation);
        visual.StartAnimation("Opacity", fade);
        batch.End();
        batch.Completed += (_, _) => _dispatcher.TryEnqueue(() =>
        {
            // A toggle during the fade reopened the flyout; keep it.
            if (generation == _generation)
            {
                AppWindow.Hide();
            }
        });
    }

    private double ContentHeight =>
        (_content?.DesiredHeight ?? FlyoutPlacement.MaxHeight) + _clientShortfall;

    private PixelRect TrayCorner()
    {
        var area = DisplayArea.Primary;
        return FlyoutPlacement.TrayCorner(
            ToPixelRect(area.WorkArea),
            ToPixelRect(area.OuterBounds),
            ScaleOf(area),
            ContentHeight);
    }

    /// <returns>The visible rectangle the flyout now has.</returns>
    private PixelRect PlaceAtTrayCorner()
    {
        var target = TrayCorner();
        MoveVisible(target);
        _atTrayCorner = true;
        return target;
    }

    /// <summary>
    /// Where the open slide starts and the close slide ends: just behind the taskbar's inner
    /// edge, so the topmost taskbar covers the flyout there.
    /// </summary>
    private static PixelRect BehindTaskbar(PixelRect target)
    {
        var area = DisplayArea.Primary;
        var workArea = ToPixelRect(area.WorkArea);
        var y = FlyoutPlacement.IsTaskbarAtTop(workArea, ToPixelRect(area.OuterBounds))
            ? workArea.Y - target.Height
            : workArea.Bottom;
        return target with { Y = y };
    }

    private void StartSlide(WindowSlide slide)
    {
        StopSlide();
        _slide = slide;
        Measure.Animation(slide.Closes ? "close-slide" : "open-slide", slide.Duration);
        CompositionTarget.Rendering += OnSlideFrame;
    }

    private void StopSlide()
    {
        if (_slide is null)
        {
            return;
        }

        _slide = null;
        CompositionTarget.Rendering -= OnSlideFrame;
    }

    // Moves rather than resizes, so each frame is one cheap SetWindowPos.
    private void OnSlideFrame(object? sender, object args)
    {
        if (_slide is not { } slide || slide.Generation != _generation)
        {
            StopSlide();
            return;
        }

        var progress = Stopwatch.GetElapsedTime(slide.StartedAt) / slide.Duration;
        var eased = slide.Curve.Ease(progress);
        var y = (int)Math.Round(slide.From.Y + ((slide.To.Y - slide.From.Y) * eased));
        AppWindow.Move(new PointInt32(slide.To.X - _insets.Left, y - _insets.Top));
        if (progress < 1)
        {
            return;
        }

        StopSlide();
        if (slide.Closes)
        {
            AppWindow.Hide();
        }
        else
        {
            // A height change during the slide waited for it to end.
            OnDesiredHeightChanged();
        }
    }

    /// <summary>
    /// Puts the visible edge exactly on <paramref name="target"/>. The window rectangle includes
    /// an invisible frame (7 px at 100%, 9 px at 150%); its last known size is applied first, and
    /// a second move follows only when it changed.
    /// </summary>
    private void MoveVisible(PixelRect target)
    {
        // Content heights arrive on every animation frame; most leave the window as it is.
        if (AppWindow.IsVisible && VisibleBounds() == target)
        {
            return;
        }

        var window = _insets.Around(target);
        CountResize(window);
        AppWindow.MoveAndResize(new RectInt32(window.X, window.Y, window.Width, window.Height));
        var actual = CurrentInsets();
        if (actual != _insets)
        {
            _insets = actual;
            window = actual.Around(target);
            AppWindow.MoveAndResize(
                new RectInt32(window.X, window.Y, window.Width, window.Height));
            Report(string.Create(
                CultureInfo.InvariantCulture,
                $"frame insets {actual.Left},{actual.Top},{actual.Right},{actual.Bottom}"));
        }

        CheckClient(target);
    }

    private void CountResize(PixelRect window)
    {
        if (Measure.Enabled && WindowBounds() is var before
            && (before.Width != window.Width || before.Height != window.Height))
        {
            Measure.Resized();
        }
    }

    // DWM keeps a 1 px edge of the visible frame outside the client area (seen: client
    // 358x227 in visible 360x228), and the body then scrolled by that 1 px. The shortfall is
    // added to the content height, and the flyout is placed again when it changes. Growing the
    // client area over the edge with WM_NCCALCSIZE does not work: DWM moves the visible frame
    // out with it, and the measured insets then shrink on every placement.
    private void CheckClient(PixelRect target)
    {
        var client = AppWindow.ClientSize;
        var missing = target.Height - client.Height;
        if (missing is < 0 or > MaxClientShortfall)
        {
            return;
        }

        var area = DisplayArea.GetFromWindowId(AppWindow.Id, DisplayAreaFallback.Nearest);
        var shortfall = missing / ScaleOf(area);
        if (shortfall == _clientShortfall && _clientReported)
        {
            return;
        }

        _clientReported = true;
        Report(string.Create(
            CultureInfo.InvariantCulture,
            $"client {client.Width}x{client.Height} in visible {target.Width}x{target.Height}"));
        if (shortfall != _clientShortfall)
        {
            _clientShortfall = shortfall;
            _ = _dispatcher.TryEnqueue(OnDesiredHeightChanged);
        }
    }

    private void OnDesiredHeightChanged()
    {
        // Moving the window from code during Windows' move loop fights the user's drag.
        if (!_behavior.IsOpen || _prewarming || _dragging || _slide is not null)
        {
            return;
        }

        // A pinned flyout nobody dragged still belongs to the tray corner.
        if (!_behavior.Pinned || _atTrayCorner)
        {
            _ = PlaceAtTrayCorner();
            return;
        }

        var area = DisplayArea.GetFromWindowId(AppWindow.Id, DisplayAreaFallback.Nearest);
        var workArea = ToPixelRect(area.WorkArea);
        var scale = ScaleOf(area);
        var height = FlyoutPlacement.ToPixels(
            FlyoutPlacement.ClampHeight(ContentHeight, workArea.Height / scale), scale);
        var visible = VisibleBounds();
        if (height != visible.Height)
        {
            MoveVisible(FlyoutPlacement.Regrow(visible, height, workArea));
        }
    }

    private void SetContentOpacity(float opacity)
    {
        if (_root is not null)
        {
            ElementCompositionPreview.GetElementVisual(_root).Opacity = opacity;
        }
    }

    // With reduced motion the flyout fades in where it stands.
    private void FadeIn()
    {
        if (_root is null)
        {
            return;
        }

        var visual = ElementCompositionPreview.GetElementVisual(_root);
        var compositor = visual.Compositor;
        var fade = compositor.CreateScalarKeyFrameAnimation();
        fade.InsertKeyFrame(1, 1, compositor.CreateLinearEasingFunction());
        fade.Duration = FadeInDuration;
        Measure.Animation("open-fade", FadeInDuration);
        visual.StartAnimation("Opacity", fade);
    }

    private void ApplyModes()
    {
        _presenter.IsAlwaysOnTop = _behavior.Pinned;
        // A flyout kept open is a mini monitor; it keeps acrylic instead of turning solid
        // when another window is active.
        _backdrop.KeepActiveLook = _behavior.StayOpen;
        Report($"modes: stay {_behavior.StayOpen}, pin {_behavior.Pinned}");
        ModesChanged?.Invoke();
        ApplyDragRegion();

        // Unpinning (directly or by turning Stay open off) returns a dragged flyout home.
        if (_behavior.IsOpen && _wasPinned && !_behavior.Pinned)
        {
            _ = PlaceAtTrayCorner();
        }

        _wasPinned = _behavior.Pinned;
    }

    private void ApplyDragRegion()
    {
        var source = InputNonClientPointerSource.GetForWindowId(AppWindow.Id);
        var scale = _root?.XamlRoot?.RasterizationScale ?? 1.0;
        if (!_behavior.Pinned || _dragRegion.Width <= 0 || _dragRegion.Height <= 0)
        {
            source.ClearRegionRects(NonClientRegionKind.Caption);
            return;
        }

        var caption = new RectInt32(
            FlyoutPlacement.ToPixels(_dragRegion.X, scale),
            FlyoutPlacement.ToPixels(_dragRegion.Y, scale),
            FlyoutPlacement.ToPixels(_dragRegion.Width, scale),
            FlyoutPlacement.ToPixels(_dragRegion.Height, scale));
        source.SetRegionRects(NonClientRegionKind.Caption, [caption]);
        Report($"drag region {caption.X},{caption.Y} {caption.Width}x{caption.Height}");
    }

    private void OnActivated(object sender, WindowActivatedEventArgs args)
    {
        var active = args.WindowActivationState != WindowActivationState.Deactivated;
        _backdrop.SetWindowActive(active);
        if (!active && _behavior.Deactivated(Now))
        {
            Hide();
        }
    }

    private void OnKeyDown(object sender, KeyRoutedEventArgs args)
    {
        // A Kill? confirm handles its own Esc first (handoff, Interactions).
        if (args.Key == VirtualKey.Escape && !args.Handled && _behavior.Escape())
        {
            args.Handled = true;
            Hide();
        }
    }

    private void OnClosing(AppWindow sender, AppWindowClosingEventArgs args)
    {
        // Alt+F4 must not destroy the only flyout window; treat it like a dismiss.
        args.Cancel = true;
        if (_behavior.IsOpen)
        {
            _ = _behavior.Toggle(Now);
            ApplyModes();
            Hide();
        }
    }

    private void OnXamlRootChanged(XamlRoot sender, XamlRootChangedEventArgs args) =>
        // Region rects are physical pixels; a monitor with another scale needs them redone.
        ApplyDragRegion();

    private FrameInsets CurrentInsets()
    {
        var window = WindowBounds();
        var visible = VisibleBounds();
        return new FrameInsets(
            visible.X - window.X,
            visible.Y - window.Y,
            window.Right - visible.Right,
            window.Bottom - visible.Bottom);
    }

    private PixelRect WindowBounds() =>
        NativeMethods.GetWindowRect(_hwnd, out var rect) ? ToPixelRect(rect) : default;

    private PixelRect VisibleBounds()
    {
        var hr = NativeMethods.DwmGetWindowAttribute(
            _hwnd,
            NativeMethods.DwmwaExtendedFrameBounds,
            out var rect,
            Marshal.SizeOf<NativeMethods.NativeRect>());
        return hr == 0 ? ToPixelRect(rect) : WindowBounds();
    }

    private void Report(string message) => Diagnostic?.Invoke(message);

    /// <param name="From">The visible rectangle the slide starts at.</param>
    /// <param name="To">The visible rectangle it ends at.</param>
    /// <param name="StartedAt">The <see cref="Stopwatch"/> timestamp of the first frame.</param>
    /// <param name="Generation">The show or hide it belongs to; the next one ends it.</param>
    /// <param name="Duration">How long it takes.</param>
    /// <param name="Curve">Its timing curve.</param>
    /// <param name="Closes">The window hides when it ends.</param>
    private sealed record WindowSlide(
        PixelRect From,
        PixelRect To,
        long StartedAt,
        int Generation,
        TimeSpan Duration,
        CubicBezier Curve,
        bool Closes);

    /// <summary>
    /// The invisible frame between the window rectangle and its visible edge.
    /// </summary>
    private readonly record struct FrameInsets(int Left, int Top, int Right, int Bottom)
    {
        public PixelRect Around(PixelRect visible) =>
            new(
                visible.X - Left,
                visible.Y - Top,
                visible.Width + Left + Right,
                visible.Height + Top + Bottom);

        public PixelRect Inside(PixelRect window) =>
            new(
                window.X + Left,
                window.Y + Top,
                window.Width - Left - Right,
                window.Height - Top - Bottom);
    }
}
