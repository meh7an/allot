using ResourcesHelper.Tray.Core.Contract;
using ResourcesHelper.Tray.Core.Daemon;
using ResourcesHelper.Tray.Core.Samples;
using ResourcesHelper.Tray.Core.ViewModels;

namespace ResourcesHelper.Tray.Core.Tests;

/// <summary>
/// Every detail and time string of the handoff's job cards (<c>JOBS</c> and <c>SCEN</c> in
/// <c>Flyout.dc.html</c>), rebuilt from the daemon state the sample jobs stand for. Kill, Stopped
/// and Removed lines and the benchmark status lines are this tray's own additions.
/// </summary>
[TestClass]
public sealed class JobCardTests
{
    private const double Tolerance = 0.0001;

    [TestMethod]
    [DataRow("cargo", "cargo.exe 18244 · 8 cores · 4 GB", "3:06 / ~5:00", false)]
    [DataRow(
        "py", "python.exe 22108 · 2 cores · 4 GB · GPU 80% · 4 GB VRAM", "1:40 over", true)]
    [DataRow(
        "vite", "node.exe 9120 · 4 cores · 2 GB · GPU 10% · 1 GB VRAM", "0:52 / ~1:30", false)]
    [DataRow("go", "go.exe 14772 · 1 core · 1 GB", "0:20 / ~1:00", false)]
    public void ActiveJobShowsProcessReservationAndTime(
        string job,
        string detail,
        string time,
        bool warning)
    {
        var card = CardOf(MaxState(), job);

        Assert.AreEqual(detail, card.Detail);
        Assert.AreEqual(detail, card.DetailTooltip);
        Assert.AreEqual(time, card.Time);
        Assert.AreEqual(warning, card.TimeIsWarning);
        Assert.AreEqual(CardAction.Kill, card.Action);
    }

    [TestMethod]
    [DataRow("cargo", "CA", "CPU heavy", "Kill cargo.exe")]
    [DataRow("py", "PY", "GPU heavy", "Kill python.exe")]
    [DataRow("vite", "NO", "Mixed", "Kill node.exe")]
    [DataRow("go", "GO", "Light", "Kill go.exe")]
    public void ActiveJobHasInitialsChipAndKillName(
        string job,
        string tile,
        string chip,
        string killName)
    {
        var card = CardOf(MaxState(), job);

        Assert.AreEqual(tile, card.Tile);
        Assert.IsFalse(card.IsQueued);
        Assert.AreEqual(chip, card.Chip);
        Assert.AreEqual(chip, card.ChipTooltip);
        Assert.IsFalse(card.IsBenchmark);
        Assert.AreEqual(killName, card.ActionName);
    }

    [TestMethod]
    public void RunningBarIsElapsedOverTheEstimate()
    {
        var card = CardOf(Handoff.State(run: ["cargo"]), "cargo");

        Assert.AreEqual(EdgeBarTone.Running, card.BarTone);
        Assert.AreEqual(186.0 / 300, card.BarFraction, Tolerance);
    }

    [TestMethod]
    public void PastTheEstimateTheBarIsFullAndAmber()
    {
        var card = CardOf(Handoff.State(run: ["py"]), "py");

        Assert.AreEqual(EdgeBarTone.Over, card.BarTone);
        Assert.AreEqual(1, card.BarFraction, Tolerance);
    }

    [TestMethod]
    public void QueuedJobWithAnEstimateShowsWaitedAgainstTheTotal()
    {
        // rhd's wait estimate is what is left: 42 s waited and 78 s left read "~2 min".
        var card = CardOf(Handoff.State(run: ["cargo", "py"], wait: ["npm"]), "npm");

        Assert.AreEqual("#1", card.Tile);
        Assert.IsTrue(card.IsQueued);
        Assert.IsNull(card.ExePath);
        Assert.AreEqual("2 cores · 4 GB", card.Detail);
        Assert.AreEqual("", card.DetailTooltip);
        Assert.AreEqual("0:42 / ~2 min", card.Time);
        Assert.IsTrue(card.TimeIsWarning);
        Assert.AreEqual(EdgeBarTone.Waiting, card.BarTone);
        Assert.AreEqual(42.0 / 120, card.BarFraction, Tolerance);
        Assert.AreEqual(CardAction.Remove, card.Action);
        Assert.AreEqual("Remove npm run test:e2e from queue", card.ActionName);
    }

    [TestMethod]
    public void QueuedJobWithoutAnEstimateDependsOnTheQueue()
    {
        var card = CardOf(MaxState(), "ctest");

        Assert.AreEqual("#2", card.Tile);
        Assert.AreEqual("8 cores · 4 GB", card.Detail);
        Assert.AreEqual("0:18 · depends on queue", card.Time);
        Assert.AreEqual(EdgeBarTone.WaitingDashed, card.BarTone);
        Assert.AreEqual(1, card.BarFraction, Tolerance);
        Assert.AreEqual("Custom", card.Chip);
        Assert.AreEqual("Custom reservation", card.ChipTooltip);
    }

    [TestMethod]
    [DataRow("drain")]
    [DataRow("quiet")]
    [DataRow("run")]
    public void QueuedJobBehindABenchmarkWaitsAfterIt(string phase)
    {
        var card = CardOf(Handoff.State(wait: ["npmB"], bench: phase), "npmB");

        Assert.AreEqual("1:12 · after benchmark", card.Time);
        Assert.AreEqual(EdgeBarTone.WaitingDashed, card.BarTone);
    }

    [TestMethod]
    public void QueuedJobAtTheHeadWithAMinuteLeftStartsNext()
    {
        // bench-done: the benchmark is over and npm waited 196 s with 17 s left.
        var card = CardOf(Handoff.State(wait: ["npmN"]), "npmN");

        Assert.AreEqual("starts next · ~1 min", card.Time);
        Assert.AreEqual(EdgeBarTone.Waiting, card.BarTone);
        Assert.AreEqual(196.0 / 213, card.BarFraction, Tolerance);
    }

    [TestMethod]
    public void QueuedJobThatFitsStartsNextWithoutAnEstimate()
    {
        var state = Handoff.State(wait: ["ctest"]);
        var queued = state.Queue[0];
        state = state with
        {
            Queue = [queued with { Verdict = queued.Verdict with { Fits = true } }],
        };

        var card = CardOf(state, "ctest");

        Assert.AreEqual("starts next", card.Time);
        Assert.AreEqual(EdgeBarTone.Waiting, card.BarTone);
        Assert.AreEqual(1, card.BarFraction, Tolerance);
    }

    [TestMethod]
    public void SecondInLineWithAMinuteLeftDoesNotStartNext()
    {
        var state = Handoff.State(wait: ["npm", "npmN"]);

        var card = CardOf(state, "npmN");

        Assert.AreEqual("3:16 / ~4 min", card.Time);
    }

    [TestMethod]
    public void QueuedCardOfASessionThatAlsoRunsSaysSo()
    {
        var snapshot = FlyoutSnapshot.From(ServiceStatus.Running, MaxState());

        var cards = snapshot.Groups.SelectMany(group => group.Jobs).ToList();
        var cargo = cards.Single(job => job.Id == Handoff.LeaseId("cargo"));
        var ctest = cards.Single(job => job.Id == Handoff.LeaseId("ctest"));
        var npm = cards.Single(job => job.Id == Handoff.LeaseId("npm"));
        Assert.AreEqual("7.8 cores · 2.9 GB", cargo.Session.Usage);
        Assert.AreEqual("also running", ctest.Session.Usage);
        Assert.AreEqual("0.4 cores · 1.1 GB", npm.Session.Usage);
    }

    [TestMethod]
    public void BenchmarkCardHasThePurpleChip()
    {
        var card = CardOf(Handoff.State(wait: ["npmB"], bench: "run"), "bench");

        Assert.IsTrue(card.IsBenchmark);
        Assert.AreEqual("Benchmark", card.Chip);
        Assert.AreEqual("Benchmark", card.ChipTooltip);
    }

    [TestMethod]
    public void LastKnownJobShowsWhereItWasWithoutAnAction()
    {
        var snapshot = FlyoutSnapshot.From(
            ServiceStatus.Stopped,
            Handoff.State(run: ["cargo"]),
            nowMs: Handoff.NowMs + 60_000);

        var card = snapshot.Groups.Single().Jobs.Single().Card;

        Assert.AreEqual("cargo.exe 18244 · was at 3:06 / ~5:00", card.Detail);
        Assert.AreEqual("", card.Time);
        Assert.AreEqual(EdgeBarTone.Frozen, card.BarTone);
        Assert.AreEqual(CardAction.None, card.Action);
    }

    [TestMethod]
    public void LastKnownQueuedJobHasNoRemove()
    {
        var card = FlyoutSnapshot.From(ServiceStatus.Stopped, Handoff.State(wait: ["npm"]))
            .Groups.Single().Jobs.Single().Card;

        Assert.AreEqual(CardAction.None, card.Action);
        Assert.AreEqual(EdgeBarTone.Frozen, card.BarTone);
    }

    [TestMethod]
    public void TimesTickWithTheClock()
    {
        var state = Handoff.State(run: ["cargo"], wait: ["npm"]);

        var snapshot = FlyoutSnapshot.From(
            ServiceStatus.Running,
            state,
            nowMs: Handoff.NowMs + 1_500);

        var cards = snapshot.Groups.SelectMany(group => group.Jobs).Select(job => job.Card);
        Assert.AreEqual(
            "3:07 / ~5:00 | 0:43 / ~2 min",
            string.Join(" | ", cards.Select(card => card.Time)));
    }

    [TestMethod]
    public void ActiveJobWithoutAProcessUsesTheCommand()
    {
        var state = Handoff.State(run: ["cargo"]);
        state = state with { Active = [state.Active[0] with { Process = null }] };

        var card = CardOf(state, "cargo");

        Assert.AreEqual("CA", card.Tile);
        Assert.AreEqual("8 cores · 4 GB", card.Detail);
        Assert.AreEqual("Kill cargo build --release", card.ActionName);
        Assert.IsNull(card.ExePath);
    }

    [TestMethod]
    public void ActiveJobWithoutAnEstimateShowsElapsedOnly()
    {
        var state = Handoff.State(run: ["cargo"]);
        var job = state.Active[0];
        var lease = job.Lease with { Profile = job.Lease.Profile with { EstSecs = 0 } };
        state = state with { Active = [job with { Lease = lease }] };

        var card = CardOf(state, "cargo");

        Assert.AreEqual("3:06", card.Time);
        Assert.IsFalse(card.TimeIsWarning);
        Assert.AreEqual(0, card.BarFraction, Tolerance);
    }

    [TestMethod]
    public void IconsScenarioCarriesExePaths()
    {
        var (_, state) = Handoff.Scenario("icons");

        var card = CardOf(state, "cargo");

        Assert.IsTrue(
            card.ExePath!.EndsWith(@"\System32\cmd.exe", StringComparison.OrdinalIgnoreCase));
    }

    [TestMethod]
    public void RecentlyFinishedShowsRunTimeAndAge()
    {
        // empty: no jobs, two recent runs.
        var snapshot = FlyoutSnapshot.From(ServiceStatus.Running, Handoff.State(recent: true));

        Assert.AreEqual(
            "cargo build --release 3f2a9c1e 4:52 · 4 min ago True | "
                + "npm run test:e2e 77a0e5d2 2:10 · 11 min ago False",
            string.Join(
                " | ",
                snapshot.Recent.Select(line =>
                    $"{line.Command} {line.ShortId} {line.Meta} {line.First}")));
        Assert.IsFalse(snapshot.ShowNothing);
    }

    [TestMethod]
    public void RecentlyFinishedShowsOnlyWhileNothingRunsOrWaits()
    {
        var state = Handoff.State(run: ["cargo"], recent: true);

        var snapshot = FlyoutSnapshot.From(ServiceStatus.Running, state);

        Assert.IsEmpty(snapshot.Recent);
        Assert.IsFalse(snapshot.ShowNothing);
    }

    [TestMethod]
    public void NoJobsAndNoRecentRunsSaysSo()
    {
        Assert.IsTrue(FlyoutSnapshot.From(ServiceStatus.Running, Handoff.State()).ShowNothing);
        Assert.IsFalse(FlyoutSnapshot.From(ServiceStatus.Stopped, Handoff.State()).ShowNothing);
    }

    [TestMethod]
    public void RecentRunWithoutASessionHasNoId()
    {
        var state = Handoff.State(recent: true);
        state = state with { Recent = [state.Recent[0] with { Session = null }] };

        var line = FlyoutSnapshot.From(ServiceStatus.Running, state).Recent.Single();

        Assert.AreEqual("", line.ShortId);
    }

    [TestMethod]
    [DynamicData(nameof(ScenarioNames))]
    public void EveryAppScenarioBuilds(string name)
    {
        var (status, state) = Handoff.Scenario(name);

        var snapshot = FlyoutSnapshot.From(status, state);

        Assert.IsNotNull(snapshot.Header.Summary);
    }

    public static IEnumerable<object[]> ScenarioNames =>
        Handoff.ScenarioNames.Select(name => new object[] { name });

    private static UiState MaxState() =>
        Handoff.State(run: ["cargo", "py", "vite", "go"], wait: ["npm", "ctest"], idle: true);

    private static JobCard CardOf(UiState state, string job) =>
        FlyoutSnapshot.From(ServiceStatus.Running, state)
            .Groups.SelectMany(group => group.Jobs)
            .Single(item => item.Id == Handoff.LeaseId(job))
            .Card;
}
