using ResourcesHelper.Tray.Core.Contract;
using ResourcesHelper.Tray.Core.Daemon;
using ResourcesHelper.Tray.Core.Samples;
using ResourcesHelper.Tray.Core.ViewModels;

namespace ResourcesHelper.Tray.Core.Tests;

/// <summary>
/// The benchmark card in each phase (handoff section 4, Benchmark, and <c>mkBench</c> in
/// <c>Flyout.dc.html</c>): its status line, leading icon, stepper and action.
/// </summary>
[TestClass]
public sealed class BenchmarkCardTests
{
    private const double Tolerance = 0.0001;

    [TestMethod]
    public void DrainingBenchmarkWaitsForRunningJobs()
    {
        var card = BenchCard(Handoff.State(run: ["cargo"], wait: ["npmB"], bench: "drain"));

        Assert.AreEqual("Draining · waiting for running jobs to finish", card.Detail);
        Assert.AreEqual(DetailIcon.Timer, card.Icon);
        Assert.AreEqual(new BenchmarkSteps(BenchmarkStep.Drain, 0, 0), card.Steps);
        Assert.AreEqual("", card.Time);
        Assert.AreEqual(EdgeBarTone.None, card.BarTone);
    }

    [TestMethod]
    public void DrainedBenchmarkThatDoesNotFitWaitsForRoom()
    {
        // Seen live: quiet was measured, then 16 cores did not fit beside other load.
        var card = BenchCard(WithoutProgress(["cores: 16.0 requested, 12.4 free"]));

        Assert.AreEqual("Waiting for room · cores: 16.0 requested, 12.4 free", card.Detail);
        Assert.AreEqual(DetailIcon.Timer, card.Icon);
        Assert.AreEqual(new BenchmarkSteps(BenchmarkStep.Drain, 0, 0), card.Steps);
    }

    [TestMethod]
    public void BenchmarkWaitingForRoomWithoutAReasonSaysSoPlainly()
    {
        var card = BenchCard(WithoutProgress([]));

        Assert.AreEqual("Waiting for room", card.Detail);
    }

    [TestMethod]
    public void BenchmarkWithoutProgressDrainsWhileAnotherLeaseRuns()
    {
        var state = WithoutProgress(["1 other lease(s) still active"]);
        state = state with { Active = Handoff.State(run: ["cargo"]).Active };

        var card = BenchCard(state);

        Assert.AreEqual("Draining · waiting for running jobs to finish", card.Detail);
    }

    [TestMethod]
    public void QueuedBenchmarkHasTheQueueTileAndRemove()
    {
        var card = BenchCard(Handoff.State(bench: "quiet"));

        Assert.AreEqual("#1", card.Tile);
        Assert.IsTrue(card.IsQueued);
        Assert.AreEqual(CardAction.Remove, card.Action);
        Assert.AreEqual("Remove cargo bench --bench parser from queue", card.ActionName);
    }

    [TestMethod]
    public void WaitingForQuietCountsSecondsAndBackgroundLoad()
    {
        var card = BenchCard(Handoff.State(wait: ["npmB"], bench: "quiet"));

        Assert.AreEqual("Waiting for quiet · 6 / 10 s · background 3% CPU", card.Detail);
        Assert.AreEqual(DetailIcon.Timer, card.Icon);
        Assert.AreEqual(new BenchmarkSteps(BenchmarkStep.Quiet, 6, 0), card.Steps);
    }

    [TestMethod]
    [DataRow(5UL, 20UL, 2)]
    [DataRow(20UL, 20UL, 10)]
    [DataRow(3UL, 5UL, 6)]
    [DataRow(0UL, 0UL, 10)]
    public void QuietTicksShowTheShareOfTheWindow(ulong measured, ulong needed, int ticks)
    {
        var state = WithProgress(new BenchmarkProgress.Measuring
        {
            CpuAvgPct = 3,
            GpuAvgPct = 0,
            MeasuredSecs = measured,
            NeededSecs = needed,
        });

        var card = BenchCard(state);

        Assert.AreEqual(ticks, card.Steps?.QuietTicks);
        StringAssert.StartsWith(card.Detail, $"Waiting for quiet · {measured} / {needed} s");
    }

    [TestMethod]
    public void NoisyMachineSaysWhenTheBenchmarkStartsAnyway()
    {
        var card = BenchCard(Handoff.State(wait: ["npmB"], bench: "noisy"));

        Assert.AreEqual(
            "Not quiet · background 14% CPU (limit 5%) · starting anyway in 0:42",
            card.Detail);
        Assert.AreEqual(DetailIcon.Timer, card.Icon);
        Assert.AreEqual(new BenchmarkSteps(BenchmarkStep.Quiet, 10, 0), card.Steps);
    }

    [TestMethod]
    public void NoisyMachineAtTheLongestWaitStartsNow()
    {
        var state = WithProgress(new BenchmarkProgress.Noisy
        {
            CpuAvgPct = 14,
            CpuLimitPct = 5,
            GpuAvgPct = 0,
            GpuLimitPct = 10,
            GrantAnywayInSecs = 0,
        });

        var card = BenchCard(state);

        Assert.AreEqual(
            "Not quiet · background 14% CPU (limit 5%) · starting anyway now",
            card.Detail);
    }

    [TestMethod]
    public void RunningAloneShowsElapsedOnTheRunStep()
    {
        var snapshot = Snapshot(Handoff.State(wait: ["npmB"], bench: "run"));
        var item = snapshot.Groups[0].Jobs.Single();
        var card = item.Card;

        Assert.AreEqual("Running alone · 0:48 / ~2:00", card.Detail);
        Assert.AreEqual("", card.Time);
        Assert.AreEqual(DetailIcon.Timer, card.Icon);
        Assert.AreEqual(BenchmarkStep.Run, card.Steps?.Current);
        Assert.AreEqual(0.4, card.Steps!.RunFraction, Tolerance);
        Assert.AreEqual(EdgeBarTone.None, card.BarTone);
        Assert.AreEqual(CardAction.Kill, card.Action);
        Assert.AreEqual("CA", card.Tile);
        Assert.AreEqual("15.6 cores · 3.1 GB", item.Session.Usage);
    }

    [TestMethod]
    public void ForcedStartSaysTheMachineWasNotQuiet()
    {
        var card = BenchCard(Handoff.State(wait: ["npmB"], bench: "forced"));

        Assert.AreEqual("Running alone · not quiet at start · 0:48 / ~2:00", card.Detail);
    }

    [TestMethod]
    public void RunningPastTheEstimateFillsTheRunStep()
    {
        var card = BenchCard(Handoff.State(bench: "run"), Handoff.NowMs + 102_000);

        Assert.AreEqual("Running alone · 0:30 over", card.Detail);
        Assert.AreEqual(1, card.Steps!.RunFraction, Tolerance);
    }

    [TestMethod]
    public void RunningWithoutAnEstimateShowsElapsedOnly()
    {
        var state = Handoff.State(bench: "run");
        var job = state.Active[0];
        var lease = job.Lease with { Profile = job.Lease.Profile with { EstSecs = 0 } };
        state = state with { Active = [job with { Lease = lease }] };

        var card = BenchCard(state);

        Assert.AreEqual("Running alone · 0:48", card.Detail);
        Assert.AreEqual(0, card.Steps!.RunFraction, Tolerance);
    }

    [TestMethod]
    public void FinishedBenchmarkShowsItsNoiseWithACheck()
    {
        var snapshot = Snapshot(Handoff.State(wait: ["npmN"], bench: "done"));
        var benchmark = snapshot.Groups[0];
        var item = benchmark.Jobs.Single();
        var card = item.Card;

        Assert.AreEqual(JobGroupKind.Benchmark, benchmark.Kind);
        Assert.AreEqual(Handoff.LeaseId("bench"), item.Id);
        Assert.AreEqual("cargo bench --bench parser", item.Command);
        Assert.IsNotNull(item.Finished);
        Assert.AreEqual(
            "Finished in 2:04 · background noise avg 1.8%, peak 6.2% CPU",
            card.Detail);
        Assert.AreEqual(DetailIcon.Check, card.Icon);
        Assert.AreEqual(new BenchmarkSteps(BenchmarkStep.Done, 10, 1), card.Steps);
        Assert.AreEqual(CardAction.None, card.Action);
        Assert.AreEqual("CA", card.Tile);
        Assert.IsTrue(card.IsBenchmark);
        Assert.IsFalse(card.IsEnded);
        Assert.AreEqual("parser-perf", item.Session.Project);
    }

    [TestMethod]
    public void FinishedBenchmarkNoLongerCounts()
    {
        var snapshot = Snapshot(Handoff.State(wait: ["npmN"], bench: "done"));

        Assert.IsTrue(snapshot.Groups[0].Jobs.Single().Ending);
        Assert.IsNull(snapshot.Benchmark);
        Assert.AreEqual("1 waiting", snapshot.Header.Summary);
        Assert.AreEqual(HeaderGlyph.Idle, snapshot.Header.Glyph);
        var waiting = snapshot.Groups.Single(group => group.Kind == JobGroupKind.Waiting);
        Assert.AreEqual("starts next · ~1 min", waiting.Jobs.Single().Card.Time);
    }

    [TestMethod]
    public void FinishedWithoutQuietSaysSo()
    {
        var state = Handoff.State(wait: ["npmN"]) with
        {
            Recent = [Handoff.FinishedBenchmark(quietAtStart: false)],
        };

        var card = BenchCard(state);

        Assert.AreEqual(
            "Finished in 2:04 · not quiet at start · background noise avg 1.8%, peak 6.2% CPU",
            card.Detail);
    }

    [TestMethod]
    [DataRow(0L, true)]
    [DataRow(49_999L, true)]
    [DataRow(50_000L, false)]
    public void FinishedCardStaysAMinuteAfterTheEnd(long laterMs, bool shown)
    {
        // The run ended 10 s before the scenario's moment.
        var state = Handoff.State(wait: ["npmN"], bench: "done");

        var snapshot = Snapshot(state, Handoff.NowMs + laterMs);

        Assert.AreEqual(
            shown,
            snapshot.Groups.Any(group => group.Kind == JobGroupKind.Benchmark));
    }

    [TestMethod]
    [DataRow(EndCause.Killed)]
    [DataRow(EndCause.Expired)]
    public void BenchmarkThatDidNotFinishOnItsOwnShowsNoFinishedCard(EndCause cause)
    {
        var state = Handoff.State(wait: ["npmN"]) with
        {
            Recent = [Handoff.FinishedBenchmark() with { EndCause = cause }],
        };

        var snapshot = Snapshot(state);

        Assert.IsFalse(snapshot.Groups.Any(group => group.Kind == JobGroupKind.Benchmark));
    }

    [TestMethod]
    public void NormalRunInRecentShowsNoFinishedCard()
    {
        var state = Handoff.State(wait: ["npmN"]) with
        {
            Recent = [Handoff.FinishedBenchmark() with { Noise = null }],
        };

        var snapshot = Snapshot(state);

        Assert.IsFalse(snapshot.Groups.Any(group => group.Kind == JobGroupKind.Benchmark));
    }

    [TestMethod]
    public void FinishedRunOfALiveLeaseIsNotShownTwice()
    {
        var state = Handoff.State(bench: "run") with
        {
            Recent = [Handoff.FinishedBenchmark()],
        };

        var snapshot = Snapshot(state);

        Assert.IsNotNull(snapshot.Groups[0].Jobs.Single().Active);
    }

    [TestMethod]
    public void KillConfirmKeepsTheStepperWithTheRedLine()
    {
        var card = BenchCard(
            Handoff.State(bench: "run"),
            stop: new StopView(StopKind.Kill, StopPhase.Confirming, SecondsLeft: 3));

        Assert.AreEqual("Stops cargo.exe and its child processes", card.Detail);
        Assert.IsTrue(card.DetailIsDanger);
        Assert.AreEqual("3s", card.Time);
        Assert.AreEqual(DetailIcon.None, card.Icon);
        Assert.AreEqual(BenchmarkStep.Run, card.Steps?.Current);
        Assert.AreEqual(CardAction.Confirm, card.Action);
    }

    [TestMethod]
    public void RefusedKillShowsNoTime()
    {
        var card = BenchCard(
            Handoff.State(bench: "run"),
            stop: new StopView(StopKind.Kill, StopPhase.Failed, Message: "it is protected"));

        Assert.AreEqual("Not stopped: it is protected", card.Detail);
        Assert.AreEqual("", card.Time);
        Assert.IsNotNull(card.Steps);
        Assert.AreEqual(CardAction.Kill, card.Action);
    }

    [TestMethod]
    public void StoppedBenchmarkDropsTheStepper()
    {
        var card = BenchCard(
            Handoff.State(bench: "run"),
            stop: new StopView(StopKind.Kill, StopPhase.Ended, EndedAtMs: Handoff.NowMs));

        Assert.AreEqual("Stopped", card.Strong);
        Assert.AreEqual(" · Claude was notified", card.Detail);
        Assert.AreEqual("", card.Time);
        Assert.IsNull(card.Steps);
        Assert.AreEqual(DetailIcon.None, card.Icon);
        Assert.AreEqual(EdgeBarTone.None, card.BarTone);
        Assert.IsTrue(card.IsEnded);
    }

    [TestMethod]
    public void RemovingQueuedBenchmarkKeepsTheStepper()
    {
        var card = BenchCard(
            Handoff.State(bench: "quiet"),
            stop: new StopView(StopKind.Remove, StopPhase.Sending));

        Assert.AreEqual("Removing…", card.Detail);
        Assert.AreEqual(BenchmarkStep.Quiet, card.Steps?.Current);
        Assert.AreEqual(CardAction.None, card.Action);
    }

    [TestMethod]
    public void RemovedBenchmarkDropsTheStepper()
    {
        var card = BenchCard(
            Handoff.State(bench: "quiet"),
            stop: new StopView(StopKind.Remove, StopPhase.Ended));

        Assert.AreEqual("Removed", card.Strong);
        Assert.IsNull(card.Steps);
        Assert.AreEqual(EdgeBarTone.None, card.BarTone);
    }

    [TestMethod]
    public void LastKnownBenchmarkShowsWhereItWasWithoutTheStepper()
    {
        var snapshot = FlyoutSnapshot.From(ServiceStatus.Stopped, Handoff.State(bench: "run"));

        var card = snapshot.Groups.Single().Jobs.Single().Card;

        Assert.AreEqual("cargo.exe 30412 · was at 0:48 / ~2:00", card.Detail);
        Assert.IsNull(card.Steps);
        Assert.AreEqual(EdgeBarTone.None, card.BarTone);
        Assert.AreEqual(CardAction.None, card.Action);
    }

    [TestMethod]
    public void QuietTickChangesOnlyTheTicksAndTheLine()
    {
        var model = new FlyoutModel();
        model.Apply(Snapshot(Handoff.State(bench: "quiet")));
        var row = model.Rows.OfType<JobCardRow>().Single();
        var properties = new List<string?>();
        row.PropertyChanged += (_, args) => properties.Add(args.PropertyName);

        model.Apply(Snapshot(WithProgress(new BenchmarkProgress.Measuring
        {
            CpuAvgPct = 3,
            GpuAvgPct = 0,
            MeasuredSecs = 7,
            NeededSecs = 10,
        })));

        Assert.AreEqual(7, row.QuietTicks);
        Assert.AreEqual("Detail QuietTicks Card", string.Join(" ", properties));
    }

    [TestMethod]
    public void PhaseChangeMovesTheStepper()
    {
        var model = new FlyoutModel();
        model.Apply(Snapshot(Handoff.State(bench: "quiet")));
        var row = model.Rows.OfType<JobCardRow>().Single();

        model.Apply(Snapshot(Handoff.State(bench: "run")));

        Assert.AreSame(row, model.Rows.OfType<JobCardRow>().Single());
        Assert.AreEqual(BenchmarkStep.Run, row.Step);
        Assert.IsTrue(row.HasSteps);
        Assert.AreEqual(0.4, row.RunFraction, Tolerance);
        Assert.AreEqual(CardAction.Kill, row.Action);
    }

    // The quiet benchmark alone, with its progress replaced.
    private static UiState WithProgress(BenchmarkProgress progress)
    {
        var state = Handoff.State(bench: "quiet");
        var queued = state.Queue[0];
        return state with
        {
            Queue = [queued with { Verdict = queued.Verdict with { Benchmark = progress } }],
        };
    }

    // The benchmark alone at the head of the queue, without progress from rhd.
    private static UiState WithoutProgress(string[] reasons)
    {
        var state = Handoff.State(bench: "quiet");
        var queued = state.Queue[0];
        var verdict = queued.Verdict with { Benchmark = null, Reasons = reasons };
        return state with { Queue = [queued with { Verdict = verdict }] };
    }

    private static FlyoutSnapshot Snapshot(UiState state, long? nowMs = null) =>
        FlyoutSnapshot.From(ServiceStatus.Running, state, nowMs: nowMs);

    private static JobCard BenchCard(UiState state, long? nowMs = null, StopView? stop = null)
    {
        var stops = new Dictionary<long, StopView>();
        if (stop is not null)
        {
            stops[Handoff.LeaseId("bench")] = stop;
        }

        return FlyoutSnapshot.From(ServiceStatus.Running, state, stops, nowMs)
            .Groups.Single(group => group.Kind == JobGroupKind.Benchmark)
            .Jobs.Single()
            .Card;
    }
}
