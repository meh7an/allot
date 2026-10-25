using Allot.Tray.Core.Daemon;
using Allot.Tray.Core.Samples;
using Allot.Tray.Core.Settings;
using Allot.Tray.Core.Startup;
using Allot.Tray.Core.ViewModels;

namespace Allot.Tray.Core.Tests;

/// <summary>
/// The first-run card (handoff section 4) and Launch at logon behind its switch.
/// </summary>
[TestClass]
public sealed class FirstRunTests
{
    private const string Exe = @"C:\Users\me\.allot\bin\tray\allot-tray.exe";

    [TestMethod]
    public void FreshInstallShowsTheCardInsteadOfTheEmptyLine()
    {
        var (status, state) = Handoff.Scenario("first-run");

        var snapshot = FlyoutSnapshot.From(status, state, firstRun: new(false, true));

        Assert.AreEqual(new WelcomeCard(LaunchAtLogon: true), snapshot.Welcome);
        Assert.IsFalse(snapshot.ShowNothing);
        Assert.AreEqual("Idle", snapshot.Header.Summary);
    }

    [TestMethod]
    public void DoneHidesTheCard()
    {
        var (status, state) = Handoff.Scenario("first-run");

        var snapshot = FlyoutSnapshot.From(status, state, firstRun: new(true, true));

        Assert.IsNull(snapshot.Welcome);
        Assert.IsTrue(snapshot.ShowNothing);
    }

    [TestMethod]
    public void FirstSessionHidesTheCard()
    {
        var snapshot = FlyoutSnapshot.From(
            ServiceStatus.Running,
            Handoff.State(),
            firstRun: new(false, true));

        Assert.IsNull(snapshot.Welcome);
    }

    [TestMethod]
    public void NoCardWhileTheServiceIsDown()
    {
        var (_, state) = Handoff.Scenario("first-run");

        var snapshot = FlyoutSnapshot.From(
            ServiceStatus.Stopped, state, firstRun: new(false, true));

        Assert.IsNull(snapshot.Welcome);
    }

    [TestMethod]
    public void SwitchFollowsTheAutostartEntry()
    {
        var (status, state) = Handoff.Scenario("first-run");

        var snapshot = FlyoutSnapshot.From(status, state, firstRun: new(false, false));

        Assert.AreEqual(new WelcomeCard(LaunchAtLogon: false), snapshot.Welcome);
    }

    [TestMethod]
    public void CardSitsAfterTheIdleLineAndUpdatesInPlace()
    {
        var (status, state) = Handoff.Scenario("first-run");
        state = state with { Sessions = [Handoff.Session(Handoff.IdleSession, null, 0.1f, 307)] };
        var model = new FlyoutModel();
        model.Apply(FlyoutSnapshot.From(status, state, firstRun: new(false, true)));
        var row = model.Rows.OfType<WelcomeRow>().Single();
        var changes = new List<string?>();
        row.PropertyChanged += (_, args) => changes.Add(args.PropertyName);

        model.Apply(FlyoutSnapshot.From(status, state, firstRun: new(false, false)));

        Assert.AreEqual("idle welcome", string.Join(" ", model.Rows.Select(r => r.Key)));
        Assert.AreSame(row, model.Rows[1]);
        Assert.IsFalse(row.LaunchAtLogon);
        Assert.AreEqual(nameof(WelcomeRow.LaunchAtLogon), changes.Single());
    }

    [TestMethod]
    public void FlyoutOpensByItselfOnlyTheFirstTimeTheCardShows()
    {
        var (status, state) = Handoff.Scenario("first-run");
        var shown = FlyoutSnapshot.From(status, state, firstRun: new(false, true));
        var hidden = FlyoutSnapshot.From(status, state, firstRun: new(true, true));

        Assert.IsTrue(FirstRun.ShowsForTheFirstTime(shown, new TraySettings()));
        Assert.IsFalse(FirstRun.ShowsForTheFirstTime(
            shown, new TraySettings { FirstRunShown = true }));
        Assert.IsFalse(FirstRun.ShowsForTheFirstTime(hidden, new TraySettings()));
    }

    [TestMethod]
    public void AutostartIsOffWithoutAnEntry()
    {
        var store = new FakeAutostartStore();

        Assert.IsFalse(new Autostart(store, Exe).IsOn);
    }

    [TestMethod]
    public void TurningOnWritesTheQuotedExeAndDropsADisabledMark()
    {
        var store = new FakeAutostartStore { Approval = [3, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8] };
        var autostart = new Autostart(store, Exe);

        autostart.Set(true);

        Assert.AreEqual($"\"{Exe}\"", store.Command);
        Assert.IsNull(store.Approval);
        Assert.IsTrue(autostart.IsOn);
    }

    [TestMethod]
    public void TurningOffRemovesBothValues()
    {
        var store = new FakeAutostartStore { Command = $"\"{Exe}\"", Approval = [2, 0, 0, 0] };
        var autostart = new Autostart(store, Exe);

        autostart.Set(false);

        Assert.IsNull(store.Command);
        Assert.IsNull(store.Approval);
        Assert.IsFalse(autostart.IsOn);
    }

    [TestMethod]
    public void DisabledInTaskManagerReadsAsOff()
    {
        var disabled = new FakeAutostartStore
        {
            Command = $"\"{Exe}\"",
            Approval = [3, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8],
        };
        var enabled = new FakeAutostartStore { Command = $"\"{Exe}\"", Approval = [2, 0, 0, 0] };

        Assert.IsFalse(new Autostart(disabled, Exe).IsOn);
        Assert.IsTrue(new Autostart(enabled, Exe).IsOn);
    }

    [TestMethod]
    public void EntryForAnotherPathStillReadsAsOn()
    {
        var store = new FakeAutostartStore { Command = @"""D:\old\allot-tray.exe""" };

        Assert.IsTrue(new Autostart(store, Exe).IsOn);
    }

    private sealed class FakeAutostartStore : IAutostartStore
    {
        public string? Command { get; set; }

        public byte[]? Approval { get; set; }

        public string? ReadCommand() => Command;

        public byte[]? ReadApproval() => Approval;

        public void WriteCommand(string command) => Command = command;

        public void DeleteCommand() => Command = null;

        public void DeleteApproval() => Approval = null;
    }
}
