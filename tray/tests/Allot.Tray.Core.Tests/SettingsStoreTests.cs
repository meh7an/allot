using Allot.Tray.Core.Settings;
using Allot.Tray.Core.Tests.Support;

namespace Allot.Tray.Core.Tests;

[TestClass]
public sealed class SettingsStoreTests
{
    [TestMethod]
    public void DefaultsFollowTheHandoff()
    {
        var settings = new TraySettings();

        Assert.AreEqual(
            new MeterToggles
            {
                Cpu = true,
                Ram = true,
                Gpu = true,
                Vram = true,
                Drives = true,
                DriveActivity = true,
            },
            settings.Meters);
        Assert.AreEqual(
            new NotificationToggles
            {
                ServiceStopped = true,
                RunningLong = true,
                BenchmarkFinished = true,
                JobFinished = false,
                KilledOutsideFlyout = false,
            },
            settings.Notifications);
        Assert.IsFalse(settings.ResourcesExpanded);
        Assert.IsFalse(settings.FirstRunDone);
        Assert.IsFalse(settings.FirstRunShown);
    }

    [TestMethod]
    public void MissingFileGivesDefaults()
    {
        using var directory = new TempDirectory();

        var settings = new SettingsStore(directory.File("tray.json")).Load();

        Assert.AreEqual(new TraySettings(), settings);
    }

    [TestMethod]
    public void EverySettingRoundTrips()
    {
        using var directory = new TempDirectory();
        var store = new SettingsStore(directory.File("tray.json"));
        var changed = new TraySettings
        {
            Meters = new MeterToggles { Vram = false, DriveActivity = false },
            Notifications = new NotificationToggles { RunningLong = false, JobFinished = true },
            ResourcesExpanded = true,
            FirstRunDone = true,
            FirstRunShown = true,
        };

        store.Save(changed);

        Assert.AreEqual(changed, store.Load());
    }

    [TestMethod]
    public void FileIsSnakeCaseWithLfAndNoTemporaryLeft()
    {
        using var directory = new TempDirectory();
        var path = directory.File("tray.json");

        new SettingsStore(path).Save(new TraySettings { FirstRunDone = true });

        var text = File.ReadAllText(path);
        StringAssert.Contains(text, "\"first_run_done\": true");
        StringAssert.Contains(text, "\"drive_activity\": true");
        StringAssert.Contains(text, "\"killed_outside_flyout\": false");
        Assert.IsFalse(text.Contains('\r', StringComparison.Ordinal));
        Assert.IsTrue(text.EndsWith("}\n", StringComparison.Ordinal));
        Assert.AreEqual(
            "tray.json",
            Path.GetFileName(Directory.GetFiles(directory.Path).Single()));
    }

    [TestMethod]
    public void SaveCreatesTheDataDirectory()
    {
        using var directory = new TempDirectory();
        var path = Path.Combine(directory.Path, ".allot", "tray.json");

        new SettingsStore(path).Save(new TraySettings());

        Assert.IsTrue(File.Exists(path));
    }

    [TestMethod]
    public void SaveReplacesAnExistingFile()
    {
        using var directory = new TempDirectory();
        var store = new SettingsStore(directory.File("tray.json"));
        store.Save(new TraySettings { ResourcesExpanded = true });

        store.Save(new TraySettings { ResourcesExpanded = false });

        Assert.IsFalse(store.Load().ResourcesExpanded);
    }

    [TestMethod]
    public void MissingMembersKeepTheirDefaults()
    {
        using var directory = new TempDirectory();
        var path = directory.File("tray.json");
        File.WriteAllText(path, """{ "meters": { "gpu": false }, "first_run_done": true }""");

        var settings = new SettingsStore(path).Load();

        Assert.IsFalse(settings.Meters.Gpu);
        Assert.IsTrue(settings.Meters.Cpu);
        Assert.IsTrue(settings.Meters.DriveActivity);
        Assert.AreEqual(new NotificationToggles(), settings.Notifications);
        Assert.IsTrue(settings.FirstRunDone);
    }

    [TestMethod]
    public void UnknownMembersCommentsAndTrailingCommasAreAccepted()
    {
        using var directory = new TempDirectory();
        var path = directory.File("tray.json");
        File.WriteAllText(
            path,
            """
            {
              // written by a newer build
              "theme_override": "dark",
              "resources_expanded": true,
            }
            """);

        Assert.IsTrue(new SettingsStore(path).Load().ResourcesExpanded);
    }

    [TestMethod]
    [DataRow("")]
    [DataRow("{ not json")]
    [DataRow("""{ "meters": null }""")]
    [DataRow("""{ "resources_expanded": "yes" }""")]
    [DataRow("[]")]
    public void CorruptFileGivesDefaults(string content)
    {
        using var directory = new TempDirectory();
        var path = directory.File("tray.json");
        File.WriteAllText(path, content);

        Assert.AreEqual(new TraySettings(), new SettingsStore(path).Load());
    }
}
