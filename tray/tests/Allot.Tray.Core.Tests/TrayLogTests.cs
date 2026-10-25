using Allot.Tray.Core.Logging;
using Allot.Tray.Core.Tests.Support;

namespace Allot.Tray.Core.Tests;

[TestClass]
public sealed class TrayLogTests
{
    private static readonly ManualTimeProvider Clock = new();

    [TestMethod]
    public void WritesTimestampedLinesAndCreatesTheDirectory()
    {
        using var directory = new TempDirectory();
        var log = new TrayLog(directory.File(@"logs\tray.log"), 1024, Clock);

        log.Write("started");
        log.Write("tray icon added");

        var lines = File.ReadAllText(log.FilePath).Split('\n');
        var stamp = Clock.GetLocalNow().ToString(
            "yyyy-MM-dd HH:mm:ss.fff", System.Globalization.CultureInfo.InvariantCulture);
        Assert.AreEqual($"{stamp} started|{stamp} tray icon added|", string.Join("|", lines));
    }

    [TestMethod]
    public void HasNoByteOrderMark()
    {
        using var directory = new TempDirectory();
        var log = new TrayLog(directory.File("tray.log"), 1024, Clock);

        log.Write("x");

        Assert.AreEqual((byte)'2', File.ReadAllBytes(log.FilePath)[0]);
    }

    [TestMethod]
    public void RotatesOnceTheFileIsFull()
    {
        using var directory = new TempDirectory();
        var log = new TrayLog(directory.File("tray.log"), 64, Clock);

        log.Write(new string('a', 60));
        log.Write("second");

        StringAssert.EndsWith(File.ReadAllText(log.RotatedPath), new string('a', 60) + "\n");
        StringAssert.EndsWith(File.ReadAllText(log.FilePath), " second\n");
    }

    [TestMethod]
    public void ARotationReplacesTheOlderFile()
    {
        using var directory = new TempDirectory();
        var log = new TrayLog(directory.File("tray.log"), 40, Clock);

        log.Write(new string('a', 30));
        log.Write(new string('b', 30));
        log.Write("third");

        StringAssert.Contains(File.ReadAllText(log.RotatedPath), new string('b', 30));
        Assert.DoesNotContain(new string('a', 30), File.ReadAllText(log.RotatedPath));
    }

    [TestMethod]
    public void AnUnwritableLogDoesNotThrow()
    {
        using var directory = new TempDirectory();
        // A directory where the file should be makes every append fail.
        _ = Directory.CreateDirectory(directory.File("tray.log"));
        var log = new TrayLog(directory.File("tray.log"), 1024, Clock);

        log.Write("lost");

        Assert.IsTrue(Directory.Exists(log.FilePath));
    }
}
