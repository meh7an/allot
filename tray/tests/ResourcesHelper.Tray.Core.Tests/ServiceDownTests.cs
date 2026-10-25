using System.Globalization;

using ResourcesHelper.Tray.Core.Daemon;
using ResourcesHelper.Tray.Core.Samples;
using ResourcesHelper.Tray.Core.ViewModels;

namespace ResourcesHelper.Tray.Core.Tests;

/// <summary>
/// The service-down banner, its caption, and the last known state below them (handoff
/// section 6).
/// </summary>
[TestClass]
public sealed class ServiceDownTests
{
    private static readonly CultureInfo Invariant = CultureInfo.InvariantCulture;

    [TestMethod]
    public void RunningServiceHasNoBanner()
    {
        var banner = Banner(ServiceStatus.Running, lastKnown: true, problem: "ignored");

        Assert.IsNull(banner);
    }

    [TestMethod]
    public void StoppedServiceOffersStartWithTheLastKnownTime()
    {
        var banner = Banner(ServiceStatus.Stopped, lastKnown: true);

        Assert.AreEqual(
            new ServiceBanner(
                "Service stopped · commands run unchecked",
                "",
                BannerButton.Start,
                "Last known · 06:52"),
            banner);
    }

    [TestMethod]
    public void CaptionIsInTheUsersTimeZone()
    {
        var tehran = TimeZoneInfo.CreateCustomTimeZone(
            "test +03:30", TimeSpan.FromMinutes(210), "test +03:30", "test +03:30");

        var banner = ServiceBanner.From(
            ServiceStatus.Stopped, Handoff.State(), null, tehran, Invariant);

        Assert.AreEqual("Last known · 10:22", banner?.Caption);
    }

    [TestMethod]
    public void CaptionUsesTheUsersTimeFormat()
    {
        var culture = CultureInfo.GetCultureInfo("en-US");

        var time = ServiceBanner.LocalTime(Handoff.NowMs, TimeZoneInfo.Utc, culture);

        // ICU may put a narrow no-break space before AM.
        Assert.AreEqual("6:52 AM", time.Replace(' ', ' '));
    }

    [TestMethod]
    public void StoppedWithoutAnyStateHasNoCaptionAndNoRows()
    {
        var banner = Banner(ServiceStatus.Stopped, lastKnown: false);
        var snapshot = FlyoutSnapshot.From(ServiceStatus.Stopped, null);

        Assert.AreEqual("", banner?.Caption);
        Assert.AreEqual(BannerButton.Start, banner?.Button);
        Assert.AreEqual(0, snapshot.Groups.Count);
        Assert.IsFalse(snapshot.ShowNothing);
        Assert.IsEmpty(snapshot.Recent);
    }

    [TestMethod]
    public void StartingMutesTheButton()
    {
        var banner = Banner(ServiceStatus.Starting, lastKnown: true, problem: "run rh install");

        Assert.AreEqual(BannerButton.Starting, banner?.Button);
        Assert.AreEqual("Service stopped · commands run unchecked", banner?.Text);
        Assert.AreEqual("", banner?.Problem);
        Assert.AreEqual("Last known · 06:52", banner?.Caption);
    }

    [TestMethod]
    public void FailedStartSaysWhyBelowTheText()
    {
        var banner = Banner(
            ServiceStatus.Stopped,
            lastKnown: true,
            problem: "rhd did not answer within 10 s; see logs\\rhd.log");

        Assert.AreEqual("rhd did not answer within 10 s; see logs\\rhd.log", banner?.Problem);
        Assert.AreEqual(BannerButton.Start, banner?.Button);
    }

    [TestMethod]
    public void IncompatibleServiceAsksForAnUpdateWithoutAButton()
    {
        var banner = Banner(ServiceStatus.Incompatible, lastKnown: true, problem: "ignored");

        Assert.AreEqual(
            new ServiceBanner(
                "Service needs an update · run rh install",
                "",
                BannerButton.None,
                "Last known · 06:52"),
            banner);
    }

    [TestMethod]
    public void LastKnownStateShowsItsOwnMomentAndNoActions()
    {
        var state = Handoff.State(run: ["cargo"], wait: ["npm"]);

        var snapshot = FlyoutSnapshot.From(
            ServiceStatus.Stopped, state, nowMs: Handoff.NowMs + 600_000);

        var cards = snapshot.Groups.SelectMany(group => group.Jobs).Select(job => job.Card);
        Assert.IsTrue(snapshot.IsLastKnown);
        Assert.IsTrue(cards.All(card => card.Action == CardAction.None));
        Assert.AreEqual(
            "cargo.exe 18244 · was at 3:06 / ~5:00",
            snapshot.Groups[0].Jobs[0].Card.Detail);
    }

    [TestMethod]
    public void StartingAndIncompatibleShowTheLastKnownStateToo()
    {
        var state = Handoff.State(run: ["cargo"]);

        var starting = FlyoutSnapshot.From(ServiceStatus.Starting, state);
        var incompatible = FlyoutSnapshot.From(ServiceStatus.Incompatible, state);

        Assert.IsTrue(starting.IsLastKnown);
        Assert.AreEqual("Starting service…", starting.Header.Summary);
        Assert.IsTrue(incompatible.IsLastKnown);
        Assert.AreEqual("Service needs an update", incompatible.Header.Summary);
        Assert.AreEqual(1, incompatible.Groups.Count);
    }

    [TestMethod]
    public void ModelDropsTheBannerOnceTheServiceRuns()
    {
        var model = new FlyoutModel();
        var changes = new List<string?>();
        model.PropertyChanged += (_, args) => changes.Add(args.PropertyName);
        model.ApplyBanner(Banner(ServiceStatus.Stopped, lastKnown: true));
        model.ApplyBanner(Banner(ServiceStatus.Stopped, lastKnown: true));
        var down = model.Banner;

        model.ApplyBanner(Banner(ServiceStatus.Running, lastKnown: true));

        Assert.IsNotNull(down);
        Assert.IsNull(model.Banner);
        Assert.AreEqual(2, changes.Count(name => name == nameof(FlyoutModel.Banner)));
    }

    private static ServiceBanner? Banner(
        ServiceStatus status,
        bool lastKnown,
        string? problem = null) =>
        ServiceBanner.From(
            status,
            lastKnown ? Handoff.State(run: ["cargo"]) : null,
            problem,
            TimeZoneInfo.Utc,
            Invariant);
}
