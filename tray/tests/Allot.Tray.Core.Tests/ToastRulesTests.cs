using Allot.Tray.Core.Contract;
using Allot.Tray.Core.Daemon;
using Allot.Tray.Core.Notifications;
using Allot.Tray.Core.Samples;
using Allot.Tray.Core.Settings;
using Allot.Tray.Core.Tests.Support;

namespace Allot.Tray.Core.Tests;

/// <summary>
/// When each toast of handoff section 9 fires, and its text.
/// </summary>
[TestClass]
public sealed class ToastRulesTests
{
    private static readonly NotificationToggles AllOn = new()
    {
        JobFinished = true,
        KilledOutsideFlyout = true,
    };

    private static readonly NotificationToggles AllOff = new()
    {
        ServiceStopped = false,
        RunningLong = false,
        BenchmarkFinished = false,
    };

    private readonly ManualTimeProvider _time = new();
    private readonly ToastRules _rules;
    private UiState? _lastKnown;

    public ToastRulesTests() => _rules = new ToastRules(_time);

    [TestMethod]
    public void FirstLiveStateIsABaseline()
    {
        var state = Busy(at: 300) with
        {
            Recent = [Run(20, "cargo test", endedAtMs: Handoff.NowMs)],
            Notices = [Killed(21, "cargo bench")],
        };

        Assert.AreEqual("", Titles(Poll(state)));
        Assert.AreEqual("", Titles(Poll(state)));
    }

    [TestMethod]
    public void RunningLongFiresOnceAtOneAndAHalfTimesTheEstimate()
    {
        _ = Poll(Busy(at: 0));

        var before = Poll(Busy(at: 199));
        var at = Poll(Busy(at: 200));
        var after = Poll(Busy(at: 260));

        Assert.AreEqual("", Titles(before));
        Assert.AreEqual(
            new ToastMessage(
                ToastKind.RunningLong,
                "long-2",
                "python train.py --epochs 20 --data ./shards is running long",
                "15:00 so far against a ~10:00 estimate · b81d44f0 · ml-trainer",
                at.Show[0].Buttons,
                "job:2"),
            at.Show.Single());
        Assert.AreEqual("Show", Buttons(at.Show[0]));
        Assert.AreEqual("", Titles(after));
    }

    [TestMethod]
    public void JobWithoutAnEstimateNeverRunsLong()
    {
        var state = Busy(at: 0);
        var job = state.Active[1];
        var unknown = job with
        {
            Lease = job.Lease with { Profile = job.Lease.Profile with { EstSecs = 0 } },
        };
        _ = Poll(state with { Active = [unknown] });

        var dayLater = state.GeneratedAtMs + 86_400_000;
        var update = Poll(state with { Active = [unknown], GeneratedAtMs = dayLater });

        Assert.AreEqual("", Titles(update));
    }

    [TestMethod]
    public void ServiceStoppedNeedsTwoStoppedPolls()
    {
        _ = Poll(Busy(at: 0));

        var first = Poll(null, ServiceStatus.Stopped);
        var second = Poll(null, ServiceStatus.Stopped);
        var third = Poll(null, ServiceStatus.Stopped);

        Assert.AreEqual("", Titles(first));
        Assert.AreEqual(
            new ToastMessage(
                ToastKind.ServiceStopped,
                "service",
                "Service stopped",
                "Heavy commands now run unchecked. 3 Claude sessions are connected.",
                second.Show[0].Buttons,
                null),
            second.Show.Single());
        Assert.AreEqual("StartService OpenLogs", Buttons(second.Show[0]));
        Assert.AreEqual("", Titles(third));
    }

    [TestMethod]
    public void ServiceStoppedToastIsWithdrawnWhenTheServiceIsBack()
    {
        _ = Poll(Busy(at: 0));
        _ = Poll(null, ServiceStatus.Stopped);
        _ = Poll(null, ServiceStatus.Stopped);

        var back = Poll(Busy(at: 10));
        var again = Poll(Busy(at: 11));

        Assert.AreEqual("service", string.Join(" ", back.Withdraw));
        Assert.AreSame(ToastUpdate.None, again);
    }

    [TestMethod]
    [DataRow(1, "1 Claude session is connected.")]
    [DataRow(0, "No Claude sessions are connected.")]
    public void ServiceStoppedCountsTheLastKnownSessions(int sessions, string expected)
    {
        var state = Busy(at: 0);
        _ = Poll(state with { Sessions = state.Sessions.Take(sessions).ToList() });
        _ = Poll(null, ServiceStatus.Stopped);

        var update = Poll(null, ServiceStatus.Stopped);

        Assert.AreEqual($"Heavy commands now run unchecked. {expected}", update.Show[0].Body);
    }

    [TestMethod]
    public void RestartSeenOnOnePollDoesNotToast()
    {
        _ = Poll(Busy(at: 0));

        var updates = new[]
        {
            Poll(null, ServiceStatus.Stopped),
            Poll(null, ServiceStatus.Starting),
            Poll(Busy(at: 3)),
        };

        Assert.AreEqual("", string.Concat(updates.Select(Titles)));
    }

    [TestMethod]
    public void ServiceDownAtStartupOrAfterAFailedStartDoesNotToast()
    {
        var updates = new[]
        {
            Poll(null, ServiceStatus.Stopped),
            Poll(null, ServiceStatus.Stopped),
            Poll(null, ServiceStatus.Starting),
            Poll(null, ServiceStatus.Stopped),
            Poll(null, ServiceStatus.Stopped),
        };

        Assert.AreEqual("", string.Concat(updates.Select(Titles)));
    }

    [TestMethod]
    public void StopChosenFromTheTrayMenuDoesNotToast()
    {
        _ = Poll(Busy(at: 0));
        _rules.ExpectStop();
        _ = Poll(Busy(at: 1));
        _ = Poll(null, ServiceStatus.Stopped);

        var chosen = Poll(null, ServiceStatus.Stopped);
        _ = Poll(Busy(at: 30));
        _ = Poll(null, ServiceStatus.Stopped);
        var later = Poll(null, ServiceStatus.Stopped);

        Assert.AreEqual("", Titles(chosen));
        Assert.AreEqual("Service stopped", Titles(later));
    }

    [TestMethod]
    public void StopLongAfterChoosingStopServiceToasts()
    {
        _ = Poll(Busy(at: 0));
        _rules.ExpectStop();
        _time.Now += ToastRules.ExpectedStopWindow + TimeSpan.FromSeconds(1);
        _ = Poll(null, ServiceStatus.Stopped);

        var update = Poll(null, ServiceStatus.Stopped);

        Assert.AreEqual("Service stopped", Titles(update));
    }

    [TestMethod]
    public void JobFinishedNamesTheRunItsSessionAndProject()
    {
        _ = Poll(Idle(at: 0));

        var update = Poll(Idle(at: 5, Run(30, "cargo build --release", Handoff.NowMs)));

        Assert.AreEqual(
            new ToastMessage(
                ToastKind.JobFinished,
                "done-1",
                "cargo build --release finished",
                "4:52 · 3f2a9c1e · allot",
                update.Show[0].Buttons,
                "recent"),
            update.Show.Single());
        Assert.AreEqual("Show", Buttons(update.Show[0]));
    }

    [TestMethod]
    public void FinishesWithinSixtySecondsMergeIntoOneToast()
    {
        var cargo = Run(30, "cargo build --release", Handoff.NowMs);
        var vite = Run(31, "pnpm build", Handoff.NowMs + 30_000);
        var go = Run(32, "go vet ./...", Handoff.NowMs + 59_000);
        var test = Run(33, "cargo test", Handoff.NowMs + 61_000);
        _ = Poll(Idle(at: 0));

        var first = Poll(Idle(at: 1, cargo));
        _time.Now += TimeSpan.FromSeconds(30);
        var second = Poll(Idle(at: 31, vite, cargo));
        _time.Now += TimeSpan.FromSeconds(29);
        var third = Poll(Idle(at: 60, go, vite, cargo));
        _time.Now += TimeSpan.FromSeconds(1);
        var fourth = Poll(Idle(at: 61, test, go, vite, cargo));

        Assert.AreEqual("done-1: cargo build --release finished", Tagged(first));
        Assert.AreEqual("done-1: 2 jobs finished", Tagged(second));
        Assert.AreEqual("cargo build --release and pnpm build", second.Show[0].Body);
        Assert.AreEqual("done-1: 3 jobs finished", Tagged(third));
        Assert.AreEqual("cargo build --release, pnpm build and 1 more", third.Show[0].Body);
        Assert.AreEqual("done-2: cargo test finished", Tagged(fourth));
    }

    [TestMethod]
    public void FinishesOfOnePollMergeOldestFirst()
    {
        _ = Poll(Idle(at: 0));

        var update = Poll(Idle(
            at: 5,
            Run(32, "go vet ./...", Handoff.NowMs + 3_000),
            Run(31, "pnpm build", Handoff.NowMs + 2_000),
            Run(30, "cargo build --release", Handoff.NowMs + 1_000)));

        Assert.AreEqual("done-1: 3 jobs finished", Tagged(update));
        Assert.AreEqual("cargo build --release, pnpm build and 1 more", update.Show[0].Body);
    }

    [TestMethod]
    public void KilledRunIsNotAFinish()
    {
        _ = Poll(Idle(at: 0));

        var update = Poll(Idle(
            at: 5,
            Run(30, "cargo build", Handoff.NowMs, cause: EndCause.Killed)));

        Assert.AreEqual("", Titles(update));
    }

    [TestMethod]
    public void BenchmarkFinishedGivesItsTimeAndNoise()
    {
        _ = Poll(Idle(at: 0));

        var update = Poll(Idle(at: 5, Handoff.FinishedBenchmark()));

        Assert.AreEqual(
            new ToastMessage(
                ToastKind.BenchmarkFinished,
                "bench-9",
                "Benchmark finished in 2:04",
                "Background noise avg 1.8%, peak 6.2% CPU · cargo bench --bench parser",
                update.Show[0].Buttons,
                "job:9"),
            update.Show.Single());
        Assert.AreEqual("", Buttons(update.Show[0]));
    }

    [TestMethod]
    public void BenchmarkNotQuietAtStartSaysSo()
    {
        _ = Poll(Idle(at: 0));

        var update = Poll(Idle(at: 5, Handoff.FinishedBenchmark(quietAtStart: false)));

        Assert.AreEqual(
            "Background noise avg 1.8%, peak 6.2% CPU · not quiet at start · "
                + "cargo bench --bench parser",
            update.Show[0].Body);
    }

    [TestMethod]
    public void KillFromOutsideTheFlyoutToasts()
    {
        _ = Poll(Idle(at: 0));

        var update = Poll(Idle(at: 5) with { Notices = [Killed(40, "cargo build --release")] });

        Assert.AreEqual(
            new ToastMessage(
                ToastKind.KilledOutsideFlyout,
                "killed-40",
                "cargo build --release was stopped",
                "Killed from the command line · Claude was notified",
                update.Show[0].Buttons,
                null),
            update.Show.Single());
        Assert.AreEqual("", Buttons(update.Show[0]));
    }

    [TestMethod]
    public void KillFromThisFlyoutAndRemovalsDoNotToast()
    {
        _ = Poll(Idle(at: 0));
        _rules.NoteLocalKill(41);

        var update = Poll(Idle(at: 5) with
        {
            Notices =
            [
                Killed(41, "cargo build"),
                Killed(42, "npm run test:e2e") with { Cause = EndCause.Removed },
            ],
        });

        Assert.AreEqual("", Titles(update));
    }

    [TestMethod]
    public void JobFinishedAndKilledOutsideAreOffByDefault()
    {
        var defaults = new NotificationToggles();
        _ = Poll(Idle(at: 0), toggles: defaults);

        var update = Poll(
            Idle(at: 5, Run(30, "cargo build", Handoff.NowMs)) with
            {
                Notices = [Killed(40, "cargo test")],
            },
            toggles: defaults);

        Assert.AreEqual("", Titles(update));
    }

    [TestMethod]
    public void EventsWhileTheirToggleIsOffNeverToastLater()
    {
        _ = Poll(Busy(at: 0), toggles: AllOff);
        var events = Busy(at: 200) with
        {
            Recent = [Run(30, "cargo test", Handoff.NowMs), Handoff.FinishedBenchmark()],
            Notices = [Killed(40, "cargo bench")],
        };

        var off = Poll(events, toggles: AllOff);
        _ = Poll(null, ServiceStatus.Stopped, toggles: AllOff);
        var stoppedOff = Poll(null, ServiceStatus.Stopped, toggles: AllOff);
        var on = Poll(events with { GeneratedAtMs = events.GeneratedAtMs + 10_000 });

        Assert.AreEqual("", Titles(off) + Titles(stoppedOff) + Titles(on));
    }

    [TestMethod]
    public void EventsWhileTheFlyoutIsOpenCountAsSeen()
    {
        _ = Poll(Busy(at: 0));
        var events = Busy(at: 200) with { Notices = [Killed(40, "cargo bench")] };

        var open = Poll(events, open: true);
        var closed = Poll(events with { GeneratedAtMs = events.GeneratedAtMs + 10_000 });

        Assert.AreEqual("", Titles(open) + Titles(closed));
    }

    [TestMethod]
    public void EachToastFiresAloneUnderItsOwnToggle()
    {
        var toggles = new[]
        {
            AllOff with { RunningLong = true },
            AllOff with { BenchmarkFinished = true },
            AllOff with { JobFinished = true },
            AllOff with { KilledOutsideFlyout = true },
        };
        var kinds = new List<string>();
        foreach (var toggle in toggles)
        {
            var rules = new ToastRules(_time);
            _ = rules.Observe(ServiceStatus.Running, Busy(at: 0), null, toggle, false);
            var events = Busy(at: 200) with
            {
                Recent = [Run(30, "cargo test", Handoff.NowMs), Handoff.FinishedBenchmark()],
                Notices = [Killed(40, "cargo bench")],
            };
            var update = rules.Observe(ServiceStatus.Running, events, null, toggle, false);
            kinds.Add(string.Join("+", update.Show.Select(toast => toast.Kind)));
        }

        Assert.AreEqual(
            "RunningLong BenchmarkFinished JobFinished KilledOutsideFlyout",
            string.Join(" ", kinds));
    }

    // cargo (186 of 300 s) and python (700 of 600 s, so 1.5x is 200 s later), and npm queued:
    // three sessions.
    private static UiState Busy(long at) =>
        Handoff.State(run: ["cargo", "py"], wait: ["npm"]) with
        {
            GeneratedAtMs = Handoff.NowMs + (at * 1000),
        };

    private static UiState Idle(long at, params RecentRun[] recent) =>
        Handoff.State() with
        {
            GeneratedAtMs = Handoff.NowMs + (at * 1000),
            Recent = recent,
            Sessions = [Handoff.Session(Session, @"E:\code\allot", 0.1f, 300)],
        };

    private const string Session = "3f2a9c1e-1111-4000-8000-000000000000";

    private static RecentRun Run(
        long lease,
        string command,
        long endedAtMs,
        EndCause cause = EndCause.Released) =>
        new()
        {
            Command = command,
            Cwd = "",
            Declared = new JobProfile
            {
                Cores = 1,
                EstSecs = 300,
                GpuPct = 0,
                RamMb = 0,
                VramMb = 0,
            },
            EndCause = cause,
            EndedAtMs = endedAtMs,
            LeaseId = lease,
            Session = Session,
            StartedAtMs = endedAtMs - 292_000,
        };

    private static Notice Killed(long lease, string command) =>
        new()
        {
            Cause = EndCause.Killed,
            Command = command,
            EndedAtMs = Handoff.NowMs,
            LeaseId = lease,
            Session = Session,
        };

    private ToastUpdate Poll(
        UiState? live,
        ServiceStatus status = ServiceStatus.Running,
        NotificationToggles? toggles = null,
        bool open = false)
    {
        _lastKnown = live ?? _lastKnown;
        return _rules.Observe(status, live, _lastKnown, toggles ?? AllOn, open);
    }

    private static string Titles(ToastUpdate update) =>
        string.Join(" | ", update.Show.Select(toast => toast.Title));

    private static string Tagged(ToastUpdate update) =>
        string.Join(" | ", update.Show.Select(toast => $"{toast.Tag}: {toast.Title}"));

    private static string Buttons(ToastMessage toast) => string.Join(" ", toast.Buttons);
}
