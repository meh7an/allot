using Allot.Tray.Core.Contract;
using Allot.Tray.Core.Daemon;
using Allot.Tray.Core.Samples;
using Allot.Tray.Core.TrayIcon;
using Allot.Tray.Core.ViewModels;

namespace Allot.Tray.Core.Tests;

[TestClass]
public sealed class TrayStatusTests
{
    private static TrayStatus StatusOf(
        UiState? state,
        ServiceStatus status = ServiceStatus.Running,
        params string[] ending)
    {
        var leases = ending.ToDictionary(
            Handoff.LeaseId,
            _ => new StopView(StopKind.Remove, StopPhase.Ended));
        return TrayStatus.From(FlyoutSnapshot.From(status, state, leases));
    }

    [TestMethod]
    public void IdleWithoutJobs()
    {
        var status = StatusOf(Handoff.State(idle: true));

        Assert.AreEqual(new TrayStatus(TrayIconState.Idle, "Idle"), status);
        Assert.AreEqual("Allot · Idle", status.Tooltip);
    }

    [TestMethod]
    public void BusyWhileOnlyRunning() =>
        Assert.AreEqual(
            new TrayStatus(TrayIconState.Busy, "2 running"),
            StatusOf(Handoff.State(run: ["cargo", "py"])));

    [TestMethod]
    public void QueuedWhileAnythingWaits() =>
        Assert.AreEqual(
            new TrayStatus(TrayIconState.Queued, "1 running · 1 waiting"),
            StatusOf(Handoff.State(run: ["cargo"], wait: ["npm"])));

    [TestMethod]
    public void QueuedWithOnlyWaitingJobs() =>
        Assert.AreEqual(TrayIconState.Queued, StatusOf(Handoff.State(wait: ["npm"])).Icon);

    [TestMethod]
    public void BenchmarkWhileABenchmarkDrainsAheadOfRunningAndWaitingJobs() =>
        Assert.AreEqual(
            TrayIconState.Benchmark,
            StatusOf(Handoff.State(run: ["cargo"], wait: ["npmB"], bench: "drain")).Icon);

    [TestMethod]
    public void BenchmarkWhileABenchmarkWaitsForQuiet() =>
        Assert.AreEqual(
            new TrayStatus(TrayIconState.Benchmark, "Benchmark waiting for quiet"),
            StatusOf(Handoff.State(bench: "quiet")));

    [TestMethod]
    public void BenchmarkWhileABenchmarkRunsAlone() =>
        Assert.AreEqual(
            new TrayStatus(TrayIconState.Benchmark, "Benchmark running alone"),
            StatusOf(Handoff.State(bench: "run")));

    [TestMethod]
    public void ARemovedRequestNoLongerCountsAsWaiting() =>
        Assert.AreEqual(
            TrayIconState.Busy,
            StatusOf(Handoff.State(run: ["cargo"], wait: ["npm"]), ending: "npm").Icon);

    [TestMethod]
    public void DownWhileStoppedEvenWithLastKnownJobs() =>
        Assert.AreEqual(
            new TrayStatus(TrayIconState.Down, "Service stopped"),
            StatusOf(Handoff.State(run: ["cargo"], wait: ["npm"]), ServiceStatus.Stopped));

    [TestMethod]
    public void DownWhileStartingOrIncompatible()
    {
        Assert.AreEqual(TrayIconState.Down, StatusOf(null, ServiceStatus.Starting).Icon);
        Assert.AreEqual(
            new TrayStatus(TrayIconState.Down, "Service needs an update"),
            StatusOf(null, ServiceStatus.Incompatible));
    }
}
