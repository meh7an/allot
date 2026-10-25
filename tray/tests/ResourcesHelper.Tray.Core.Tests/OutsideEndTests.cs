using ResourcesHelper.Tray.Core.Contract;
using ResourcesHelper.Tray.Core.Daemon;
using ResourcesHelper.Tray.Core.Samples;
using ResourcesHelper.Tray.Core.Tests.Support;
using ResourcesHelper.Tray.Core.ViewModels;

namespace ResourcesHelper.Tray.Core.Tests;

/// <summary>
/// Running jobs that end without this flyout stopping them hold their card for 4 s, as Stopped
/// does: Finished, Stopped outside the flyout, or Ended by rhd, from rhd's ended
/// list.
/// </summary>
[TestClass]
public sealed class OutsideEndTests
{
    private static readonly long Cargo = Handoff.LeaseId("cargo");
    private static readonly long Npm = Handoff.LeaseId("npm");

    private readonly ManualTimeProvider _time = new();
    private readonly JobStops _stops;

    public OutsideEndTests()
    {
        _stops = new JobStops(_time);
        _stops.Observe(Several());
    }

    [TestMethod]
    [DataRow(EndCause.Released, StopKind.Finished)]
    [DataRow(EndCause.Killed, StopKind.KilledOutside)]
    [DataRow(EndCause.Expired, StopKind.EndedByRhd)]
    public void ARunningJobThatEndsElsewhereHolds(EndCause cause, StopKind kind)
    {
        _stops.Observe(Without(Cargo, cause));

        var view = _stops.Views[Cargo];
        Assert.AreEqual(kind, view.Kind);
        Assert.AreEqual(StopPhase.Ended, view.Phase);
        Assert.AreEqual(Handoff.NowMs, view.EndedAtMs);
    }

    [TestMethod]
    public void OnlyRhdsOwnEndKeepsItsReason()
    {
        _stops.Observe(Without(Cargo, EndCause.Expired));
        Assert.AreEqual("no command running", _stops.Views[Cargo].Message);

        var other = new JobStops(_time);
        other.Observe(Several());
        other.Observe(Without(Cargo, EndCause.Released));
        Assert.AreEqual("", other.Views[Cargo].Message);
    }

    [TestMethod]
    public void TheHeldCardStaysWhereItWas()
    {
        var state = Without(Cargo, EndCause.Released);
        _stops.Observe(state);

        var merged = _stops.Merge(state);

        Assert.AreEqual(Cargo, merged.Active[0].Lease.Id);
        Assert.AreEqual(2, merged.Active.Count);
    }

    [TestMethod]
    public void TheHoldEndsAfterFourSeconds()
    {
        _stops.Observe(Without(Cargo, EndCause.Released));

        _time.Now += JobStops.EndedFor;

        Assert.IsTrue(_stops.Expire());
        Assert.AreEqual(0, _stops.Views.Count);
    }

    [TestMethod]
    [DataRow(EndCause.Removed)]
    public void RemovedRunningJobsAreNotHeld(EndCause cause)
    {
        _stops.Observe(Without(Cargo, cause));

        Assert.IsFalse(_stops.Views.ContainsKey(Cargo));
    }

    [TestMethod]
    public void QueuedRequestsThatEndAreNotHeld()
    {
        var state = Several();
        _stops.Observe(state with
        {
            Queue = [],
            Ended = [Ended(Npm, EndCause.Released)],
        });

        Assert.IsFalse(_stops.Views.ContainsKey(Npm));
    }

    [TestMethod]
    public void AnOlderRhdWithoutAnEndedListHoldsNothing()
    {
        var state = Several();
        _stops.Observe(state with { Active = state.Active.Skip(1).ToList() });

        Assert.AreEqual(0, _stops.Views.Count);
    }

    [TestMethod]
    public void AKillFromThisFlyoutKeepsItsOwnStop()
    {
        Assert.AreEqual(StopPress.Confirming, _stops.PressKill(Cargo));
        _time.Now += TimeSpan.FromMilliseconds(500);
        Assert.AreEqual(StopPress.Send, _stops.PressKill(Cargo));

        _stops.Observe(Without(Cargo, EndCause.Killed));

        Assert.AreEqual(StopKind.Kill, _stops.Views[Cargo].Kind);
    }

    [TestMethod]
    public void FinishedHasTheCheckAndItsRunTimeAndIsNotStruck()
    {
        var card = CardOf(
            new StopView(StopKind.Finished, StopPhase.Ended, EndedAtMs: Handoff.NowMs));

        Assert.AreEqual("Finished", card.Strong);
        Assert.AreEqual(" · 3:06", card.Detail);
        Assert.AreEqual("", card.Time);
        Assert.AreEqual(DetailIcon.Check, card.Icon);
        Assert.AreEqual(EdgeBarTone.Frozen, card.BarTone);
        Assert.AreEqual(CardAction.None, card.Action);
        Assert.IsTrue(card.IsEnded);
        Assert.IsFalse(card.IsStruck);
    }

    [TestMethod]
    public void AKillFromElsewhereReadsAsAStop()
    {
        var card = CardOf(
            new StopView(StopKind.KilledOutside, StopPhase.Ended, EndedAtMs: Handoff.NowMs));

        Assert.AreEqual("Stopped outside the flyout", card.Strong);
        Assert.AreEqual(" · Claude was notified", card.Detail);
        Assert.AreEqual("at 3:06", card.Time);
        Assert.IsTrue(card.IsStruck);
    }

    [TestMethod]
    public void RhdsEndGivesItsReason()
    {
        var card = CardOf(new StopView(
            StopKind.EndedByRhd,
            StopPhase.Ended,
            Message: "no command running",
            EndedAtMs: Handoff.NowMs));

        Assert.AreEqual("Ended by rhd", card.Strong);
        Assert.AreEqual(" · no command running", card.Detail);
        Assert.AreEqual("no command running", card.DetailTooltip);
        Assert.IsTrue(card.IsEnded);
        Assert.IsFalse(card.IsStruck);
    }

    [TestMethod]
    public void HeldEndsLeaveTheSummary()
    {
        var state = Without(Cargo, EndCause.Released);
        _stops.Observe(state);

        var snapshot = FlyoutSnapshot.From(
            ServiceStatus.Running, _stops.Merge(state), _stops.Views, Handoff.NowMs);

        Assert.AreEqual("1 running · 1 waiting", snapshot.Header.Summary);
    }

    private static UiState Several() =>
        Handoff.State(run: ["cargo", "py"], wait: ["npm"], idle: true);

    private static UiState Without(long lease, EndCause cause)
    {
        var state = Several();
        return state with
        {
            Active = state.Active.Where(job => job.Lease.Id != lease).ToList(),
            Ended = [Ended(lease, cause)],
        };
    }

    private static EndedLease Ended(long lease, EndCause cause) =>
        new()
        {
            LeaseId = lease,
            Session = Handoff.SessionOf("cargo"),
            Command = "cargo build",
            Cause = cause,
            Reason = "no command running",
            GrantedAtMs = Handoff.NowMs - 186_000,
            EndedAtMs = Handoff.NowMs,
        };

    private static JobCard CardOf(StopView stop) =>
        JobCard.ForActive(Handoff.State(run: ["cargo"]).Active[0], Handoff.NowMs, false, stop);
}
