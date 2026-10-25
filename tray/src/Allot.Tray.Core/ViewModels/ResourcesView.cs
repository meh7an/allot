using System.Globalization;

using Allot.Tray.Core.Contract;
using Allot.Tray.Core.Daemon;
using Allot.Tray.Core.Settings;

namespace Allot.Tray.Core.ViewModels;

/// <summary>
/// One bullet-bar row of the Resources card: CPU, RAM, or a GPU's engine load or video memory.
/// </summary>
/// <param name="Key">Stable across polls: <c>cpu</c>, <c>ram</c>, <c>gpu:0</c>,
/// <c>vram:0</c>.</param>
/// <param name="Label">For example <c>CPU</c>, or <c>GPU 0</c> with several adapters.</param>
/// <param name="Used">The used fill, as a fraction of the bar.</param>
/// <param name="Reserved">Where the reserved tick sits, as a fraction of the bar; <c>null</c>
/// without a reservation.</param>
/// <param name="Headroom">Where the hatched headroom starts, as a fraction of the bar.</param>
/// <param name="Value">For example <c>64%</c> or <c>22.4 / 32 GB</c>.</param>
/// <param name="Tooltip">The bar's detail, for example <c>10 of 16 cores reserved</c>.</param>
/// <param name="Spark">The CPU sparkline's samples as fractions, oldest first; empty on other
/// rows.</param>
public sealed record MeterRow(
    string Key,
    string Label,
    double Used,
    double? Reserved,
    double Headroom,
    string Value,
    string Tooltip,
    IReadOnlyList<double> Spark)
{
    public bool Equals(MeterRow? other) =>
        other is not null
        && Key == other.Key
        && Label == other.Label
        && Used == other.Used
        && Reserved == other.Reserved
        && Headroom == other.Headroom
        && Value == other.Value
        && Tooltip == other.Tooltip
        && Spark.SequenceEqual(other.Spark);

    public override int GetHashCode() => HashCode.Combine(Key, Used, Reserved, Value);
}

/// <summary>
/// One fixed drive of the Resources card.
/// </summary>
/// <param name="Label">The drive letter with its colon, for example <c>C:</c>.</param>
/// <param name="Used">The usage bar's fill, as a fraction.</param>
/// <param name="Activity">For example <c>R 42 · W 8 MB/s</c> or <c>Idle</c>; empty while Drive
/// activity is off.</param>
/// <param name="Value">For example <c>612 / 953 GB</c> or <c>1.21 / 1.82 TB</c>.</param>
public sealed record DriveRow(string Label, double Used, string Activity, string Value);

/// <summary>
/// The docked Resources card for one poll (handoff section 5), built by a pure function so that
/// every string is testable.
/// </summary>
/// <param name="Visible">The service serves live state; there is no card without data.</param>
/// <param name="Expanded">The card shows its rows instead of the summary.</param>
/// <param name="Summary">The one-line summary shown while collapsed, for example
/// <c>CPU 64% · RAM 22.4 / 32 GB · GPU 78%</c>; empty while expanded.</param>
/// <param name="Meters">CPU, RAM, then GPU and VRAM per adapter, as the toggles allow.</param>
/// <param name="Drives">One row per fixed drive while Drives is on.</param>
/// <param name="ShowSeparator">The hairline between the meters and the drives.</param>
/// <param name="AllOff">Every meter is off; the card says how to turn them on.</param>
/// <param name="ShowLegend">Some bar has a reserved tick, so the legend explains it.</param>
public sealed record ResourcesView(
    bool Visible,
    bool Expanded,
    string Summary,
    IReadOnlyList<MeterRow> Meters,
    IReadOnlyList<DriveRow> Drives,
    bool ShowSeparator,
    bool AllOff,
    bool ShowLegend)
{
    private const int SparkSamples = 24;
    private const double MebibytesPerTebibyte = 1024 * 1024;
    private const double BytesPerMebibyte = 1024 * 1024;
    private const double IdleRateMb = 0.1;
    private const double WholeRateMb = 10;

    public static ResourcesView Hidden { get; } =
        new(false, false, "", [], [], false, false, false);

    /// <param name="status">The service status of the latest poll.</param>
    /// <param name="state">The latest state; ignored unless the service is running.</param>
    /// <param name="toggles">The ⋯ › Meters check items.</param>
    /// <param name="expanded">The card shows its rows instead of the summary.</param>
    public static ResourcesView From(
        ServiceStatus status,
        UiState? state,
        MeterToggles toggles,
        bool expanded)
    {
        if (status != ServiceStatus.Running || state is null)
        {
            return Hidden;
        }

        var meters = state.Meters;
        var reservations = Reservations.Of(state);
        var headroom = Math.Clamp(1 - (meters.HeadroomPct / 100.0), 0, 1);
        var rows = new List<MeterRow>();
        if (toggles.Cpu)
        {
            rows.Add(Cpu(meters.Cpu, reservations, headroom));
        }

        if (toggles.Ram)
        {
            rows.Add(Ram(meters.Ram, reservations, headroom));
        }

        var several = meters.Gpus.Count > 1;
        for (var i = 0; i < meters.Gpus.Count; i++)
        {
            var gpu = meters.Gpus[i];
            var suffix = several ? $" {i}" : "";
            var prefix = several ? $"{gpu.Name} · " : "";
            if (toggles.Gpu)
            {
                rows.Add(Gpu(gpu, $"gpu:{i}", $"GPU{suffix}", prefix, reservations, headroom));
            }

            if (toggles.Vram)
            {
                rows.Add(Vram(gpu, $"vram:{i}", $"VRAM{suffix}", prefix, reservations, headroom));
            }
        }

        var drives = toggles.Drives
            ? meters.Drives.Select(drive => Drive(drive, toggles.DriveActivity)).ToList()
            : [];
        var allOff =
            !(toggles.Cpu || toggles.Ram || toggles.Gpu || toggles.Vram || toggles.Drives);
        return new ResourcesView(
            Visible: true,
            Expanded: expanded,
            Summary: expanded ? "" : SummaryOf(meters, toggles),
            Meters: rows,
            Drives: drives,
            ShowSeparator: rows.Count > 0 && drives.Count > 0,
            AllOff: allOff,
            ShowLegend: rows.Any(row => row.Reserved is not null));
    }

    private static MeterRow Cpu(CpuMeter cpu, Reservations reservations, double headroom)
    {
        var tooltip = reservations.Benchmark ? "Whole machine reserved for the benchmark"
            : cpu.ReservedCores > 0
                ? $"{Number(cpu.ReservedCores)} of {cpu.LogicalCores} cores reserved"
            : reservations.None;
        var reserved = reservations.Benchmark ? 1
            : Fraction(cpu.ReservedCores, cpu.LogicalCores);
        var spark = cpu.HistoryPct.TakeLast(SparkSamples)
            .Select(pct => Math.Clamp(pct / 100.0, 0, 1))
            .ToList();
        return new MeterRow(
            "cpu",
            "CPU",
            Math.Clamp(cpu.UsagePct / 100.0, 0, 1),
            Tick(reserved),
            headroom,
            Percent(cpu.UsagePct),
            tooltip,
            spark);
    }

    private static MeterRow Ram(RamMeter ram, Reservations reservations, double headroom)
    {
        var tooltip = reservations.Benchmark ? Reservations.WholeMachine
            : ram.ReservedMb > 0 ? $"{Format.Size(ram.ReservedMb)} GB reserved"
            : reservations.None;
        var reserved = reservations.Benchmark ? 1 : Fraction(ram.ReservedMb, ram.TotalMb);
        return new MeterRow(
            "ram",
            "RAM",
            Fraction(ram.UsedMb, ram.TotalMb),
            Tick(reserved),
            headroom,
            $"{Format.Gigabytes(ram.UsedMb)} / {Format.Size(ram.TotalMb)} GB",
            tooltip,
            []);
    }

    private static MeterRow Gpu(
        GpuMeter gpu,
        string key,
        string label,
        string prefix,
        Reservations reservations,
        double headroom)
    {
        var tooltip = reservations.Benchmark ? Reservations.WholeMachine
            : gpu.ReservedPct > 0 ? $"{Percent(gpu.ReservedPct)} reserved"
            : reservations.None;
        var reserved = reservations.Benchmark ? 1 : Math.Clamp(gpu.ReservedPct / 100.0, 0, 1);
        return new MeterRow(
            key,
            label,
            Math.Clamp(gpu.BusiestPct / 100.0, 0, 1),
            Tick(reserved),
            headroom,
            Percent(gpu.BusiestPct),
            prefix + tooltip,
            []);
    }

    // Without a reported total there is nothing to measure the bar against: it stays empty, and
    // the value reads the used memory alone.
    private static MeterRow Vram(
        GpuMeter gpu,
        string key,
        string label,
        string prefix,
        Reservations reservations,
        double headroom)
    {
        var tooltip = reservations.Benchmark ? Reservations.WholeMachine
            : gpu.ReservedVramMb > 0 ? $"{Format.Size(gpu.ReservedVramMb)} GB reserved"
            : reservations.None;
        if (gpu.VramTotalMb is not { } total)
        {
            return new MeterRow(
                key,
                label,
                0,
                null,
                headroom,
                $"{Format.Gigabytes(gpu.VramUsedMb)} GB",
                $"{prefix}{tooltip} · total not reported",
                []);
        }

        var reserved = reservations.Benchmark ? 1 : Fraction(gpu.ReservedVramMb, total);
        return new MeterRow(
            key,
            label,
            Fraction(gpu.VramUsedMb, total),
            Tick(reserved),
            headroom,
            $"{Format.Gigabytes(gpu.VramUsedMb)} / {Format.Size(total)} GB",
            prefix + tooltip,
            []);
    }

    private static DriveRow Drive(DriveUsage drive, bool activity) =>
        new(
            drive.Name,
            Fraction(drive.UsedMb, drive.TotalMb),
            activity ? Activity(drive) : "",
            Space(drive.UsedMb, drive.TotalMb));

    /// <summary>
    /// <c>612 / 953 GB</c> in whole gigabytes, or from 1000 GB <c>1.21 / 1.82 TB</c>.
    /// </summary>
    private static string Space(ulong usedMb, ulong totalMb)
    {
        var totalGb = totalMb / 1024.0;
        if (Math.Round(totalGb) < 1000)
        {
            return string.Create(
                CultureInfo.InvariantCulture,
                $"{usedMb / 1024.0:0} / {totalGb:0} GB");
        }

        return string.Create(
            CultureInfo.InvariantCulture,
            $"{usedMb / MebibytesPerTebibyte:0.##} / {totalMb / MebibytesPerTebibyte:0.##} TB");
    }

    /// <summary>
    /// <c>R 42 · W 8 MB/s</c>, one side alone when the other is idle, or <c>Idle</c>.
    /// </summary>
    private static string Activity(DriveUsage drive)
    {
        var parts = new List<string>(2);
        if (Rate(drive.ReadBytesPerSec) is { } read)
        {
            parts.Add($"R {read}");
        }

        if (Rate(drive.WriteBytesPerSec) is { } write)
        {
            parts.Add($"W {write}");
        }

        return parts.Count == 0 ? "Idle" : $"{string.Join(" · ", parts)} MB/s";
    }

    // Megabytes per second with one decimal below 10, whole above; null under 0.1 MB/s.
    private static string? Rate(ulong bytesPerSec)
    {
        var megabytes = bytesPerSec / BytesPerMebibyte;
        var format = megabytes < WholeRateMb ? "0.#" : "0";
        return megabytes < IdleRateMb
            ? null
            : megabytes.ToString(format, CultureInfo.InvariantCulture);
    }

    // The first adapter stands for the GPU in the summary, as on the handoff's board.
    private static string SummaryOf(Meters meters, MeterToggles toggles)
    {
        var parts = new List<string>(3);
        if (toggles.Cpu)
        {
            parts.Add($"CPU {Percent(meters.Cpu.UsagePct)}");
        }

        if (toggles.Ram)
        {
            var ram = meters.Ram;
            parts.Add($"RAM {Format.Gigabytes(ram.UsedMb)} / {Format.Size(ram.TotalMb)} GB");
        }

        if (toggles.Gpu && meters.Gpus.Count > 0)
        {
            parts.Add($"GPU {Percent(meters.Gpus[0].BusiestPct)}");
        }

        return string.Join(" · ", parts);
    }

    private static double? Tick(double reserved) => reserved > 0 ? reserved : null;

    private static double Fraction(double part, double whole) =>
        whole > 0 ? Math.Clamp(part / whole, 0, 1) : 0;

    private static string Percent(float pct) =>
        string.Create(CultureInfo.InvariantCulture, $"{pct:0}%");

    private static string Number(float value) =>
        value.ToString("0.#", CultureInfo.InvariantCulture);

    /// <summary>
    /// What the leases reserve as a whole: nothing at all reads "No reservations" on every bar,
    /// a bar of its own reads "Not reserved", and a benchmark running alone has the whole
    /// machine.
    /// </summary>
    private sealed record Reservations(bool Benchmark, bool Any)
    {
        public const string WholeMachine = "Whole machine reserved";

        public string None => Any ? "Not reserved" : "No reservations";

        public static Reservations Of(UiState state)
        {
            var meters = state.Meters;
            var benchmark = state.Active
                .Any(job => job.Lease.Kind == LeaseKind.BenchmarkExclusive);
            var any = meters.Cpu.ReservedCores > 0
                || meters.Ram.ReservedMb > 0
                || meters.Gpus.Any(gpu => gpu.ReservedPct > 0 || gpu.ReservedVramMb > 0);
            return new Reservations(benchmark, any);
        }
    }
}
