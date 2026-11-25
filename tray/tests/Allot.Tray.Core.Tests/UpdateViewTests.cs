using System.Globalization;

using Allot.Tray.Core.Contract;
using Allot.Tray.Core.Daemon;
using Allot.Tray.Core.Samples;
using Allot.Tray.Core.ViewModels;

namespace Allot.Tray.Core.Tests;

/// <summary>
/// What the update banner and the Service page's update card say in each of allotd's states.
/// </summary>
[TestClass]
public sealed class UpdateViewTests
{
    private static readonly DateTimeOffset Now = DateTimeOffset.FromUnixTimeMilliseconds(
        Handoff.NowMs);

    [TestMethod]
    public void AvailableOffersUpdateAndCheckNowInBothPlaces()
    {
        var view = View(UpdateState.Available);

        Assert.AreEqual(
            new UpdateView(
                "Allot 0.7.0 is available",
                "",
                "Last checked 06:52",
                "Update",
                true,
                "Check now",
                true,
                InFlyout: true,
                Failed: false),
            view);
    }

    [TestMethod]
    public void InstallingShowsUpdatingWithNothingToPress()
    {
        var view = View(UpdateState.Installing);

        Assert.AreEqual("Updating to 0.7.0…", view.Title);
        Assert.AreEqual("Updating…", view.UpdateText);
        Assert.IsFalse(view.CanUpdate);
        Assert.AreEqual("", view.CheckText);
        Assert.IsTrue(view.InFlyout);
    }

    [TestMethod]
    public void AFailedInstallSaysWhyAndOffersAnotherTry()
    {
        var view = View(UpdateState.InstallFailed, reason: "the setup exited with code 5");

        Assert.AreEqual("The update to 0.7.0 did not finish", view.Title);
        Assert.AreEqual("The setup exited with code 5.", view.Detail);
        Assert.AreEqual("Try again", view.UpdateText);
        Assert.IsTrue(view.CanUpdate);
        Assert.IsTrue(view.InFlyout);
        Assert.IsTrue(view.Failed);
    }

    [TestMethod]
    [DataRow(UpdateState.UpToDate, "Allot 0.6.1 is up to date", "Check now")]
    [DataRow(UpdateState.Unchecked, "Not checked for updates yet", "Check now")]
    [DataRow(UpdateState.CheckFailed, "Could not check for updates", "Check now")]
    [DataRow(UpdateState.Disabled, "Update checks are off", "")]
    public void QuietStatesShowInSettingsOnly(UpdateState state, string title, string check)
    {
        var view = View(state);

        Assert.AreEqual(title, view.Title);
        Assert.AreEqual(check, view.CheckText);
        Assert.AreEqual("", view.UpdateText);
        Assert.IsFalse(view.InFlyout);
    }

    [TestMethod]
    public void AFailedCheckSaysWhy()
    {
        var view = View(UpdateState.CheckFailed, reason: "could not reach github.com");

        Assert.AreEqual("Could not reach github.com.", view.Detail);
    }

    [TestMethod]
    public void CheckingAgainKeepsTheOfferOnScreen()
    {
        var view = View(UpdateState.Checking);

        Assert.AreEqual("Allot 0.7.0 is available", view.Title);
        Assert.IsFalse(view.CanUpdate);
        Assert.AreEqual("Checking…", view.CheckText);
        Assert.IsFalse(view.CanCheck);
        Assert.IsTrue(view.InFlyout);
    }

    [TestMethod]
    [DataRow("0.6.1")]
    [DataRow(null)]
    public void CheckingWithNothingNewerShowsInSettingsOnly(string? latest)
    {
        var view = View(UpdateState.Checking, latest);

        Assert.AreEqual("Checking for updates…", view.Title);
        Assert.IsFalse(view.InFlyout);
    }

    [TestMethod]
    public void APressOfUpdateShowsUpdatingAtOnce()
    {
        var view = View(UpdateState.Available, press: new(UpdateAction.Starting, null));

        Assert.AreEqual("Updating…", view.UpdateText);
        Assert.IsFalse(view.CanUpdate);
        Assert.IsFalse(view.CanCheck);
    }

    [TestMethod]
    public void ARefusedPressSaysWhy()
    {
        var view = View(
            UpdateState.Available,
            press: new(UpdateAction.None, "an update is already running"));

        Assert.AreEqual("An update is already running.", view.Detail);
        Assert.IsTrue(view.CanUpdate);
    }

    [TestMethod]
    public void ACheckOfAnEarlierDayShowsItsDate()
    {
        var view = View(UpdateState.UpToDate, now: Now.AddDays(1));

        Assert.AreEqual("Last checked 10/24/2026 06:52", view.Checked);
    }

    [TestMethod]
    [DataRow(ServiceStatus.Stopped)]
    [DataRow(ServiceStatus.Starting)]
    [DataRow(ServiceStatus.Incompatible)]
    public void AServiceThatIsNotRunningShowsNothing(ServiceStatus status)
    {
        var view = View(UpdateState.Available, status: status);

        Assert.AreSame(UpdateView.Hidden, view);
    }

    [TestMethod]
    public void AnAllotdThatDoesNotCheckShowsNothing()
    {
        var view = UpdateView.From(
            ServiceStatus.Running,
            null,
            UpdatePress.None,
            Now,
            TimeZoneInfo.Utc,
            CultureInfo.InvariantCulture);

        Assert.AreSame(UpdateView.Hidden, view);
    }

    [TestMethod]
    public void TheFlyoutKeepsOnlyWhatItShows()
    {
        var model = new FlyoutModel();

        model.ApplyUpdate(View(UpdateState.Available));
        var offered = model.UpdateBanner.Title;
        model.ApplyUpdate(View(UpdateState.UpToDate));

        Assert.AreEqual("Allot 0.7.0 is available", offered);
        Assert.AreSame(UpdateView.Hidden, model.UpdateBanner);
    }

    private static UpdateView View(
        UpdateState state,
        string? latest = "0.7.0",
        string? reason = null,
        UpdatePress? press = null,
        ServiceStatus status = ServiceStatus.Running,
        DateTimeOffset? now = null) =>
        UpdateView.From(
            status,
            new UpdateStatus
            {
                Installed = "0.6.1",
                State = state,
                Latest = latest,
                CheckedAtMs = Handoff.NowMs,
                Reason = reason,
                Notes = [],
            },
            press ?? UpdatePress.None,
            now ?? Now,
            TimeZoneInfo.Utc,
            CultureInfo.InvariantCulture);
}
