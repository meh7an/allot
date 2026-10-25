using System.Numerics;

using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Hosting;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Animation;

using Windows.Foundation;
using Windows.UI.ViewManagement;

namespace Allot.Tray.Flyout;

/// <summary>
/// A meter's bullet bar. <see cref="Used"/> slides the fill over 600 ms, as the
/// prototype's transition does; <see cref="Reserved"/> places the tick and
/// <see cref="Headroom"/> where the hatching starts, all as fractions of the bar.
/// </summary>
public sealed partial class BulletBar : UserControl
{
    public static readonly DependencyProperty UsedProperty = DependencyProperty.Register(
        nameof(Used),
        typeof(double),
        typeof(BulletBar),
        new PropertyMetadata(0.0, (sender, _) => ((BulletBar)sender).FillTo(animate: true)));

    public static readonly DependencyProperty ReservedProperty = DependencyProperty.Register(
        nameof(Reserved),
        typeof(double),
        typeof(BulletBar),
        new PropertyMetadata(0.0, (sender, _) => ((BulletBar)sender).PlaceTick()));

    public static readonly DependencyProperty HasReservedProperty = DependencyProperty.Register(
        nameof(HasReserved),
        typeof(bool),
        typeof(BulletBar),
        new PropertyMetadata(false, (sender, _) => ((BulletBar)sender).PlaceTick()));

    public static readonly DependencyProperty HeadroomProperty = DependencyProperty.Register(
        nameof(Headroom),
        typeof(double),
        typeof(BulletBar),
        new PropertyMetadata(0.85, (sender, _) => ((BulletBar)sender).Layout()));

    private static readonly TimeSpan FillDuration = TimeSpan.FromMilliseconds(600);
    private static readonly UISettings Settings = new();

    private const float Radius = 2;
    private const double TickWidth = 4;

    // repeating-linear-gradient(135deg, strong 0 1.5px, weak 1.5px 4px): a 4 px period across
    // the stripes is 4 * sqrt(2) along the bar.
    private static readonly double StripePitch = 4 * Math.Sqrt(2);

    private readonly Storyboard _fill = new();
    // A render transform animates on the compositor: no layout pass and no UI-thread frames on
    // every poll, unlike the width it replaced.
    private readonly DoubleAnimation _shift = new() { Duration = FillDuration };

    public BulletBar()
    {
        InitializeComponent();
        Storyboard.SetTarget(_shift, FillShift);
        Storyboard.SetTargetProperty(_shift, nameof(TranslateTransform.X));
        _fill.Children.Add(_shift);
        Bar.SizeChanged += (_, _) => Layout();
    }

    public double Used
    {
        get => (double)GetValue(UsedProperty);
        set => SetValue(UsedProperty, value);
    }

    public double Reserved
    {
        get => (double)GetValue(ReservedProperty);
        set => SetValue(ReservedProperty, value);
    }

    public bool HasReserved
    {
        get => (bool)GetValue(HasReservedProperty);
        set => SetValue(HasReservedProperty, value);
    }

    public double Headroom
    {
        get => (double)GetValue(HeadroomProperty);
        set => SetValue(HeadroomProperty, value);
    }

    private void Layout()
    {
        var size = new Size(Bar.ActualWidth, Bar.ActualHeight);
        ClipToCorners(size);
        var hatch = Math.Max(0, size.Width * (1 - Headroom));
        Hatch.Width = hatch;
        Stripes.Data = StripesFor(new Size(hatch, size.Height));
        Stripes.Clip = new RectangleGeometry { Rect = new Rect(0, 0, hatch, size.Height) };
        Fill.Width = size.Width;
        FillTo(animate: false);
        PlaceTick();
    }

    // The first value, a resize, a value set while the window is hidden, and every value without
    // animations jump; later polls glide from where the fill is.
    private void FillTo(bool animate)
    {
        var shift = -(1 - Math.Clamp(Used, 0, 1)) * Bar.ActualWidth;
        if (!animate || !IsLoaded || !Settings.AnimationsEnabled
            || XamlRoot?.IsHostVisible != true)
        {
            _fill.Stop();
            FillShift.X = shift;
            return;
        }

        _shift.To = shift;
        _fill.Begin();
    }

    private void PlaceTick()
    {
        Tick.Visibility = HasReserved ? Visibility.Visible : Visibility.Collapsed;
        Canvas.SetLeft(Tick, (Math.Clamp(Reserved, 0, 1) * Bar.ActualWidth) - (TickWidth / 2));
    }

    private static GeometryGroup StripesFor(Size size)
    {
        var stripes = new GeometryGroup();
        for (var x = 0.0; x < size.Width + size.Height; x += StripePitch)
        {
            stripes.Children.Add(new LineGeometry
            {
                StartPoint = new Point(x, 0),
                EndPoint = new Point(x - size.Height, size.Height),
            });
        }

        return stripes;
    }

    // A XAML panel does not clip its children, so the track, hatching and fill share one
    // rounded clip on the composition visual.
    private void ClipToCorners(Size size)
    {
        var visual = ElementCompositionPreview.GetElementVisual(Bar);
        var compositor = visual.Compositor;
        var shape = compositor.CreateRoundedRectangleGeometry();
        shape.CornerRadius = new Vector2(Radius, Radius);
        shape.Size = new Vector2((float)size.Width, (float)size.Height);
        visual.Clip = compositor.CreateGeometricClip(shape);
    }
}
