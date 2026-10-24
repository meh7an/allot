using System.Collections.Specialized;

using ResourcesHelper.Tray.Core.Contract;
using ResourcesHelper.Tray.Core.Daemon;
using ResourcesHelper.Tray.Core.Samples;
using ResourcesHelper.Tray.Core.ViewModels;

namespace ResourcesHelper.Tray.Core.Tests;

[TestClass]
public sealed class FlyoutModelTests
{
    private static FlyoutModel Applied(UiState state, ServiceStatus status = ServiceStatus.Running)
    {
        var model = new FlyoutModel();
        model.Apply(FlyoutSnapshot.From(status, state));
        return model;
    }

    private static string Keys(FlyoutModel model) =>
        string.Join(" ", model.Rows.Select(row => row.Key));

    private static List<NotifyCollectionChangedAction> Changes(FlyoutModel model)
    {
        var changes = new List<NotifyCollectionChangedAction>();
        model.Rows.CollectionChanged += (_, args) => changes.Add(args.Action);
        return changes;
    }

    [TestMethod]
    public void RowsFollowTheGroupsThenTheIdleLine()
    {
        var model = Applied(Handoff.State(run: ["cargo"], wait: ["npm"], idle: true));

        Assert.AreEqual("group:Running job:1 group:Waiting job:5 idle", Keys(model));
        Assert.AreEqual("1 running · 1 waiting", model.Summary);
        Assert.AreEqual(HeaderGlyph.Busy, model.Glyph);
        Assert.IsFalse(model.IsLastKnown);
    }

    [TestMethod]
    public void CardShowsTheSessionStripAndCommand()
    {
        var card = Applied(Handoff.State(run: ["cargo"])).Rows.OfType<JobCardRow>().Single();

        Assert.AreEqual("3f2a9c1e", card.ShortId);
        Assert.AreEqual("resources-helper-mcp", card.Project);
        Assert.AreEqual(@"E:\code\resources-helper-mcp", card.ProjectPath);
        Assert.AreEqual("7.8 cores · 2.9 GB", card.Usage);
        Assert.AreEqual("cargo build --release", card.Command);
        Assert.IsFalse(card.IsQueued);
    }

    [TestMethod]
    public void ASecondIdenticalPollChangesNothing()
    {
        var state = Handoff.State(run: ["cargo", "py"], wait: ["npm"]);
        var model = Applied(state);
        var rows = model.Rows.ToList();
        var changes = Changes(model);
        var properties = new List<string?>();
        foreach (var row in rows)
        {
            row.PropertyChanged += (_, args) => properties.Add(args.PropertyName);
        }

        model.PropertyChanged += (_, args) => properties.Add(args.PropertyName);
        model.Apply(FlyoutSnapshot.From(ServiceStatus.Running, state));

        CollectionAssert.AreEqual(rows, model.Rows.ToList());
        Assert.IsEmpty(changes);
        Assert.IsEmpty(properties);
    }

    [TestMethod]
    public void ANewJobIsInsertedAndTheOthersStay()
    {
        var model = Applied(Handoff.State(run: ["cargo"]));
        var cargo = model.Rows[1];
        var changes = Changes(model);

        model.Apply(FlyoutSnapshot.From(ServiceStatus.Running, Handoff.State(run: ["cargo", "py"])));

        Assert.AreEqual("group:Running job:1 job:2", Keys(model));
        Assert.AreSame(cargo, model.Rows[1]);
        CollectionAssert.AreEqual(new[] { NotifyCollectionChangedAction.Add }, changes);
    }

    [TestMethod]
    public void AFinishedJobAndItsEmptyGroupAreRemoved()
    {
        var model = Applied(Handoff.State(run: ["cargo"], wait: ["npm"]));
        var changes = Changes(model);

        model.Apply(FlyoutSnapshot.From(ServiceStatus.Running, Handoff.State(wait: ["npm"])));

        Assert.AreEqual("group:Waiting job:5", Keys(model));
        CollectionAssert.AreEqual(
            new[] { NotifyCollectionChangedAction.Remove, NotifyCollectionChangedAction.Remove },
            changes);
    }

    [TestMethod]
    public void ChangedValuesUpdateTheRowInPlace()
    {
        var model = Applied(Handoff.State(run: ["cargo"], wait: ["npm"]));
        var heading = model.Rows.OfType<GroupHeadingRow>().First();
        var properties = new List<string?>();
        heading.PropertyChanged += (_, args) => properties.Add(args.PropertyName);

        model.Apply(FlyoutSnapshot.From(
            ServiceStatus.Running,
            Handoff.State(run: ["cargo", "py"], wait: ["npm"])));

        Assert.AreEqual("Running · 2", heading.Heading);
        Assert.AreEqual("Heading", properties.Single());
    }

    [TestMethod]
    public void AReorderedRowIsMovedNotRebuilt()
    {
        var model = Applied(Handoff.State(wait: ["npm", "ctest"]));
        var npm = model.Rows[1];
        var changes = Changes(model);

        model.Apply(FlyoutSnapshot.From(
            ServiceStatus.Running,
            Handoff.State(wait: ["ctest", "npm"])));

        Assert.AreEqual("group:Waiting job:8 job:5", Keys(model));
        Assert.AreSame(npm, model.Rows[2]);
        CollectionAssert.AreEqual(new[] { NotifyCollectionChangedAction.Move }, changes);
    }

    [TestMethod]
    public void ATickChangesOnlyTheTimeAndTheBar()
    {
        var state = Handoff.State(run: ["cargo"]);
        var model = Applied(state);
        var card = model.Rows.OfType<JobCardRow>().Single();
        var properties = new List<string?>();
        card.PropertyChanged += (_, args) => properties.Add(args.PropertyName);

        var later = Handoff.NowMs + 1_000;
        model.Apply(FlyoutSnapshot.From(ServiceStatus.Running, state, nowMs: later));

        Assert.AreEqual("3:07 / ~5:00", card.Time);
        Assert.AreEqual("Time BarFraction Card", string.Join(" ", properties));
    }

    [TestMethod]
    public void CardShowsTheJobRow()
    {
        var card = Applied(Handoff.State(run: ["cargo"])).Rows.OfType<JobCardRow>().Single();

        Assert.AreEqual("CA", card.Tile);
        Assert.AreEqual("CPU heavy", card.Chip);
        Assert.AreEqual("cargo.exe 18244 · 8 cores · 4 GB", card.Detail);
        Assert.AreEqual("3:06 / ~5:00", card.Time);
        Assert.AreEqual(CardAction.Kill, card.Action);
        Assert.AreEqual(EdgeBarTone.Running, card.BarTone);
    }

    [TestMethod]
    public void NothingAndRecentRowsFollowTheIdleLine()
    {
        Assert.AreEqual("nothing", Keys(Applied(Handoff.State())));
        Assert.AreEqual("recent", Keys(Applied(Handoff.State(recent: true))));
    }

    [TestMethod]
    public void RecentRowUpdatesOnlyWhenItsLinesChange()
    {
        var state = Handoff.State(recent: true);
        var model = Applied(state);
        var recent = model.Rows.OfType<RecentRow>().Single();
        var properties = new List<string?>();
        recent.PropertyChanged += (_, args) => properties.Add(args.PropertyName);

        model.Apply(FlyoutSnapshot.From(ServiceStatus.Running, state));
        var later = Handoff.NowMs + 60_000;
        model.Apply(FlyoutSnapshot.From(ServiceStatus.Running, state, nowMs: later));

        Assert.AreEqual("Lines", properties.Single());
        Assert.AreEqual("4:52 · 5 min ago", recent.Lines[0].Meta);
    }

    [TestMethod]
    public void LastKnownRowsStayWhileTheServiceIsDown()
    {
        var model = Applied(Handoff.State(run: ["cargo"]));

        model.Apply(FlyoutSnapshot.From(ServiceStatus.Stopped, Handoff.State(run: ["cargo"])));

        Assert.AreEqual("group:Running job:1", Keys(model));
        Assert.IsTrue(model.IsLastKnown);
        Assert.AreEqual(HeaderGlyph.Error, model.Glyph);
        Assert.AreEqual("Service stopped", model.Summary);
    }
}
