using ResourcesHelper.Tray.Core.Contract;

namespace ResourcesHelper.Tray.Core.Tests.Support;

/// <summary>
/// The sample jobs of the handoff prototype (<c>JOBS</c> in <c>Flyout.dc.html</c>) as daemon
/// state, so that each <c>SCEN</c> scenario can be rebuilt from what rhd would send.
/// </summary>
internal static class Handoff
{
    public const long NowMs = 1_792_824_720_000;

    public const string IdleSession = "e09c7b3a-1111-4000-8000-000000000000";

    private static readonly Dictionary<string, Job> Jobs = new(StringComparer.Ordinal)
    {
        ["cargo"] = new(1, "3f2a9c1e", @"E:\code\resources-helper-mcp", 7.8f, 2970,
            "cargo build --release", JobClass.CpuHeavy, "cargo.exe", 18244),
        ["py"] = new(2, "b81d44f0", @"E:\code\ml-trainer", 1.9f, 6554,
            "python train.py --epochs 20 --data ./shards", JobClass.GpuHeavy, "python.exe", 22108),
        ["vite"] = new(3, "5d2e8b61", @"E:\code\design-tokens-and-a-very-long-folder-name", 3.2f,
            1843, "pnpm --filter web vite build --mode production", JobClass.Mixed, "node.exe",
            9120),
        ["go"] = new(4, "9a7c03de", @"E:\code\ingest-svc", 0.9f, 614,
            "go vet ./...", JobClass.Light, "go.exe", 14772),
        ["npm"] = new(5, "77a0e5d2", @"E:\code\customer-portal-monorepo-experimental", 0.4f, 1126,
            "npm run test:e2e", JobClass.CpuHeavy, null, 0),
        ["npmB"] = new(6, "77a0e5d2", @"E:\code\customer-portal-monorepo-experimental", 0.4f,
            1126, "npm run test:e2e", JobClass.CpuHeavy, null, 0),
        ["npmN"] = new(7, "77a0e5d2", @"E:\code\customer-portal-monorepo-experimental", 0.4f,
            1126, "npm run test:e2e", JobClass.CpuHeavy, null, 0),
        ["ctest"] = new(8, "3f2a9c1e", @"E:\code\resources-helper-mcp", 7.8f, 2970,
            "cargo test --workspace", null, null, 0),
        ["bench"] = new(9, "c41f9a07", @"E:\code\parser-perf", 0.2f, 410,
            "cargo bench --bench parser", null, "cargo.exe", 30412),
    };

    public static long LeaseId(string job) => Jobs[job].Id;

    public static string SessionOf(string job) => FullSession(Jobs[job].Session);

    /// <param name="run">Active jobs, in grant order.</param>
    /// <param name="wait">Queued jobs, in queue order after a queued benchmark.</param>
    /// <param name="bench"><c>drain</c>, <c>quiet</c> or <c>run</c>; <c>null</c> without
    /// one.</param>
    /// <param name="idle">Adds the idle session <c>e09c7b3a</c>.</param>
    public static UiState State(
        string[]? run = null,
        string[]? wait = null,
        string? bench = null,
        bool idle = false)
    {
        run ??= [];
        wait ??= [];
        var active = run.Select(name => Active(Jobs[name])).ToList();
        var queue = new List<QueuedJob>();
        var used = run.Concat(wait).ToList();
        if (bench is not null)
        {
            used.Add("bench");
            var job = Jobs["bench"];
            if (bench == "run")
            {
                active.Add(Active(job, LeaseKind.BenchmarkExclusive));
            }
            else
            {
                BenchmarkProgress progress = bench == "quiet"
                    ? new BenchmarkProgress.Measuring
                    {
                        CpuAvgPct = 3,
                        GpuAvgPct = 0,
                        MeasuredSecs = 6,
                        NeededSecs = 10,
                    }
                    : new BenchmarkProgress.Draining { ActiveLeases = (uint)run.Length };
                queue.Add(Queued(job, 1, LeaseKind.BenchmarkExclusive, progress));
            }
        }

        foreach (var name in wait)
        {
            queue.Add(Queued(Jobs[name], (uint)queue.Count + 1));
        }

        var sessions = used.Select(name => Jobs[name])
            .DistinctBy(job => job.Session)
            .Select(job => Session(FullSession(job.Session), job.Cwd, job.Cores, job.RamMb))
            .ToList();
        if (idle)
        {
            sessions.Add(Session(IdleSession, @"E:\code\notes", 0.1f, 307));
        }

        return Build(sessions, active, queue);
    }

    public static UiState Build(
        IReadOnlyList<UiSession> sessions,
        IReadOnlyList<ActiveJob> active,
        IReadOnlyList<QueuedJob> queue) =>
        new()
        {
            Active = active,
            GeneratedAtMs = NowMs,
            Meters = new Meters
            {
                Cpu = new CpuMeter
                {
                    HistoryPct = [],
                    LogicalCores = 16,
                    ReservedCores = 0,
                    UsagePct = 6,
                },
                Drives = [],
                Gpus = [],
                HeadroomPct = 15,
                Ram = new RamMeter { ReservedMb = 0, TotalMb = 32768, UsedMb = 14438 },
            },
            Notices = [],
            Queue = queue,
            Recent = [],
            Service = new HealthResponse { Pid = 4242, UptimeSecs = 600, Version = "0.2.0" },
            Sessions = sessions,
        };

    public static UiSession Session(string session, string? cwd, float cores, ulong ramMb) =>
        new()
        {
            Session = session,
            Cwd = cwd,
            Usage = new ResourceUsage { Cores = cores, GpuPct = 0, RamMb = ramMb, VramMb = 0 },
        };

    private static string FullSession(string shortId) => $"{shortId}-1111-4000-8000-000000000000";

    private static ActiveJob Active(Job job, LeaseKind kind = LeaseKind.Normal) =>
        new()
        {
            Lease = Lease(job, LeaseState.Active, kind),
            Process = job.Process is null
                ? null
                : new JobProcess { Bound = true, Name = job.Process, Pid = job.Pid },
        };

    private static QueuedJob Queued(
        Job job,
        uint position,
        LeaseKind kind = LeaseKind.Normal,
        BenchmarkProgress? progress = null) =>
        new()
        {
            Lease = Lease(job, LeaseState.Queued, kind),
            Position = position,
            Verdict = new Verdict
            {
                Benchmark = progress,
                Fits = false,
                Reasons = ["not enough free cores"],
                WaitEstimateSecs = null,
            },
        };

    private static Lease Lease(Job job, LeaseState state, LeaseKind kind) =>
        new()
        {
            Class = job.Class,
            Cwd = job.Cwd,
            ExpiresAtMs = NowMs + 30_000,
            GrantedAtMs = state == LeaseState.Active ? NowMs - 186_000 : null,
            Holder = new Holder
            {
                Pid = 1000 + (uint)job.Id,
                PidStartTime = 1_792_824_000,
                Session = FullSession(job.Session),
            },
            Id = job.Id,
            Kind = kind,
            Label = job.Command,
            Profile = new JobProfile
            {
                Cores = 8,
                EstSecs = 300,
                GpuPct = 0,
                RamMb = 4096,
                VramMb = 0,
            },
            RequestedAtMs = NowMs - 200_000,
            State = state,
        };

    private sealed record Job(
        long Id,
        string Session,
        string Cwd,
        float Cores,
        ulong RamMb,
        string Command,
        JobClass? Class,
        string? Process,
        uint Pid);
}
