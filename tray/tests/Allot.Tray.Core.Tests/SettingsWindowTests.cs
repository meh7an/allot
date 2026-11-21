using System.Globalization;
using System.Text.Json;

using Allot.Tray.Core.Contract;
using Allot.Tray.Core.Daemon;
using Allot.Tray.Core.ServiceSettings;
using Allot.Tray.Core.Settings;
using Allot.Tray.Core.Tests.Support;

namespace Allot.Tray.Core.Tests;

/// <summary>
/// The Settings window's view models (M4.1): banners, live sentences, the service row, the Panel
/// page's toggles, field reset with Undo, and the window's fixed sentences.
/// </summary>
[TestClass]
public sealed class SettingsWindowTests
{
    private const string Held =
        "config.toml is open in another program that blocks changes. Close it there and try again.";

    private static readonly Func<double, string> Number =
        SettingsText.Numbers(CultureInfo.InvariantCulture);

    [TestMethod]
    public void SaveStatusReadsAsTheDesignSays()
    {
        Assert.AreEqual(
            "All changes saved",
            SettingsText.SaveStatus(new SaveStatus(SaveKind.Saved)));
        Assert.AreEqual("Saving…", SettingsText.SaveStatus(new SaveStatus(SaveKind.Saving)));
        Assert.AreEqual(
            "1 change not saved",
            SettingsText.SaveStatus(new SaveStatus(SaveKind.Unsaved, 1)));
        Assert.AreEqual(
            "3 changes not saved",
            SettingsText.SaveStatus(new SaveStatus(SaveKind.Unsaved, 3)));
    }

    [TestMethod]
    public void NumbersAreGroupedWithAtMostTwoDecimals()
    {
        Assert.AreEqual("5,000", Number(5000));
        Assert.AreEqual("3.3", Number(3.3));
        Assert.AreEqual("2.46", Number(2.456));
        Assert.AreEqual("15", Number(15));
    }

    [TestMethod]
    public async Task ThePanelPageShowsNoServiceBanner()
    {
        var session = await LoadAsync(Unreachable(), stopRecord: Fixtures.Read("stop-record.json"));

        Assert.AreEqual(SessionStatus.Stopped, session.Status);
        Assert.IsEmpty(SettingsBanners.For(session, SettingsBanners.PanelPage, 47823, Number));
    }

    [TestMethod]
    public async Task StoppedByYouOffersStartService()
    {
        var session = await LoadAsync(Unreachable(), stopRecord: Fixtures.Read("stop-record.json"));

        var banner = SettingsBanners.For(session, "admission", 47823, Number).Single();

        Assert.AreEqual(BannerSeverity.Informational, banner.Severity);
        Assert.AreEqual("The service is stopped. Start it to change these settings.", banner.Message);
        CollectionAssert.AreEqual(
            Actions(BannerAction.StartService),
            banner.Actions.ToList());
    }

    [TestMethod]
    public async Task NotAnsweringNamesThePortThisPanelDials()
    {
        var session = await LoadAsync(Unreachable());

        var banner = SettingsBanners.For(session, "leases", 47823, Number).Single();

        Assert.AreEqual(BannerSeverity.Warning, banner.Severity);
        Assert.AreEqual(
            "The service isn't answering on port 47823. Trying again every 5 s.",
            banner.Message);
        CollectionAssert.AreEqual(
            Actions(BannerAction.TryNow, BannerAction.OpenLogs),
            banner.Actions.ToList());
    }

    [TestMethod]
    public async Task ARefusedKeyOffersRestartService()
    {
        var session = await LoadAsync(
            FakeSettingsClient.Failure<SettingsState>(SettingsFailureKind.Token));

        var banner = SettingsBanners.For(session, "service", 47823, Number).Single();

        Assert.AreEqual(BannerSeverity.Error, banner.Severity);
        Assert.AreEqual(
            "The service refused this panel's key. Restart the service to make a new one.",
            banner.Message);
        CollectionAssert.AreEqual(
            Actions(BannerAction.RestartService),
            banner.Actions.ToList());
    }

    [TestMethod]
    public async Task AnOutdatedServiceOffersRestartAndLogsButNoTryNow()
    {
        var session = await LoadAsync(
            FakeSettingsClient.Failure<SettingsState>(
                SettingsFailureKind.Outdated, "the service answered 404"));

        var banner = SettingsBanners.For(session, "admission", 47823, Number).Single();

        Assert.AreEqual(SessionStatus.Outdated, session.Status);
        Assert.AreEqual(BannerSeverity.Error, banner.Severity);
        Assert.AreEqual(
            "The service running now is older than this app and has no settings. Restart it to "
                + "run the version installed with this app, or update it with allot install.",
            banner.Message);
        CollectionAssert.AreEqual(
            Actions(BannerAction.RestartService, BannerAction.OpenLogs),
            banner.Actions.ToList());
    }

    [TestMethod]
    public async Task ADetailThatStartsInLowercaseStartsItsSentenceInUppercase()
    {
        var session = await LoadAsync(
            FakeSettingsClient.Failure<SettingsState>(
                SettingsFailureKind.Failed, "the service answered 404"));

        var banner = SettingsBanners.For(session, "admission", 47823, Number).Single();

        Assert.AreEqual(
            "The service sent an answer Settings can't use. The service answered 404.",
            banner.Message);
        Assert.AreEqual("Closed it.", SettingsBanners.Sentence(" closed it"));
        Assert.AreEqual("Already. Fine.", SettingsBanners.Sentence("Already. Fine."));
        Assert.AreEqual(string.Empty, SettingsBanners.Sentence(null));
        Assert.AreEqual(
            "config.toml is open.",
            SettingsBanners.Sentence("config.toml is open"));
        Assert.AreEqual(
            "allotd was not found.",
            SettingsBanners.Sentence("allotd was not found", capitalize: false));
    }

    [TestMethod]
    public async Task ANewPortWaitsForARestartOnTheServicePageOnly()
    {
        var session = await LoadAsync();

        var banner = SettingsBanners.For(session, "service", 47823, Number).Single();

        Assert.AreEqual(BannerSeverity.Informational, banner.Severity);
        Assert.AreEqual("Restart the service to use port 47999.", banner.Message);
        CollectionAssert.AreEqual(
            Actions(BannerAction.RestartService),
            banner.Actions.ToList());
        Assert.IsEmpty(SettingsBanners.For(session, "admission", 47823, Number));
        Assert.AreEqual(
            "Restart needed. The service still listens on 47823.",
            SettingsBanners.RestartNote(session, "daemon.port"));
        Assert.IsNull(SettingsBanners.RestartNote(session, "admission.headroom_pct"));
    }

    [TestMethod]
    public async Task ARejectedFileIsAnErrorBannerAndKeepsThePagesReadOnly()
    {
        var client = new FakeSettingsClient();
        client.State = client.State with { FileError = "expected a number at line 3" };
        var session = await LoadAsync(client: client);

        var banner = SettingsBanners.For(session, "admission", 47823, Number).Single();

        Assert.IsTrue(session.ReadOnly);
        Assert.AreEqual(BannerSeverity.Error, banner.Severity);
        Assert.AreEqual(
            "config.toml has an error. Expected a number at line 3. The service keeps using its "
                + "last good settings. Fix the file to change settings here.",
            banner.Message);
        CollectionAssert.AreEqual(
            Actions(BannerAction.OpenFile, BannerAction.CheckAgain),
            banner.Actions.ToList());
    }

    [TestMethod]
    public async Task AHandEditConflictNamesTheChangeThatWasNotSaved()
    {
        var client = new FakeSettingsClient();
        var timers = new FakeSessionTimers();
        var session = await LoadAsync(client: client, timers: timers);
        client.Writes.Enqueue(FakeSettingsClient.Failure<SettingsState>(
            SettingsFailureKind.Conflict));
        Setting<NumberModel>(session, "estimates.margin_pct").Commit(15);

        Fire(client, timers);

        var banner = SettingsBanners.For(session, "admission", 47823, Number).Single();
        Assert.AreEqual(BannerSeverity.Warning, banner.Severity);
        Assert.AreEqual(
            "config.toml changed outside Settings. Showing the file's values. Your change to "
                + "Safety margin (15 %) was not saved.",
            banner.Message);
        CollectionAssert.AreEqual(
            Actions(BannerAction.ApplyMine, BannerAction.KeepFileValues),
            banner.Actions.ToList());
    }

    [TestMethod]
    public async Task AHeldFileKeepsTheChangesAndOffersTryAgain()
    {
        var client = new FakeSettingsClient();
        var timers = new FakeSessionTimers();
        var session = await LoadAsync(client: client, timers: timers);
        client.Writes.Enqueue(
            FakeSettingsClient.Failure<SettingsState>(SettingsFailureKind.Held, Held));
        Setting<NumberModel>(session, "admission.headroom_pct").Commit(20);

        Fire(client, timers);

        var banner = SettingsBanners.For(session, "admission", 47823, Number).Single();
        Assert.AreEqual($"Your changes are not saved yet. {Held}", banner.Message);
        CollectionAssert.AreEqual(Actions(BannerAction.SaveAgain), banner.Actions.ToList());
        Assert.AreEqual(new SaveStatus(SaveKind.Unsaved, 1), session.SaveStatus);
    }

    [TestMethod]
    public void HeadroomSentenceNamesWhatIsKeptFree()
    {
        var kept = new KeptFree
        {
            Cores = 2.4,
            TotalCores = 16,
            RamMb = 4915.2,
            TotalRamMb = 32768,
            GpuPct = 15,
            VramMb = 1228.8,
            TotalVramMb = 8192,
        };

        Assert.AreEqual(
            "Keeps 2.4 of 16 cores, 4.8 of 32 GB memory, 15 % of the GPU and 1.2 of 8 GB video "
                + "memory free.",
            LiveText.Headroom(kept, Number));
        Assert.AreEqual(
            "Keeps 2.4 of 16 cores and 4.8 of 32 GB memory free.",
            LiveText.Headroom(
                kept with { GpuPct = null, VramMb = null, TotalVramMb = null },
                Number));
    }

    [TestMethod]
    public void EstimatesSentenceFollowsMarginAndRuns()
    {
        Assert.AreEqual(
            "A command that asks for 8 cores but used about 3 in its last 3 runs is reserved 3.3 "
                + "cores. Requests never go below 0.25 cores, 256 MB, or their minimum cores, and "
                + "run time never lowers.",
            LiveText.Estimates(true, 3, 10, Number));
        StringAssert.Contains(
            LiveText.Estimates(true, 1, 0, Number),
            "in its last run is reserved 3 cores.");
        Assert.AreEqual(LiveText.LoweringOff, LiveText.Estimates(false, 3, 10, Number));
    }

    [TestMethod]
    public void FitsReadAtFullSizeAndShrunk()
    {
        Assert.AreEqual("None fit", LiveText.Fits(Counts(0, 0), 4, Number));
        Assert.AreEqual("1 at 4 cores", LiveText.Fits(Counts(0, 1), 4, Number));
        Assert.AreEqual(
            "1 at full size, 2 more at 4 cores",
            LiveText.Fits(Counts(1, 2), 4, Number));
        Assert.AreEqual("1 at full size", LiveText.Fits(Counts(1, 0), 1, Number));
        Assert.AreEqual(
            "at least 99 at full size",
            LiveText.Fits(Counts(99, 0, more: true), 1, Number));
        Assert.AreEqual(
            "2 at full size, at least 99 more at 0.5 cores",
            LiveText.Fits(Counts(2, 99, more: true), 0.5, Number));
    }

    [TestMethod]
    public void TheFixturePreviewFillsEveryClass()
    {
        var preview = JsonSerializer.Deserialize(
            Fixtures.Read("fit-preview.json"),
            ContractJsonContext.Default.FitPreview)!;
        var cpu = preview.Classes.Single(fit => fit.Name == "cpu_heavy");

        Assert.AreEqual("1 at 4 cores", LiveText.Fits(cpu.Now, cpu.MinCores, Number));
        Assert.AreEqual(
            "1 at full size, 1 more at 4 cores",
            LiveText.Fits(cpu.Idle, cpu.MinCores, Number));
        StringAssert.StartsWith(
            LiveText.FitsCaption(37.6, Number),
            "Based on this machine now (38 %");
    }

    [TestMethod]
    public void BenchmarksSentenceJoinsEqualLimits()
    {
        Assert.AreEqual(
            "A benchmark starts once CPU and GPU stay at or below 10 % for 10 seconds. If that "
                + "takes longer than 1 minute, it starts anyway, marked not quiet.",
            LiveText.Benchmarks(10, 10, 10, 60, Number));
        Assert.AreEqual(
            "A benchmark starts once CPU stays at or below 10 % and GPU at or below 20 % for 90 "
                + "seconds. If that takes longer than 10 minutes, it starts anyway, marked not "
                + "quiet.",
            LiveText.Benchmarks(10, 20, 90, 600, Number));
    }

    [TestMethod]
    public void HistorySentenceGroupsLargeCounts() =>
        Assert.AreEqual(
            "Keeps ended leases for 30 days and job runs for 90 days. Each command keeps at least "
                + "its newest 20 runs, and never more than 5,000 runs are stored.",
            LiveText.History(30, 90, 20, 5000, Number));

    [TestMethod]
    public void ServiceRowFollowsThePollAndTheStopRecord()
    {
        var running = ServiceRow.From(ServiceStatus.Running, SessionStatus.Ready, "0.5.0", 47823);
        Assert.AreEqual("Running · version 0.5.0 · port 47823", running.Text);
        Assert.AreEqual("Stop service", running.ButtonText);
        Assert.IsFalse(running.OpenLogs);

        var stopped = ServiceRow.From(ServiceStatus.Stopped, SessionStatus.Stopped, null, 47823);
        Assert.AreEqual(ServiceRow.StoppedByYou, stopped.Text);
        Assert.AreEqual(ServiceButton.Start, stopped.Button);
        Assert.IsFalse(stopped.OpenLogs);

        var down = ServiceRow.From(ServiceStatus.Stopped, SessionStatus.Down, null, 47823);
        Assert.AreEqual("Not answering on port 47823.", down.Text);
        Assert.AreEqual("Start service", down.ButtonText);
        Assert.IsTrue(down.OpenLogs);

        var starting = ServiceRow.From(ServiceStatus.Starting, SessionStatus.Down, null, 47823);
        Assert.AreEqual(ServiceButton.Starting, starting.Button);
        Assert.AreEqual("Starting…", starting.ButtonText);
    }

    [TestMethod]
    public void BannerListsAreTheSameWhenTheyReadAndActTheSame()
    {
        static SettingsBanner Banner(string message, params BannerAction[] actions) =>
            new(BannerSeverity.Warning, message, actions);

        var shown = new[] { Banner("Stopped.", BannerAction.StartService) };

        Assert.IsTrue(SettingsBanners.Same(
            shown, [Banner("Stopped.", BannerAction.StartService)]));
        Assert.IsFalse(SettingsBanners.Same(
            shown, [Banner("Stopped!", BannerAction.StartService)]));
        Assert.IsFalse(SettingsBanners.Same(
            shown, [Banner("Stopped.", BannerAction.OpenLogs)]));
        Assert.IsFalse(SettingsBanners.Same(shown, []));
    }

    [TestMethod]
    public void ServiceRowShowsWhyTheLastStartFailed()
    {
        var failed = ServiceRow.From(
            ServiceStatus.Stopped, SessionStatus.Down, null, 47823, "allotd.exe was not found");

        Assert.AreEqual("The service did not start. allotd.exe was not found.", failed.Text);
        Assert.AreEqual(ServiceButton.Start, failed.Button);
        Assert.IsTrue(failed.OpenLogs);
    }

    [TestMethod]
    public void PanelTogglesWriteTheirOwnSetting()
    {
        var settings = new TraySettings();
        foreach (var toggle in PanelToggles.Meters.Concat(PanelToggles.Notifications))
        {
            var before = Copy(settings);
            toggle.Set(settings, !toggle.Default);

            Assert.AreEqual(!toggle.Default, toggle.Get(settings), toggle.Label);
            Assert.AreNotEqual(before, Copy(settings), toggle.Label);
            toggle.Set(settings, toggle.Default);
        }

        Assert.AreEqual(Copy(new TraySettings()), Copy(settings));
    }

    [TestMethod]
    public void DriveActivityNeedsDrives()
    {
        var activity = PanelToggles.Meters.Single(toggle => toggle.Label == "Drive activity");
        var settings = new TraySettings();

        Assert.IsTrue(activity.EnabledIn(settings));
        settings.Meters.Drives = false;
        Assert.IsFalse(activity.EnabledIn(settings));
    }

    [TestMethod]
    public void PanelResetPageResetsOnlyTheToggles()
    {
        var settings = new TraySettings
        {
            Meters = new MeterToggles { Cpu = false },
            Notifications = new NotificationToggles { JobFinished = true },
            ResourcesExpanded = true,
            FirstRunDone = true,
        };

        CollectionAssert.AreEqual(
            new List<string> { "CPU", "Job finished" },
            PanelToggles.Changed(settings).Select(toggle => toggle.Label).ToList());
        PanelToggles.Reset(settings);

        Assert.IsEmpty(PanelToggles.Changed(settings));
        Assert.AreEqual(new MeterToggles(), settings.Meters);
        Assert.AreEqual(new NotificationToggles(), settings.Notifications);
        Assert.IsTrue(settings.ResourcesExpanded);
        Assert.IsTrue(settings.FirstRunDone);
    }

    [TestMethod]
    public async Task ResetThenUndoWritesTheOldNumberBack()
    {
        var host = new RecordingHost();
        var spec = await SpecOf("leases.heartbeat_ttl_secs");
        var model = new DurationModel(spec, host, 120);

        var undo = FieldReset.Reset(model);

        Assert.AreEqual(60, model.Value);
        Assert.IsInstanceOfType<Change.Reset>(host.Changes.Last());
        Assert.IsNotNull(undo);
        undo();
        Assert.AreEqual(120, model.Value);
        Assert.AreEqual(120, ((Change.SetNumber)host.Changes.Last()).Value);
    }

    [TestMethod]
    public async Task ResetThenUndoTurnsAnOffDurationOffAgain()
    {
        var host = new RecordingHost();
        var spec = await SpecOf("leases.idle_release_secs");
        var model = new DurationModel(spec, host, 0);

        var undo = FieldReset.Reset(model);

        Assert.IsTrue(model.IsOn);
        undo!();
        Assert.IsFalse(model.IsOn);
        Assert.AreEqual(0, ((Change.SetNumber)host.Changes.Last()).Value);
    }

    [TestMethod]
    public async Task ResetThenUndoTurnsASwitchBack()
    {
        var host = new RecordingHost();
        var model = new SwitchModel(await SpecOf("hook.fail_open"), host, false);

        var undo = FieldReset.Reset(model);

        Assert.IsTrue(model.Value);
        undo!();
        Assert.IsFalse(model.Value);
        Assert.IsFalse(((Change.SetSwitch)host.Changes.Last()).Value);
    }

    [TestMethod]
    public async Task ResetTextsNameTheDefault()
    {
        var session = await LoadAsync();
        var port = Setting<NumberModel>(session, "daemon.port");
        var days = Setting<NumberModel>(session, "retention.lease_days");
        var idle = Setting<DurationModel>(session, "leases.idle_release_secs");

        Assert.AreEqual("Port reset to 47823.", SettingsText.FieldReset(port, Number));
        Assert.AreEqual("Reset Port to 47823", SettingsText.ResetButtonName(port, Number));
        Assert.AreEqual("Idle for reset to 3 minutes.", SettingsText.FieldReset(idle, Number));

        var page = SettingsText.ResetPage("Leases", [days, port], Number);
        Assert.AreEqual("Reset 2 settings on Leases?", page.Title);
        Assert.AreEqual(
            "These go back to their defaults:\nKeep ended leases for: 30 days\nPort: 47823",
            page.Body);
        Assert.AreEqual("Reset 2 settings", page.Button);

        Assert.AreEqual("Reset the 1 changed setting?", SettingsText.ResetAll(1).Title);
        Assert.AreEqual("Reset 1 setting", SettingsText.ResetAll(1).Button);
        Assert.AreEqual("Reset all 12 changed settings?", SettingsText.ResetAll(12).Title);
    }

    private static async Task<SettingsSession> LoadAsync(
        SettingsReply<SettingsState>? firstRead = null,
        string? stopRecord = null,
        FakeSettingsClient? client = null,
        FakeSessionTimers? timers = null)
    {
        client ??= new FakeSettingsClient();
        if (firstRead is not null)
        {
            client.Reads.Enqueue(firstRead);
        }

        var session = new SettingsSession(
            client,
            timers ?? new FakeSessionTimers(),
            Platform.Windows,
            _ => Task.FromResult(stopRecord));
        await session.RefreshAsync();
        return session;
    }

    private static async Task<SettingSpec> SpecOf(string key)
    {
        var session = await LoadAsync();
        var model = session.Setting(key);
        Assert.IsNotNull(model);
        return model.Spec;
    }

    private static T Setting<T>(SettingsSession session, string key)
        where T : ValueModel
    {
        var model = session.Setting(key);
        Assert.IsNotNull(model);
        Assert.IsInstanceOfType<T>(model);
        return (T)model;
    }

    private static SettingsReply<SettingsState> Unreachable() =>
        FakeSettingsClient.Failure<SettingsState>(SettingsFailureKind.Unreachable);

    private static void Fire(FakeSettingsClient client, FakeSessionTimers timers)
    {
        client.CompleteAll();
        timers.FireAll();
        client.CompleteAll();
    }

    private static Fits Counts(uint full, uint shrunk, bool more = false) =>
        new() { Full = full, Shrunk = shrunk, More = more };

    private static List<BannerAction> Actions(params BannerAction[] actions) => [.. actions];

    // The toggles are mutable records; a copy keeps their values at this moment.
    private static (MeterToggles Meters, NotificationToggles Notifications) Copy(
        TraySettings settings) =>
        (settings.Meters with { }, settings.Notifications with { });

    /// <summary>
    /// A host that only records what its models report.
    /// </summary>
    private sealed class RecordingHost : IValueHost
    {
        public List<Change> Changes { get; } = [];

        public bool? SwitchOf(string key) => null;

        public double? NumberOf(string key) => null;

        public IReadOnlyList<SizeRow>? RowsOf(string key) => null;

        public string LabelOf(string key) => key;

        public void Edited(ValueModel model, IReadOnlyList<Change> changes, bool now) =>
            Changes.AddRange(changes);
    }
}
