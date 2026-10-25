using System.Diagnostics;
using System.Globalization;

using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml.Media;

using ResourcesHelper.Tray.Core.Diagnostics;
using ResourcesHelper.Tray.Core.ViewModels;

namespace ResourcesHelper.Tray.Diagnostics;

/// <summary>
/// Performance probes, off unless <c>RH_TRAY_MEASURE</c> is set: startup marks, open latency, the
/// frame intervals of each animation, and a usage line every minute, all written to tray.log
/// after <c>measure:</c>. Frames are the UI thread's <see cref="CompositionTarget.Rendering"/>
/// callbacks, which run only while a probe needs them; DWM's presents are not seen. With the
/// value <c>cycle</c> the app also runs <see cref="MeasureCycle"/>. UI thread only.
/// </summary>
internal static class Measure
{
    private const string Variable = "RH_TRAY_MEASURE";
    private static readonly TimeSpan Interval = TimeSpan.FromSeconds(60);
    private const double Mebibyte = 1024 * 1024;
    // Frames still counted after an animation's nominal end, so a late last frame shows.
    private static readonly TimeSpan Slack = TimeSpan.FromMilliseconds(50);
    // A click this long before the flyout starts to show is the one that opened it.
    private static readonly TimeSpan ClickWindow = TimeSpan.FromSeconds(1);
    private static readonly List<Probe> Probes = [];
    private static readonly UpdateTally Updates = new();

    private static Process? s_process;
    private static DispatcherQueueTimer? s_timer;
    private static ProcessReading s_last;
    private static long s_lastChanges;
    private static long s_rowChanges;
    private static int s_resizes;
    private static int s_lastResizes;
    private static long s_clickAt;
    private static long s_showAt;
    private static bool s_rendering;
    private static bool s_open;
    private static bool s_wasOpen;
    private static bool s_wasClosed = true;

    /// <summary>
    /// The variable's value, or <c>null</c> when it is unset, empty or <c>0</c>.
    /// </summary>
    public static string? Mode { get; } =
        Environment.GetEnvironmentVariable(Variable) is { Length: > 0 } value && value != "0"
            ? value
            : null;

    public static bool Enabled => Mode is not null;

    public static bool Cycle => Mode == "cycle";

    public static bool Stress => Mode == "stress";

    public static bool StartupOnly => Mode == "startup";

    public static bool Soak => Mode == "soak";

    public static void Start(DispatcherQueue dispatcher, FlyoutModel model)
    {
        if (!Enabled)
        {
            return;
        }

        s_process = Process.GetCurrentProcess();
        s_last = Read();
        s_lastChanges = Changes;
        model.Rows.CollectionChanged += (_, _) => s_rowChanges++;
        Write($"on ({Variable}={Mode})");
        if (Cycle || Stress || StartupOnly || Soak)
        {
            return;
        }

        s_timer = dispatcher.CreateTimer();
        s_timer.Interval = Interval;
        s_timer.Tick += (_, _) => Lap();
        s_timer.Start();
    }

    /// <summary>
    /// Logs the time since the process started, for example when the tray icon appears.
    /// </summary>
    public static void Startup(string what)
    {
        if (!Enabled)
        {
            return;
        }

        using var process = Process.GetCurrentProcess();
        var since = DateTime.Now - process.StartTime;
        Write(string.Create(
            CultureInfo.InvariantCulture,
            $"startup: {what} at {since.TotalMilliseconds:0} ms"));
    }

    /// <summary>
    /// A click on the tray icon arrived.
    /// </summary>
    public static void Clicked()
    {
        if (Enabled)
        {
            s_clickAt = Stopwatch.GetTimestamp();
        }
    }

    /// <summary>
    /// The flyout starts to show; the next frame gives the open latency.
    /// </summary>
    public static void Showing()
    {
        if (!Enabled)
        {
            return;
        }

        s_showAt = Stopwatch.GetTimestamp();
        Render(true);
    }

    public static void FlyoutOpen(bool open)
    {
        s_open = open;
        s_wasOpen |= open;
        s_wasClosed |= !open;
    }

    /// <summary>
    /// An animation starts; its frames are counted until it ends. Another start of the same name
    /// while it runs extends it, so a dozen bars filling at once count as one.
    /// </summary>
    public static void Animation(string name, TimeSpan duration)
    {
        if (!Enabled)
        {
            return;
        }

        var until = Stopwatch.GetTimestamp()
            + (long)((duration + Slack).TotalSeconds * Stopwatch.Frequency);
        if (Probes.Find(probe => probe.Name == name) is { } running)
        {
            running.Until = Math.Max(running.Until, until);
            return;
        }

        Probes.Add(new Probe(name, until, s_resizes));
        Render(true);
    }

    /// <summary>
    /// The window changed size, which re-lays out the whole content.
    /// </summary>
    public static void Resized() => s_resizes++;

    /// <summary>
    /// Times one snapshot applied to the flyout model.
    /// </summary>
    /// <returns>The value to pass to <see cref="Updated"/>.</returns>
    public static (long Timestamp, long Allocated) Updating() =>
        Enabled
            ? (Stopwatch.GetTimestamp(), GC.GetAllocatedBytesForCurrentThread())
            : default;

    public static void Updated((long Timestamp, long Allocated) start)
    {
        if (Enabled)
        {
            Updates.Add(
                Stopwatch.GetElapsedTime(start.Timestamp),
                GC.GetAllocatedBytesForCurrentThread() - start.Allocated);
        }
    }

    public static void Write(string message) => AppLog.Write($"measure: {message}");

    private static long Changes => Observable.Raised + s_rowChanges;

    private static ProcessReading Read()
    {
        var process = s_process!;
        process.Refresh();
        return new ProcessReading(
            Stopwatch.GetElapsedTime(0),
            process.TotalProcessorTime,
            GC.GetTotalAllocatedBytes(),
            GC.CollectionCount(0),
            GC.CollectionCount(1),
            GC.CollectionCount(2),
            process.PrivateMemorySize64,
            process.WorkingSet64);
    }

    /// <summary>
    /// Writes the usage since the last line and starts a new one; the timer does it every minute,
    /// and <see cref="MeasureCycle"/> at the end of each of its phases instead.
    /// </summary>
    public static void Lap()
    {
        var now = Read();
        var flyout = s_wasOpen && s_wasClosed ? "mixed" : s_wasOpen ? "open" : "closed";
        var changes = Changes;
        Write(Usage.Line(
            flyout, s_last, now, Updates, changes - s_lastChanges, s_resizes - s_lastResizes));

        // A leak is private memory that keeps growing after a full collection; without one, the
        // rare collections of the AOT build make the plain number climb and drop.
        GC.Collect();
        GC.WaitForPendingFinalizers();
        GC.Collect();
        now = Read();
        Write(string.Create(
            CultureInfo.InvariantCulture,
            $"after a full collection: private {now.PrivateBytes / Mebibyte:0.0} MB, "
                + $"managed {GC.GetTotalMemory(false) / Mebibyte:0.0} MB"));
        s_last = now;
        s_lastChanges = changes;
        s_lastResizes = s_resizes;
        Updates.Clear();
        s_wasOpen = s_open;
        s_wasClosed = !s_open;
    }

    private static void Render(bool on)
    {
        if (on == s_rendering)
        {
            return;
        }

        s_rendering = on;
        if (on)
        {
            CompositionTarget.Rendering += OnFrame;
        }
        else
        {
            CompositionTarget.Rendering -= OnFrame;
        }
    }

    private static void OnFrame(object? sender, object args)
    {
        var now = Stopwatch.GetTimestamp();
        if (s_showAt != 0)
        {
            var show = Milliseconds(s_showAt, now);
            var click = s_clickAt != 0
                && Stopwatch.GetElapsedTime(s_clickAt, s_showAt) < ClickWindow
                    ? string.Create(
                        CultureInfo.InvariantCulture,
                        $", {Milliseconds(s_clickAt, now):0.0} ms from the click")
                    : ", no click";
            Write(string.Create(
                CultureInfo.InvariantCulture,
                $"open: first frame {show:0.0} ms after show{click}"));
            s_showAt = 0;
            s_clickAt = 0;
        }

        for (var i = Probes.Count - 1; i >= 0; i--)
        {
            var probe = Probes[i];
            probe.Frame(now);
            if (now >= probe.Until)
            {
                Probes.RemoveAt(i);
                var stats = FrameStats.From(probe.Intervals);
                Write(stats.Line(probe.Name, s_resizes - probe.Resizes));
            }
        }

        if (Probes.Count == 0)
        {
            Render(false);
        }
    }

    private static double Milliseconds(long from, long to) =>
        Stopwatch.GetElapsedTime(from, to).TotalMilliseconds;

    private sealed class Probe(string name, long until, int resizes)
    {
        private long _last;

        public string Name { get; } = name;

        public long Until { get; set; } = until;

        public int Resizes { get; } = resizes;

        public List<double> Intervals { get; } = [];

        // The first frame only starts the clock: the gap before the animation is not its own.
        public void Frame(long now)
        {
            if (_last != 0)
            {
                Intervals.Add(Milliseconds(_last, now));
            }

            _last = now;
        }
    }
}
