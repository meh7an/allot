using ResourcesHelper.Tray.Core.Daemon;
using ResourcesHelper.Tray.Core.Samples;
using ResourcesHelper.Tray.Core.ViewModels;

namespace ResourcesHelper.Tray.Core.Tests;

/// <summary>
/// The Kill?, Stopped and Removed cards of the handoff's <c>kill-confirm</c>, <c>killed</c>,
/// <c>c-confirm</c>, <c>c-killed</c> and <c>c-removed</c> scenarios (<c>Flyout.dc.html</c>), plus
/// the Stopping, Removing and refused lines this tray adds.
/// </summary>
[TestClass]
public sealed class StopCardTests
{
    private const double Tolerance = 0.0001;

    [TestMethod]
    [DataRow("kill-confirm")]
    [DataRow("c-confirm")]
    public void ConfirmAsksInRedWithTheCountdown(string scenario)
    {
        var card = CardOf(scenario, "cargo");

        Assert.AreEqual("", card.Strong);
        Assert.AreEqual("Stops cargo.exe and its child processes", card.Detail);
        Assert.IsTrue(card.DetailIsDanger);
        Assert.AreEqual("3s", card.Time);
        Assert.IsFalse(card.TimeIsWarning);
        Assert.AreEqual(CardAction.Confirm, card.Action);
        Assert.AreEqual("Confirm kill cargo.exe", card.ActionName);
        Assert.IsFalse(card.IsEnded);
    }

    [TestMethod]
    public void ConfirmKeepsTheRunningBar()
    {
        var card = CardOf("c-confirm", "cargo");

        Assert.AreEqual(EdgeBarTone.Running, card.BarTone);
        Assert.AreEqual(186.0 / 300, card.BarFraction, Tolerance);
    }

    [TestMethod]
    [DataRow("killed")]
    [DataRow("c-killed")]
    public void StoppedCardIsStruckGrayAndNotified(string scenario)
    {
        var card = CardOf(scenario, "cargo");

        Assert.AreEqual("Stopped", card.Strong);
        Assert.AreEqual(" · Claude was notified", card.Detail);
        Assert.IsFalse(card.DetailIsDanger);
        Assert.AreEqual("at 3:06", card.Time);
        Assert.AreEqual(EdgeBarTone.Frozen, card.BarTone);
        Assert.AreEqual(186.0 / 300, card.BarFraction, Tolerance);
        Assert.AreEqual(CardAction.None, card.Action);
        Assert.IsTrue(card.IsEnded);
    }

    [TestMethod]
    public void RemovedCardHasNoTimeAndNoBar()
    {
        var card = CardOf("c-removed", "npm");

        Assert.AreEqual("#1", card.Tile);
        Assert.AreEqual("Removed", card.Strong);
        Assert.AreEqual(" · Claude was notified", card.Detail);
        Assert.AreEqual("", card.Time);
        Assert.AreEqual(EdgeBarTone.None, card.BarTone);
        Assert.AreEqual(CardAction.None, card.Action);
        Assert.IsTrue(card.IsEnded);
    }

    [TestMethod]
    [DataRow("killed", "1 running · 1 waiting")]
    [DataRow("c-killed", "Idle")]
    [DataRow("c-removed", "Idle")]
    [DataRow("c-confirm", "1 running")]
    public void EndedCardsLeaveTheSummary(string scenario, string summary)
    {
        Assert.AreEqual(summary, Snapshot(scenario).Header.Summary);
    }

    [TestMethod]
    public void StoppedTimeIsWhenRhdEndedTheLease()
    {
        var card = CardWith(
            "cargo",
            new StopView(StopKind.Kill, StopPhase.Ended, EndedAtMs: Handoff.NowMs - 66_000),
            nowMs: Handoff.NowMs + 3_000);

        Assert.AreEqual("at 2:00", card.Time);
    }

    [TestMethod]
    [DataRow("nothing was running", " · nothing was running · Claude was notified")]
    [DataRow("1 process still running", " · 1 process still running · Claude was notified")]
    public void StoppedNoteSitsBeforeTheNotice(string note, string detail)
    {
        var card = CardWith("cargo", new StopView(StopKind.Kill, StopPhase.Ended, Message: note));

        Assert.AreEqual(detail, card.Detail);
    }

    [TestMethod]
    public void StoppedBenchmarkShowsNoTime()
    {
        var state = Handoff.State(wait: ["npmB"], bench: "run");
        var bench = Handoff.LeaseId("bench");
        var snapshot = FlyoutSnapshot.From(
            ServiceStatus.Running,
            state,
            new Dictionary<long, StopView> { [bench] = new(StopKind.Kill, StopPhase.Ended) });

        var card = snapshot.Groups[0].Jobs.Single().Card;
        Assert.AreEqual("Stopped", card.Strong);
        Assert.AreEqual("", card.Time);
    }

    [TestMethod]
    public void SendingKillShowsStoppingWithoutAnAction()
    {
        var card = CardWith("cargo", new StopView(StopKind.Kill, StopPhase.Sending));

        Assert.AreEqual("Stopping cargo.exe…", card.Detail);
        Assert.IsFalse(card.DetailIsDanger);
        Assert.AreEqual("", card.Time);
        Assert.AreEqual(CardAction.None, card.Action);
        Assert.IsFalse(card.IsEnded);
    }

    [TestMethod]
    public void RefusedKillSaysWhyInRedWithKillBack()
    {
        var card = CardWith(
            "cargo",
            new StopView(StopKind.Kill, StopPhase.Failed, Message: Handoff.RefusedReason));

        var why = "Not stopped: the bound process 18244 (cargo.exe) is protected";
        Assert.AreEqual(why, card.Detail);
        Assert.AreEqual(why, card.DetailTooltip);
        Assert.IsTrue(card.DetailIsDanger);
        Assert.AreEqual("3:06 / ~5:00", card.Time);
        Assert.AreEqual(CardAction.Kill, card.Action);
    }

    [TestMethod]
    public void SendingRemoveShowsRemovingAndKeepsTheWait()
    {
        var card = CardWith("npm", new StopView(StopKind.Remove, StopPhase.Sending));

        Assert.AreEqual("Removing…", card.Detail);
        Assert.AreEqual("0:42 / ~2 min", card.Time);
        Assert.AreEqual(CardAction.None, card.Action);
    }

    [TestMethod]
    public void RefusedRemoveSaysWhyWithRemoveBack()
    {
        var card = CardWith(
            "npm",
            new StopView(StopKind.Remove, StopPhase.Failed, Message: "the service is not running"));

        Assert.AreEqual("Not removed: the service is not running", card.Detail);
        Assert.IsTrue(card.DetailIsDanger);
        Assert.AreEqual(CardAction.Remove, card.Action);
    }

    [TestMethod]
    public void ConfirmOfAJobWithoutAProcessNamesTheCommand()
    {
        var state = Handoff.State(run: ["cargo"]);
        var job = state.Active[0] with { Process = null };

        var card = JobCard.ForActive(
            job,
            Handoff.NowMs,
            lastKnown: false,
            new StopView(StopKind.Kill, StopPhase.Confirming, 4));

        Assert.AreEqual("Stops the command and its child processes", card.Detail);
        Assert.AreEqual("Confirm kill cargo build --release", card.ActionName);
    }

    [TestMethod]
    public void StopOfTheOtherKindIsIgnored()
    {
        var card = CardWith("npm", new StopView(StopKind.Kill, StopPhase.Confirming, 4));

        Assert.AreEqual(CardAction.Remove, card.Action);
        Assert.AreEqual("2 cores · 4 GB", card.Detail);
    }

    [TestMethod]
    public void RowRaisesOnlyTheMembersAStopChanges()
    {
        var state = Handoff.State(run: ["cargo"]);
        var model = new FlyoutModel();
        model.Apply(FlyoutSnapshot.From(ServiceStatus.Running, state));
        var row = model.Rows.OfType<JobCardRow>().Single();
        var raised = new List<string>();
        row.PropertyChanged += (_, args) => raised.Add(args.PropertyName!);

        model.Apply(FlyoutSnapshot.From(ServiceStatus.Running, state, Handoff.Stops("c-killed")));

        Assert.AreEqual(
            "Strong Detail DetailTooltip Time BarTone Action ActionName IsEnded Card",
            string.Join(" ", raised));
        Assert.AreSame(row, model.Rows.OfType<JobCardRow>().Single());
    }

    private static FlyoutSnapshot Snapshot(string scenario)
    {
        var (status, state) = Handoff.Scenario(scenario);
        return FlyoutSnapshot.From(status, state, Handoff.Stops(scenario));
    }

    private static JobCard CardOf(string scenario, string job) =>
        Snapshot(scenario).Groups.SelectMany(group => group.Jobs)
            .Single(item => item.Id == Handoff.LeaseId(job))
            .Card;

    private static JobCard CardWith(string job, StopView stop, long? nowMs = null)
    {
        var state = Handoff.State(run: ["cargo"], wait: ["npm"]);
        var snapshot = FlyoutSnapshot.From(
            ServiceStatus.Running,
            state,
            new Dictionary<long, StopView> { [Handoff.LeaseId(job)] = stop },
            nowMs);
        return snapshot.Groups.SelectMany(group => group.Jobs)
            .Single(item => item.Id == Handoff.LeaseId(job))
            .Card;
    }
}
