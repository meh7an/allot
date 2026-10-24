using ResourcesHelper.Tray.Core.Daemon;
using ResourcesHelper.Tray.Core.Tests.Support;

namespace ResourcesHelper.Tray.Core.Tests;

[TestClass]
public sealed class DaemonPollerTests
{
    public TestContext TestContext { get; set; } = null!;

    [TestMethod]
    public async Task DownDaemonBacksOffToTheClosedCap()
    {
        using var directory = new TempDirectory();
        var poller = Poller(directory, new FakeHttpHandler(_ => throw FakeHttpHandler.Refused()));

        var delays = await PollTimes(poller, 7);

        CollectionAssert.AreEqual(Seconds(1, 2, 4, 8, 16, 30, 30), delays);
        Assert.AreEqual(ServiceStatus.Stopped, poller.Latest?.Status);
    }

    [TestMethod]
    public async Task DownDaemonBacksOffLessWhileTheFlyoutIsOpen()
    {
        using var directory = new TempDirectory();
        var poller = Poller(directory, new FakeHttpHandler(_ => throw FakeHttpHandler.Refused()));
        poller.SetFlyoutOpen(true);

        var delays = await PollTimes(poller, 5);

        CollectionAssert.AreEqual(Seconds(1, 2, 4, 5, 5), delays);
    }

    [TestMethod]
    public async Task OpeningTheFlyoutRestartsTheBackoff()
    {
        using var directory = new TempDirectory();
        var poller = Poller(directory, new FakeHttpHandler(_ => throw FakeHttpHandler.Refused()));
        _ = await PollTimes(poller, 4);

        poller.SetFlyoutOpen(true);

        CollectionAssert.AreEqual(Seconds(1, 2), await PollTimes(poller, 2));
    }

    [TestMethod]
    public async Task RecoveredDaemonPollsAtTheNormalCadence()
    {
        using var directory = new TempDirectory();
        var up = false;
        var handler = new FakeHttpHandler(_ => up
            ? FakeHttpHandler.Json(Fixtures.Read("ui-state-idle.json"))
            : throw FakeHttpHandler.Refused());
        var poller = Poller(directory, handler);
        _ = await PollTimes(poller, 3);
        up = true;

        var closed = await PollTimes(poller, 1);
        poller.SetFlyoutOpen(true);
        var open = await PollTimes(poller, 1);

        CollectionAssert.AreEqual(Seconds(5), closed);
        CollectionAssert.AreEqual(Seconds(1), open);
        Assert.AreEqual(ServiceStatus.Running, poller.Latest?.Status);
    }

    [TestMethod]
    public async Task LastKnownStateSurvivesTheDaemonGoingAway()
    {
        using var directory = new TempDirectory();
        var up = true;
        var handler = new FakeHttpHandler(_ => up
            ? FakeHttpHandler.Json(Fixtures.Read("ui-state-busy.json"))
            : throw FakeHttpHandler.Refused());
        var poller = Poller(directory, handler);
        _ = await PollTimes(poller, 1);
        var served = poller.LastKnown;
        up = false;

        _ = await PollTimes(poller, 1);

        Assert.IsNotNull(served);
        Assert.AreSame(served, poller.LastKnown);
        Assert.IsNull(poller.Latest?.State);
    }

    [TestMethod]
    public async Task StartServiceReportsStartingUntilTheTimeout()
    {
        using var directory = new TempDirectory();
        var time = new ManualTimeProvider();
        var launcher = new FakeLauncher();
        var poller = Poller(
            directory,
            new FakeHttpHandler(_ => throw FakeHttpHandler.Refused()),
            launcher,
            time);
        _ = await PollTimes(poller, 3);

        Assert.IsNull(poller.StartService());
        var starting = await PollTimes(poller, 2);
        var startingStatus = poller.Latest?.Status;
        time.Now += DaemonPoller.StartTimeout;
        var gaveUp = await PollTimes(poller, 1);

        Assert.AreEqual(1, launcher.Starts);
        CollectionAssert.AreEqual(Seconds(0.5, 0.5), starting);
        Assert.AreEqual(ServiceStatus.Starting, startingStatus);
        Assert.AreEqual(ServiceStatus.Stopped, poller.Latest?.Status);
        StringAssert.Contains(poller.Latest?.Detail, "did not answer within 10 s");
        CollectionAssert.AreEqual(Seconds(1), gaveUp);
    }

    [TestMethod]
    public async Task StartedDaemonEndsStarting()
    {
        using var directory = new TempDirectory();
        var up = false;
        var handler = new FakeHttpHandler(_ => up
            ? FakeHttpHandler.Json(Fixtures.Read("ui-state-idle.json"))
            : throw FakeHttpHandler.Refused());
        var poller = Poller(directory, handler);
        _ = poller.StartService();
        _ = await PollTimes(poller, 1);
        up = true;
        _ = await PollTimes(poller, 1);
        up = false;

        _ = await PollTimes(poller, 1);

        Assert.AreEqual(ServiceStatus.Stopped, poller.Latest?.Status);
    }

    [TestMethod]
    public void FailedLaunchSaysWhyAndChangesNothing()
    {
        using var directory = new TempDirectory();
        var launcher = new FakeLauncher { Problem = "rhd.exe is missing" };
        var handler = new FakeHttpHandler(_ => throw FakeHttpHandler.Refused());
        var poller = Poller(directory, handler, launcher);

        Assert.AreEqual("rhd.exe is missing", poller.StartService());
        Assert.IsNull(poller.Latest);
    }

    [TestMethod]
    public async Task UpdatedIsRaisedForEveryPoll()
    {
        using var directory = new TempDirectory();
        var poller = Poller(directory, new FakeHttpHandler(_ => throw FakeHttpHandler.Refused()));
        var raised = new List<ServiceStatus>();
        poller.Updated += (_, result) => raised.Add(result.Status);

        _ = await PollTimes(poller, 2);

        CollectionAssert.AreEqual(new[] { ServiceStatus.Stopped, ServiceStatus.Stopped }, raised);
    }

    [TestMethod]
    [Timeout(10_000, CooperativeCancellation = true)]
    public async Task OpeningTheFlyoutWakesAClosedPoller()
    {
        using var directory = new TempDirectory();
        using var polls = new SemaphoreSlim(0);
        var handler = new FakeHttpHandler(_ =>
        {
            polls.Release();
            return FakeHttpHandler.Json(Fixtures.Read("ui-state-idle.json"));
        });
        var poller = Poller(directory, handler, time: TimeProvider.System);
        using var stop =
            CancellationTokenSource.CreateLinkedTokenSource(TestContext.CancellationToken);
        var loop = poller.RunAsync(stop.Token);
        await polls.WaitAsync(TestContext.CancellationToken);

        // Closed and running, the next poll is 5 s away; opening must not wait for it.
        poller.SetFlyoutOpen(true);
        var woke = await polls.WaitAsync(TimeSpan.FromSeconds(3), TestContext.CancellationToken);
        await stop.CancelAsync();

        Assert.IsTrue(woke);
        _ = await Assert.ThrowsAsync<OperationCanceledException>(() => loop);
    }

    private static DaemonPoller Poller(
        TempDirectory directory,
        FakeHttpHandler handler,
        IServiceLauncher? launcher = null,
        TimeProvider? time = null) =>
        new(
            DaemonClientTests.Client(directory, handler, "secret"),
            launcher ?? new FakeLauncher(),
            time ?? new ManualTimeProvider());

    private static TimeSpan[] Seconds(params double[] seconds) =>
        [.. seconds.Select(TimeSpan.FromSeconds)];

    private async Task<TimeSpan[]> PollTimes(DaemonPoller poller, int count)
    {
        var delays = new TimeSpan[count];
        for (var i = 0; i < count; i++)
        {
            delays[i] = await poller.PollOnceAsync(TestContext.CancellationToken);
        }

        return delays;
    }

    private sealed class FakeLauncher : IServiceLauncher
    {
        public string? Problem { get; init; }

        public int Starts { get; private set; }

        public string? Start()
        {
            Starts++;
            return Problem;
        }
    }
}
