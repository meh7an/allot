namespace ResourcesHelper.Tray.Core.Flyout;

/// <summary>
/// A CSS <c>cubic-bezier(x1, y1, x2, y2)</c> timing curve, for animations the window layer
/// drives frame by frame rather than through the compositor.
/// </summary>
public readonly record struct CubicBezier(double X1, double Y1, double X2, double Y2)
{
    /// <summary>
    /// The handoff's decelerate curve, used for opening the flyout.
    /// </summary>
    public static CubicBezier Decelerate { get; } = new(0.1, 0.9, 0.2, 1);

    /// <summary>
    /// Fluent's accelerate curve, used for closing: it leaves slowly and speeds up.
    /// </summary>
    public static CubicBezier Accelerate { get; } = new(0.7, 0, 1, 0.5);

    /// <summary>
    /// The eased progress for linear progress <paramref name="x"/>, both from 0 to 1.
    /// </summary>
    public double Ease(double x)
    {
        if (x <= 0)
        {
            return 0;
        }

        if (x >= 1)
        {
            return 1;
        }

        return Sample(Y1, Y2, ParameterFor(x));
    }

    // B(t) for one axis, with the end points fixed at 0 and 1.
    private static double Sample(double p1, double p2, double t)
    {
        var u = 1 - t;
        return (3 * u * u * t * p1) + (3 * u * t * t * p2) + (t * t * t);
    }

    private static double Slope(double p1, double p2, double t)
    {
        var u = 1 - t;
        return (3 * u * u * p1) + (6 * u * t * (p2 - p1)) + (3 * t * t * (1 - p2));
    }

    // Newton's method from x itself, then bisection where the slope is too flat to trust.
    private double ParameterFor(double x)
    {
        var t = x;
        for (var i = 0; i < 8; i++)
        {
            var error = Sample(X1, X2, t) - x;
            if (Math.Abs(error) < 1e-7)
            {
                return t;
            }

            var slope = Slope(X1, X2, t);
            if (Math.Abs(slope) < 1e-6)
            {
                break;
            }

            t -= error / slope;
        }

        double low = 0;
        double high = 1;
        t = x;
        for (var i = 0; i < 40; i++)
        {
            var value = Sample(X1, X2, t);
            if (Math.Abs(value - x) < 1e-7)
            {
                break;
            }

            if (value < x)
            {
                low = t;
            }
            else
            {
                high = t;
            }

            t = (low + high) / 2;
        }

        return t;
    }
}
