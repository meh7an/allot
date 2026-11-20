using Allot.Tray.Core.Flyout;
using Allot.Tray.Interop;

namespace Allot.Tray.TrayIcon;

/// <summary>
/// Reads where the taskbar holding the tray is. Windows 11 can move it to any edge while the
/// app runs, so callers read it again each time they place something next to it.
/// </summary>
internal static unsafe class Taskbar
{
    /// <summary>
    /// The taskbar's edge, rectangle and monitor now. When Windows reports no taskbar, the
    /// primary monitor and its work area stand in, and the edge comes from the work area.
    /// </summary>
    public static TaskbarPlace Read()
    {
        // ABM_GETTASKBARPOS documents only rc; the edge comes from that rectangle instead of
        // uEdge, which the documentation does not promise to fill.
        var data = new NativeMethods.AppBarData { Size = (uint)sizeof(NativeMethods.AppBarData) };
        PixelRect? bounds =
            NativeMethods.SHAppBarMessage(NativeMethods.AbmGetTaskbarPos, ref data) != 0
                ? ToPixelRect(data.Rect)
                : null;

        var monitor = bounds is not null
            ? NativeMethods.MonitorFromRect(data.Rect, NativeMethods.MonitorDefaultToPrimary)
            : NativeMethods.MonitorFromPoint(default, NativeMethods.MonitorDefaultToPrimary);
        var info = new NativeMethods.MonitorInfo { Size = (uint)sizeof(NativeMethods.MonitorInfo) };
        var (monitorBounds, workArea) = NativeMethods.GetMonitorInfo(monitor, ref info)
            ? (ToPixelRect(info.Monitor), ToPixelRect(info.Work))
            : PrimaryArea();
        return new TaskbarPlace(
            TaskbarEdges.Resolve(bounds, workArea, monitorBounds),
            bounds,
            monitorBounds,
            workArea,
            ScaleOf(monitor));
    }

    public static double ScaleOf(nint monitor)
    {
        var hr = NativeMethods.GetDpiForMonitor(
            monitor, NativeMethods.MdtEffectiveDpi, out var dpi, out _);
        return hr == 0 ? dpi / 96.0 : 1.0;
    }

    public static NativeMethods.NativeRect ToNativeRect(PixelRect rect) =>
        new() { Left = rect.X, Top = rect.Y, Right = rect.Right, Bottom = rect.Bottom };

    private static (PixelRect Monitor, PixelRect WorkArea) PrimaryArea()
    {
        var primary = Microsoft.UI.Windowing.DisplayArea.Primary;
        return (ToPixelRect(primary.OuterBounds), ToPixelRect(primary.WorkArea));
    }

    private static PixelRect ToPixelRect(NativeMethods.NativeRect rect) =>
        new(rect.Left, rect.Top, rect.Right - rect.Left, rect.Bottom - rect.Top);

    private static PixelRect ToPixelRect(Windows.Graphics.RectInt32 rect) =>
        new(rect.X, rect.Y, rect.Width, rect.Height);
}
