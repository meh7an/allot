using System.Numerics;

using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Hosting;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Animation;

using ResourcesHelper.Tray.Core.ViewModels;

using Windows.Foundation;
using Windows.UI.ViewManagement;

namespace ResourcesHelper.Tray.Flyout;

/// <summary>
/// A job card's edge bar. <see cref="Fraction"/> scales the fill, so a tick animates on the
/// composition thread without a layout pass; <see cref="Tone"/> picks its colour or the dashes.
/// </summary>
public sealed partial class EdgeBar : UserControl
{
    public static readonly DependencyProperty FractionProperty = DependencyProperty.Register(
        nameof(Fraction),
        typeof(double),
        typeof(EdgeBar),
        new PropertyMetadata(0.0, OnFractionChanged));

    public static readonly DependencyProperty ToneProperty = DependencyProperty.Register(
        nameof(Tone),
        typeof(EdgeBarTone),
        typeof(EdgeBar),
        new PropertyMetadata(EdgeBarTone.Running, OnToneChanged));

    private static readonly TimeSpan TickDuration = TimeSpan.FromMilliseconds(900);
    private static readonly UISettings Settings = new();

    // The card's inner corner radius: 6 outside its 1 px border.
    private const float InnerRadius = 5;
    private const double DashWidth = 6;
    private const double DashPitch = 12;

    private readonly Storyboard _tick = new();
    private readonly DoubleAnimation _scale = new() { Duration = TickDuration };

    public EdgeBar()
    {
        InitializeComponent();
        Storyboard.SetTarget(_scale, FillScale);
        Storyboard.SetTargetProperty(_scale, nameof(ScaleTransform.ScaleX));
        _tick.Children.Add(_scale);
        SizeChanged += OnSizeChanged;
    }

    /// <summary>
    /// How much of the bar is filled, from 0 to 1.
    /// </summary>
    public double Fraction
    {
        get => (double)GetValue(FractionProperty);
        set => SetValue(FractionProperty, value);
    }

    public EdgeBarTone Tone
    {
        get => (EdgeBarTone)GetValue(ToneProperty);
        set => SetValue(ToneProperty, value);
    }

    private static void OnFractionChanged(
        DependencyObject sender,
        DependencyPropertyChangedEventArgs e) =>
        ((EdgeBar)sender).ScaleTo((double)e.NewValue);

    private static void OnToneChanged(
        DependencyObject sender,
        DependencyPropertyChangedEventArgs e) =>
        ((EdgeBar)sender).ShowTone((EdgeBarTone)e.NewValue);

    // The first value and every value without animations jump; later ticks glide from where the
    // bar is, so a new poll mid-animation continues smoothly.
    private void ScaleTo(double fraction)
    {
        if (!IsLoaded || !Settings.AnimationsEnabled)
        {
            _tick.Stop();
            FillScale.ScaleX = fraction;
            return;
        }

        _scale.To = fraction;
        _tick.Begin();
    }

    private void ShowTone(EdgeBarTone tone)
    {
        var dashed = tone == EdgeBarTone.WaitingDashed;
        Dashes.Visibility = dashed ? Visibility.Visible : Visibility.Collapsed;
        Fill.Visibility = dashed ? Visibility.Collapsed : Visibility.Visible;
        RunningFill.Visibility = Shown(tone == EdgeBarTone.Running);
        WaitingFill.Visibility = Shown(tone is EdgeBarTone.Waiting or EdgeBarTone.Over);
        FrozenFill.Visibility = Shown(tone == EdgeBarTone.Frozen);
    }

    private static Visibility Shown(bool shown) =>
        shown ? Visibility.Visible : Visibility.Collapsed;

    private void OnSizeChanged(object sender, SizeChangedEventArgs e)
    {
        var size = e.NewSize;
        ClipToCorners(size);

        var dashes = new GeometryGroup();
        for (double x = 0; x < size.Width; x += DashPitch)
        {
            dashes.Children.Add(new RectangleGeometry
            {
                Rect = new Rect(x, 0, Math.Min(DashWidth, size.Width - x), size.Height),
            });
        }

        DashPath.Data = dashes;
    }

    // The card clips its content to its rounded corners in the handoff; a XAML Border does not
    // clip its children, so the bar takes a rounded clip that reaches above it, leaving only the
    // bottom corners rounded.
    private void ClipToCorners(Size size)
    {
        var visual = ElementCompositionPreview.GetElementVisual(this);
        var compositor = visual.Compositor;
        var shape = compositor.CreateRoundedRectangleGeometry();
        shape.CornerRadius = new Vector2(InnerRadius, InnerRadius);
        shape.Offset = new Vector2(0, -InnerRadius);
        shape.Size = new Vector2((float)size.Width, (float)size.Height + InnerRadius);
        visual.Clip = compositor.CreateGeometricClip(shape);
    }
}
