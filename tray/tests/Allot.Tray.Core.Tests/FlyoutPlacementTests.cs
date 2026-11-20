using Allot.Tray.Core.Flyout;
using Allot.Tray.Core.Tests.Support;

namespace Allot.Tray.Core.Tests;

[TestClass]
public sealed class FlyoutPlacementTests
{
    [TestMethod]
    [DataRow(TaskbarEdge.Bottom, 1.0, 1548, 620)]
    [DataRow(TaskbarEdge.Top, 1.0, 1548, 60)]
    [DataRow(TaskbarEdge.Left, 1.0, 60, 668)]
    [DataRow(TaskbarEdge.Right, 1.0, 1500, 668)]
    [DataRow(TaskbarEdge.Bottom, 1.5, 2002, 750)]
    [DataRow(TaskbarEdge.Top, 1.5, 2002, 90)]
    [DataRow(TaskbarEdge.Left, 1.5, 90, 822)]
    [DataRow(TaskbarEdge.Right, 1.5, 1930, 822)]
    public void SitsTwelveEffectivePixelsInFromTheTrayCorner(
        TaskbarEdge edge, double scale, int x, int y)
    {
        var rect = FlyoutPlacement.TrayCorner(Taskbars.WorkArea(edge, scale), edge, scale, 400);

        var size = new PixelRect(
            x, y, FlyoutPlacement.ToPixels(360, scale), FlyoutPlacement.ToPixels(400, scale));
        Assert.AreEqual(size, rect);
    }

    [TestMethod]
    [DataRow(TaskbarEdge.Left, 1.0, 100, 12)]
    [DataRow(TaskbarEdge.Right, 1.0, 100, 12)]
    [DataRow(TaskbarEdge.Left, 1.0, 900, 668)]
    [DataRow(TaskbarEdge.Right, 1.0, 900, 668)]
    [DataRow(TaskbarEdge.Left, 1.5, 300, 18)]
    [DataRow(TaskbarEdge.Right, 1.5, 300, 18)]
    [DataRow(TaskbarEdge.Left, 1.5, 1300, 822)]
    [DataRow(TaskbarEdge.Right, 1.5, 1300, 822)]
    public void OpensAtTheEndOfASideTaskbarThatHoldsTheIcon(
        TaskbarEdge edge, double scale, int iconY, int y)
    {
        var rect = FlyoutPlacement.TrayCorner(
            Taskbars.WorkArea(edge, scale), edge, scale, 400, iconY);

        Assert.AreEqual(y, rect.Y);
    }

    [TestMethod]
    [DataRow(TaskbarEdge.Bottom, 10, 620)]
    [DataRow(TaskbarEdge.Top, 1000, 60)]
    public void TopAndBottomTaskbarsIgnoreTheIcon(TaskbarEdge edge, int iconY, int y)
    {
        var rect = FlyoutPlacement.TrayCorner(
            Taskbars.WorkArea(edge, 1.0), edge, 1.0, 400, iconY);

        Assert.AreEqual(y, rect.Y);
    }

    [TestMethod]
    public void UsesWorkAreaOffsetOfNonOriginMonitor()
    {
        var workArea = new PixelRect(-1920, 200, 1920, 1032);

        var rect = FlyoutPlacement.TrayCorner(workArea, TaskbarEdge.Bottom, 1.0, 300);

        Assert.AreEqual(new PixelRect(-12 - 360, 200 + 1032 - 12 - 300, 360, 300), rect);
    }

    [TestMethod]
    public void UsesWorkAreaOffsetOfNonOriginMonitorWithALeftTaskbar()
    {
        var workArea = new PixelRect(-1920 + 48, 200, 1920 - 48, 1080);

        var rect = FlyoutPlacement.TrayCorner(workArea, TaskbarEdge.Left, 1.0, 300, 250);

        Assert.AreEqual(new PixelRect(-1920 + 48 + 12, 200 + 12, 360, 300), rect);
    }

    [TestMethod]
    [DataRow(TaskbarEdge.Bottom, 1.0, 1548, 1032)]
    [DataRow(TaskbarEdge.Top, 1.0, 1548, 48 - 400)]
    [DataRow(TaskbarEdge.Left, 1.0, 48 - 360, 668)]
    [DataRow(TaskbarEdge.Right, 1.0, 1872, 668)]
    [DataRow(TaskbarEdge.Bottom, 1.5, 2002, 1368)]
    [DataRow(TaskbarEdge.Top, 1.5, 2002, 72 - 600)]
    [DataRow(TaskbarEdge.Left, 1.5, 72 - 540, 822)]
    [DataRow(TaskbarEdge.Right, 1.5, 2488, 822)]
    public void SlidesFromJustBehindTheTaskbar(TaskbarEdge edge, double scale, int x, int y)
    {
        var workArea = Taskbars.WorkArea(edge, scale);
        var target = FlyoutPlacement.TrayCorner(workArea, edge, scale, 400);

        var origin = FlyoutPlacement.SlideOrigin(target, workArea, edge);

        Assert.AreEqual(target with { X = x, Y = y }, origin);
    }

    [TestMethod]
    [DataRow(TaskbarEdge.Bottom, 1.0, 1548, 1080, 360, 352)]
    [DataRow(TaskbarEdge.Top, 1.0, 1548, -352, 360, 352)]
    [DataRow(TaskbarEdge.Left, 1.0, -312, 668, 312, 400)]
    [DataRow(TaskbarEdge.Right, 1.0, 1920, 668, 312, 400)]
    [DataRow(TaskbarEdge.Bottom, 1.5, 2002, 1440, 540, 528)]
    [DataRow(TaskbarEdge.Top, 1.5, 2002, -528, 540, 528)]
    [DataRow(TaskbarEdge.Left, 1.5, -468, 822, 468, 600)]
    [DataRow(TaskbarEdge.Right, 1.5, 2560, 822, 468, 600)]
    public void TheSlideOriginOverhangsTheMonitorOnTheTaskbarSide(
        TaskbarEdge edge, double scale, int x, int y, int width, int height)
    {
        var workArea = Taskbars.WorkArea(edge, scale);
        var target = FlyoutPlacement.TrayCorner(workArea, edge, scale, 400);
        var origin = FlyoutPlacement.SlideOrigin(target, workArea, edge);

        var overhang = FlyoutPlacement.Overhang(origin, Taskbars.Monitor(scale), edge);

        Assert.AreEqual(new PixelRect(x, y, width, height), overhang);
    }

    [TestMethod]
    public void AFlyoutShorterThanTheTaskbarHasNoOverhang()
    {
        var origin = new PixelRect(1548, 1032, 360, 40);

        Assert.IsNull(FlyoutPlacement.Overhang(origin, Taskbars.Monitor(1.0), TaskbarEdge.Bottom));
    }

    [TestMethod]
    [DataRow(TaskbarEdge.Top, -1, true)]
    [DataRow(TaskbarEdge.Bottom, 10, false)]
    [DataRow(TaskbarEdge.Left, -1, false)]
    [DataRow(TaskbarEdge.Left, 539, true)]
    [DataRow(TaskbarEdge.Right, 540, false)]
    public void OpensAtTopFollowsTheEdgeAndTheIcon(TaskbarEdge edge, int iconY, bool atTop) =>
        Assert.AreEqual(
            atTop,
            FlyoutPlacement.OpensAtTop(edge, Taskbars.WorkArea(edge, 1.0), Taskbars.Icon(iconY)));

    [TestMethod]
    public void CapsHeightAtSixHundredForty() =>
        Assert.AreEqual(640, FlyoutPlacement.ClampHeight(900, 1400));

    [TestMethod]
    public void CapsHeightAtSeventyPercentOfShortWorkArea() =>
        Assert.AreEqual(560, FlyoutPlacement.ClampHeight(900, 800), 0.001);

    [TestMethod]
    public void KeepsContentHeightWhenItFits() =>
        Assert.AreEqual(320, FlyoutPlacement.ClampHeight(320, 1032));

    [TestMethod]
    [DataRow(TaskbarEdge.Bottom, 638.4)]
    [DataRow(TaskbarEdge.Top, 638.4)]
    [DataRow(TaskbarEdge.Left, 640.0)]
    [DataRow(TaskbarEdge.Right, 640.0)]
    public void ClampsHeightInEffectivePixelsOfTheWorkArea(TaskbarEdge edge, double height)
    {
        // A bottom or top taskbar leaves 1368 physical px, 912 effective at 150 %, and 70 % of
        // that (638.4) is below 640; a side taskbar leaves the full 960.
        var rect = FlyoutPlacement.TrayCorner(Taskbars.WorkArea(edge, 1.5), edge, 1.5, 900);

        Assert.AreEqual(FlyoutPlacement.ToPixels(height, 1.5), rect.Height);
    }
}
