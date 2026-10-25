using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

using Windows.Foundation;

namespace Allot.Tray.Flyout;

/// <summary>
/// Draws <see cref="Samples"/> (fractions, oldest first) into 24 fixed slots aligned to the
/// right, so a short history grows in from the right as samples arrive.
/// </summary>
public sealed partial class Sparkline : UserControl
{
    public static readonly DependencyProperty SamplesProperty = DependencyProperty.Register(
        nameof(Samples),
        typeof(IReadOnlyList<double>),
        typeof(Sparkline),
        new PropertyMetadata(null, (sender, _) => ((Sparkline)sender).Draw()));

    private const int Slots = 24;

    // The prototype draws into a 44 x 16 view box, from y 15 (0%) up to y 1 (100%).
    private const double ViewWidth = 44;
    private const double ViewHeight = 16;
    private const double ViewBottom = 15;
    private const double ViewRange = 14;

    public Sparkline() => InitializeComponent();

    public IReadOnlyList<double>? Samples
    {
        get => (IReadOnlyList<double>?)GetValue(SamplesProperty);
        set => SetValue(SamplesProperty, value);
    }

    private void Draw()
    {
        var shown = (Samples ?? []).TakeLast(Slots).ToList();
        var scaleX = Width / ViewWidth;
        var scaleY = Height / ViewHeight;
        var first = Slots - shown.Count;
        var points = new PointCollection();
        for (var i = 0; i < shown.Count; i++)
        {
            var x = (first + i) * ViewWidth / (Slots - 1);
            var y = ViewBottom - (shown[i] * ViewRange);
            points.Add(new Point(x * scaleX, y * scaleY));
        }

        Line.Points = points;
    }
}
