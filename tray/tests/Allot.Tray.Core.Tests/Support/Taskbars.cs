using Allot.Tray.Core.Flyout;

namespace Allot.Tray.Core.Tests.Support;

/// <summary>
/// A 48 px taskbar on each edge: on a 1920 x 1080 monitor at 100 %, and on a 2560 x 1440 one
/// at 150 %, where it is 72 px.
/// </summary>
internal static class Taskbars
{
    public static PixelRect Monitor(double scale) =>
        scale == 1.0 ? new PixelRect(0, 0, 1920, 1080) : new PixelRect(0, 0, 2560, 1440);

    public static PixelRect Bounds(TaskbarEdge edge, double scale)
    {
        var monitor = Monitor(scale);
        var bar = FlyoutPlacement.ToPixels(48, scale);
        return edge switch
        {
            TaskbarEdge.Top => monitor with { Height = bar },
            TaskbarEdge.Left => monitor with { Width = bar },
            TaskbarEdge.Right => monitor with { X = monitor.Right - bar, Width = bar },
            _ => monitor with { Y = monitor.Bottom - bar, Height = bar },
        };
    }

    public static PixelRect WorkArea(TaskbarEdge edge, double scale)
    {
        var monitor = Monitor(scale);
        var bar = FlyoutPlacement.ToPixels(48, scale);
        return edge switch
        {
            TaskbarEdge.Top => monitor with { Y = bar, Height = monitor.Height - bar },
            TaskbarEdge.Left => monitor with { X = bar, Width = monitor.Width - bar },
            TaskbarEdge.Right => monitor with { Width = monitor.Width - bar },
            _ => monitor with { Height = monitor.Height - bar },
        };
    }

    /// <summary>
    /// DataRow cannot carry a null int, so -1 stands for "no click yet".
    /// </summary>
    public static int? Icon(int y) => y < 0 ? null : y;
}
