using ResourcesHelper.Tray.Core.Contract;
using ResourcesHelper.Tray.Core.Daemon;
using ResourcesHelper.Tray.Core.Samples;
using ResourcesHelper.Tray.Core.Tests.Support;
using ResourcesHelper.Tray.Core.ViewModels;

namespace ResourcesHelper.Tray.Core.Tests;

/// <summary>
/// The Kill and Remove state machine (handoff section 4): the 4 s Kill? countdown, the 300 ms
/// double-press guard, Esc and focus loss, the 4 s Stopped hold and the 6 s refusal.
/// </summary>
[TestClass]
public sealed class JobStopsTests
{
    private static readonly long Cargo = Handoff.LeaseId("cargo");
    private static readonly long Npm = Handoff.LeaseId("npm");

    private readonly ManualTimeProvider _time = new();
    private readonly JobStops _stops;

    public JobStopsTests()
    {
        _stops = new JobStops(_time);
        _stops.Observe(Several());
    }

    [TestMethod]
    public void FirstPressAsksWithFourSecondsLeft()
    {
        Assert.AreEqual(StopPress.Confirming, _stops.PressKill(Cargo));

        Assert.AreEqual(
            new StopView(StopKind.Kill, StopPhase.Confirming, 4),
            _stops.Views[Cargo]);
    }

    [TestMethod]
    [DataRow(999, 4)]
    [DataRow(1000, 3)]
    [DataRow(2500, 2)]
    [DataRow(3999, 1)]
    public void CountdownTicksEachSecond(int afterMs, int left)
    {
        _ = _stops.PressKill(Cargo);

        Wait(afterMs);
        _ = _stops.Expire();

        Assert.AreEqual(left, _stops.Views[Cargo].SecondsLeft);
    }

    [TestMethod]
    public void ConfirmRevertsAfterFourSeconds()
    {
        _ = _stops.PressKill(Cargo);

        Wait(4000);

        Assert.IsTrue(_stops.Expire());
        Assert.IsFalse(_stops.Views.ContainsKey(Cargo));
    }

    [TestMethod]
    [DataRow(0)]
    [DataRow(150)]
    [DataRow(300)]
    public void SecondPressWithin300MsIsIgnored(int afterMs)
    {
        _ = _stops.PressKill(Cargo);

        Wait(afterMs);

        Assert.AreEqual(StopPress.Ignored, _stops.PressKill(Cargo));
        Assert.AreEqual(StopPhase.Confirming, _stops.Views[Cargo].Phase);
    }

    [TestMethod]
    public void SecondPressAfter300MsSends()
    {
        _ = _stops.PressKill(Cargo);

        Wait(301);

        Assert.AreEqual(StopPress.Send, _stops.PressKill(Cargo));
        Assert.AreEqual(StopPhase.Sending, _stops.Views[Cargo].Phase);
    }

    [TestMethod]
    public void PressAfterTheCountdownAsksAgain()
    {
        _ = _stops.PressKill(Cargo);

        Wait(4200);

        Assert.AreEqual(StopPress.Confirming, _stops.PressKill(Cargo));
        Assert.AreEqual(4, _stops.Views[Cargo].SecondsLeft);
    }

    [TestMethod]
    public void EscOrFocusLossCancelsTheConfirm()
    {
        _ = _stops.PressKill(Cargo);

        Assert.IsTrue(_stops.Cancel(Cargo));
        Assert.IsEmpty(_stops.Views);
        Assert.IsFalse(_stops.Cancel(Cargo), "a second cancel changes nothing");
    }

    [TestMethod]
    public void CancelLeavesAStopUnderWay()
    {
        Send(Cargo);

        Assert.IsFalse(_stops.Cancel(Cargo));
        Assert.AreEqual(StopPhase.Sending, _stops.Views[Cargo].Phase);
    }

    [TestMethod]
    public void ClosingTheFlyoutCancelsEveryConfirmOnly()
    {
        _ = _stops.PressKill(Cargo);
        _ = _stops.PressKill(Handoff.LeaseId("py"));
        _ = _stops.PressRemove(Npm);

        Assert.IsTrue(_stops.CancelConfirms());

        Assert.AreEqual(Npm, _stops.Views.Keys.Single());
        Assert.IsFalse(_stops.CancelConfirms());
    }

    [TestMethod]
    public void PressesDuringASendAreIgnored()
    {
        Send(Cargo);

        Wait(1000);

        Assert.AreEqual(StopPress.Ignored, _stops.PressKill(Cargo));
        Assert.AreEqual(StopPhase.Sending, _stops.Views[Cargo].Phase);
    }

    [TestMethod]
    public void KillEndsAndHoldsForFourSeconds()
    {
        Send(Cargo);
        _stops.Finished(Cargo, new StopResult(true, "", 1_792_824_720_000));

        Assert.AreEqual(
            new StopView(StopKind.Kill, StopPhase.Ended, EndedAtMs: 1_792_824_720_000),
            _stops.Views[Cargo]);
        Wait(3999);
        Assert.IsFalse(_stops.Expire());
        Wait(1);
        Assert.IsTrue(_stops.Expire());
        Assert.IsEmpty(_stops.Views);
    }

    [TestMethod]
    public void RefusalHoldsForSixSecondsAndCanBePressedAgain()
    {
        Send(Cargo);
        _stops.Finished(Cargo, StopResult.Refused("protected"));

        Assert.AreEqual(
            new StopView(StopKind.Kill, StopPhase.Failed, Message: "protected"),
            _stops.Views[Cargo]);
        Assert.AreEqual(StopPress.Confirming, _stops.PressKill(Cargo));

        Wait(500);
        Assert.AreEqual(StopPress.Send, _stops.PressKill(Cargo));
        _stops.Finished(Cargo, StopResult.Refused("protected"));
        Wait(5999);
        Assert.IsFalse(_stops.Expire());
        Wait(1);
        Assert.IsTrue(_stops.Expire());
    }

    [TestMethod]
    public void RemoveSendsAtOnceWithoutAConfirm()
    {
        Assert.IsTrue(_stops.PressRemove(Npm));
        Assert.AreEqual(new StopView(StopKind.Remove, StopPhase.Sending), _stops.Views[Npm]);
        Assert.IsFalse(_stops.PressRemove(Npm), "a second press while sending is ignored");
    }

    [TestMethod]
    public void KillOnlyActsOnRunningJobsAndRemoveOnQueuedOnes()
    {
        Assert.AreEqual(StopPress.Ignored, _stops.PressKill(Npm));
        Assert.IsFalse(_stops.PressRemove(Cargo));
        Assert.AreEqual(StopPress.Ignored, _stops.PressKill(999));
    }

    [TestMethod]
    public void AnswerWithoutASendIsIgnored()
    {
        _ = _stops.PressKill(Cargo);

        _stops.Finished(Cargo, new StopResult(true, ""));

        Assert.AreEqual(StopPhase.Confirming, _stops.Views[Cargo].Phase);
    }

    [TestMethod]
    public void ConfirmEndsWhenTheJobEnds()
    {
        _ = _stops.PressKill(Cargo);

        _stops.Observe(Handoff.State(run: ["py"], wait: ["npm"]));

        Assert.IsEmpty(_stops.Views);
    }

    [TestMethod]
    public void RefusedRemoveEndsWhenTheRequestStarts()
    {
        _ = _stops.PressRemove(Npm);
        _stops.Finished(Npm, StopResult.Refused("lease 5 is active; kill it instead"));

        var started = Handoff.State(run: ["cargo", "py"]);
        var npm = Handoff.State(wait: ["npm"]).Queue[0].Lease;
        _stops.Observe(started with
        {
            Active = [.. started.Active, new ActiveJob { Lease = npm }],
        });

        Assert.IsEmpty(_stops.Views);
    }

    [TestMethod]
    public void EndedJobIsHeldWhereItWasAfterRhdDropsIt()
    {
        Send(Cargo);
        var after = Handoff.State(run: ["py"], wait: ["npm"], idle: true);
        _stops.Observe(after);
        _stops.Finished(Cargo, new StopResult(true, "", Handoff.NowMs));

        var merged = _stops.Merge(after);

        Assert.AreEqual(
            $"{Cargo} {Handoff.LeaseId("py")}",
            string.Join(" ", merged.Active.Select(job => job.Lease.Id)));
        Assert.AreEqual(Npm, merged.Queue.Single().Lease.Id);
    }

    [TestMethod]
    public void RemovedRequestIsHeldInTheQueue()
    {
        _ = _stops.PressRemove(Npm);
        var after = Handoff.State(run: ["cargo", "py"], idle: true);
        _stops.Observe(after);

        var merged = _stops.Merge(after);

        Assert.AreEqual(Npm, merged.Queue.Single().Lease.Id);
    }

    [TestMethod]
    public void MergeKeepsTheStateWhileRhdStillListsTheJob()
    {
        Send(Cargo);
        var state = Several();

        Assert.AreSame(state, _stops.Merge(state));
    }

    [TestMethod]
    public void HeldJobGoesOnceTheHoldEnds()
    {
        Send(Cargo);
        _stops.Finished(Cargo, new StopResult(true, ""));
        var after = Handoff.State(run: ["py"], wait: ["npm"]);
        _stops.Observe(after);

        Wait(4000);
        _ = _stops.Expire();

        Assert.AreSame(after, _stops.Merge(after));
    }

    [TestMethod]
    public void NextChangeIsTheNextCountdownSecond()
    {
        _ = _stops.PressKill(Cargo);

        Wait(400);

        Assert.AreEqual(TimeSpan.FromMilliseconds(600), _stops.NextChange);
    }

    [TestMethod]
    public void NextChangeIsTheEarliestHoldEnd()
    {
        Send(Cargo);
        _stops.Finished(Cargo, StopResult.Refused("protected"));
        Wait(1000);
        _ = _stops.PressRemove(Npm);
        _stops.Finished(Npm, new StopResult(true, ""));

        Assert.AreEqual(TimeSpan.FromSeconds(4), _stops.NextChange);
    }

    [TestMethod]
    public void NothingChangesByItselfWhileSending()
    {
        Assert.IsNull(_stops.NextChange);
        Send(Cargo);
        Assert.IsNull(_stops.NextChange);
    }

    private static UiState Several() =>
        Handoff.State(run: ["cargo", "py"], wait: ["npm"], idle: true);

    private void Send(long lease)
    {
        Assert.AreEqual(StopPress.Confirming, _stops.PressKill(lease));
        Wait(500);
        Assert.AreEqual(StopPress.Send, _stops.PressKill(lease));
    }

    private void Wait(int milliseconds) => _time.Now += TimeSpan.FromMilliseconds(milliseconds);
}
