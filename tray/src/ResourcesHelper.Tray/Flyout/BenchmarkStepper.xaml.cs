using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Animation;

using ResourcesHelper.Tray.Core.ViewModels;

using Windows.Foundation;
using Windows.UI.ViewManagement;

namespace ResourcesHelper.Tray.Flyout;

/// <summary>
/// A benchmark card's stepper. <see cref="Step"/> picks the visual state, <see cref="Ticks"/>
/// fills the seconds of quiet, and <see cref="RunFraction"/> scales the Run fill, animated like
/// the edge bar.
/// </summary>
public sealed partial class BenchmarkStepper : UserControl
{
    public static readonly DependencyProperty StepProperty = DependencyProperty.Register(
        nameof(Step),
        typeof(BenchmarkStep),
        typeof(BenchmarkStepper),
        new PropertyMetadata(BenchmarkStep.Drain, OnStepChanged));

    public static readonly DependencyProperty TicksProperty = DependencyProperty.Register(
        nameof(Ticks),
        typeof(int),
        typeof(BenchmarkStepper),
        new PropertyMetadata(0, OnTicksChanged));

    public static readonly DependencyProperty RunFractionProperty = DependencyProperty.Register(
        nameof(RunFraction),
        typeof(double),
        typeof(BenchmarkStepper),
        new PropertyMetadata(0.0, OnRunFractionChanged));

    // The prototype's Run fill eases its width over 700 ms.
    private static readonly TimeSpan RunDuration = TimeSpan.FromMilliseconds(700);
    private static readonly UISettings Settings = new();

    private const double TickGap = 2;

    private readonly Storyboard _run = new();
    private readonly DoubleAnimation _scale = new() { Duration = RunDuration };

    public BenchmarkStepper()
    {
        InitializeComponent();
        Storyboard.SetTarget(_scale, RunScale);
        Storyboard.SetTargetProperty(_scale, nameof(ScaleTransform.ScaleX));
        _run.Children.Add(_scale);
        Loaded += OnLoaded;
        ShowStep();
    }

    public BenchmarkStep Step
    {
        get => (BenchmarkStep)GetValue(StepProperty);
        set => SetValue(StepProperty, value);
    }

    /// <summary>
    /// Filled quiet ticks, out of <see cref="BenchmarkSteps.TickCount"/>; shown only while
    /// <see cref="Step"/> is Quiet.
    /// </summary>
    public int Ticks
    {
        get => (int)GetValue(TicksProperty);
        set => SetValue(TicksProperty, value);
    }

    /// <summary>
    /// How much of the Run segment is filled, from 0 to 1.
    /// </summary>
    public double RunFraction
    {
        get => (double)GetValue(RunFractionProperty);
        set => SetValue(RunFractionProperty, value);
    }

    private static void OnStepChanged(
        DependencyObject sender,
        DependencyPropertyChangedEventArgs e) =>
        ((BenchmarkStepper)sender).ShowStep();

    private static void OnTicksChanged(
        DependencyObject sender,
        DependencyPropertyChangedEventArgs e) =>
        ((BenchmarkStepper)sender).ShowTicks();

    private static void OnRunFractionChanged(
        DependencyObject sender,
        DependencyPropertyChangedEventArgs e) =>
        ((BenchmarkStepper)sender).ScaleRun((double)e.NewValue);

    // Each tick layer is one Path whose fill is a theme resource in the XAML, so a theme change
    // recolours it; the code only lays out the ticks.
    private static GeometryGroup TicksOf(double width, double height, int count)
    {
        var tick = (width - TickGap * (BenchmarkSteps.TickCount - 1)) / BenchmarkSteps.TickCount;
        var ticks = new GeometryGroup();
        for (var i = 0; i < count && tick > 0; i++)
        {
            ticks.Children.Add(new RectangleGeometry
            {
                Rect = new Rect(i * (tick + TickGap), 0, tick, height),
            });
        }

        return ticks;
    }

    // A state set before the control is in the tree may not apply; set it again once it is.
    private void OnLoaded(object sender, RoutedEventArgs e) => ShowStep();

    private void OnTickRowSizeChanged(object sender, SizeChangedEventArgs e)
    {
        UpcomingTicks.Data = TicksOf(e.NewSize.Width, e.NewSize.Height, BenchmarkSteps.TickCount);
        DoneTicks.Data = TicksOf(e.NewSize.Width, e.NewSize.Height, BenchmarkSteps.TickCount);
        ShowTicks();
    }

    private void ShowStep()
    {
        _ = VisualStateManager.GoToState(this, Step.ToString(), false);
        ShowTicks();
    }

    private void ShowTicks()
    {
        var filled = Step == BenchmarkStep.Quiet
            ? Math.Clamp(Ticks, 0, BenchmarkSteps.TickCount)
            : 0;
        QuietTicks.Data = TicksOf(TickRow.ActualWidth, TickRow.ActualHeight, filled);
    }

    // The first value and every value without animations jump; later ones glide.
    private void ScaleRun(double fraction)
    {
        if (!IsLoaded || !Settings.AnimationsEnabled)
        {
            _run.Stop();
            RunScale.ScaleX = fraction;
            return;
        }

        _scale.To = fraction;
        _run.Begin();
    }
}
