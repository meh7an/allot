using Allot.Tray.Core.Daemon;
using Allot.Tray.Core.Tests.Support;

namespace Allot.Tray.Core.Tests;

[TestClass]
public sealed class DaemonConfigTests
{
    [TestMethod]
    [DataRow("")]
    [DataRow("[admission]\nheadroom_pct = 20\n")]
    [DataRow("[sampler]\nport = 1234\n")]
    [DataRow("[daemon]\nport = \"eighty\"\n")]
    [DataRow("[daemon]\nport = 0\n")]
    [DataRow("[daemon]\nport = 70000\n")]
    [DataRow("port = 1234\n")]
    public void FallsBackToTheDefault(string toml) =>
        Assert.AreEqual(DaemonConfig.DefaultPort, DaemonConfig.PortFrom(toml));

    [TestMethod]
    [DataRow("[daemon]\nport = 48000\n")]
    [DataRow("[daemon]\r\nport=48000\r\n")]
    [DataRow("# allotd\n[ daemon ]\n  port = 48_000  # moved\n")]
    [DataRow("daemon.port = 48000\n[admission]\nheadroom_pct = 20\n")]
    [DataRow("[admission]\nheadroom_pct = 20\n[daemon]\nport = 48000\n")]
    public void ReadsTheDaemonPort(string toml) =>
        Assert.AreEqual((ushort)48000, DaemonConfig.PortFrom(toml));

    [TestMethod]
    public void MissingConfigFileGivesTheDefault()
    {
        using var directory = new TempDirectory();

        var port = DaemonConfig.ReadPort(new DataDirectory(directory.Path));

        Assert.AreEqual(DaemonConfig.DefaultPort, port);
    }

    [TestMethod]
    public void ReadsTheConfigFileInTheDataDirectory()
    {
        using var directory = new TempDirectory();
        File.WriteAllText(directory.File("config.toml"), "[daemon]\nport = 47901\n");

        var port = DaemonConfig.ReadPort(new DataDirectory(directory.Path));

        Assert.AreEqual((ushort)47901, port);
    }

    [TestMethod]
    public void DataDirectoryMatchesTheRustPaths()
    {
        var data = DataDirectory.Under(@"C:\Users\someone");

        Assert.AreEqual(@"C:\Users\someone\.allot", data.Root);
        Assert.AreEqual(@"C:\Users\someone\.allot\token", data.TokenFile);
        Assert.AreEqual(@"C:\Users\someone\.allot\config.toml", data.ConfigFile);
        Assert.AreEqual(@"C:\Users\someone\.allot\bin\allotd.exe", data.DaemonExe);
        Assert.AreEqual(@"C:\Users\someone\.allot\logs", data.LogDirectory);
        Assert.AreEqual(@"C:\Users\someone\.allot\tray.json", data.SettingsFile);
    }
}
