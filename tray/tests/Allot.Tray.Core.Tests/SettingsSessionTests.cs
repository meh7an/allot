using System.Globalization;
using System.Text.Json;

using Allot.Tray.Core.Contract;
using Allot.Tray.Core.Daemon;
using Allot.Tray.Core.ServiceSettings;
using Allot.Tray.Core.Tests.Support;

namespace Allot.Tray.Core.Tests;

[TestClass]
public sealed class SettingsSessionTests
{
    public TestContext TestContext { get; set; } = null!;

    [TestMethod]
    public async Task FixtureLoadsEverySettingWithItsViewModel()
    {
        var harness = await LoadAsync();
        var session = harness.Session;

        Assert.AreEqual(SessionStatus.Ready, session.Status);
        Assert.IsFalse(session.ReadOnly);
        CollectionAssert.AreEqual(Strings("daemon.port"), session.PendingRestart.ToList());
        CollectionAssert.AreEqual(
            Strings("admission", "sizes", "commands", "benchmarks", "leases", "service"),
            session.Pages.Select(page => page.Spec.Id).ToList());
        _ = As<SwitchModel>(session.Setting("estimates.lower"));
        _ = As<NumberModel>(session.Setting("admission.headroom_pct"));
        _ = As<DurationModel>(session.Setting("leases.idle_release_secs"));
        _ = As<StringListModel>(session.Setting("classifier.light_flags"));
        _ = As<SizeTableModel>(session.Setting("presets"));
        _ = As<RuleListModel>(session.Setting("classifier.rules"));
        Assert.AreEqual(new SaveStatus(SaveKind.Saved), session.SaveStatus);
    }

    [TestMethod]
    public async Task TheLinuxGroupShowsOnlyOnLinux()
    {
        var linux = await LoadAsync(Platform.Linux);
        var windows = await LoadAsync(Platform.Windows);

        CollectionAssert.AreEqual(Strings("connection", "updates", "linux"), GroupIds(linux.Session));
        CollectionAssert.AreEqual(Strings("connection", "updates"), GroupIds(windows.Session));
        Assert.IsNull(windows.Session.Setting("linux.scopes"));
    }

    [TestMethod]
    public async Task ResetShowsOnlyForValuesThatDifferFromTheDefault()
    {
        var session = (await LoadAsync()).Session;

        Assert.IsTrue(session.Setting("daemon.port")?.CanReset);
        Assert.IsFalse(session.Setting("admission.headroom_pct")?.CanReset);
        Assert.IsTrue(session.Setting("classes")?.CanReset);
        Assert.IsFalse(session.Setting("presets")?.CanReset);
        CollectionAssert.AreEqual(
            Strings("daemon.port"),
            session.ChangedOn("service").Select(model => model.Key).ToList());
    }

    [TestMethod]
    public async Task SingleValuesSaveAsOneBurstAfterTheLastChange()
    {
        var harness = await LoadAsync();
        var session = harness.Session;
        var lower = As<SwitchModel>(session.Setting("estimates.lower"));
        var headroom = As<NumberModel>(session.Setting("admission.headroom_pct"));

        lower.Turn(false);
        headroom.Commit(20);
        headroom.Commit(25);

        AssertDelays(harness.Timers, SettingsSession.SaveDelay);
        Assert.AreEqual(new SaveStatus(SaveKind.Unsaved, 2), session.SaveStatus);
        harness.Fire();

        var patch = harness.Client.Patches.Single();
        Assert.AreEqual("69fd99abe4f654c7", patch.Revision);
        AssertChanges(
            patch.Changes,
            new Change.SetSwitch { Key = "estimates.lower", Value = false },
            new Change.SetNumber { Key = "admission.headroom_pct", Value = 25 });
        Assert.AreEqual(new SaveStatus(SaveKind.Saved), session.SaveStatus);
        Assert.IsFalse(headroom.Dirty);
    }

    [TestMethod]
    public async Task AValueOutsideItsRangeShowsWhyAndIsNotSent()
    {
        var harness = await LoadAsync();
        var session = harness.Session;
        var headroom = As<NumberModel>(session.Setting("admission.headroom_pct"));

        headroom.Commit(100);

        Assert.IsEmpty(harness.Timers.Delays);
        Assert.AreEqual(100.0, headroom.Value);
        Assert.IsNotNull(headroom.Issue);
        Assert.AreEqual(
            "Enter a number at least 0 and below 100.",
            ValueText.IssueText(headroom.Issue, Plain));
        Assert.AreEqual(new SaveStatus(SaveKind.Unsaved, 1), session.SaveStatus);
        headroom.Undo();
        Assert.AreEqual(15.0, headroom.Value);
        Assert.AreEqual(new SaveStatus(SaveKind.Saved), session.SaveStatus);
        Assert.IsEmpty(harness.Client.Patches);
    }

    [TestMethod]
    public async Task AValueBelowTheSettingItDependsOnNamesThatSetting()
    {
        var session = (await LoadAsync()).Session;
        var wait = As<DurationModel>(session.Setting("benchmark.max_quiet_wait_secs"));

        wait.SetUnit("seconds");
        wait.Commit(5);

        Assert.AreEqual(ValueIssue.AtLeast("Quiet for", 10, Quantity.Seconds), wait.Issue);
        Assert.IsNotNull(wait.Issue);
        Assert.AreEqual(
            "Must be at least Quiet for (10 seconds).",
            ValueText.IssueText(wait.Issue, Plain));
    }

    [TestMethod]
    public async Task AllotdsRefusalLandsUnderItsFieldAndTheRestIsSentAgain()
    {
        var harness = await LoadAsync();
        var session = harness.Session;
        var problem = new FieldError
        {
            Key = "admission.headroom_pct",
            Message = "must be at least 0 and below 100",
        };
        harness.Client.Writes.Enqueue(
            FakeSettingsClient.Failure<SettingsState>(
                SettingsFailureKind.Invalid, problems: [problem]));
        As<NumberModel>(session.Setting("admission.headroom_pct")).Commit(50);
        As<SwitchModel>(session.Setting("estimates.lower")).Turn(false);

        harness.Fire();
        harness.Fire();

        var headroom = As<NumberModel>(session.Setting("admission.headroom_pct"));
        Assert.AreEqual(
            ValueIssue.Service("Headroom must be at least 0 and below 100"),
            headroom.Issue);
        Assert.AreEqual(50.0, headroom.Value);
        AssertChanges(
            harness.Client.Patches[1].Changes,
            new Change.SetSwitch { Key = "estimates.lower", Value = false });
        Assert.AreEqual(new SaveStatus(SaveKind.Unsaved, 1), session.SaveStatus);
    }

    [TestMethod]
    public async Task AHandEditInBetweenShowsTheFilesValuesAndCanApplyTheChangeAgain()
    {
        var harness = await LoadAsync();
        var session = harness.Session;
        var margin = As<NumberModel>(session.Setting("estimates.margin_pct"));
        harness.Client.Writes.Enqueue(
            FakeSettingsClient.Failure<SettingsState>(
                SettingsFailureKind.Conflict));
        harness.Client.Reads.Enqueue(
            new SettingsReply<SettingsState>(
                harness.Client.State with { Revision = "f38fd8b70ab492ce" }));

        margin.Commit(15);
        harness.Fire();

        Assert.AreEqual(10.0, margin.Value);
        Assert.IsNotNull(session.Conflict);
        Assert.HasCount(1, session.Conflict.Edits);
        AssertChanges(
            session.Conflict.Edits[0].Changes,
            new Change.SetNumber { Key = "estimates.margin_pct", Value = 15 });
        session.ApplyMine();
        harness.Fire();
        Assert.IsNull(session.Conflict);
        Assert.AreEqual("f38fd8b70ab492ce", harness.Client.Patches[1].Revision);
        AssertChanges(
            harness.Client.Patches[1].Changes,
            new Change.SetNumber { Key = "estimates.margin_pct", Value = 15 });
    }

    [TestMethod]
    public async Task AHeldFileKeepsTheChangesUnsavedUntilTheNextSave()
    {
        var harness = await LoadAsync();
        var session = harness.Session;
        harness.Client.Writes.Enqueue(
            FakeSettingsClient.Failure<SettingsState>(
                SettingsFailureKind.Held, "config.toml is open in another program"));

        As<SwitchModel>(session.Setting("hook.fail_open")).Turn(false);
        harness.Fire();

        Assert.AreEqual("config.toml is open in another program", session.WriteProblem);
        Assert.AreEqual(new SaveStatus(SaveKind.Unsaved, 1), session.SaveStatus);
        harness.Fire();
        Assert.HasCount(2, harness.Client.Patches);
        Assert.IsNull(session.WriteProblem);
    }

    [TestMethod]
    public async Task AFailedWriteOtherThanHeldWaitsForTryAgainOrTheNextEdit()
    {
        var harness = await LoadAsync();
        var session = harness.Session;
        harness.Client.Writes.Enqueue(
            FakeSettingsClient.Failure<SettingsState>(
                SettingsFailureKind.Failed, "config.toml is read-only"));

        As<SwitchModel>(session.Setting("hook.fail_open")).Turn(false);
        harness.Fire();
        await session.RefreshAsync();

        Assert.AreEqual("config.toml is read-only", session.WriteProblem);
        Assert.IsEmpty(harness.Timers.Delays);
        harness.Fire();
        Assert.HasCount(1, harness.Client.Patches);
        Assert.AreEqual(new SaveStatus(SaveKind.Unsaved, 1), session.SaveStatus);

        var again = session.FlushAsync();
        harness.Client.CompleteAll();
        await again;

        Assert.HasCount(2, harness.Client.Patches);
        Assert.IsNull(session.WriteProblem);
    }

    [TestMethod]
    public async Task ChangesAllotdDidNotAnswerGoOutAgainOnceARefreshFindsItBack()
    {
        var harness = await LoadAsync();
        var session = harness.Session;
        harness.Client.Writes.Enqueue(
            FakeSettingsClient.Failure<SettingsState>(
                SettingsFailureKind.Unreachable, "Connection refused"));

        As<SwitchModel>(session.Setting("hook.fail_open")).Turn(false);
        harness.Fire();
        harness.Fire();
        Assert.HasCount(1, harness.Client.Patches);
        Assert.AreEqual(new SaveStatus(SaveKind.Unsaved, 1), session.SaveStatus);

        await session.RefreshAsync();
        harness.Fire();
        Assert.HasCount(2, harness.Client.Patches);
    }

    [TestMethod]
    public async Task AnOutdatedServiceKeepsThePagesReadOnlyAndIsAskedAgainByTheRefresh()
    {
        var harness = await LoadAsync();
        var session = harness.Session;
        harness.Client.Reads.Enqueue(
            FakeSettingsClient.Failure<SettingsState>(
                SettingsFailureKind.Outdated, "the service answered 404"));

        await session.RefreshAsync();

        Assert.AreEqual(SessionStatus.Outdated, session.Status);
        Assert.AreEqual("the service answered 404", session.Detail);
        Assert.IsTrue(session.ReadOnly);

        await session.RefreshAsync();

        Assert.AreEqual(SessionStatus.Ready, session.Status);
        Assert.IsNull(session.Detail);
    }

    [TestMethod]
    public async Task ASaveRefusedAsOutdatedIsNotSentAgainOnTheTimer()
    {
        var harness = await LoadAsync();
        var session = harness.Session;
        harness.Client.Writes.Enqueue(
            FakeSettingsClient.Failure<SettingsState>(
                SettingsFailureKind.Outdated, "the service answered 404"));

        As<SwitchModel>(session.Setting("hook.fail_open")).Turn(false);
        harness.Fire();

        Assert.AreEqual(SessionStatus.Outdated, session.Status);
        Assert.IsNull(session.WriteProblem);
        Assert.IsEmpty(harness.Timers.Delays);
        Assert.HasCount(1, harness.Client.Patches);
        harness.Fire();
        Assert.HasCount(1, harness.Client.Patches);

        await session.RefreshAsync();
        harness.Fire();
        Assert.HasCount(2, harness.Client.Patches);
    }

    [TestMethod]
    public async Task ARefreshSentBeforeASaveDoesNotBringBackTheOldRevision()
    {
        var harness = await LoadAsync();
        var session = harness.Session;
        harness.Client.HoldReads = true;
        var refresh = session.RefreshAsync();
        harness.Client.Writes.Enqueue(
            new SettingsReply<SettingsState>(
                harness.Client.State with { Revision = "after-save" }));

        As<SwitchModel>(session.Setting("hook.fail_open")).Turn(false);
        harness.Fire();
        harness.Client.CompleteReads();
        await refresh;
        As<SwitchModel>(session.Setting("estimates.lower")).Turn(false);
        harness.Fire();

        Assert.AreEqual("after-save", harness.Client.Patches[1].Revision);
    }

    [TestMethod]
    public async Task ARefusalNoBatchedSettingOwnsIsShownBeforeTheEditsAreUndone()
    {
        var harness = await LoadAsync();
        var session = harness.Session;
        var headroom = As<NumberModel>(session.Setting("admission.headroom_pct"));
        harness.Client.Writes.Enqueue(
            FakeSettingsClient.Failure<SettingsState>(
                SettingsFailureKind.Invalid,
                "config.toml is not valid TOML",
                [new FieldError { Key = "classifier.nowhere", Message = "unknown" }]));

        headroom.Commit(50);
        harness.Fire();

        Assert.AreEqual("config.toml is not valid TOML", session.WriteProblem);
        Assert.AreEqual(15.0, headroom.Value);
        await session.FlushAsync();
        Assert.IsNull(session.WriteProblem);
    }

    [TestMethod]
    public async Task AWriteThatThrowsKeepsTheChangesAndSaysSo()
    {
        var harness = await LoadAsync();
        var session = harness.Session;
        harness.Client.WriteError = new InvalidOperationException("could not encode the change");

        As<SwitchModel>(session.Setting("hook.fail_open")).Turn(false);
        harness.Fire();

        Assert.AreEqual("could not encode the change", session.WriteProblem);
        Assert.AreEqual(new SaveStatus(SaveKind.Unsaved, 1), session.SaveStatus);
        Assert.IsEmpty(harness.Timers.Delays);
    }

    [TestMethod]
    public async Task ANumberTooLargeForAFloatIsRefusedAsNotANumber()
    {
        var presets = As<SizeTableModel>((await LoadAsync()).Session.Setting("presets"));
        var cores = Field(presets.Add(), "cores");

        cores.Commit(double.Parse(new string('9', 40), CultureInfo.InvariantCulture));

        Assert.AreEqual(ValueIssue.NotANumber, cores.Issue);
    }

    [TestMethod]
    public async Task ABlankChipFlagGoesWhenTheEntryChangesAndNeverCountsAsUnsaved()
    {
        var session = (await LoadAsync()).Session;
        var flags = As<StringListModel>(session.Setting("classifier.light_flags"));

        Assert.IsFalse(flags.Add(" "));
        Assert.AreEqual(new SaveStatus(SaveKind.Saved), session.SaveStatus);
        flags.EntryChanged();
        Assert.IsNull(flags.Issue);
        Assert.IsFalse(flags.Add("--help"));
        Assert.AreEqual(ValueIssue.Duplicate, flags.Issue);
        flags.EntryChanged();
        Assert.IsNull(flags.Issue);
    }

    [TestMethod]
    public async Task ACancelledTimersLateCallbackLeavesTheNewerTimerAlone()
    {
        var harness = await LoadAsync();
        var session = harness.Session;

        As<SwitchModel>(session.Setting("estimates.lower")).Turn(false);
        As<NumberModel>(session.Setting("admission.headroom_pct")).Commit(20);
        harness.Timers.FireCancelled();

        Assert.IsEmpty(harness.Client.Patches);
        AssertDelays(harness.Timers, SettingsSession.SaveDelay);
    }

    [TestMethod]
    public async Task SettlingSendsWhatIsPendingAndWaitsForTheAnswer()
    {
        var harness = await LoadAsync();
        var session = harness.Session;
        As<SwitchModel>(session.Setting("estimates.lower")).Turn(false);

        var settle = session.SettleAsync(TimeSpan.FromSeconds(30));
        harness.Client.CompleteAll();
        await settle;

        Assert.HasCount(1, harness.Client.Patches);
        Assert.AreEqual(new SaveStatus(SaveKind.Saved), session.SaveStatus);
    }

    [TestMethod]
    public async Task SettlingGivesUpWhenAllotdDoesNotAnswerInTime()
    {
        var harness = await LoadAsync();
        var session = harness.Session;
        As<SwitchModel>(session.Setting("estimates.lower")).Turn(false);

        await session.SettleAsync(TimeSpan.FromMilliseconds(50));

        Assert.HasCount(1, harness.Client.Patches);
        Assert.AreEqual(new SaveStatus(SaveKind.Saving), session.SaveStatus);
    }

    [TestMethod]
    public async Task APercentSliderStopsOneStepBelowAnExclusiveMaximum()
    {
        var session = (await LoadAsync()).Session;
        var headroom = As<NumberModel>(session.Setting("admission.headroom_pct"));

        Assert.AreEqual((0.0, 99.0), headroom.SliderRange);
    }

    [TestMethod]
    public async Task NumbersWithUnitsShowDecimalsAndWholeOnesDoNot()
    {
        var session = (await LoadAsync()).Session;

        Assert.IsTrue(As<NumberModel>(session.Setting("daemon.port")).ShowsWholeNumbers);
        Assert.IsFalse(As<DurationModel>(session.Setting("leases.idle_release_secs"))
            .ShowsWholeNumbers);
    }

    [TestMethod]
    public async Task ARejectedFileMakesThePagesReadOnly()
    {
        var harness = await LoadAsync();
        harness.Client.Reads.Enqueue(
            new SettingsReply<SettingsState>(
                harness.Client.State with { FileError = "expected `=`" }));

        await harness.Session.RefreshAsync();

        Assert.IsTrue(harness.Session.ReadOnly);
        Assert.AreEqual("expected `=`", harness.Session.FileError);
    }

    [TestMethod]
    public async Task NoAnswerReadsTheStopRecordToTellAStopFromAFailure()
    {
        var stopped = await LoadAsync(stopRecord: Fixtures.Read("stop-record.json"));
        var down = await LoadAsync();
        foreach (var harness in new[] { stopped, down })
        {
            harness.Client.Reads.Enqueue(
                FakeSettingsClient.Failure<SettingsState>(
                    SettingsFailureKind.Unreachable, "Connection refused"));
            await harness.Session.RefreshAsync();
        }

        Assert.AreEqual(SessionStatus.Stopped, stopped.Session.Status);
        Assert.AreEqual(StoppedBy.Panel, stopped.Session.StopRecord?.By);
        Assert.AreEqual(SessionStatus.Down, down.Session.Status);
        Assert.AreEqual("Connection refused", down.Session.Detail);
        Assert.IsTrue(down.Session.ReadOnly);
        Assert.AreEqual(
            47_999.0, As<NumberModel>(down.Session.Setting("daemon.port")).Value);
    }

    [TestMethod]
    public void AStopRecordThatIsNotOneStillCountsAsAStop()
    {
        var cli = new StopRecord { By = StoppedBy.Cli, StoppedAtMs = 0 };

        Assert.AreEqual(cli, SettingsSession.ParseStopRecord("not json"));
        Assert.IsNull(SettingsSession.ParseStopRecord(null));
        Assert.AreEqual(
            StoppedBy.Panel,
            SettingsSession.ParseStopRecord(Fixtures.Read("stop-record.json"))?.By);
    }

    [TestMethod]
    public async Task TheStopRecordFileIsReadWhenItExists()
    {
        using var directory = new TempDirectory();
        var path = directory.File("stopped.json");

        var token = TestContext.CancellationToken;

        var missing = await SettingsSession.ReadStopRecordAsync(path, token);
        await File.WriteAllTextAsync(path, "{}", token);
        var present = await SettingsSession.ReadStopRecordAsync(path, token);

        Assert.IsNull(missing);
        Assert.AreEqual("{}", present);
    }

    [TestMethod]
    public async Task ShowingTheWindowFetchesNowAndEveryFiveSecondsAndHidingItSaves()
    {
        var harness = await LoadAsync();
        var session = harness.Session;

        session.SetVisible(true);
        AssertDelays(harness.Timers, SettingsSession.RefreshInterval);
        As<SwitchModel>(session.Setting("hook.fail_open")).Turn(false);
        session.SetVisible(false);
        harness.Fire();

        Assert.HasCount(1, harness.Client.Patches);
        Assert.IsEmpty(harness.Timers.Delays);
    }

    [TestMethod]
    public async Task DurationsShowTheirLargestWholeUnitAndSaveSeconds()
    {
        var harness = await LoadAsync();
        var idle = As<DurationModel>(harness.Session.Setting("leases.idle_release_secs"));

        Assert.AreEqual("minutes", idle.Unit.Name);
        Assert.AreEqual(3.0, idle.Shown);
        idle.SetUnit("seconds");
        Assert.AreEqual(180.0, idle.Shown);
        idle.SetUnit("minutes");
        idle.Commit(5);
        Assert.AreEqual(300.0, idle.Value);
        idle.SetOn(false);
        Assert.IsFalse(idle.IsOn);
        idle.SetOn(true);
        harness.Fire();

        AssertChanges(
            harness.Client.Patches[0].Changes,
            new Change.SetNumber { Key = "leases.idle_release_secs", Value = 300 });
    }

    [TestMethod]
    public async Task TurningADurationOffWritesItsOffValue()
    {
        var harness = await LoadAsync();

        As<DurationModel>(harness.Session.Setting("leases.idle_release_secs")).SetOn(false);
        harness.Fire();

        AssertChanges(
            harness.Client.Patches[0].Changes,
            new Change.SetNumber { Key = "leases.idle_release_secs", Value = 0 });
    }

    [TestMethod]
    public async Task ChipsSaveOnAddAndRemoveNeverBlankOrTwice()
    {
        var harness = await LoadAsync();
        var flags = As<StringListModel>(harness.Session.Setting("classifier.light_flags"));

        Assert.IsFalse(flags.Add("  "));
        Assert.AreEqual(ValueIssue.Blank, flags.Issue);
        Assert.IsFalse(flags.Add("--help"));
        Assert.AreEqual(ValueIssue.Duplicate, flags.Issue);
        Assert.IsTrue(flags.Add(" --dry-run "));
        harness.Fire();

        AssertChanges(
            harness.Client.Patches[0].Changes,
            new Change.SetList
            {
                Key = "classifier.light_flags",
                Value = ["--help", "-h", "--version", "-V", "/?", "--dry-run"],
            });
    }

    [TestMethod]
    public async Task ANewPresetNeedsAFreeNameAndItsSizesThenSavesAtOnce()
    {
        var harness = await LoadAsync();
        var presets = As<SizeTableModel>(harness.Session.Setting("presets"));
        var draft = presets.Add();

        Assert.IsTrue(draft.CanRename);
        draft.SetName("Render");
        Assert.AreEqual(ValueIssue.Pattern, draft.NameIssue);
        draft.SetName("light");
        Assert.AreEqual(ValueIssue.NameTaken, draft.NameIssue);
        draft.SetName("render");
        Assert.IsFalse(presets.Save(draft));
        var cores = Field(draft, "cores");
        Assert.AreEqual(ValueIssue.Required, cores.Issue);
        Assert.IsNull(Field(draft, "min_cores").Issue);

        cores.Commit(6);
        var ram = As<MemoryModel>(draft.Field("ram_mb"));
        ram.SetUnit("GB");
        ram.Commit(1.5);
        Assert.AreEqual(1536.0, ram.Value);
        Assert.IsTrue(presets.Save(draft));
        harness.Fire();

        AssertChanges(
            harness.Client.Patches[0].Changes,
            new Change.SetRow
            {
                Table = "presets",
                Name = "render",
                Profile = new JobProfile
                {
                    Cores = 6,
                    RamMb = 1536,
                    GpuPct = 0,
                    VramMb = 0,
                    EstSecs = 300,
                },
                MinCores = null,
            });
    }

    [TestMethod]
    public async Task AMinimumAboveTheCoresIsRefusedInTheDialog()
    {
        var session = (await LoadAsync()).Session;
        var classes = As<SizeTableModel>(session.Setting("classes"));
        var draft = classes.Edit("mixed");

        Assert.IsNotNull(draft);
        Assert.IsFalse(draft.CanRename);
        var minimum = Field(draft, "min_cores");
        minimum.Commit(8);

        Assert.AreEqual(ValueIssue.AtMost("Cores", 6, Quantity.Cores), minimum.Issue);
        Assert.IsFalse(classes.Save(draft));
        Field(draft, "cores").Commit(8);
        Assert.IsNull(minimum.Issue);
    }

    [TestMethod]
    public async Task RenamingAPresetRemovesTheOldRowInTheSameWrite()
    {
        var harness = await LoadAsync(state: FakeSettingsClient.StateWithRender());
        var presets = As<SizeTableModel>(harness.Session.Setting("presets"));
        var draft = presets.Edit("render");

        Assert.IsNotNull(draft);
        Assert.AreEqual("GB", As<MemoryModel>(draft.Field("ram_mb")).Unit.Name);
        draft.SetName("render_big");
        Assert.IsTrue(presets.Save(draft));
        CollectionAssert.AreEqual(
            Strings("render_big"),
            presets.Value
                .Select(row => row.Name)
                .Where(name => name.StartsWith("render", StringComparison.Ordinal))
                .ToList());
        harness.Fire();

        CollectionAssert.AreEqual(
            Strings("reset_row render", "set_row render_big"),
            harness.Client.Patches[0].Changes.Select(Describe).ToList());
    }

    [TestMethod]
    public async Task DeletingAUserPresetAndResettingABuiltInOneAreRowResets()
    {
        var harness = await LoadAsync(state: FakeSettingsClient.StateWithRender());
        var presets = As<SizeTableModel>(harness.Session.Setting("presets"));

        Assert.IsFalse(presets.Rows.Single(row => row.Row.Name == "render").BuiltIn);
        presets.Remove("render");
        harness.Fire();

        AssertChanges(
            harness.Client.Patches[0].Changes,
            new Change.ResetRow { Table = "presets", Name = "render" });
        Assert.AreEqual(
            "render: Minimum cores must be above 0",
            presets.DescribeProblem("presets.render.min_cores", "must be above 0"));
    }

    [TestMethod]
    public async Task RulesAreWrittenWholeOnEveryEditMoveAndDelete()
    {
        var harness = await LoadAsync();
        var rules = As<RuleListModel>(harness.Session.Setting("classifier.rules"));
        List<string> Programs() => [.. rules.Value.Select(rule => rule.Program)];

        rules.Move(0, 2);
        CollectionAssert.AreEqual(
            Strings("pnpm", "msbuild", "cargo", "cmake", "python*", "py"), Programs());
        rules.Remove(5);
        var draft = rules.Edit(1);
        Assert.IsNotNull(draft);
        draft.SetProgram(" ");
        Assert.IsFalse(rules.Save(draft));
        Assert.AreEqual(ValueIssue.Blank, draft.ProgramIssue);
        draft.SetProgram("ninja");
        Assert.IsTrue(draft.Args.Add("-j"));
        draft.SetClass("mixed");
        Assert.IsTrue(rules.Save(draft));
        harness.Fire();

        AssertChanges(
            harness.Client.Patches[^1].Changes,
            new Change.SetRules
            {
                Rules =
                [
                    Rule("pnpm", JobClass.CpuHeavy, "build*", "test*"),
                    Rule("ninja", JobClass.Mixed, "-j"),
                    Rule("cargo", JobClass.CpuHeavy, "build", "test", "run", "bench"),
                    Rule("cmake", JobClass.CpuHeavy, "--build"),
                    Rule("python*", JobClass.GpuHeavy, "train*.py"),
                ],
            });
        Assert.AreEqual(
            "Rule 3: Program must not be blank",
            rules.DescribeProblem("classifier.rules[2].program", "must not be blank"));
    }

    [TestMethod]
    public async Task ResetPageRemovesEveryChangedKeyOnItInOneWrite()
    {
        var harness = await LoadAsync();

        harness.Session.ResetChanged("service");
        harness.Fire();

        AssertChanges(
            harness.Client.Patches[0].Changes,
            new Change.Reset { Key = "daemon.port" });
        Assert.AreEqual(
            47_999.0, As<NumberModel>(harness.Session.Setting("daemon.port")).Value);
    }

    [TestMethod]
    public async Task APreviewSendsTheUnsavedChanges()
    {
        var harness = await LoadAsync();
        As<NumberModel>(harness.Session.Setting("admission.headroom_pct")).Commit(30);

        _ = await harness.Session.PreviewAsync(cancellationToken: TestContext.CancellationToken);

        AssertChanges(
            harness.Client.Previews.Single(),
            new Change.SetNumber { Key = "admission.headroom_pct", Value = 30 });
    }

    [TestMethod]
    public void ValuesReadWithTheirUnit()
    {
        Assert.AreEqual("15 %", ValueText.QuantityText(15, Quantity.Percent, Plain));
        Assert.AreEqual("3 minutes", ValueText.QuantityText(180, Quantity.Seconds, Plain));
        Assert.AreEqual("1 second", ValueText.QuantityText(1, Quantity.Seconds, Plain));
        Assert.AreEqual("2 GB", ValueText.QuantityText(2048, Quantity.Megabytes, Plain));
        Assert.AreEqual("1000 MB", ValueText.QuantityText(1000, Quantity.Megabytes, Plain));
        Assert.AreEqual("1 core", ValueText.QuantityText(1, Quantity.Cores, Plain));
        Assert.AreEqual("47823", ValueText.QuantityText(47_823, Quantity.Port, Grouped));
        Assert.AreEqual(
            "Enter a number from 1 to 65535.",
            ValueText.IssueText(
                ValueIssue.Range(
                    new Bound { Value = 1, Exclusive = false },
                    new Bound { Value = 65_535, Exclusive = false }),
                Plain));
        Assert.AreEqual(
            "Enter a number above 0.",
            ValueText.IssueText(
                ValueIssue.Range(new Bound { Value = 0, Exclusive = true }, null), Plain));
    }

    private static async Task<Harness> LoadAsync(
        Platform platform = Platform.Linux,
        string? stopRecord = null,
        SettingsState? state = null)
    {
        var client = new FakeSettingsClient();
        if (state is not null)
        {
            client.State = state;
        }

        var timers = new FakeSessionTimers();
        var session = new SettingsSession(
            client, timers, platform, _ => Task.FromResult(stopRecord));
        await session.RefreshAsync();
        return new Harness(session, client, timers);
    }

    private static T As<T>(ValueModel? model)
        where T : ValueModel
    {
        Assert.IsNotNull(model);
        Assert.IsInstanceOfType<T>(model);
        return (T)model;
    }

    private static NumberModel Field(RowDraft draft, string key)
    {
        var field = draft.Field(key);
        Assert.IsNotNull(field);
        return field;
    }

    private static List<string> Strings(params string[] values) => [.. values];

    private static List<string> GroupIds(SettingsSession session)
    {
        var page = session.Pages.Single(candidate => candidate.Spec.Id == "service");
        return page.Groups.Select(group => group.Spec.Id).ToList();
    }

    private static string Grouped(double value) =>
        value.ToString("N0", CultureInfo.InvariantCulture);

    private static string Plain(double value) => value.ToString(CultureInfo.InvariantCulture);

    private static HeavyRule Rule(string program, JobClass jobClass, params string[] args) =>
        new() { Program = program, Class = jobClass, Args = args };

    private static string Describe(Change change) => change switch
    {
        Change.ResetRow reset => $"reset_row {reset.Name}",
        Change.SetRow row => $"set_row {row.Name}",
        _ => change.GetType().Name,
    };

    private static void AssertDelays(FakeSessionTimers timers, params TimeSpan[] expected) =>
        CollectionAssert.AreEqual(expected, timers.Delays);

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
