using System.Collections.Specialized;

using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Animation;

using ResourcesHelper.Tray.Core.Settings;
using ResourcesHelper.Tray.Core.ViewModels;

using Windows.Foundation;
using Windows.System;
using Windows.UI.Text;

namespace ResourcesHelper.Tray.Flyout;

/// <summary>
/// The flyout's header and body. Rows of the <see cref="FlyoutModel"/> become controls that enter
/// and leave with a 240 ms height and opacity animation while the flyout is open; a row that
/// stays keeps its control, and its bindings update only the values that changed.
/// </summary>
public sealed partial class FlyoutShell : UserControl, IFlyoutContent
{
    private static readonly TimeSpan RowDuration = TimeSpan.FromMilliseconds(240);
    private static readonly TimeSpan PinRevealDuration = TimeSpan.FromMilliseconds(200);
    private static readonly TimeSpan PinFadeDuration = TimeSpan.FromMilliseconds(150);
    private const double PinWrapperWidth = 36;
    private const double RowGap = 6;

    // The flyout's 1 px edge, top and bottom.
    private const double EdgeHeight = 2;

    private readonly FlyoutWindow _window;
    private readonly Action _openLogs;
    private readonly Dictionary<FlyoutRow, ContentControl> _hosts =
        new(ReferenceEqualityComparer.Instance);
    private readonly DataTemplateSelector _templates;
    private bool _pinShown;
    private double _reportedHeight;

    // A check item was toggled, so the menu's coming close is cancelled; set while the item's
    // click runs, and cleared after it.
    private bool _keepMenuOpen;

    // Check items are being set from the settings, not by a click.
    private bool _showingToggles;

    // The Resources card's running expand or collapse, if any.
    private Storyboard? _resourcesStoryboard;

    public FlyoutShell(FlyoutWindow window, FlyoutModel model, Action openLogs)
    {
        _window = window;
        _openLogs = openLogs;
        Model = model;
        InitializeComponent();
        _templates = (DataTemplateSelector)Resources["RowTemplates"];

        foreach (var row in model.Rows)
        {
            RowsPanel.Children.Add(HostFor(row));
        }

        UpdateGaps();
        model.Rows.CollectionChanged += OnRowsChanged;
        _window.ModesChanged += SyncToggles;

        foreach (var item in CheckItems())
        {
            _ = item.RegisterPropertyChangedCallback(
                ToggleMenuFlyoutItem.IsCheckedProperty,
                OnCheckItemToggled);
        }

        ResourcesRows.Visibility =
            model.Resources.Expanded ? Visibility.Visible : Visibility.Collapsed;
        model.Resources.PropertyChanged += (_, args) =>
        {
            if (args.PropertyName == nameof(ResourcesModel.Expanded))
            {
                ShowResourcesRows(Model.Resources.Expanded);
            }
        };

        // A collapsed card raises no SizeChanged, so its visibility reports the height too.
        _ = ResourcesCard.RegisterPropertyChangedCallback(
            VisibilityProperty,
            (_, _) => DispatcherQueue.TryEnqueue(ReportHeight));
    }

    public event Action? DesiredHeightChanged;

    /// <summary>
    /// A press of a card's Kill or Kill?, with its lease.
    /// </summary>
    public event Action<long>? KillPressed;

    /// <summary>
    /// A press of a card's Remove, with its lease.
    /// </summary>
    public event Action<long>? RemovePressed;

    /// <summary>
    /// Esc on a card's Kill?, or focus leaving it, with its lease.
    /// </summary>
    public event Action<long>? ConfirmCancelled;

    /// <summary>
    /// A press of the Resources card's chevron.
    /// </summary>
    public event Action? ResourcesToggled;

    /// <summary>
    /// A ⋯ › Meters item was toggled; carries every meter item's state.
    /// </summary>
    public event Action<MeterToggles>? MetersChanged;

    /// <summary>
    /// A ⋯ › Notifications item was toggled; carries every notification item's state.
    /// </summary>
    public event Action<NotificationToggles>? NotificationsChanged;

    /// <summary>
    /// A press of the service-down banner's Start service.
    /// </summary>
    public event Action? StartServicePressed;

    /// <summary>
    /// The first-run card's Launch at logon switch was flipped, with its new state.
    /// </summary>
    public event Action<bool>? LaunchAtLogonChanged;

    /// <summary>
    /// A press of the first-run card's Done.
    /// </summary>
    public event Action? FirstRunDonePressed;

    public FlyoutModel Model { get; }

    public double DesiredHeight =>
        EdgeHeight + Header.ActualHeight + BodyPanel.ActualHeight + ResourcesHeight;

    // The docked card with its margin, or nothing while it is hidden.
    private double ResourcesHeight =>
        ResourcesCard.Visibility == Visibility.Visible
            ? ResourcesCard.ActualHeight + ResourcesCard.Margin.Top + ResourcesCard.Margin.Bottom
            : 0;

    public static string ResourcesToggleName(bool expanded) =>
        expanded ? "Collapse resources" : "Expand resources";

    /// <summary>
    /// A drive bar's fill as a star column; <see cref="RestShare"/> is the track after it.
    /// </summary>
    public static GridLength Share(double fraction) =>
        new(Math.Clamp(fraction, 0, 1), GridUnitType.Star);

    public static GridLength RestShare(double fraction) =>
        new(1 - Math.Clamp(fraction, 0, 1), GridUnitType.Star);

    public static Visibility Shows(HeaderGlyph glyph, string name) =>
        glyph.ToString() == name ? Visibility.Visible : Visibility.Collapsed;

    public static Visibility ShowsAction(CardAction action, string name) =>
        action.ToString() == name ? Visibility.Visible : Visibility.Collapsed;

    public static Visibility ShowsIcon(DetailIcon icon, string name) =>
        icon.ToString() == name ? Visibility.Visible : Visibility.Collapsed;

    /// <summary>
    /// A detail line with a leading icon, a benchmark's status, wraps instead of truncating.
    /// </summary>
    public static TextWrapping Wraps(DetailIcon icon) =>
        icon == DetailIcon.None ? TextWrapping.NoWrap : TextWrapping.WrapWholeWords;

    public static Visibility Hides(bool hidden) =>
        hidden ? Visibility.Collapsed : Visibility.Visible;

    /// <summary>
    /// A tooltip for non-empty text; an empty string would still show an empty tooltip.
    /// </summary>
    public static object? Tip(string text) => string.IsNullOrEmpty(text) ? null : text;

    /// <summary>
    /// The hairline above every Recently finished row but the first.
    /// </summary>
    public static Thickness TopLine(bool first) => new(0, first ? 0 : 1, 0, 0);

    /// <summary>
    /// Last known rows show at 55% while the service is down (handoff section 6).
    /// </summary>
    public static double Dimmed(bool isLastKnown) => isLastKnown ? 0.55 : 1;

    public static Visibility ShowsBanner(ServiceBanner? banner) => Hides(banner is null);

    public static string BannerText(ServiceBanner? banner) => banner?.Text ?? "";

    public static string BannerProblem(ServiceBanner? banner) => banner?.Problem ?? "";

    public static Visibility ShowsProblem(ServiceBanner? banner) =>
        Hides(string.IsNullOrEmpty(banner?.Problem));

    public static Visibility ShowsStart(ServiceBanner? banner) =>
        Hides(banner is null or { Button: BannerButton.None });

    /// <summary>
    /// Starting… is the same button, muted and disabled.
    /// </summary>
    public static bool CanStart(ServiceBanner? banner) =>
        banner is { Button: BannerButton.Start };

    public static string StartText(ServiceBanner? banner) =>
        banner is { Button: BannerButton.Starting } ? "Starting…" : "Start service";

    public static string Caption(ServiceBanner? banner) => banner?.Caption ?? "";

    public static Visibility ShowsCaption(ServiceBanner? banner) =>
        Hides(string.IsNullOrEmpty(banner?.Caption));

    public static string OnOff(bool on) => on ? "On" : "Off";

    /// <summary>
    /// Stopped and Removed cards show tile, command and chip at 50%.
    /// </summary>
    public static double Ended(bool isEnded) => isEnded ? 0.5 : 1;

    public static TextDecorations StruckThrough(bool isEnded) =>
        isEnded ? TextDecorations.Strikethrough : TextDecorations.None;

    public void FocusFirst()
    {
        Control first = _pinShown ? PinButton : StayOpenButton;
        _ = first.Focus(FocusState.Programmatic);
    }

    /// <summary>
    /// Sets the ⋯ menu's check items from the settings; Drive activity is disabled while Drives
    /// is off.
    /// </summary>
    public void ShowToggles(TraySettings settings)
    {
        _showingToggles = true;
        var meters = settings.Meters;
        CpuItem.IsChecked = meters.Cpu;
        RamItem.IsChecked = meters.Ram;
        GpuItem.IsChecked = meters.Gpu;
        VramItem.IsChecked = meters.Vram;
        DrivesItem.IsChecked = meters.Drives;
        DriveActivityItem.IsChecked = meters.DriveActivity;
        DriveActivityItem.IsEnabled = meters.Drives;
        var notifications = settings.Notifications;
        ServiceStoppedItem.IsChecked = notifications.ServiceStopped;
        RunningLongItem.IsChecked = notifications.RunningLong;
        BenchmarkFinishedItem.IsChecked = notifications.BenchmarkFinished;
        JobFinishedItem.IsChecked = notifications.JobFinished;
        KilledOutsideItem.IsChecked = notifications.KilledOutsideFlyout;
        _showingToggles = false;
    }

#if DEBUG
    /// <summary>
    /// Lists the handoff scenarios under More options while a Debug build shows one.
    /// </summary>
    public void AddScenarios(IReadOnlyList<string> names, Action<string> show)
    {
        var scenarios = new MenuFlyoutSubItem { Text = "Scenario" };
        foreach (var name in names)
        {
            var item = new MenuFlyoutItem { Text = name };
            item.Click += (_, _) => show(name);
            scenarios.Items.Add(item);
        }

        MoreMenu.Items.Insert(0, scenarios);
    }
#endif

    private static KeySpline Spline(double x1, double y1, double x2, double y2) =>
        new() { ControlPoint1 = new Point(x1, y1), ControlPoint2 = new Point(x2, y2) };

    private static DoubleAnimationUsingKeyFrames SplineTo(
        DependencyObject target,
        string property,
        double value,
        TimeSpan duration,
        KeySpline spline)
    {
        var animation = new DoubleAnimationUsingKeyFrames { EnableDependentAnimation = true };
        animation.KeyFrames.Add(new SplineDoubleKeyFrame
        {
            KeyTime = KeyTime.FromTimeSpan(duration),
            Value = value,
            KeySpline = spline,
        });
        Storyboard.SetTarget(animation, target);
        Storyboard.SetTargetProperty(animation, property);
        return animation;
    }

    private static DoubleAnimation FadeTo(DependencyObject target, double value, TimeSpan duration)
    {
        var animation = new DoubleAnimation { To = value, Duration = duration };
        Storyboard.SetTarget(animation, target);
        Storyboard.SetTargetProperty(animation, nameof(Opacity));
        return animation;
    }

    // The handoff's standard curve for card height and opacity, cubic-bezier(.4,0,.2,1).
    private static KeySpline RowCurve() => Spline(0.4, 0, 0.2, 1);

    private bool Animate => _window.Behavior.IsOpen && IsLoaded;

    /// <summary>
    /// Expands or collapses the Resources card's rows over 240 ms, height and opacity together
    /// as cards enter and leave; with reduced motion they only fade. The summary, which the model
    /// fills as soon as the card collapses, fades in while the rows slide away. A toggle during
    /// the animation turns around from where it is.
    /// </summary>
    private void ShowResourcesRows(bool expanded)
    {
        var height = ResourcesRows.ActualHeight;
        var opacity = ResourcesRows.Opacity;
        _resourcesStoryboard?.Stop();
        _resourcesStoryboard = null;
        ResourcesRows.ClearValue(HeightProperty);
        ResourcesSummary.Opacity = 1;

        if (!Animate || ResourcesCard.Visibility != Visibility.Visible)
        {
            ResourcesRows.Opacity = 1;
            ResourcesRows.Visibility = expanded ? Visibility.Visible : Visibility.Collapsed;
            return;
        }

        var wasShown = ResourcesRows.Visibility == Visibility.Visible;
        ResourcesRows.Visibility = Visibility.Visible;
        ResourcesRows.Opacity = wasShown ? opacity : 0;
        var storyboard = new Storyboard();
        storyboard.Children.Add(FadeTo(ResourcesRows, expanded ? 1 : 0, RowDuration));
        if (_window.AnimationsEnabled)
        {
            var target = 0.0;
            if (expanded)
            {
                ResourcesRows.Measure(new Size(
                    ResourcesCard.ActualWidth - ResourcesCard.BorderThickness.Left
                        - ResourcesCard.BorderThickness.Right,
                    double.PositiveInfinity));
                target = ResourcesRows.DesiredSize.Height;
            }

            ResourcesRows.Height = wasShown ? height : 0;
            storyboard.Children.Add(
                SplineTo(ResourcesRows, nameof(Height), target, RowDuration, RowCurve()));
        }

        if (!expanded)
        {
            ResourcesSummary.Opacity = 0;
            storyboard.Children.Add(FadeTo(ResourcesSummary, 1, RowDuration));
        }

        storyboard.Completed += (_, _) =>
        {
            if (_resourcesStoryboard != storyboard)
            {
                return;
            }

            storyboard.Stop();
            _resourcesStoryboard = null;
            ResourcesRows.ClearValue(HeightProperty);
            ResourcesRows.Opacity = 1;
            ResourcesRows.Visibility = expanded ? Visibility.Visible : Visibility.Collapsed;
            ResourcesSummary.Opacity = 1;
        };
        _resourcesStoryboard = storyboard;
        storyboard.Begin();
    }

    private void OnStayOpenClick(object sender, RoutedEventArgs e) =>
        _window.SetStayOpen(StayOpenButton.IsChecked == true);

    private void OnPinClick(object sender, RoutedEventArgs e) =>
        _window.SetPinned(PinButton.IsChecked == true);

    private void OnMenuOpened(object? sender, object e)
    {
        MorePressed.Opacity = 1;
        _window.SetMenuOpen(true);
    }

    private void OnMenuClosed(object? sender, object e)
    {
        MorePressed.Opacity = 0;
        _window.SetMenuOpen(false);
    }

    private void OnOpenLogsClick(object sender, RoutedEventArgs e) => _openLogs();

    private IEnumerable<ToggleMenuFlyoutItem> CheckItems() =>
    [
        CpuItem, RamItem, GpuItem, VramItem, DrivesItem, DriveActivityItem,
        ServiceStoppedItem, RunningLongItem, BenchmarkFinishedItem, JobFinishedItem,
        KilledOutsideItem,
    ];

    // A check item flips before its click closes the menu, so the close that follows is the
    // toggle's: the handoff keeps the menu open. The flag clears once the click is done, so a
    // later dismissal still closes it.
    private void OnCheckItemToggled(DependencyObject sender, DependencyProperty property)
    {
        if (_showingToggles)
        {
            return;
        }

        _keepMenuOpen = true;
        _ = DispatcherQueue.TryEnqueue(
            Microsoft.UI.Dispatching.DispatcherQueuePriority.Low,
            () => _keepMenuOpen = false);
    }

    private void OnMenuClosing(FlyoutBase sender, FlyoutBaseClosingEventArgs args)
    {
        if (_keepMenuOpen)
        {
            args.Cancel = true;
            _keepMenuOpen = false;
        }
    }

    private void OnMeterClick(object sender, RoutedEventArgs e)
    {
        DriveActivityItem.IsEnabled = DrivesItem.IsChecked;
        MetersChanged?.Invoke(new MeterToggles
        {
            Cpu = CpuItem.IsChecked,
            Ram = RamItem.IsChecked,
            Gpu = GpuItem.IsChecked,
            Vram = VramItem.IsChecked,
            Drives = DrivesItem.IsChecked,
            DriveActivity = DriveActivityItem.IsChecked,
        });
    }

    private void OnNotificationClick(object sender, RoutedEventArgs e) =>
        NotificationsChanged?.Invoke(new NotificationToggles
        {
            ServiceStopped = ServiceStoppedItem.IsChecked,
            RunningLong = RunningLongItem.IsChecked,
            BenchmarkFinished = BenchmarkFinishedItem.IsChecked,
            JobFinished = JobFinishedItem.IsChecked,
            KilledOutsideFlyout = KilledOutsideItem.IsChecked,
        });

    private void OnResourcesToggleClick(object sender, RoutedEventArgs e) =>
        ResourcesToggled?.Invoke();

    // A tap on the chevron is its Click's; anywhere else in the header toggles the card too.
    private void OnResourcesHeaderTapped(object sender, TappedRoutedEventArgs e)
    {
        for (var element = e.OriginalSource as DependencyObject;
            element is not null;
            element = VisualTreeHelper.GetParent(element))
        {
            if (element == ResourcesToggle)
            {
                return;
            }
        }

        ResourcesToggled?.Invoke();
    }

    private static JobCardRow? RowOf(object sender) =>
        (sender as FrameworkElement)?.DataContext as JobCardRow;

    // Kill and Kill? share one cell of the card's top line.
    private static Button? Sibling(DependencyObject button, string name) =>
        (VisualTreeHelper.GetParent(button) as Panel)?.Children.OfType<Button>()
            .FirstOrDefault(sibling => sibling.Name == name);

    /// <summary>
    /// Kill turns into Kill? and focus moves to it; a press of Kill? that goes through moves
    /// focus on to the next control, as the button is gone.
    /// </summary>
    private void OnKillClick(object sender, RoutedEventArgs e)
    {
        if (RowOf(sender) is not { } row)
        {
            return;
        }

        var button = (Button)sender;
        var confirming = row.Action == CardAction.Confirm;
        var next = confirming
            ? FocusManager.FindNextElement(
                FocusNavigationDirection.Next,
                new FindNextElementOptions { SearchRoot = XamlRoot.Content }) as Control
            : null;
        KillPressed?.Invoke(row.LeaseId);

        if (!confirming && row.Action == CardAction.Confirm
            && Sibling(button, "ConfirmButton") is { } confirm)
        {
            _ = DispatcherQueue.TryEnqueue(() => confirm.Focus(FocusState.Programmatic));
        }
        else if (confirming && row.Action != CardAction.Confirm)
        {
            _ = DispatcherQueue.TryEnqueue(() =>
            {
                if (next?.Focus(FocusState.Programmatic) != true)
                {
                    FocusFirst();
                }
            });
        }
    }

    private void OnStartServiceClick(object sender, RoutedEventArgs e) =>
        StartServicePressed?.Invoke();

    // Toggled also fires when the binding sets the switch from the model; only a flip by the
    // user differs from the row.
    private void OnLaunchAtLogonToggled(object sender, RoutedEventArgs e)
    {
        if (sender is not ToggleSwitch { DataContext: WelcomeRow row } toggle
            || toggle.IsOn == row.LaunchAtLogon)
        {
            return;
        }

        LaunchAtLogonChanged?.Invoke(toggle.IsOn);

        // A failed write leaves the entry as it was, and the switch shows that.
        if (toggle.IsOn != row.LaunchAtLogon)
        {
            toggle.IsOn = row.LaunchAtLogon;
        }
    }

    // The card leaves with Done, so focus goes back to the header.
    private void OnFirstRunDoneClick(object sender, RoutedEventArgs e)
    {
        FirstRunDonePressed?.Invoke();
        _ = DispatcherQueue.TryEnqueue(FocusFirst);
    }

    private void OnRemoveClick(object sender, RoutedEventArgs e)
    {
        if (RowOf(sender) is { } row)
        {
            RemovePressed?.Invoke(row.LeaseId);
        }
    }

    // Esc cancels the confirm and stays in the flyout; the window's own Esc handler skips a
    // handled key.
    private void OnConfirmKeyDown(object sender, KeyRoutedEventArgs e)
    {
        if (e.Key == VirtualKey.Escape && RowOf(sender) is { } row)
        {
            e.Handled = true;
            ConfirmCancelled?.Invoke(row.LeaseId);
        }
    }

    private void OnConfirmLostFocus(object sender, RoutedEventArgs e)
    {
        if (RowOf(sender) is { Action: CardAction.Confirm } row)
        {
            ConfirmCancelled?.Invoke(row.LeaseId);
        }
    }

    private void OnConfirmLoaded(object sender, RoutedEventArgs e)
    {
        var confirm = (Button)sender;
        confirm.Loaded -= OnConfirmLoaded;
        _ = confirm.RegisterPropertyChangedCallback(VisibilityProperty, OnConfirmVisibilityChanged);
    }

    /// <summary>
    /// When Kill? goes back to Kill (the countdown ran out, or Esc) while it has focus, focus
    /// goes back to Kill instead of being lost.
    /// </summary>
    private void OnConfirmVisibilityChanged(DependencyObject sender, DependencyProperty property)
    {
        var confirm = (Button)sender;
        if (confirm.Visibility != Visibility.Collapsed || confirm.FocusState == FocusState.Unfocused)
        {
            return;
        }

        _ = DispatcherQueue.TryEnqueue(() =>
        {
            if (Sibling(confirm, "KillButton") is { Visibility: Visibility.Visible } kill)
            {
                _ = kill.Focus(FocusState.Programmatic);
            }
        });
    }

    private void OnHeaderSizeChanged(object sender, SizeChangedEventArgs e)
    {
        ReportDragRegion();
        ReportHeight();
    }

    private void OnBodySizeChanged(object sender, SizeChangedEventArgs e) => ReportHeight();

    private void OnResourcesSizeChanged(object sender, SizeChangedEventArgs e) => ReportHeight();

    // Pin slides out of a wrapper that clips it, so the summary truncates as the space shrinks.
    private void OnPinWrapperSizeChanged(object sender, SizeChangedEventArgs e) =>
        PinWrapper.Clip = new RectangleGeometry
        {
            Rect = new Rect(0, 0, e.NewSize.Width, e.NewSize.Height),
        };

    private void ReportHeight()
    {
        var height = DesiredHeight;
        if (height != _reportedHeight)
        {
            _reportedHeight = height;
            DesiredHeightChanged?.Invoke();
        }
    }

    /// <summary>
    /// The header from its left edge up to the first button: glyph and summary, no controls.
    /// </summary>
    private void ReportDragRegion()
    {
        FrameworkElement firstButton = _pinShown ? PinWrapper : StayOpenButton;
        var header = Header.TransformToVisual(this).TransformPoint(default);
        var buttonLeft = firstButton.TransformToVisual(this).TransformPoint(default).X;
        _window.SetDragRegion(
            new Rect(header.X, header.Y, buttonLeft - header.X, Header.ActualHeight));
    }

    private void SyncToggles()
    {
        var behavior = _window.Behavior;
        StayOpenButton.IsChecked = behavior.StayOpen;
        LockOpenIcon.Visibility = behavior.StayOpen ? Visibility.Collapsed : Visibility.Visible;
        LockClosedIcon.Visibility = behavior.StayOpen ? Visibility.Visible : Visibility.Collapsed;
        PinButton.IsChecked = behavior.Pinned;
        PinIcon.Visibility = behavior.Pinned ? Visibility.Collapsed : Visibility.Visible;
        PinnedIcon.Visibility = behavior.Pinned ? Visibility.Visible : Visibility.Collapsed;
        ShowPin(behavior.StayOpen);
        ReportDragRegion();
    }

    /// <summary>
    /// Reveals or hides Pin: the wrapper's width animates 0 to 36 over 200 ms while Pin fades in
    /// over 150 ms. While hidden Pin is out of the tab order and hidden from assistive tech.
    /// </summary>
    private void ShowPin(bool show)
    {
        if (show == _pinShown)
        {
            return;
        }

        _pinShown = show;
        PinButton.IsTabStop = show;
        PinButton.IsHitTestVisible = show;
        AutomationProperties.SetAccessibilityView(
            PinButton, show ? AccessibilityView.Content : AccessibilityView.Raw);

        var width = show ? PinWrapperWidth : 0;
        var opacity = show ? 1 : 0;
        if (!_window.AnimationsEnabled || !Animate)
        {
            PinWrapper.Width = width;
            PinButton.Opacity = opacity;
            return;
        }

        var storyboard = new Storyboard();
        storyboard.Children.Add(SplineTo(
            PinWrapper, nameof(Width), width, PinRevealDuration, Spline(0.1, 0.9, 0.2, 1)));
        storyboard.Children.Add(FadeTo(PinButton, opacity, PinFadeDuration));
        storyboard.Completed += (_, _) =>
        {
            // Keep the final values as local values, so the next reveal starts from them.
            storyboard.Stop();
            PinWrapper.Width = width;
            PinButton.Opacity = opacity;
            ReportDragRegion();
        };
        storyboard.Begin();
    }

    private ContentControl HostFor(FlyoutRow row)
    {
        var host = new ContentControl
        {
            Content = row,
            ContentTemplateSelector = _templates,
            HorizontalContentAlignment = HorizontalAlignment.Stretch,
            IsTabStop = false,
        };
        _hosts[row] = host;
        return host;
    }

    private void OnRowsChanged(object? sender, NotifyCollectionChangedEventArgs e)
    {
        var entering = new List<ContentControl>();
        switch (e.Action)
        {
            case NotifyCollectionChangedAction.Add:
                for (var i = 0; i < e.NewItems!.Count; i++)
                {
                    var row = (FlyoutRow)e.NewItems[i]!;
                    var host = HostFor(row);
                    RowsPanel.Children.Insert(PanelIndexAfter(e.NewStartingIndex + i), host);
                    entering.Add(host);
                }

                break;
            case NotifyCollectionChangedAction.Remove:
                foreach (FlyoutRow row in e.OldItems!)
                {
                    Leave(row);
                }

                break;
            case NotifyCollectionChangedAction.Move:
                var moved = _hosts[(FlyoutRow)e.NewItems![0]!];
                _ = RowsPanel.Children.Remove(moved);
                RowsPanel.Children.Insert(PanelIndexAfter(e.NewStartingIndex), moved);
                break;
            case NotifyCollectionChangedAction.Replace:
            case NotifyCollectionChangedAction.Reset:
                // FlyoutModel.Sync never replaces or resets; rebuild if that ever changes.
                RowsPanel.Children.Clear();
                _hosts.Clear();
                foreach (var row in Model.Rows)
                {
                    RowsPanel.Children.Add(HostFor(row));
                }

                break;
        }

        // Gaps first: an entering row measures its target height with its gap.
        UpdateGaps();
        foreach (var host in entering)
        {
            Enter(host);
        }
    }

    /// <summary>
    /// Where the row at <paramref name="index"/> goes in the panel: before the control of the next
    /// row that has one. Leaving controls stay where they were until they collapse.
    /// </summary>
    private int PanelIndexAfter(int index)
    {
        for (var i = index + 1; i < Model.Rows.Count; i++)
        {
            if (_hosts.TryGetValue(Model.Rows[i], out var next)
                && RowsPanel.Children.IndexOf(next) is >= 0 and var position)
            {
                return position;
            }
        }

        return RowsPanel.Children.Count;
    }

    // The body has a 6 px gap between rows; it is the top padding of every row but the first,
    // so it collapses with the row.
    private void UpdateGaps()
    {
        for (var i = 0; i < RowsPanel.Children.Count; i++)
        {
            ((ContentControl)RowsPanel.Children[i]).Padding =
                new Thickness(0, i == 0 ? 0 : RowGap, 0, 0);
        }
    }

    private void Enter(ContentControl host)
    {
        if (!Animate)
        {
            return;
        }

        host.Opacity = 0;
        var storyboard = new Storyboard();
        storyboard.Children.Add(FadeTo(host, 1, RowDuration));
        if (_window.AnimationsEnabled)
        {
            host.Measure(new Size(RowsPanel.ActualWidth - RowsPanel.Padding.Left
                - RowsPanel.Padding.Right, double.PositiveInfinity));
            var height = host.DesiredSize.Height;
            host.Height = 0;
            storyboard.Children.Add(SplineTo(host, nameof(Height), height, RowDuration, RowCurve()));
        }

        storyboard.Completed += (_, _) =>
        {
            storyboard.Stop();
            host.ClearValue(HeightProperty);
            host.Opacity = 1;
        };
        storyboard.Begin();
    }

    private void Leave(FlyoutRow row)
    {
        var host = _hosts[row];
        _ = _hosts.Remove(row);
        if (!Animate)
        {
            _ = RowsPanel.Children.Remove(host);
            return;
        }

        host.IsHitTestVisible = false;
        var storyboard = new Storyboard();
        storyboard.Children.Add(FadeTo(host, 0, RowDuration));
        if (_window.AnimationsEnabled)
        {
            host.Height = host.ActualHeight;
            storyboard.Children.Add(SplineTo(host, nameof(Height), 0, RowDuration, RowCurve()));
        }

        storyboard.Completed += (_, _) =>
        {
            _ = RowsPanel.Children.Remove(host);
            UpdateGaps();
        };
        storyboard.Begin();
    }
}
