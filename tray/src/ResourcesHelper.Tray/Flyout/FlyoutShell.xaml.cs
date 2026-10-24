using System.Collections.Specialized;

using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Animation;

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

    public FlyoutModel Model { get; }

    public double DesiredHeight => EdgeHeight + Header.ActualHeight + RowsPanel.ActualHeight;

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

    private void OnRowsSizeChanged(object sender, SizeChangedEventArgs e) => ReportHeight();

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
