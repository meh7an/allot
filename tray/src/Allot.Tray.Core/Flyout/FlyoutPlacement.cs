namespace Allot.Tray.Core.Flyout;

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
    /// The tray-corner rectangle, 12 px in from the work area's corner next to the tray: the
    /// right end of a bottom or top taskbar, and the end of a side taskbar that holds the icon.
    /// </summary>
    /// <param name="workArea">The work area of the taskbar's monitor.</param>
    /// <param name="edge">The edge the taskbar is on.</param>
    /// <param name="scale">That monitor's scale.</param>
    /// <param name="contentHeight">The content's height in effective pixels.</param>
    /// <param name="iconY">Where the tray icon was last clicked, if it was.</param>
    public static PixelRect TrayCorner(
        PixelRect workArea,
        TaskbarEdge edge,
        double scale,
        double contentHeight,
        int? iconY = null)
    {
        var height = ClampHeight(contentHeight, workArea.Height / scale);
        var widthPx = ToPixels(Width, scale);
        var heightPx = ToPixels(height, scale);
        var marginPx = ToPixels(EdgeMargin, scale);

        var x = edge == TaskbarEdge.Left
            ? workArea.X + marginPx
            : workArea.Right - marginPx - widthPx;
        var y = OpensAtTop(edge, workArea, iconY)
            ? workArea.Y + marginPx
            : workArea.Bottom - marginPx - heightPx;
        return new PixelRect(x, y, widthPx, heightPx);
    }

    /// <summary>
    /// Whether the flyout and the tray menu open at the top: always for a top taskbar, never
    /// for a bottom one, and for a side taskbar when the icon is in the upper half of the work
    /// area. Where this Windows build puts the tray on a side taskbar is not documented, so the
    /// icon decides; with no click yet, the bottom end, where Windows 10 put it.
    /// </summary>
    public static bool OpensAtTop(TaskbarEdge edge, PixelRect workArea, int? iconY) =>
        edge switch
        {
            TaskbarEdge.Top => true,
            TaskbarEdge.Bottom => false,
            _ => iconY is { } y && 2 * y < (2 * workArea.Y) + workArea.Height,
        };

    /// <summary>
    /// Where the open slide starts and the close slide ends: just past the work area's edge on
    /// the taskbar's side, so the topmost taskbar covers the flyout there.
    /// </summary>
    public static PixelRect SlideOrigin(PixelRect target, PixelRect workArea, TaskbarEdge edge) =>
        edge switch
        {
            TaskbarEdge.Top => target with { Y = workArea.Y - target.Height },
            TaskbarEdge.Left => target with { X = workArea.X - target.Width },
            TaskbarEdge.Right => target with { X = workArea.Right },
            _ => target with { Y = workArea.Bottom },
        };

    /// <summary>
    /// The part of a slide origin that reaches past the monitor on the taskbar's side. A
    /// monitor there would show the flyout during the slide, so it fades instead.
    /// </summary>
    /// <returns>Null when the origin stays on its monitor.</returns>
    public static PixelRect? Overhang(PixelRect origin, PixelRect monitor, TaskbarEdge edge)
    {
        var overhang = edge switch
        {
            TaskbarEdge.Top => origin with { Height = monitor.Y - origin.Y },
            TaskbarEdge.Left => origin with { Width = monitor.X - origin.X },
            TaskbarEdge.Right => origin with
            {
                X = monitor.Right,
                Width = origin.Right - monitor.Right,
            },
            _ => origin with { Y = monitor.Bottom, Height = origin.Bottom - monitor.Bottom },
        };
        return overhang.Width > 0 && overhang.Height > 0 ? overhang : null;
    }

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
