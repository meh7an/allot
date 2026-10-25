using ResourcesHelper.Tray.Core.Startup;

namespace ResourcesHelper.Tray.Core.Tests;

[TestClass]
public sealed class TrayCommandTests
{
    [TestMethod]
    public void NoArgumentsStartsTheApp() =>
        Assert.AreEqual(new TrayCommand(TrayAction.Run), TrayCommand.Parse([]));

    [TestMethod]
    public void OtherArgumentsStillStartTheApp() =>
        Assert.AreEqual(
            new TrayCommand(TrayAction.Run),
            TrayCommand.Parse(["--scenario", "busy"]));

    [TestMethod]
    public void QuitAlone() =>
        Assert.AreEqual(new TrayCommand(TrayAction.Quit), TrayCommand.Parse(["--quit"]));

    [TestMethod]
    [DataRow("on", true)]
    [DataRow("OFF", false)]
    [DataRow("Off", false)]
    public void LaunchAtLogonTakesOnOrOff(string value, bool on) =>
        Assert.AreEqual(
            new TrayCommand(TrayAction.SetLaunchAtLogon, on),
            TrayCommand.Parse(["--launch-at-logon", value]));

    [TestMethod]
    public void SwitchesWithAWrongValueAreInvalid()
    {
        string[][] wrong =
        [
            ["--launch-at-logon"],
            ["--launch-at-logon", "yes"],
            ["--launch-at-logon", "on", "--quit"],
            ["--quit", "now"],
        ];
        foreach (var args in wrong)
        {
            Assert.AreEqual(
                TrayAction.Invalid,
                TrayCommand.Parse(args).Action,
                string.Join(' ', args));
        }
    }

    [TestMethod]
    public void AsksToQuitFindsTheSwitchWithOrWithoutTheExePath()
    {
        Assert.IsTrue(TrayCommand.AsksToQuit("--quit"));
        Assert.IsTrue(TrayCommand.AsksToQuit(@"""C:\Users\a b\bin\tray\rh-tray.exe"" --quit"));
        Assert.IsFalse(TrayCommand.AsksToQuit(@"""C:\Users\a b\bin\tray\rh-tray.exe"""));
        Assert.IsFalse(TrayCommand.AsksToQuit("--quitting"));
        Assert.IsFalse(TrayCommand.AsksToQuit(""));
        Assert.IsFalse(TrayCommand.AsksToQuit(null));
    }
}
