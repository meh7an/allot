namespace Allot.Tray.Core.Flyout;

/// <summary>
/// The screen edge the taskbar holding the tray is on. Windows 11 build 26300 lets the user
/// put it on any of the four.
/// </summary>
public enum TaskbarEdge
{
    Bottom,
    Top,
    Left,
    Right,
}

/// <summary>
/// Where the taskbar holding the tray is, read again at each flyout open or tray menu, in
/// physical pixels.
/// </summary>
/// <param name="Edge">The edge it is on.</param>
/// <param name="Bounds">Its rectangle, or null when Windows did not report it.</param>
/// <param name="Monitor">The bounds of the monitor it is on.</param>
/// <param name="WorkArea">That monitor's work area.</param>
/// <param name="Scale">That monitor's scale.</param>
public readonly record struct TaskbarPlace(
    TaskbarEdge Edge,
    PixelRect? Bounds,
    PixelRect Monitor,
    PixelRect WorkArea,
    double Scale);

public static class TaskbarEdges
{
    /// <summary>
    /// The taskbar's edge from what Windows reports: its rectangle against its monitor first,
    /// then the side the work area leaves free, and the bottom when neither tells.
    /// </summary>
    public static TaskbarEdge Resolve(PixelRect? taskbar, PixelRect workArea, PixelRect monitor) =>
        (taskbar is { } bounds ? FromBounds(bounds, monitor) : null)
        ?? FromWorkArea(workArea, monitor)
        ?? TaskbarEdge.Bottom;

    /// <summary>
    /// A wide taskbar is at the top or bottom and a tall one at a side, on the half of the
    /// monitor its centre is in. Only the shape and centre count, so an auto-hidden taskbar
    /// that reports its shown rectangle still reads right.
    /// </summary>
    /// <returns>Null for an empty rectangle.</returns>
    public static TaskbarEdge? FromBounds(PixelRect taskbar, PixelRect monitor)
    {
        if (taskbar.Width <= 0 || taskbar.Height <= 0)
        {
            return null;
        }

        // Doubled centres keep the comparison in integers.
        if (taskbar.Width >= taskbar.Height)
        {
            return (2 * taskbar.Y) + taskbar.Height < (2 * monitor.Y) + monitor.Height
                ? TaskbarEdge.Top
                : TaskbarEdge.Bottom;
        }

        return (2 * taskbar.X) + taskbar.Width < (2 * monitor.X) + monitor.Width
            ? TaskbarEdge.Left
            : TaskbarEdge.Right;
    }

    /// <summary>
    /// The one side where the work area stops short of the monitor.
    /// </summary>
    /// <returns>
    /// Null when no side does (an auto-hidden taskbar) or several do (other appbars too).
    /// </returns>
    public static TaskbarEdge? FromWorkArea(PixelRect workArea, PixelRect monitor)
    {
        var gaps = new List<TaskbarEdge>(4);
        if (workArea.Bottom < monitor.Bottom)
        {
            gaps.Add(TaskbarEdge.Bottom);
        }

        if (workArea.Y > monitor.Y)
        {
            gaps.Add(TaskbarEdge.Top);
        }

        if (workArea.X > monitor.X)
        {
            gaps.Add(TaskbarEdge.Left);
        }

        if (workArea.Right < monitor.Right)
        {
            gaps.Add(TaskbarEdge.Right);
        }

        return gaps.Count == 1 ? gaps[0] : null;
    }
}
