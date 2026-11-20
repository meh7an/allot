using System.Globalization;
using System.Text.Json;
using System.Text.Json.Nodes;

using Allot.Tray.Core.Contract;
using Allot.Tray.Core.ServiceSettings;
using Allot.Tray.Core.Tests.Support;

namespace Allot.Tray.Core.Tests;

/// <summary>
/// The Sizes and Commands pages' view models (M4.2): table cells and Fits now, the edit dialogs'
/// titles, warnings and proposals, Undo after a row reset or a rule delete, and Test a command.
/// </summary>
[TestClass]
public sealed class SizesAndRulesTests
{
    private static readonly Func<double, string> Number =
        SettingsText.Numbers(CultureInfo.InvariantCulture);

    public TestContext TestContext { get; set; } = null!;

    [TestMethod]
    public void SizeCellsCarryTheirUnitsAndTheAutomaticMinimum()
    {
        var cells = SizesText.Cells(Row("big", 8, 4096, 50, 2048, 300, null), Number);

        Assert.AreEqual(
            new SizeCells("8", "Automatic (4)", "4 GB", "50 %", "2 GB", "5 minutes"),
            cells);
        var other = SizesText.Cells(Row("big", 8, 1000, 0, 0, 45, 6), Number);
        Assert.AreEqual("6", other.MinCores);
        Assert.AreEqual("1,000 MB", other.Memory);
        Assert.AreEqual("45 seconds", other.RunTime);
    }

    [TestMethod]
    public void TheAutomaticMinimumIsHalfTheCoresButAtLeastOne()
    {
        Assert.AreEqual(4, SizesText.AutomaticMinCores(8));
        Assert.AreEqual(1, SizesText.AutomaticMinCores(1.5));
        Assert.AreEqual(1, SizesText.AutomaticMinCores(1));
        Assert.AreEqual(0.5, SizesText.AutomaticMinCores(0.5));
        Assert.AreEqual(0, SizesText.AutomaticMinCores(0));
    }

    [TestMethod]
    public void FitsNowReadsShortInTheTable()
    {
        Assert.AreEqual(SizesText.FitsUnknown, SizesText.FitsNow(null, Number));
        Assert.AreEqual("None", SizesText.FitsNow(Fit(0, 0), Number));
        Assert.AreEqual("3", SizesText.FitsNow(Fit(3, 0), Number));
        Assert.AreEqual("2 shrunk", SizesText.FitsNow(Fit(0, 2), Number));
        Assert.AreEqual("1, 2 more shrunk", SizesText.FitsNow(Fit(1, 2), Number));
        Assert.AreEqual("at least 99", SizesText.FitsNow(Fit(99, 0, more: true), Number));
        Assert.AreEqual(
            "2, at least 99 more shrunk",
            SizesText.FitsNow(Fit(2, 99, more: true), Number));
    }

    [TestMethod]
    public void TheDialogLineNamesNowAndWithEveryLeaseEnded()
    {
        var fit = new SizeFit
        {
            Name = "cpu_heavy",
            MinCores = 4,
            Now = Counts(1, 2),
            Idle = Counts(2, 0),
        };
        Assert.AreEqual(
            "With these values, now: 1 at full size, 2 more at 4 cores. With every lease ended: "
                + "2 at full size.",
            SizesText.DialogFits(fit, Number));

        var none = fit with { Now = Counts(0, 0) };
        StringAssert.StartsWith(
            SizesText.DialogFits(none, Number),
            "With these values, now: none fit.");
    }

    [TestMethod]
    public async Task DialogTitlesNameTheClassOrPreset()
    {
        var session = (await LoadAsync(FakeSettingsClient.StateWithRender())).Session;
        var classes = As<SizeTableModel>(session.Setting("classes"));
        var presets = As<SizeTableModel>(session.Setting("presets"));

        Assert.AreEqual(
            "Edit CPU heavy",
            SizesText.DialogTitle(classes, Edit(classes, "cpu_heavy")));
        Assert.AreEqual(
            "Edit preset render",
            SizesText.DialogTitle(presets, Edit(presets, "render")));
        Assert.AreEqual(SizesText.AddPresetTitle, SizesText.DialogTitle(presets, presets.Add()));
    }

    [TestMethod]
    public async Task RenamingAPresetWarnsAboutSessionsThatAskForTheOldName()
    {
        var session = (await LoadAsync(FakeSettingsClient.StateWithRender())).Session;
        var presets = As<SizeTableModel>(session.Setting("presets"));
        var draft = Edit(presets, "render");

        Assert.IsNull(SizesText.RenameWarning(draft));
        draft.SetName("render_big");
        Assert.AreEqual(
            "Sessions that ask for render get an error until they use render_big.",
            SizesText.RenameWarning(draft));
        Assert.IsNull(SizesText.RenameWarning(Edit(presets, "cpu_small")));
    }

    [TestMethod]
    public async Task RowActionsNameTheirRowAndSayWhatHappened()
    {
        var session = (await LoadAsync(FakeSettingsClient.StateWithRender())).Session;
        var classes = As<SizeTableModel>(session.Setting("classes"));
        var presets = As<SizeTableModel>(session.Setting("presets"));
        var cpu = classes.Rows.Single(row => row.Row.Name == "cpu_heavy");
        var small = presets.Rows.Single(row => row.Row.Name == "cpu_small");
        var render = presets.Rows.Single(row => row.Row.Name == "render");

        Assert.AreEqual("Edit CPU heavy", SizesText.EditName(classes, cpu));
        Assert.AreEqual("Reset CPU heavy to its built-in size", SizesText.ResetName(classes, cpu));
        Assert.AreEqual(
            "CPU heavy reset to its built-in size.",
            SizesText.RowRemoved(classes, cpu));
        Assert.AreEqual(
            "Preset cpu_small reset to its built-in size.",
            SizesText.RowRemoved(presets, small));
        Assert.AreEqual("Delete preset render", SizesText.DeleteName(render));
        Assert.AreEqual("Preset render deleted.", SizesText.RowRemoved(presets, render));
        Assert.AreEqual(
            new Confirmation(
                "Delete preset render?",
                "Sessions that ask for it by name get an error. You can add it again later.",
                "Delete preset"),
            SizesText.DeletePreset("render"));
    }

    [TestMethod]
    public async Task DeletingAPresetThenUndoWritesItBack()
    {
        var harness = await LoadAsync(FakeSettingsClient.StateWithRender());
        var presets = As<SizeTableModel>(harness.Session.Setting("presets"));
        var old = presets.Rows.Single(row => row.Row.Name == "render").Row;

        presets.Remove("render");
        Assert.IsFalse(presets.Value.Any(row => row.Name == "render"));
        harness.Fire();
        presets.Restore(old);
        harness.Fire();

        Assert.AreEqual(2, harness.Client.Patches.Count);
        AssertChanges(
            harness.Client.Patches[0].Changes,
            new Change.ResetRow { Table = "presets", Name = "render" });
        AssertChanges(
            harness.Client.Patches[1].Changes,
            new Change.SetRow
            {
                Table = "presets",
                Name = "render",
                Profile = old.Profile,
                MinCores = old.MinCores,
            });
    }

    [TestMethod]
    public async Task ARowProposalFollowsTheDialogAndWaitsForAValidRow()
    {
        var harness = await LoadAsync(FakeSettingsClient.StateWithRender());
        var presets = As<SizeTableModel>(harness.Session.Setting("presets"));
        var draft = Edit(presets, "render");
        Field(draft, "cores").Commit(10);

        var proposal = presets.Proposal(draft);
        Assert.IsNotNull(proposal);
        var row = (Change.SetRow)proposal.Single();
        Assert.AreEqual(10f, row.Profile.Cores);

        draft.SetName("render_big");
        CollectionAssert.AreEqual(
            new List<string> { "reset_row render", "set_row render_big" },
            presets.Proposal(draft)!.Select(Describe).ToList());

        draft.SetName("Render");
        Assert.IsFalse(draft.Ready);
        Assert.IsNull(presets.Proposal(draft));

        var added = presets.Add();
        added.SetName("fresh");
        Assert.IsNull(presets.Proposal(added), "a new preset needs its cores and memory first");
        Assert.IsNull(Field(added, "cores").Issue, "a proposal shows no issue");
        Assert.IsEmpty(harness.Client.Patches);
    }

    [TestMethod]
    public async Task ThePreviewAddsTheDialogsChangesAfterTheUnsavedOnes()
    {
        var harness = await LoadAsync();
        As<NumberModel>(harness.Session.Setting("admission.headroom_pct")).Commit(30);
        var classes = As<SizeTableModel>(harness.Session.Setting("classes"));
        var draft = Edit(classes, "mixed");
        Field(draft, "cores").Commit(12);

        _ = await harness.Session.PreviewAsync(
            classes.Proposal(draft),
            TestContext.CancellationToken);

        CollectionAssert.AreEqual(
            new List<string> { "SetNumber", "set_row mixed" },
            harness.Client.Previews.Single().Select(Describe).ToList());
    }

    [TestMethod]
    public async Task ResettingPrintOnlyFlagsThenUndoWritesTheOldListBack()
    {
        var harness = await LoadAsync(StateWith(
            "classifier.light_flags",
            """["--help", "--dry-run"]"""));
        var flags = As<StringListModel>(harness.Session.Setting("classifier.light_flags"));
        Assert.IsTrue(flags.CanReset);
        var saved = flags.Saved;

        var undo = FieldReset.Reset(flags);
        harness.Fire();
        Assert.IsNotNull(undo);
        undo();
        harness.Fire();

        Assert.AreEqual(2, harness.Client.Patches.Count);
        AssertChanges(
            harness.Client.Patches[0].Changes,
            new Change.Reset { Key = "classifier.light_flags" });
        AssertChanges(
            harness.Client.Patches[1].Changes,
            new Change.SetList { Key = "classifier.light_flags", Value = saved });
    }

    [TestMethod]
    public async Task DeletingARuleThenUndoWritesTheWholeListBack()
    {
        var harness = await LoadAsync();
        var rules = As<RuleListModel>(harness.Session.Setting("classifier.rules"));
        var old = rules.Value;

        rules.Remove(0);
        Assert.AreEqual("pnpm", rules.Value[0].Program);
        harness.Fire();
        rules.Replace(old);
        harness.Fire();

        var last = (Change.SetRules)harness.Client.Patches[^1].Changes.Single();
        CollectionAssert.AreEqual(
            old.Select(rule => rule.Program).ToList(),
            last.Rules.Select(rule => rule.Program).ToList());
    }

    [TestMethod]
    public async Task ARuleProposalPutsTheDraftInItsPlaceOrAtTheEnd()
    {
        var session = (await LoadAsync()).Session;
        var rules = As<RuleListModel>(session.Setting("classifier.rules"));
        var count = rules.Value.Count;

        var edited = rules.Edit(1);
        Assert.IsNotNull(edited);
        edited.SetProgram("bun");
        var replaced = Proposed(rules, edited);
        Assert.AreEqual(count, replaced.Count);
        Assert.AreEqual("bun", replaced[1].Program);

        var added = rules.Add();
        Assert.IsNull(rules.Proposal(added), "a new rule needs its program first");
        Assert.IsNull(added.ProgramIssue, "a proposal shows no issue");
        added.SetProgram("ninja");
        added.SetClass("mixed");
        var appended = Proposed(rules, added);
        Assert.AreEqual(count + 1, appended.Count);
        Assert.AreEqual(JobClass.Mixed, appended[^1].Class);
    }

    [TestMethod]
    public async Task RuleListTextsFollowTheList()
    {
        var harness = await LoadAsync();
        var rules = As<RuleListModel>(harness.Session.Setting("classifier.rules"));

        Assert.AreEqual(RulesText.BuiltIn, RulesText.Status(rules));
        Assert.AreEqual("build · test · run · bench", RulesText.Arguments(rules.Value[0]));
        Assert.AreEqual(RulesText.AnyArguments, RulesText.Arguments(rules.Value[2]));
        Assert.AreEqual(
            "Rule 1: cargo, build · test · run · bench, CPU heavy",
            RulesText.RowName(0, rules.Value[0], ClassLabels(harness.Session)));
        Assert.AreEqual("More options for rule 3", RulesText.MoreOptionsName(2));
        Assert.AreEqual("Rule 3 deleted.", RulesText.Deleted(2));
        Assert.AreEqual("Edit rule 2", RulesText.DialogTitle(rules.Edit(1)!));
        Assert.AreEqual("Add rule", RulesText.DialogTitle(rules.Add()));

        var yours = (await LoadAsync(StateWith(
            "classifier.rules",
            """[{"program": "ninja", "args": [], "class": "cpu_heavy"}]"""))).Session;
        Assert.AreEqual(
            RulesText.Yours,
            RulesText.Status(As<RuleListModel>(yours.Setting("classifier.rules"))));
        Assert.AreEqual(
            new Confirmation(
                "Use the built-in rules?",
                "Your 5 rules are replaced by the 6 built-in ones. This can't be undone.",
                "Use built-in rules"),
            RulesText.UseBuiltIn(5, 6));
        Assert.AreEqual(
            "Your 1 rule is replaced by the 6 built-in ones. This can't be undone.",
            RulesText.UseBuiltIn(1, 6).Body);
    }

    [TestMethod]
    public async Task TheFixtureTestReadsOneLinePerCommand()
    {
        var session = (await LoadAsync()).Session;
        var rules = As<RuleListModel>(session.Setting("classifier.rules"));
        var test = JsonSerializer.Deserialize(
            Fixtures.Read("command-test.json"),
            ContractJsonContext.Default.CommandTest)!;

        var lines = RulesText.TestLines(test, rules.Value, ClassLabels(session), Number);

        CollectionAssert.AreEqual(
            new List<string>
            {
                "Heavy: needs a CPU heavy lease (8 cores, 4 GB memory). Rule 1: cargo with build, "
                    + "test, run or bench.",
                "Print-only: --help only prints and exits, so no lease is needed.",
                "Not heavy: no rule matches cargo fmt.",
            },
            lines.ToList());
        CollectionAssert.AreEqual(new List<int> { 0 }, RulesText.Matched(test).ToList());
        Assert.IsEmpty(RulesText.Matched(null));
    }

    [TestMethod]
    public void TwoHeavyCommandsNameTheCombinedLease()
    {
        List<HeavyRule> rules =
        [
            Rule("cargo", JobClass.CpuHeavy),
            Rule("python*", JobClass.GpuHeavy, "train*.py"),
        ];
        var mixed = Size(JobClass.Mixed, 6, 8192, 50, 2048);
        var test = new CommandTest
        {
            Lease = mixed,
            Commands =
            [
                Heavy("cargo build", 0, Size(JobClass.CpuHeavy, 8, 4096, 0, 0)),
                Heavy("python train.py", 1, Size(JobClass.GpuHeavy, 2, 4096, 50, 2048)),
            ],
        };

        var lines = RulesText.TestLines(test, rules, Labels, Number);

        Assert.AreEqual(
            "Heavy: needs a CPU heavy lease (8 cores, 4 GB memory). Rule 1: cargo with any "
                + "arguments.",
            lines[0]);
        Assert.AreEqual(
            "Heavy: needs a GPU heavy lease (2 cores, 4 GB memory, 50 % of the GPU, 2 GB video "
                + "memory). Rule 2: python* with train*.py.",
            lines[1]);
        Assert.AreEqual(
            "The whole line needs one Mixed lease (6 cores, 8 GB memory, 50 % of the GPU, 2 GB "
                + "video memory).",
            lines[2]);
        CollectionAssert.AreEquivalent(new List<int> { 0, 1 }, RulesText.Matched(test).ToList());
    }

    [TestMethod]
    public void ALineWithNoCommandSaysSo()
    {
        var test = new CommandTest { Commands = [] };

        CollectionAssert.AreEqual(
            new List<string> { RulesText.NoCommand },
            RulesText.TestLines(test, [], Labels, Number).ToList());
    }

    [TestMethod]
    public void ClassNamesRoundTrip()
    {
        foreach (var jobClass in Enum.GetValues<JobClass>())
        {
            Assert.AreEqual(jobClass, ClassNames.Parse(ClassNames.Of(jobClass)));
        }

        Assert.IsNull(ClassNames.Parse("huge"));
        Assert.IsNull(ClassNames.Parse(null));
    }

    private static async Task<Harness> LoadAsync(SettingsState? state = null)
    {
        var client = new FakeSettingsClient();
        if (state is not null)
        {
            client.State = state;
        }

        var timers = new FakeSessionTimers();
        var session = new SettingsSession(
            client, timers, Platform.Windows, _ => Task.FromResult<string?>(null));
        await session.RefreshAsync();
        return new Harness(session, client, timers);
    }

    // The fixture's state with the value of `key` replaced; writes answer with it too, so it
    // stays the saved value.
    private static SettingsState StateWith(string key, string valueJson)
    {
        var root = JsonNode.Parse(Fixtures.Read("settings-state.json"))!.AsObject();
        var value = root["values"]!.AsArray()
            .Single(candidate => candidate!["key"]!.GetValue<string>() == key)!;
        value["value"] = JsonNode.Parse(valueJson);
        return JsonSerializer.Deserialize(
            root.ToJsonString(),
            ContractJsonContext.Default.SettingsState)!;
    }

    private static T As<T>(ValueModel? model)
        where T : ValueModel
    {
        Assert.IsNotNull(model);
        Assert.IsInstanceOfType<T>(model);
        return (T)model;
    }

    private static RowDraft Edit(SizeTableModel table, string name)
    {
        var draft = table.Edit(name);
        Assert.IsNotNull(draft);
        return draft;
    }

    private static NumberModel Field(RowDraft draft, string key)
    {
        var field = draft.Field(key);
        Assert.IsNotNull(field);
        return field;
    }

    private static IReadOnlyList<HeavyRule> Proposed(RuleListModel rules, RuleDraft draft)
    {
        var proposal = rules.Proposal(draft);
        Assert.IsNotNull(proposal);
        return ((Change.SetRules)proposal.Single()).Rules;
    }

    private static Func<JobClass, string> ClassLabels(SettingsSession session) =>
        RulesText.ClassLabels(As<SizeTableModel>(session.Setting("classes")).Control);

    private static string Labels(JobClass jobClass) => jobClass switch
    {
        JobClass.CpuHeavy => "CPU heavy",
        JobClass.GpuHeavy => "GPU heavy",
        JobClass.Mixed => "Mixed",
        _ => "Light",
    };

    private static SizeRow Row(
        string name,
        float cores,
        ulong ramMb,
        float gpuPct,
        ulong vramMb,
        ulong estSecs,
        float? minCores) =>
        new()
        {
            Name = name,
            Profile = new JobProfile
            {
                Cores = cores,
                RamMb = ramMb,
                GpuPct = gpuPct,
                VramMb = vramMb,
                EstSecs = estSecs,
            },
            MinCores = minCores,
        };

    private static ClassSize Size(
        JobClass jobClass,
        float cores,
        ulong ramMb,
        float gpuPct,
        ulong vramMb) =>
        new()
        {
            Class = jobClass,
            MinCores = 1,
            Profile = Row(string.Empty, cores, ramMb, gpuPct, vramMb, 300, null).Profile,
        };

    private static CommandResult Heavy(string command, uint ruleIndex, ClassSize size) =>
        new() { Command = command, Kind = CommandKind.Heavy, RuleIndex = ruleIndex, Size = size };

    private static HeavyRule Rule(string program, JobClass jobClass, params string[] args) =>
        new() { Program = program, Class = jobClass, Args = args };

    private static Fits Counts(uint full, uint shrunk, bool more = false) =>
        new() { Full = full, Shrunk = shrunk, More = more };

    private static SizeFit Fit(uint full, uint shrunk, bool more = false) =>
        new()
        {
            Name = "cpu_heavy",
            MinCores = 4,
            Now = Counts(full, shrunk, more),
            Idle = Counts(0, 0),
        };

    private static string Describe(Change change) => change switch
    {
        Change.ResetRow reset => $"reset_row {reset.Name}",
        Change.SetRow row => $"set_row {row.Name}",
        _ => change.GetType().Name,
    };

    // Changes hold lists, which records compare by reference; their JSON compares the values.
    private static void AssertChanges(IReadOnlyList<Change> actual, params Change[] expected) =>
        Assert.AreEqual(Json(expected), Json(actual));

    private static string Json(IReadOnlyList<Change> changes) =>
        JsonSerializer.Serialize(
            new PreviewRequest { Changes = changes }, ContractJsonContext.Default.PreviewRequest);

    private sealed record Harness(
        SettingsSession Session,
        FakeSettingsClient Client,
        FakeSessionTimers Timers)
    {
        /// <summary>
        /// Lets a save in flight finish, fires the burst timer, and lets that save finish.
        /// </summary>
        public void Fire()
        {
            Client.CompleteAll();
            Timers.FireAll();
            Client.CompleteAll();
        }
    }
}
