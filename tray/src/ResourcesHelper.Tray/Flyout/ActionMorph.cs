using System.Runtime.CompilerServices;

using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media.Animation;

using Windows.Foundation;
using Windows.UI.ViewManagement;

namespace ResourcesHelper.Tray.Flyout;

/// <summary>
/// Shows and hides a card's action buttons with motion instead of a jump. A button with
/// <c>GrowsFrom</c> set (Kill?) grows out of that width at its right edge while it fades in, and
/// shrinks back into it as it fades out; one with <c>Slides</c> (the spacer) grows from and
/// shrinks to nothing, so what stands beside it slides; one with neither (Kill) only fades. With
/// reduced motion all only fade, and while the flyout is hidden or not yet loaded they switch at
/// once. Set
/// <c>IsShown</c> from x:Bind; this class owns the element's Visibility, which starts Collapsed.
/// </summary>
public static class ActionMorph
{
    public static readonly DependencyProperty IsShownProperty =
        DependencyProperty.RegisterAttached(
            "IsShown",
            typeof(bool),
            typeof(ActionMorph),
            new PropertyMetadata(false, OnIsShownChanged));

    public static readonly DependencyProperty GrowsFromProperty =
        DependencyProperty.RegisterAttached(
            "GrowsFrom",
            typeof(double),
            typeof(ActionMorph),
            new PropertyMetadata(0.0));

    public static readonly DependencyProperty SlidesProperty =
        DependencyProperty.RegisterAttached(
            "Slides",
            typeof(bool),
            typeof(ActionMorph),
            new PropertyMetadata(false));

    private static readonly TimeSpan ShowDuration = TimeSpan.FromMilliseconds(167);
    private static readonly TimeSpan HideDuration = TimeSpan.FromMilliseconds(120);
    private static readonly UISettings Settings = new();

    // The running show or hide of each element, so a reversal starts from where it is.
    private static readonly ConditionalWeakTable<FrameworkElement, Storyboard> Running = [];

    public static bool GetIsShown(DependencyObject element) =>
        (bool)element.GetValue(IsShownProperty);

    public static void SetIsShown(DependencyObject element, bool value) =>
        element.SetValue(IsShownProperty, value);

    public static bool GetSlides(DependencyObject element) =>
        (bool)element.GetValue(SlidesProperty);

    public static void SetSlides(DependencyObject element, bool value) =>
        element.SetValue(SlidesProperty, value);

    public static double GetGrowsFrom(DependencyObject element) =>
        (double)element.GetValue(GrowsFromProperty);

    public static void SetGrowsFrom(DependencyObject element, double value) =>
        element.SetValue(GrowsFromProperty, value);

    private static void OnIsShownChanged(
        DependencyObject sender,
        DependencyPropertyChangedEventArgs e)
    {
        var element = (FrameworkElement)sender;
        var show = (bool)e.NewValue;

        // Where a running show or hide has got to, so a reversal turns around from there.
        var shown = element.Visibility == Visibility.Visible;
        var width = shown ? element.ActualWidth : 0;
        var opacity = shown ? element.Opacity : 0;
        if (Running.TryGetValue(element, out var running))
        {
            _ = Running.Remove(element);
            running.Stop();
        }

        if (!element.IsLoaded || element.XamlRoot?.IsHostVisible != true)
        {
            Settle(element, show);
            return;
        }

        Settle(element, true);
        var natural = Natural(element);
        var duration = show ? ShowDuration : HideDuration;
        var storyboard = new Storyboard();
        storyboard.Children.Add(Fade(element, opacity, show ? 1 : 0, duration));
        var small = GetSlides(element) ? 0 : GetGrowsFrom(element);
        if ((small > 0 || GetSlides(element)) && Settings.AnimationsEnabled)
        {
            var start = width > 0 ? width : (show ? small : natural);
            element.Width = start;
            storyboard.Children.Add(Grow(element, start, show ? natural : small, duration, show));
        }

        element.Opacity = opacity;
        storyboard.Completed += (_, _) =>
        {
            if (Running.TryGetValue(element, out var current) && current == storyboard)
            {
                _ = Running.Remove(element);
                storyboard.Stop();
                Settle(element, show);
            }
        };
        Running.AddOrUpdate(element, storyboard);
        storyboard.Begin();
    }

    // The end state of a show or hide, with no animated values left behind.
    private static void Settle(FrameworkElement element, bool show)
    {
        element.ClearValue(FrameworkElement.WidthProperty);
        element.Opacity = 1;
        element.Visibility = show ? Visibility.Visible : Visibility.Collapsed;
    }

    private static double Natural(FrameworkElement element)
    {
        element.Measure(new Size(double.PositiveInfinity, double.PositiveInfinity));
        return element.DesiredSize.Width;
    }

    private static DoubleAnimation Fade(
        DependencyObject target,
        double from,
        double to,
        TimeSpan duration)
    {
        var animation = new DoubleAnimation { From = from, To = to, Duration = duration };
        Storyboard.SetTarget(animation, target);
        Storyboard.SetTargetProperty(animation, nameof(UIElement.Opacity));
        return animation;
    }

    // Fluent's decelerate curve to grow, accelerate to shrink.
    private static DoubleAnimationUsingKeyFrames Grow(
        DependencyObject target,
        double from,
        double to,
        TimeSpan duration,
        bool show)
    {
        var curve = show
            ? new KeySpline { ControlPoint1 = new(0.1, 0.9), ControlPoint2 = new(0.2, 1) }
            : new KeySpline { ControlPoint1 = new(0.7, 0), ControlPoint2 = new(1, 0.5) };
        var animation = new DoubleAnimationUsingKeyFrames { EnableDependentAnimation = true };
        animation.KeyFrames.Add(new DiscreteDoubleKeyFrame { KeyTime = TimeSpan.Zero, Value = from });
        animation.KeyFrames.Add(new SplineDoubleKeyFrame
        {
            KeyTime = KeyTime.FromTimeSpan(duration),
            Value = to,
            KeySpline = curve,
        });
        Storyboard.SetTarget(animation, target);
        Storyboard.SetTargetProperty(animation, nameof(FrameworkElement.Width));
        return animation;
    }
}
