using ResourcesHelper.Tray.Core.Flyout;

namespace ResourcesHelper.Tray.Core.Tests;

[TestClass]
public sealed class PinnedPlacementTests
{
    private static readonly PixelRect WorkArea = new(0, 0, 1920, 1032);

    [TestMethod]
    public void SnapsToTheBottomRightCornerWithinSixteenPixels()
    {
        // The corner target is 12 px in: x 1548, y 620 for a 360 x 400 flyout.
        var near = new PixelRect(1548 - 16, 620 + 10, 360, 400);

        var snapped = FlyoutPlacement.SnapToCorner(near, WorkArea, 1.0);

        Assert.AreEqual(new PixelRect(1548, 620, 360, 400), snapped);
    }

    [TestMethod]
    public void StaysPutSeventeenPixelsAway()
    {
        var far = new PixelRect(1548 - 17, 620, 360, 400);

        Assert.AreEqual(far, FlyoutPlacement.SnapToCorner(far, WorkArea, 1.0));
    }

    [TestMethod]
    public void NeedsBothAxesNearTheCorner()
    {
        var alongTheRightEdge = new PixelRect(1548, 300, 360, 400);

        Assert.AreEqual(
            alongTheRightEdge,
            FlyoutPlacement.SnapToCorner(alongTheRightEdge, WorkArea, 1.0));
    }

    [TestMethod]
    public void SnapsToTheTopLeftCornerOfAnOffsetMonitor()
    {
        var area = new PixelRect(-1920, 200, 1920, 1080);
        var near = new PixelRect(-1920 + 3, 200 + 20, 360, 400);

        var snapped = FlyoutPlacement.SnapToCorner(near, area, 1.0);

        Assert.AreEqual(new PixelRect(-1920 + 12, 200 + 12, 360, 400), snapped);
    }

    [TestMethod]
    public void ScalesTheMarginAndDistance()
    {
        // At 150% the margin is 18 px and the distance 24 px.
        var area = new PixelRect(0, 0, 2560, 1368);
        var near = new PixelRect(18 + 24, 18 - 24, 540, 600);

        var snapped = FlyoutPlacement.SnapToCorner(near, area, 1.5);

        Assert.AreEqual(new PixelRect(18, 18, 540, 600), snapped);
    }

    [TestMethod]
    public void PicksTheNearestCornerOnATinyWorkArea()
    {
        // 40 px taller than the flyout, so the top and bottom targets are 16 px apart.
        var area = new PixelRect(0, 0, 400, 440);
        var flyout = new PixelRect(12, 13, 360, 400);

        var snapped = FlyoutPlacement.SnapToCorner(flyout, area, 1.0);

        Assert.AreEqual(new PixelRect(12, 12, 360, 400), snapped);
    }

    [TestMethod]
    public void GrowsUpwardInTheLowerHalf()
    {
        var pinned = new PixelRect(1548, 620, 360, 400);

        var grown = FlyoutPlacement.Regrow(pinned, 500, WorkArea);

        Assert.AreEqual(new PixelRect(1548, 520, 360, 500), grown);
    }

    [TestMethod]
    public void GrowsDownwardInTheUpperHalf()
    {
        var pinned = new PixelRect(12, 12, 360, 400);

        var grown = FlyoutPlacement.Regrow(pinned, 500, WorkArea);

        Assert.AreEqual(new PixelRect(12, 12, 360, 500), grown);
    }

    [TestMethod]
    public void GrowingStaysInsideTheWorkArea()
    {
        var pinned = new PixelRect(12, 400, 360, 100);

        var grown = FlyoutPlacement.Regrow(pinned, 640, WorkArea);

        Assert.AreEqual(new PixelRect(12, 1032 - 640, 360, 640), grown);
    }
}
