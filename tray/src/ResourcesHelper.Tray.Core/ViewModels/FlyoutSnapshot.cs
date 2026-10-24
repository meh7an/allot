using ResourcesHelper.Tray.Core.Contract;
using ResourcesHelper.Tray.Core.Daemon;

namespace ResourcesHelper.Tray.Core.ViewModels;

public enum HeaderGlyph
{
    /// <summary>
    /// Hollow meter mark.
    /// </summary>
    Idle,

    /// <summary>
    /// Filled bars: a job runs, or a benchmark runs alone.
    /// </summary>
    Busy,

    /// <summary>
    /// Red error icon: the service is not serving state.
    /// </summary>
    Error,
}

public enum JobGroupKind
{
    Benchmark,
    Running,
    Waiting,
}

/// <summary>
/// Where the live benchmark stands, as the summary words it.
/// </summary>
public enum BenchmarkPhase
{
    /// <summary>
    /// Queued while other leases finish, or queued behind other requests.
    /// </summary>
    Draining,

    /// <summary>
    /// The machine has drained and its background load is being measured.
    /// </summary>
    WaitingForQuiet,

    /// <summary>
    /// Granted; it has the machine to itself.
    /// </summary>
    RunningAlone,
}

public sealed record HeaderViewModel(HeaderGlyph Glyph, string Summary);

/// <summary>
/// The strip at the top of a card that names the job's session.
/// </summary>
/// <param name="ShortId">The session ID's first 8 characters.</param>
/// <param name="Project">The project folder name, or empty.</param>
/// <param name="ProjectPath">The full working directory, for the project's tooltip.</param>
/// <param name="Usage">Live session usage, or empty when the session is not registered.</param>
public sealed record SessionStrip(string ShortId, string Project, string ProjectPath, string Usage);

/// <summary>
/// One card. Exactly one of <paramref name="Active"/> and <paramref name="Queued"/> is set.
/// </summary>
/// <param name="Lease">The job's lease.</param>
/// <param name="Session">The session strip.</param>
/// <param name="Active">The running job, with the process shown for it.</param>
/// <param name="Queued">The waiting request, with its position and verdict.</param>
/// <param name="Ending">Stopped or removed in this flyout and still showing its final state; it
/// no longer counts in the summary.</param>
public sealed record JobItem(
    Lease Lease,
    SessionStrip Session,
    ActiveJob? Active,
    QueuedJob? Queued,
    bool Ending);

public sealed record JobGroup(JobGroupKind Kind, string Heading, IReadOnlyList<JobItem> Jobs);

/// <summary>
/// The plain <c>Idle · N  ids  usage</c> line after the groups.
/// </summary>
/// <param name="Label">For example <c>Idle · 2</c>.</param>
/// <param name="Ids">Short session IDs separated by two spaces.</param>
/// <param name="Usage">Live usage, shown only when exactly one session is idle.</param>
public sealed record IdleLine(string Label, string Ids, string Usage);

/// <summary>
/// Everything the flyout header and job list show for one poll, built by a pure function so that
/// every string is testable. The window layer updates only what changed between snapshots.
/// </summary>
/// <param name="Header">The header glyph and summary.</param>
/// <param name="Groups">Benchmark, Running and Waiting, in that order, each only when it has
/// jobs.</param>
/// <param name="Idle">The idle line, or <c>null</c> when no session is idle.</param>
/// <param name="Benchmark">The live benchmark's phase, or <c>null</c> without one.</param>
/// <param name="IsLastKnown">The jobs come from the last state rhd served before it went away;
/// they show dimmed and without actions.</param>
public sealed record FlyoutSnapshot(
    HeaderViewModel Header,
    IReadOnlyList<JobGroup> Groups,
    IdleLine? Idle,
    BenchmarkPhase? Benchmark,
    bool IsLastKnown)
{
    private static readonly HashSet<long> NoLeases = [];

    /// <param name="status">The service status of the latest poll.</param>
    /// <param name="state">The latest state, or the last known one while rhd is not
    /// running.</param>
    /// <param name="ending">Leases whose cards show Stopped or Removed in this flyout.</param>
    public static FlyoutSnapshot From(
        ServiceStatus status,
        UiState? state,
        IReadOnlySet<long>? ending = null)
    {
        ending ??= NoLeases;
        var sessions = new Dictionary<string, UiSession>(StringComparer.Ordinal);
        foreach (var session in state?.Sessions ?? [])
        {
            _ = sessions.TryAdd(session.Session, session);
        }

        var benchmark = new List<JobItem>();
        var running = new List<JobItem>();
        var waiting = new List<JobItem>();
        foreach (var job in state?.Active ?? [])
        {
            var strip = Strip(job.Lease, sessions);
            var item = new JobItem(job.Lease, strip, job, null, Ending(job.Lease));
            (IsBenchmark(job.Lease) ? benchmark : running).Add(item);
        }

        foreach (var job in state?.Queue ?? [])
        {
            var strip = Strip(job.Lease, sessions);
            var item = new JobItem(job.Lease, strip, null, job, Ending(job.Lease));
            (IsBenchmark(job.Lease) ? benchmark : waiting).Add(item);
        }

        var groups = new List<JobGroup>();
        AddGroup(groups, JobGroupKind.Benchmark, "Benchmark", benchmark);
        AddGroup(groups, JobGroupKind.Running, Counted("Running", running), running);
        AddGroup(groups, JobGroupKind.Waiting, Counted("Waiting", waiting), waiting);

        var phase = PhaseOf(benchmark);
        var header = HeaderOf(status, Alive(running), Alive(waiting), phase);
        var idle = IdleOf(state, sessions);
        return new FlyoutSnapshot(header, groups, idle, phase, status != ServiceStatus.Running);

        bool Ending(Lease lease) => ending.Contains(lease.Id);
    }

    private static HeaderViewModel HeaderOf(
        ServiceStatus status,
        int running,
        int waiting,
        BenchmarkPhase? phase)
    {
        switch (status)
        {
            case ServiceStatus.Starting:
                return new HeaderViewModel(HeaderGlyph.Error, "Starting service…");
            case ServiceStatus.Stopped:
                return new HeaderViewModel(HeaderGlyph.Error, "Service stopped");
            case ServiceStatus.Incompatible:
                return new HeaderViewModel(HeaderGlyph.Error, "Service needs an update");
            default:
                break;
        }

        var glyph = running > 0 || phase == BenchmarkPhase.RunningAlone
            ? HeaderGlyph.Busy
            : HeaderGlyph.Idle;
        var summary = phase switch
        {
            BenchmarkPhase.Draining when running > 0 => $"{running} running · benchmark waiting",
            BenchmarkPhase.Draining => "Benchmark waiting",
            BenchmarkPhase.WaitingForQuiet => "Benchmark waiting for quiet",
            BenchmarkPhase.RunningAlone => "Benchmark running alone",
            _ => Counts(running, waiting),
        };
        return new HeaderViewModel(glyph, summary);
    }

    private static string Counts(int running, int waiting)
    {
        var parts = new List<string>(2);
        if (running > 0)
        {
            parts.Add($"{running} running");
        }

        if (waiting > 0)
        {
            parts.Add($"{waiting} waiting");
        }

        return parts.Count == 0 ? "Idle" : string.Join(" · ", parts);
    }

    // An active benchmark runs alone; a queued one either drains the machine or, once it is at
    // the head and drained, measures the background load.
    private static BenchmarkPhase? PhaseOf(List<JobItem> benchmark)
    {
        var alive = benchmark.Where(item => !item.Ending).ToList();
        if (alive.Count == 0)
        {
            return null;
        }

        if (alive.Any(item => item.Active is not null))
        {
            return BenchmarkPhase.RunningAlone;
        }

        return alive[0].Queued?.Verdict.Benchmark switch
        {
            BenchmarkProgress.Measuring or BenchmarkProgress.Noisy =>
                BenchmarkPhase.WaitingForQuiet,
            _ => BenchmarkPhase.Draining,
        };
    }

    private static IdleLine? IdleOf(UiState? state, Dictionary<string, UiSession> sessions)
    {
        if (state is null)
        {
            return null;
        }

        var busy = state.Active.Select(job => job.Lease.Holder.Session)
            .Concat(state.Queue.Select(job => job.Lease.Holder.Session))
            .ToHashSet(StringComparer.Ordinal);
        var idle = sessions.Values.Where(session => !busy.Contains(session.Session)).ToList();
        if (idle.Count == 0)
        {
            return null;
        }

        var ids = string.Join("  ", idle.Select(session => Format.ShortId(session.Session)));
        var usage = idle.Count == 1 ? Format.Usage(idle[0].Usage) : "";
        return new IdleLine($"Idle · {idle.Count}", ids, usage);
    }

    private static void AddGroup(
        List<JobGroup> groups,
        JobGroupKind kind,
        string heading,
        List<JobItem> jobs)
    {
        if (jobs.Count > 0)
        {
            groups.Add(new JobGroup(kind, heading, jobs));
        }
    }

    // While every card in a group shows Stopped or Removed, the heading keeps their number
    // until they collapse, as the handoff prototype does.
    private static string Counted(string label, List<JobItem> jobs)
    {
        var alive = Alive(jobs);
        return $"{label} · {(alive > 0 ? alive : jobs.Count)}";
    }

    private static int Alive(List<JobItem> jobs) => jobs.Count(item => !item.Ending);

    private static bool IsBenchmark(Lease lease) => lease.Kind == LeaseKind.BenchmarkExclusive;

    private static SessionStrip Strip(Lease lease, Dictionary<string, UiSession> sessions)
    {
        var key = lease.Holder.Session;
        var session = sessions.GetValueOrDefault(key);
        var cwd = string.IsNullOrEmpty(session?.Cwd) ? lease.Cwd : session.Cwd;
        var usage = session is null ? "" : Format.Usage(session.Usage);
        return new SessionStrip(Format.ShortId(key), Format.Project(cwd), cwd, usage);
    }
}
