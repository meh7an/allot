using Allot.Tray.Core.Diagnostics;

namespace Allot.Tray.Core.Tests;

[TestClass]
public sealed class MeasureTests
{
    [TestMethod]
    public void NoFramesGiveZeros()
    {
        var stats = FrameStats.From([]);

        Assert.AreEqual(new FrameStats(0, 0, 0, 0, 0), stats);
    }

    [TestMethod]
    public void SteadyFramesDropNothing()
    {
        var stats = FrameStats.From([16.6, 16.7, 16.8, 16.7, 16.6]);

        Assert.AreEqual(5, stats.Frames);
        Assert.AreEqual(16.7, stats.MedianMs);
        Assert.AreEqual(16.8, stats.MaxMs);
        Assert.AreEqual(0, stats.Dropped);
    }

    [TestMethod]
    public void ALongFrameCountsTheFramesItMissed()
    {
        // Two periods missed one frame, three periods two.
        var stats = FrameStats.From([16.7, 33.4, 16.7, 50.0]);

        Assert.AreEqual(3, stats.Dropped);
        Assert.AreEqual(50.0, stats.MaxMs);
    }

    [TestMethod]
    public void FasterDisplaysDropNothing()
    {
        // 144 Hz frames are shorter than the 60 fps budget's period.
        var stats = FrameStats.From([6.9, 7.0, 6.9, 7.0]);

        Assert.AreEqual(0, stats.Dropped);
    }

    [TestMethod]
    public void ThePercentileIsTheNearestRank()
    {
        var intervals = Enumerable.Range(1, 20).Select(i => (double)i).ToList();

        var stats = FrameStats.From(intervals);

        Assert.AreEqual(10.0, stats.MedianMs);
        Assert.AreEqual(19.0, stats.P95Ms);
    }

    [TestMethod]
    public void FrameLineNamesTheAnimation()
    {
        var line = FrameStats.From([16.7, 33.4]).Line("open-slide", 2);

        Assert.AreEqual(
            "open-slide: 2 frames, median 16.7 ms, p95 33.4 ms, max 33.4 ms, 1 dropped, "
                + "2 resizes",
            line);
    }

    [TestMethod]
    public void UsageLineTakesTheDifferenceOfTwoReadings()
    {
        const long mebibyte = 1024 * 1024;
        var from = new ProcessReading(
            TimeSpan.FromSeconds(100), TimeSpan.FromSeconds(2), 10 * mebibyte, 4, 1, 0, 0, 0);
        var to = new ProcessReading(
            TimeSpan.FromSeconds(160),
            TimeSpan.FromSeconds(2.3),
            11 * mebibyte,
            5,
            1,
            0,
            60 * mebibyte,
            90 * mebibyte);
        var updates = new UpdateTally();
        updates.Add(TimeSpan.FromMilliseconds(1), 10 * 1024);
        updates.Add(TimeSpan.FromMilliseconds(3), 30 * 1024);

        var line = Usage.Line("open", from, to, updates, 7, 1);

        Assert.AreEqual(
            "usage open: 60 s, cpu 0.50% of one core, private 60.0 MB, working set 90.0 MB, "
                + "allocated 1.00 MB, gc 1/0/0, 2 updates, avg 2.00 ms, max 3.00 ms, "
                + "20.0 KB each, 7 changes, 1 resizes",
            line);
    }

    [TestMethod]
    public void UsageLineWithoutUpdatesHasNoAverages()
    {
        var reading = new ProcessReading(TimeSpan.FromSeconds(5), default, 0, 0, 0, 0, 0, 0);

        var line = Usage.Line("closed", reading, reading, new UpdateTally(), 0, 0);

        StringAssert.Contains(line, "cpu 0.00% of one core");
        StringAssert.Contains(line, "0 updates, avg 0.00 ms, max 0.00 ms, 0.0 KB each");
    }

    [TestMethod]
    public void ClearingTheTallyStartsOver()
    {
        var updates = new UpdateTally();
        updates.Add(TimeSpan.FromMilliseconds(4), 100);

        updates.Clear();

        Assert.AreEqual(0, updates.Count);
        Assert.AreEqual(TimeSpan.Zero, updates.Max);
        Assert.AreEqual(0, updates.AllocatedBytes);
    }
}
