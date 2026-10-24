namespace ResourcesHelper.Tray.Core.Flyout;

/// <summary>
/// Where the flyout sits next to the taskbar. Sizes are effective pixels; the result is in
/// physical pixels for the monitor whose scale is given.
/// </summary>
public static class FlyoutPlacement
{
    public const double Width = 360;
    public const double MaxHeight = 640;
    public const double MaxWorkAreaFraction = 0.7;
    public const double EdgeMargin = 12;

    /// <summary>
    /// How close a pinned flyout must be dragged to a corner to snap to it, in effective pixels.
    /// </summary>
    public const double SnapDistance = 16;

    /// <summary>
    /// The flyout height for its content: up to 640, or 70% of the work area if that is smaller.
    /// </summary>
    public static double ClampHeight(double contentHeight, double workAreaHeight) =>
        Math.Min(contentHeight, Math.Min(MaxHeight, workAreaHeight * MaxWorkAreaFraction));

    /// <summary>
    /// The tray-corner rectangle: 12 px from the right and bottom of the work area, or from the
    /// right and top when the taskbar sits at the top of the monitor.
    /// </summary>
    public static PixelRect TrayCorner(
        PixelRect workArea,
        PixelRect monitorBounds,
        double scale,
        double contentHeight)
    {
        var height = ClampHeight(contentHeight, workArea.Height / scale);
        var widthPx = ToPixels(Width, scale);
        var heightPx = ToPixels(height, scale);
        var marginPx = ToPixels(EdgeMargin, scale);

        var x = workArea.Right - marginPx - widthPx;
        var y = IsTaskbarAtTop(workArea, monitorBounds)
            ? workArea.Y + marginPx
            : workArea.Bottom - marginPx - heightPx;
        return new PixelRect(x, y, widthPx, heightPx);
    }

    /// <summary>
    /// The work area starts below the monitor's top edge only when an appbar (the taskbar, moved
    /// by a third-party tool) occupies the top. An auto-hide taskbar leaves them equal, which
    /// counts as bottom.
    /// </summary>
    public static bool IsTaskbarAtTop(PixelRect workArea, PixelRect monitorBounds) =>
        workArea.Y > monitorBounds.Y && workArea.Bottom == monitorBounds.Bottom;

    /// <summary>
    /// A pinned flyout dragged near a corner of its work area lands on that corner, 12 px in as
    /// at the tray corner. Each axis must be within 16 px of the corner; the nearest corner wins.
    /// </summary>
    /// <param name="visible">The proposed visible rectangle.</param>
    /// <param name="workArea">The work area of the monitor the flyout is on.</param>
    /// <param name="scale">That monitor's scale.</param>
    /// <returns>The snapped rectangle, or <paramref name="visible"/> when no corner is near.</returns>
    public static PixelRect SnapToCorner(PixelRect visible, PixelRect workArea, double scale)
    {
        var margin = ToPixels(EdgeMargin, scale);
        var threshold = ToPixels(SnapDistance, scale);
        int[] xs = [workArea.X + margin, workArea.Right - margin - visible.Width];
        int[] ys = [workArea.Y + margin, workArea.Bottom - margin - visible.Height];

        var best = visible;
        var bestDistance = int.MaxValue;
        foreach (var x in xs)
        {
            foreach (var y in ys)
            {
                var dx = Math.Abs(visible.X - x);
                var dy = Math.Abs(visible.Y - y);
                if (dx <= threshold && dy <= threshold && dx + dy < bestDistance)
                {
                    best = visible with { X = x, Y = y };
                    bestDistance = dx + dy;
                }
            }
        }

        return best;
    }

    /// <summary>
    /// A pinned flyout whose content changed height grows away from the nearer horizontal edge
    /// of its work area: one in the lower half keeps its bottom, one in the upper half its top.
    /// </summary>
    public static PixelRect Regrow(PixelRect visible, int heightPx, PixelRect workArea)
    {
        var center = visible.Y + (visible.Height / 2);
        var keepsBottom = center > workArea.Y + (workArea.Height / 2);
        var y = keepsBottom ? visible.Bottom - heightPx : visible.Y;
        return Within(visible with { Y = y, Height = heightPx }, workArea);
    }

    private static PixelRect Within(PixelRect rect, PixelRect area)
    {
        var x = Math.Clamp(rect.X, area.X, Math.Max(area.X, area.Right - rect.Width));
        var y = Math.Clamp(rect.Y, area.Y, Math.Max(area.Y, area.Bottom - rect.Height));
        return rect with { X = x, Y = y };
    }

    public static int ToPixels(double effective, double scale) =>
        (int)Math.Round(effective * scale, MidpointRounding.AwayFromZero);
}
