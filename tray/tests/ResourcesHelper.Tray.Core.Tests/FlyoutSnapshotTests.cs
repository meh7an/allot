using ResourcesHelper.Tray.Core.Contract;
using ResourcesHelper.Tray.Core.Daemon;
using ResourcesHelper.Tray.Core.Samples;
using ResourcesHelper.Tray.Core.ViewModels;

namespace ResourcesHelper.Tray.Core.Tests;

[TestClass]
public sealed class FlyoutSnapshotTests
{
    private static readonly string[] Several = ["cargo", "py"];

    // Every SCEN entry of the handoff's Flyout.dc.html with the summary its header shows. Local
    // UI state (Stay open, menus, the Resources card) does not change the summary, so those
    // scenarios share their job set's line. Stopped and Removed cards are passed as ending.
    private static readonly Dictionary<string, Scenario> Scenarios = new(StringComparer.Ordinal)
    {
        ["empty"] = new(Handoff.State(), "Idle", HeaderGlyph.Idle),
        ["one"] = new(Handoff.State(run: ["cargo"]), "1 running", HeaderGlyph.Busy),
        ["several"] = new(SeveralState(), "2 running · 1 waiting", HeaderGlyph.Busy),
        ["kill-confirm"] = new(SeveralState(), "2 running · 1 waiting", HeaderGlyph.Busy),
        ["killed"] = new(SeveralState(), "1 running · 1 waiting", HeaderGlyph.Busy, ["cargo"]),
        ["bench-drain"] = new(
            Handoff.State(run: ["cargo"], wait: ["npmB"], bench: "drain"),
            "1 running · benchmark waiting",
            HeaderGlyph.Busy),
        ["bench-quiet"] = new(
            Handoff.State(wait: ["npmB"], bench: "quiet"),
            "Benchmark waiting for quiet",
            HeaderGlyph.Idle),
        ["bench-run"] = new(
            Handoff.State(wait: ["npmB"], bench: "run"),
            "Benchmark running alone",
            HeaderGlyph.Busy),
        ["bench-done"] = new(Handoff.State(wait: ["npmN"]), "1 waiting", HeaderGlyph.Idle),
        ["down"] = new(
            Handoff.State(run: ["cargo"]),
            "Service stopped",
            HeaderGlyph.Error,
            Status: ServiceStatus.Stopped),
        ["down-starting"] = new(
            Handoff.State(run: ["cargo"]),
            "Starting service…",
            HeaderGlyph.Error,
            Status: ServiceStatus.Starting),
        ["stay"] = new(SeveralState(), "2 running · 1 waiting", HeaderGlyph.Busy),
        ["pinned"] = new(SeveralState(), "2 running · 1 waiting", HeaderGlyph.Busy),
        ["res-collapsed"] = new(
            Handoff.State(run: ["cargo"], wait: ["npm"]),
            "1 running · 1 waiting",
            HeaderGlyph.Busy),
        ["res-expanded"] = new(Handoff.State(run: ["cargo"]), "1 running", HeaderGlyph.Busy),
        ["menu"] = new(Handoff.State(run: ["cargo"]), "1 running", HeaderGlyph.Busy),
        ["max"] = new(
            Handoff.State(run: ["cargo", "py", "vite", "go"], wait: ["npm", "ctest"], idle: true),
            "4 running · 2 waiting",
            HeaderGlyph.Busy),
        ["first-run"] = new(Handoff.State(), "Idle", HeaderGlyph.Idle),
        ["menu-notif"] = new(Handoff.State(run: ["cargo"]), "1 running", HeaderGlyph.Busy),
        ["c-active"] = new(Handoff.State(run: ["cargo"]), "1 running", HeaderGlyph.Busy),
        ["c-over"] = new(Handoff.State(run: ["py"]), "1 running", HeaderGlyph.Busy),
        ["c-queued"] = new(Handoff.State(wait: ["npm", "ctest"]), "2 waiting", HeaderGlyph.Idle),
        ["c-bench"] = new(
            Handoff.State(bench: "quiet"),
            "Benchmark waiting for quiet",
            HeaderGlyph.Idle),
        ["c-confirm"] = new(Handoff.State(run: ["cargo"]), "1 running", HeaderGlyph.Busy),
        ["c-killed"] = new(Handoff.State(run: ["cargo"]), "Idle", HeaderGlyph.Idle, ["cargo"]),
        ["c-removed"] = new(Handoff.State(wait: ["npm"]), "Idle", HeaderGlyph.Idle, ["npm"]),
        ["outdated"] = new(
            null,
            "Service needs an update",
            HeaderGlyph.Error,
            Status: ServiceStatus.Incompatible),
    };

    public static IEnumerable<object[]> ScenarioNames =>
        Scenarios.Keys.Select(name => new object[] { name });

    [TestMethod]
    [DynamicData(nameof(ScenarioNames))]
    public void SummaryMatchesTheHandoff(string name)
    {
        var scenario = Scenarios[name];

        var header = Build(scenario).Header;

        Assert.AreEqual(scenario.Summary, header.Summary);
        Assert.AreEqual(scenario.Glyph, header.Glyph);
    }

    [TestMethod]
    public void BenchmarkSequenceWalksThroughEveryPhase()
    {
        // bench-seq: drain with a running job, quiet, run alone, then done (the lease is gone).
        UiState[] steps =
        [
            Handoff.State(run: ["cargo"], wait: ["npmB"], bench: "drain"),
            Handoff.State(wait: ["npmB"], bench: "quiet"),
            Handoff.State(wait: ["npmB"], bench: "run"),
            Handoff.State(wait: ["npmN"]),
        ];

        var summaries = steps.Select(state => FlyoutSnapshot.From(ServiceStatus.Running, state))
            .Select(snapshot => (snapshot.Benchmark, snapshot.Header.Summary))
            .ToList();

        CollectionAssert.AreEqual(
            new List<(BenchmarkPhase?, string)>
            {
                (BenchmarkPhase.Draining, "1 running · benchmark waiting"),
                (BenchmarkPhase.WaitingForQuiet, "Benchmark waiting for quiet"),
                (BenchmarkPhase.RunningAlone, "Benchmark running alone"),
                (null, "1 waiting"),
            },
            summaries);
    }

    [TestMethod]
    public void GroupsComeInBenchmarkRunningWaitingOrder()
    {
        var state = Handoff.State(run: ["cargo"], wait: ["npmB"], bench: "drain");

        var snapshot = FlyoutSnapshot.From(ServiceStatus.Running, state);

        Assert.AreEqual(
            "Benchmark | Running · 1 | Waiting · 1",
            string.Join(" | ", snapshot.Groups.Select(group => group.Heading)));
        CollectionAssert.AreEqual(
            new[] { JobGroupKind.Benchmark, JobGroupKind.Running, JobGroupKind.Waiting },
            snapshot.Groups.Select(group => group.Kind).ToArray());
    }

    [TestMethod]
    public void ActiveBenchmarkGoesToTheBenchmarkGroupNotRunning()
    {
        var state = Handoff.State(wait: ["npmB"], bench: "run");

        var snapshot = FlyoutSnapshot.From(ServiceStatus.Running, state);

        Assert.AreEqual(JobGroupKind.Benchmark, snapshot.Groups[0].Kind);
        Assert.IsNotNull(snapshot.Groups[0].Jobs.Single().Active);
        Assert.IsFalse(snapshot.Groups.Any(group => group.Kind == JobGroupKind.Running));
    }

    [TestMethod]
    public void SessionWithJobsInTwoStatesAppearsInBothGroups()
    {
        var state = Handoff.State(run: ["cargo"], wait: ["npm", "ctest"]);

        var snapshot = FlyoutSnapshot.From(ServiceStatus.Running, state);

        var running = snapshot.Groups.Single(group => group.Kind == JobGroupKind.Running);
        var waiting = snapshot.Groups.Single(group => group.Kind == JobGroupKind.Waiting);
        Assert.AreEqual("3f2a9c1e", running.Jobs.Single().Session.ShortId);
        Assert.AreEqual(
            "#1 77a0e5d2 | #2 3f2a9c1e",
            string.Join(
                " | ",
                waiting.Jobs.Select(job => $"#{job.Queued!.Position} {job.Session.ShortId}")));
    }

    [TestMethod]
    public void SessionStripNamesTheProjectAndUsage()
    {
        var snapshot = FlyoutSnapshot.From(ServiceStatus.Running, Handoff.State(run: ["cargo"]));

        var strip = snapshot.Groups[0].Jobs[0].Session;

        Assert.AreEqual(
            new SessionStrip(
                "3f2a9c1e",
                "resources-helper-mcp",
                @"E:\code\resources-helper-mcp",
                "7.8 cores · 2.9 GB"),
            strip);
    }

    [TestMethod]
    public void UnregisteredSessionFallsBackToTheLeaseCwd()
    {
        var state = Handoff.State(run: ["cargo"]) with { Sessions = [] };

        var strip = FlyoutSnapshot.From(ServiceStatus.Running, state).Groups[0].Jobs[0].Session;

        Assert.AreEqual("resources-helper-mcp", strip.Project);
        Assert.AreEqual("", strip.Usage);
    }

    [TestMethod]
    public void IdleLineListsSessionsWithoutLeases()
    {
        var snapshot = FlyoutSnapshot.From(ServiceStatus.Running, SeveralState());

        Assert.AreEqual(
            new IdleLine("Idle · 1", "e09c7b3a", "0.1 cores · 0.3 GB"),
            snapshot.Idle);
    }

    [TestMethod]
    public void IdleLineShowsUsageOnlyForOneSession()
    {
        var state = SeveralState();
        state = state with
        {
            Sessions =
            [
                .. state.Sessions,
                Handoff.Session("a1b2c3d4-0000-4000-8000-000000000000", null, 0.2f, 512),
            ],
        };

        var idle = FlyoutSnapshot.From(ServiceStatus.Running, state).Idle;

        Assert.AreEqual(new IdleLine("Idle · 2", "e09c7b3a  a1b2c3d4", ""), idle);
    }

    [TestMethod]
    public void NoIdleLineWhenEverySessionHasAJob()
    {
        var snapshot = FlyoutSnapshot.From(ServiceStatus.Running, Handoff.State(run: ["cargo"]));

        Assert.IsNull(snapshot.Idle);
    }

    [TestMethod]
    public void HeadingKeepsTheCountWhileEveryCardIsEnding()
    {
        var snapshot = Build(Scenarios["c-killed"]);

        Assert.AreEqual("Running · 1", snapshot.Groups.Single().Heading);
        Assert.IsTrue(snapshot.Groups.Single().Jobs.Single().Ending);
    }

    [TestMethod]
    public void EndingBenchmarkDoesNotCountAsLive()
    {
        var state = Handoff.State(wait: ["npmB"], bench: "run");

        var snapshot = FlyoutSnapshot.From(
            ServiceStatus.Running,
            state,
            new Dictionary<long, StopView>
            {
                [Handoff.LeaseId("bench")] = new(StopKind.Kill, StopPhase.Ended),
            });

        Assert.IsNull(snapshot.Benchmark);
        Assert.AreEqual("1 waiting", snapshot.Header.Summary);
    }

    [TestMethod]
    public void StoppedBenchmarkNoLongerHoldsTheQueueBack()
    {
        var state = Handoff.State(wait: ["npmB"], bench: "run");

        var snapshot = FlyoutSnapshot.From(
            ServiceStatus.Running,
            state,
            new Dictionary<long, StopView>
            {
                [Handoff.LeaseId("bench")] = new(StopKind.Kill, StopPhase.Ended),
            });

        var waiting = snapshot.Groups.Single(group => group.Kind == JobGroupKind.Waiting);
        Assert.AreEqual("1:12 · depends on queue", waiting.Jobs.Single().Card.Time);
    }

    [TestMethod]
    public void ConfirmingAndSendingStillCount()
    {
        var snapshot = FlyoutSnapshot.From(
            ServiceStatus.Running,
            SeveralState(),
            new Dictionary<long, StopView>
            {
                [Handoff.LeaseId("cargo")] = new(StopKind.Kill, StopPhase.Sending),
                [Handoff.LeaseId("py")] = new(StopKind.Kill, StopPhase.Confirming, 4),
            });

        Assert.AreEqual("2 running · 1 waiting", snapshot.Header.Summary);
    }

    [TestMethod]
    public void LastKnownStateIgnoresStops()
    {
        var snapshot = FlyoutSnapshot.From(
            ServiceStatus.Stopped,
            Handoff.State(run: ["cargo"]),
            Handoff.Stops("c-confirm"));

        var card = snapshot.Groups.Single().Jobs.Single().Card;
        Assert.AreEqual(CardAction.None, card.Action);
        Assert.AreEqual("cargo.exe 18244 · was at 3:06 / ~5:00", card.Detail);
    }

    [TestMethod]
    public void NoisyBenchmarkStillWaitsForQuiet()
    {
        var state = Handoff.State(bench: "quiet");
        var queued = state.Queue[0];
        var noisy = new BenchmarkProgress.Noisy
        {
            CpuAvgPct = 12,
            CpuLimitPct = 5,
            GpuAvgPct = 0,
            GpuLimitPct = 5,
            GrantAnywayInSecs = 40,
        };
        state = state with
        {
            Queue = [queued with { Verdict = queued.Verdict with { Benchmark = noisy } }],
        };

        var snapshot = FlyoutSnapshot.From(ServiceStatus.Running, state);

        Assert.AreEqual(BenchmarkPhase.WaitingForQuiet, snapshot.Benchmark);
    }

    [TestMethod]
    public void BenchmarkQueuedBehindOthersCountsAsDraining()
    {
        var state = Handoff.State(run: ["cargo"], bench: "drain");
        var queued = state.Queue[0];
        state = state with
        {
            Queue = [queued with { Verdict = queued.Verdict with { Benchmark = null } }],
        };

        var snapshot = FlyoutSnapshot.From(ServiceStatus.Running, state);

        Assert.AreEqual("1 running · benchmark waiting", snapshot.Header.Summary);
    }

    [TestMethod]
    public void DownShowsTheLastKnownJobs()
    {
        var snapshot = Build(Scenarios["down"]);

        Assert.IsTrue(snapshot.IsLastKnown);
        Assert.AreEqual("Running · 1", snapshot.Groups.Single().Heading);
    }

    [TestMethod]
    public void DownWithoutAnyStateShowsNoJobs()
    {
        var snapshot = FlyoutSnapshot.From(ServiceStatus.Stopped, null);

        Assert.AreEqual(0, snapshot.Groups.Count);
        Assert.IsNull(snapshot.Idle);
        Assert.AreEqual("Service stopped", snapshot.Header.Summary);
    }

    private static UiState SeveralState() =>
        Handoff.State(run: Several, wait: ["npm"], idle: true);

    private static FlyoutSnapshot Build(Scenario scenario)
    {
        var ending = scenario.Ending.ToDictionary(
            Handoff.LeaseId,
            _ => new StopView(StopKind.Kill, StopPhase.Ended));
        return FlyoutSnapshot.From(scenario.Status, scenario.State, ending);
    }

    private sealed record Scenario(
        UiState? State,
        string Summary,
        HeaderGlyph Glyph,
        string[]? EndingJobs = null,
        ServiceStatus Status = ServiceStatus.Running)
    {
        public string[] Ending => EndingJobs ?? [];
    }
}
