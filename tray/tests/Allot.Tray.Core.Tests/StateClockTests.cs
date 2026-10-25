using Allot.Tray.Core.Samples;
using Allot.Tray.Core.Tests.Support;
using Allot.Tray.Core.ViewModels;

namespace Allot.Tray.Core.Tests;

[TestClass]
public sealed class StateClockTests
{
    [TestMethod]
    public void NoTimeBeforeAnyState()
    {
        var clock = new StateClock(new ManualTimeProvider());

        Assert.IsNull(clock.NowMs);
    }

    [TestMethod]
    public void TimeRunsFromTheStateSinceItArrived()
    {
        var time = new ManualTimeProvider();
        var clock = new StateClock(time);

        clock.Received(Handoff.State());
        time.Now += TimeSpan.FromMilliseconds(2_400);

        Assert.AreEqual(Handoff.NowMs + 2_400, clock.NowMs);
    }

    [TestMethod]
    public void ANewStateResetsTheBase()
    {
        var time = new ManualTimeProvider();
        var clock = new StateClock(time);
        clock.Received(Handoff.State());
        time.Now += TimeSpan.FromSeconds(5);

        clock.Received(Handoff.State() with { GeneratedAtMs = Handoff.NowMs + 1_000 });

        Assert.AreEqual(Handoff.NowMs + 1_000, clock.NowMs);
    }

    [TestMethod]
    public void TheLocalClockGoingBackDoesNotRewind()
    {
        var time = new ManualTimeProvider();
        var clock = new StateClock(time);
        clock.Received(Handoff.State());

        time.Now -= TimeSpan.FromMinutes(1);

        Assert.AreEqual(Handoff.NowMs, clock.NowMs);
    }

    [TestMethod]
    public void NoLiveStateStopsTheClock()
    {
        var clock = new StateClock(new ManualTimeProvider());
        clock.Received(Handoff.State());

        clock.Received(null);

        Assert.IsNull(clock.NowMs);
    }
}
