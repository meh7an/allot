using Allot.Tray.Core.Contract;
using Allot.Tray.Core.Daemon;
using Allot.Tray.Core.Samples;
using Allot.Tray.Core.Settings;
using Allot.Tray.Core.ViewModels;

namespace Allot.Tray.Core.Tests;

/// <summary>
/// The Resources card (handoff section 5 and <c>METERS</c> and <c>DRIVES</c> in
/// <c>Flyout.dc.html</c>): bullet bars, reserved ticks, values, tooltips, drive rows, the
/// collapsed summary, and the menu counts.
/// </summary>
[TestClass]
public sealed class ResourcesCardTests
{
    private const double Tolerance = 0.0001;

    private static ResourcesView View(
        string scenario,
        MeterToggles? toggles = null,
        bool expanded = true)
    {
        var (status, state) = Handoff.Scenario(scenario);
        return ResourcesView.From(status, state, toggles ?? new MeterToggles(), expanded);
    }

    private static ResourcesView View(
        Meters meters,
        IReadOnlyList<ActiveJob>? active = null,
        MeterToggles? toggles = null) =>
        ResourcesView.From(
            ServiceStatus.Running,
            Handoff.Build([], active ?? [], []) with { Meters = meters },
            toggles ?? new MeterToggles(),
            expanded: true);

    private static MeterRow Row(ResourcesView view, string key) =>
        view.Meters.Single(row => row.Key == key);

    private static string Labels(ResourcesView view) =>
        string.Join(" ", view.Meters.Select(row => row.Label));

    [TestMethod]
    public void BusyMetersShowUsedValuesAndReservedTicks()
    {
        var view = View("several");

        Assert.AreEqual("CPU RAM GPU VRAM", Labels(view));
        var cpu = Row(view, "cpu");
        Assert.AreEqual(0.64, cpu.Used, Tolerance);
        Assert.AreEqual(0.625, cpu.Reserved!.Value, Tolerance);
        Assert.AreEqual("64%", cpu.Value);
        Assert.AreEqual("10 of 16 cores reserved", cpu.Tooltip);

        var ram = Row(view, "ram");
        Assert.AreEqual(0.7, ram.Used, 0.001);
        Assert.AreEqual(0.25, ram.Reserved!.Value, Tolerance);
        Assert.AreEqual("22.4 / 32 GB", ram.Value);
        Assert.AreEqual("8 GB reserved", ram.Tooltip);

        var gpu = Row(view, "gpu:0");
        Assert.AreEqual(0.78, gpu.Used, Tolerance);
        Assert.AreEqual(0.8, gpu.Reserved!.Value, Tolerance);
        Assert.AreEqual("78%", gpu.Value);
        Assert.AreEqual("80% reserved", gpu.Tooltip);

        var vram = Row(view, "vram:0");
        Assert.AreEqual(0.425, vram.Used, 0.001);
        Assert.AreEqual(1 / 3.0, vram.Reserved!.Value, Tolerance);
        Assert.AreEqual("5.1 / 12 GB", vram.Value);
        Assert.AreEqual("4 GB reserved", vram.Tooltip);
        Assert.IsTrue(view.ShowLegend);
    }

    [TestMethod]
    public void MetersWithoutAnyLeaseReadNoReservationsAndHaveNoTicks()
    {
        var view = View("empty");

        Assert.IsTrue(view.Meters.All(row => row.Reserved is null));
        Assert.IsTrue(view.Meters.All(row => row.Tooltip == "No reservations"));
        Assert.AreEqual("6%", Row(view, "cpu").Value);
        Assert.AreEqual("14.1 / 32 GB", Row(view, "ram").Value);
        Assert.AreEqual("1.1 / 12 GB", Row(view, "vram:0").Value);
        Assert.IsFalse(view.ShowLegend);
    }

    [TestMethod]
    public void AMeterTheLeasesDoNotReserveReadsNotReserved()
    {
        var view = View("one");

        Assert.AreEqual("8 of 16 cores reserved", Row(view, "cpu").Tooltip);
        Assert.AreEqual("4 GB reserved", Row(view, "ram").Tooltip);
        Assert.AreEqual(0.125, Row(view, "ram").Reserved!.Value, Tolerance);
        Assert.AreEqual("Not reserved", Row(view, "gpu:0").Tooltip);
        Assert.IsNull(Row(view, "gpu:0").Reserved);
        Assert.AreEqual("Not reserved", Row(view, "vram:0").Tooltip);
    }

    [TestMethod]
    public void BenchmarkRunningAloneReservesTheWholeMachine()
    {
        var view = View("bench-run");

        Assert.IsTrue(view.Meters.All(row => row.Reserved == 1));
        Assert.AreEqual("Whole machine reserved for the benchmark", Row(view, "cpu").Tooltip);
        Assert.AreEqual("Whole machine reserved", Row(view, "ram").Tooltip);
        Assert.AreEqual("Whole machine reserved", Row(view, "gpu:0").Tooltip);
        Assert.AreEqual("97%", Row(view, "cpu").Value);
        Assert.AreEqual("9.6 / 32 GB", Row(view, "ram").Value);
    }

    [TestMethod]
    public void QueuedBenchmarkDoesNotReserveTheMachine()
    {
        var view = View("bench-drain");

        Assert.AreEqual("8 of 16 cores reserved", Row(view, "cpu").Tooltip);
    }

    [TestMethod]
    public void SeveralAdaptersAreNumberedAndNamedInTheirTooltips()
    {
        var view = View("max");

        Assert.AreEqual("CPU RAM GPU 0 VRAM 0 GPU 1 VRAM 1", Labels(view));
        Assert.AreEqual("NVIDIA GeForce RTX 4070 · 80% reserved", Row(view, "gpu:0").Tooltip);
        Assert.AreEqual("NVIDIA GeForce RTX 4070 · 4 GB reserved", Row(view, "vram:0").Tooltip);
        Assert.AreEqual(
            "Intel(R) Arc(TM) A380 Graphics · Not reserved",
            Row(view, "gpu:1").Tooltip);
        Assert.AreEqual("34%", Row(view, "gpu:1").Value);
        Assert.AreEqual("2.4 / 6 GB", Row(view, "vram:1").Value);
        Assert.AreEqual(0.4, Row(view, "vram:1").Used, 0.001);
        Assert.AreEqual("13 of 16 cores reserved", Row(view, "cpu").Tooltip);
        Assert.AreEqual(0.8125, Row(view, "cpu").Reserved!.Value, Tolerance);
    }

    [TestMethod]
    public void NoAdapterMeansNoGpuRows()
    {
        var view = View(Handoff.MeterSet("idle") with { Gpus = [] });

        Assert.AreEqual("CPU RAM", Labels(view));
    }

    [TestMethod]
    public void VideoMemoryWithoutATotalHasAnEmptyBar()
    {
        var meters = Handoff.MeterSet("busy");
        meters = meters with { Gpus = [meters.Gpus[0] with { VramTotalMb = null }] };

        var vram = Row(View(meters), "vram:0");

        Assert.AreEqual(0, vram.Used);
        Assert.IsNull(vram.Reserved);
        Assert.AreEqual("5.1 GB", vram.Value);
        Assert.AreEqual("4 GB reserved · total not reported", vram.Tooltip);
    }

    [TestMethod]
    public void HeadroomComesFromTheDaemon()
    {
        Assert.IsTrue(View("several").Meters.All(row => Math.Abs(row.Headroom - 0.85) < Tolerance));

        var meters = Handoff.MeterSet("busy") with { HeadroomPct = 20 };

        Assert.AreEqual(0.8, Row(View(meters), "vram:0").Headroom, Tolerance);
    }

    [TestMethod]
    public void FractionsStayInsideTheBar()
    {
        var meters = Handoff.MeterSet("busy");
        meters = meters with
        {
            Cpu = meters.Cpu with { UsagePct = 104, ReservedCores = 20 },
            Ram = meters.Ram with { TotalMb = 0 },
        };

        var view = View(meters);

        Assert.AreEqual(1, Row(view, "cpu").Used);
        Assert.AreEqual(1, Row(view, "cpu").Reserved);
        Assert.AreEqual(0, Row(view, "ram").Used);
    }

    [TestMethod]
    public void FractionalCoresKeepTheirDecimal()
    {
        var meters = Handoff.MeterSet("busy");
        meters = meters with { Cpu = meters.Cpu with { ReservedCores = 2.5f } };

        Assert.AreEqual("2.5 of 16 cores reserved", Row(View(meters), "cpu").Tooltip);
    }

    [TestMethod]
    public void SparklineHoldsTheLast24SamplesAsFractions()
    {
        var meters = Handoff.MeterSet("busy");
        float[] history = [.. Enumerable.Range(0, 30).Select(i => (float)i * 4)];
        meters = meters with { Cpu = meters.Cpu with { HistoryPct = history } };

        var spark = Row(View(meters), "cpu").Spark;

        Assert.HasCount(24, spark);
        Assert.AreEqual(0.24, spark[0], Tolerance);
        Assert.AreEqual(1, spark[^1]);
        Assert.IsEmpty(Row(View(meters), "ram").Spark);
    }

    [TestMethod]
    public void DrivesShowSpaceAndActivity()
    {
        var view = View("max");

        Assert.HasCount(3, view.Drives);
        Assert.AreEqual("C:", view.Drives[0].Label);
        Assert.AreEqual("R 42 · W 8 MB/s", view.Drives[0].Activity);
        Assert.AreEqual("612 / 953 GB", view.Drives[0].Value);
        Assert.AreEqual(0.642, view.Drives[0].Used, 0.001);
        Assert.AreEqual("Idle", view.Drives[1].Activity);
        Assert.AreEqual("1.21 / 1.82 TB", view.Drives[1].Value);
        Assert.AreEqual(0.665, view.Drives[1].Used, 0.001);
        Assert.AreEqual("W 120 MB/s", view.Drives[2].Activity);
        Assert.AreEqual("0.4 / 1.82 TB", view.Drives[2].Value);
        Assert.IsTrue(view.ShowSeparator);
    }

    [TestMethod]
    public void SlowActivityHasOneDecimalAndTheSlowestIsIdle()
    {
        var meters = Handoff.MeterSet("busy");
        var drive = meters.Drives[0] with
        {
            ReadBytesPerSec = 3_460_300,
            WriteBytesPerSec = 100_000,
        };

        var view = View(meters with { Drives = [drive] });

        Assert.AreEqual("R 3.3 MB/s", view.Drives[0].Activity);
    }

    [TestMethod]
    public void DriveActivityOffHidesTheRates()
    {
        var view = View("several", new MeterToggles { DriveActivity = false });

        Assert.IsTrue(view.Drives.All(drive => drive.Activity == ""));
        Assert.AreEqual("612 / 953 GB", view.Drives[0].Value);
    }

    [TestMethod]
    public void TogglesLeaveOutTheirRows()
    {
        var view = View("max", new MeterToggles { Vram = false, Drives = false });

        Assert.AreEqual("CPU RAM GPU 0 GPU 1", Labels(view));
        Assert.IsEmpty(view.Drives);
        Assert.IsFalse(view.ShowSeparator);
        Assert.IsFalse(view.AllOff);
    }

    [TestMethod]
    public void OnlyDrivesHaveNoSeparatorAndNoLegend()
    {
        var toggles = new MeterToggles { Cpu = false, Ram = false, Gpu = false, Vram = false };

        var view = View("several", toggles);

        Assert.IsEmpty(view.Meters);
        Assert.HasCount(2, view.Drives);
        Assert.IsFalse(view.ShowSeparator);
        Assert.IsFalse(view.ShowLegend);
        Assert.IsFalse(view.AllOff);
    }

    [TestMethod]
    public void EveryMeterOffSaysHowToTurnThemOn()
    {
        var toggles = new MeterToggles
        {
            Cpu = false,
            Ram = false,
            Gpu = false,
            Vram = false,
            Drives = false,
        };

        var view = View("several", toggles);

        Assert.IsTrue(view.AllOff);
        Assert.IsEmpty(view.Meters);
        Assert.IsEmpty(view.Drives);
        Assert.AreEqual("", View("several", toggles, expanded: false).Summary);
    }

    [TestMethod]
    public void CollapsedCardSummarizesCpuRamAndTheFirstGpu()
    {
        Assert.AreEqual(
            "CPU 64% · RAM 22.4 / 32 GB · GPU 78%",
            View("res-collapsed", expanded: false).Summary);
        Assert.AreEqual(
            "CPU 88% · RAM 25.9 / 32 GB · GPU 78%",
            View("max", expanded: false).Summary);
        Assert.AreEqual("", View("several").Summary);
    }

    [TestMethod]
    public void SummaryFollowsTheToggles()
    {
        var view = View("several", new MeterToggles { Ram = false }, expanded: false);

        Assert.AreEqual("CPU 64% · GPU 78%", view.Summary);

        var noGpu = ResourcesView.From(
            ServiceStatus.Running,
            Handoff.Scenario("several").State with
            {
                Meters = Handoff.MeterSet("busy") with { Gpus = [] },
            },
            new MeterToggles(),
            expanded: false);

        Assert.AreEqual("CPU 64% · RAM 22.4 / 32 GB", noGpu.Summary);
    }

    [TestMethod]
    public void NoCardWhileTheServiceIsNotRunning()
    {
        var view = View("down");

        Assert.IsFalse(view.Visible);
        Assert.IsEmpty(view.Meters);
        Assert.IsFalse(ResourcesView.From(
            ServiceStatus.Running, null, new MeterToggles(), expanded: true).Visible);
    }

    [TestMethod]
    public void MenuCountsLeaveOutDriveActivity()
    {
        Assert.AreEqual("5 of 5", MenuCounts.Meters(new MeterToggles()));
        Assert.AreEqual(
            "4 of 5",
            MenuCounts.Meters(new MeterToggles { Vram = false, DriveActivity = false }));
        Assert.AreEqual("3 of 5", MenuCounts.Notifications(new NotificationToggles()));
    }

    [TestMethod]
    public void ModelUpdatesRowsInPlaceAndRebuildsOnAToggle()
    {
        var model = new ResourcesModel();
        model.Apply(View("several"));
        var cpu = model.Meters[0];
        var changed = new List<string?>();
        cpu.PropertyChanged += (_, args) => changed.Add(args.PropertyName);

        model.Apply(View("one"));

        Assert.AreSame(cpu, model.Meters[0]);
        CollectionAssert.Contains(changed, nameof(MeterRowModel.Value));
        Assert.AreEqual("52%", cpu.Value);

        model.Apply(View("one", new MeterToggles { Ram = false }));

        Assert.AreEqual("cpu gpu:0 vram:0", string.Join(" ", model.Meters.Select(row => row.Key)));
    }

    [TestMethod]
    public void UnchangedPollRaisesNothing()
    {
        var model = new ResourcesModel();
        model.Apply(View("several"));
        var changes = 0;
        model.PropertyChanged += (_, _) => changes++;
        foreach (var row in model.Meters)
        {
            row.PropertyChanged += (_, _) => changes++;
        }

        model.Apply(View("several"));

        Assert.AreEqual(0, changes);
    }
}
