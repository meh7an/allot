using System.Globalization;

using Allot.Tray.Core.Contract;
using Allot.Tray.Core.ServiceSettings;
using Allot.Tray.Core.Tests.Support;

namespace Allot.Tray.Core.Tests;

/// <summary>
/// What the Settings window speaks to screen readers by itself (M7.1): errors and outcomes, as
/// the shared JS announcer does, and the size rows' spoken names.
/// </summary>
[TestClass]
public sealed class SettingsAnnouncerTests
{
    private static readonly Func<double, string> Number =
        SettingsText.Numbers(CultureInfo.InvariantCulture);

    [TestMethod]
    public void TheFirstViewOnlySetsWhatIsOnScreen()
    {
        var announcer = new SettingsAnnouncer();

        Assert.AreEqual(string.Empty, announcer.Update(View(banners: ["The service is stopped."])));
        Assert.AreEqual(string.Empty, announcer.Update(View(banners: ["The service is stopped."])));
    }

    [TestMethod]
    public void ABannerIsSpokenOnceWhenItAppears()
    {
        var announcer = new SettingsAnnouncer();
        _ = announcer.Update(View());

        Assert.AreEqual("A B", announcer.Update(View(banners: ["A", "B"])));
        Assert.AreEqual(string.Empty, announcer.Update(View(banners: ["B"])));
        Assert.AreEqual("A", announcer.Update(View(banners: ["A", "B"])));
    }

    [TestMethod]
    public void AnotherPagesBannersAreReadWithThePage()
    {
        var announcer = new SettingsAnnouncer();
        _ = announcer.Update(View(banners: ["A"]));

        announcer.PageShown();

        Assert.AreEqual(
            string.Empty,
            announcer.Update(View(banners: ["A", "Restart the service."])));
    }

    [TestMethod]
    public void AnErrorIsSpokenWhenItAppearsOrChanges()
    {
        var announcer = new SettingsAnnouncer();
        var range = new AnnouncedIssue("a", "Headroom: Enter a number at least 0 and below 100.");
        var blank = new AnnouncedIssue("a", "Headroom: Enter a value.");

        Assert.AreEqual(range.Text, announcer.Update(View(issues: [range])));
        Assert.AreEqual(string.Empty, announcer.Update(View(issues: [range])));
        Assert.AreEqual(blank.Text, announcer.Update(View(issues: [blank])));
        Assert.AreEqual(string.Empty, announcer.Update(View()));
        Assert.AreEqual(blank.Text, announcer.Update(View(issues: [blank])));
    }

    [TestMethod]
    public void ASaveThatEndsWithNothingUnsavedSaysSoOnce()
    {
        var announcer = new SettingsAnnouncer();
        _ = announcer.Update(View());

        Assert.AreEqual(string.Empty, announcer.Update(View(save: SaveKind.Saving)));
        Assert.AreEqual(SettingsAnnouncer.SavedAnnouncement, announcer.Update(View()));
        Assert.AreEqual(string.Empty, announcer.Update(View()));
    }

    [TestMethod]
    public void ASaveThatEndsInAProblemSpeaksTheProblem()
    {
        var announcer = new SettingsAnnouncer();
        _ = announcer.Update(View(save: SaveKind.Saving));

        Assert.AreEqual(string.Empty, announcer.Update(View(save: SaveKind.Unsaved)));

        _ = announcer.Update(View(save: SaveKind.Saving));
        Assert.AreEqual(
            "config.toml changed outside Settings.",
            announcer.Update(View(banners: ["config.toml changed outside Settings."])));
    }

    [TestMethod]
    public async Task ASessionsOutOfRangeValueIsSpokenWithItsLabelThenTheSave()
    {
        var client = new FakeSettingsClient();
        var timers = new FakeSessionTimers();
        var session = new SettingsSession(
            client,
            timers,
            Platform.Windows,
            _ => Task.FromResult<string?>(null));
        await session.RefreshAsync();
        var announcer = new SettingsAnnouncer();
        string Speak() => announcer.Update(AnnouncerView.Of(
            session,
            SettingsBanners.For(session, "admission", 47823, Number),
            Number));
        _ = Speak();
        var spoken = new List<string>();
        session.PropertyChanged += (_, _) =>
        {
            if (Speak() is { Length: > 0 } text)
            {
                spoken.Add(text);
            }
        };
        var headroom = (NumberModel)session.Setting("admission.headroom_pct")!;

        headroom.Commit(150);
        headroom.Commit(20);
        client.CompleteAll();
        timers.FireAll();
        client.CompleteAll();

        CollectionAssert.AreEqual(
            new List<string>
            {
                $"{headroom.Label}: Enter a number at least 0 and below 100.",
                SettingsAnnouncer.SavedAnnouncement,
            },
            spoken);
    }

    [TestMethod]
    public void ASizeRowReadsAsOneSentence()
    {
        var row = new SizeRow
        {
            Name = "gpu_heavy",
            Profile = new JobProfile
            {
                Cores = 2,
                RamMb = 4096,
                GpuPct = 80,
                VramMb = 4096,
                EstSecs = 600,
            },
        };
        var fit = new SizeFit
        {
            Name = "gpu_heavy",
            MinCores = 1,
            Now = new Fits { Full = 1, Shrunk = 2, More = false },
            Idle = new Fits { Full = 3, Shrunk = 0, More = false },
        };

        Assert.AreEqual(
            "GPU heavy: 2 cores, at least 1 (automatic), 4 GB, GPU 80 %, 4 GB video memory, "
                + "10 minutes, fits 1, 2 more shrunk",
            SizesText.Spoken("GPU heavy", row, fit, Number));
        Assert.AreEqual(
            "GPU heavy: 2 cores, at least 1 (automatic), 4 GB, GPU 80 %, 4 GB video memory, "
                + "10 minutes",
            SizesText.Spoken("GPU heavy", row, null, Number));
    }

    private static AnnouncerView View(
        List<string>? banners = null,
        List<AnnouncedIssue>? issues = null,
        SaveKind save = SaveKind.Saved) =>
        new(banners ?? [], issues ?? [], save);
}
