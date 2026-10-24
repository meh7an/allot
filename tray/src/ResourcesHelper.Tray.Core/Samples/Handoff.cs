using ResourcesHelper.Tray.Core.Contract;
using ResourcesHelper.Tray.Core.Daemon;
using ResourcesHelper.Tray.Core.ViewModels;

namespace ResourcesHelper.Tray.Core.Samples;

/// <summary>
/// The sample jobs of the handoff prototype (<c>JOBS</c> in <c>Flyout.dc.html</c>) as daemon
/// state, so that each <c>SCEN</c> scenario can be rebuilt from what rhd would send. The tests
/// check the view models against it, and a Debug build of the tray app shows it with
/// <c>--scenario</c> for comparing against the handoff's state board.
/// </summary>
public static class Handoff
{
    public const long NowMs = 1_792_824_720_000;

    public const string IdleSession = "e09c7b3a-1111-4000-8000-000000000000";

    private static readonly Dictionary<string, Job> Jobs = new(StringComparer.Ordinal)
    {
        ["cargo"] = new(1, "3f2a9c1e", @"E:\code\resources-helper-mcp", 7.8f, 2970,
            "cargo build --release", JobClass.CpuHeavy, "cargo.exe", 18244,
            Reserve(8, 4096), Elapsed: 186, Estimate: 300),
        ["py"] = new(2, "b81d44f0", @"E:\code\ml-trainer", 1.9f, 6554,
            "python train.py --epochs 20 --data ./shards", JobClass.GpuHeavy, "python.exe", 22108,
            Reserve(2, 4096, 80, 4096), Elapsed: 700, Estimate: 600),
        ["vite"] = new(3, "5d2e8b61", @"E:\code\design-tokens-and-a-very-long-folder-name", 3.2f,
            1843, "pnpm --filter web vite build --mode production", JobClass.Mixed, "node.exe",
            9120, Reserve(4, 2048, 10, 1024), Elapsed: 52, Estimate: 90),
        ["go"] = new(4, "9a7c03de", @"E:\code\ingest-svc", 0.9f, 614,
            "go vet ./...", JobClass.Light, "go.exe", 14772,
            Reserve(1, 1024), Elapsed: 20, Estimate: 60),
        ["npm"] = new(5, "77a0e5d2", @"E:\code\customer-portal-monorepo-experimental", 0.4f, 1126,
            "npm run test:e2e", JobClass.CpuHeavy, null, 0,
            Reserve(2, 4096), Waited: 42, WaitLeft: 78),
        ["npmB"] = new(6, "77a0e5d2", @"E:\code\customer-portal-monorepo-experimental", 0.4f,
            1126, "npm run test:e2e", JobClass.CpuHeavy, null, 0,
            Reserve(2, 4096), Waited: 72),
        ["npmN"] = new(7, "77a0e5d2", @"E:\code\customer-portal-monorepo-experimental", 0.4f,
            1126, "npm run test:e2e", JobClass.CpuHeavy, null, 0,
            Reserve(2, 4096), Waited: 196, WaitLeft: 17),
        ["ctest"] = new(8, "3f2a9c1e", @"E:\code\resources-helper-mcp", 7.8f, 2970,
            "cargo test --workspace", null, null, 0,
            Reserve(8, 4096), Waited: 18),
        ["bench"] = new(9, "c41f9a07", @"E:\code\parser-perf", 0.2f, 410,
            "cargo bench --bench parser", null, "cargo.exe", 30412,
            Reserve(16, 8192), Elapsed: 48, Estimate: 120, Waited: 30),
    };

    // Windows executables for the icons scenario; go's path does not exist, so it keeps its
    // initials.
    private static readonly Dictionary<string, string> Exes = new(StringComparer.Ordinal)
    {
        ["cargo"] = @"%SystemRoot%\System32\cmd.exe",
        ["py"] = @"%SystemRoot%\System32\notepad.exe",
        ["vite"] = @"%SystemRoot%\explorer.exe",
        ["go"] = @"C:\nowhere\go.exe",
        ["bench"] = @"%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe",
    };

    private static readonly string[] Several = ["cargo", "py"];

    /// <summary>
    /// The scenarios the tray app can show, in the state board's order.
    /// </summary>
    public static IReadOnlyList<string> ScenarioNames { get; } =
    [
        "empty", "one", "several", "kill-confirm", "killed", "bench-drain", "bench-quiet",
        "bench-run", "bench-done", "down", "max", "icons", "c-active", "c-over", "c-queued",
        "c-bench", "c-confirm", "c-killed", "c-removed", "c-refused",
    ];

    /// <summary>
    /// The scenario whose Kill the Debug build's fake daemon refuses, to show the inline error;
    /// not on the handoff's board.
    /// </summary>
    public const string RefusedScenario = "c-refused";

    /// <summary>
    /// The refusal of <see cref="RefusedScenario"/>, as <c>DaemonClient</c> words a 409.
    /// </summary>
    public const string RefusedReason = "the bound process 18244 (cargo.exe) is protected";

    public static long LeaseId(string job) => Jobs[job].Id;

    public static string SessionOf(string job) => FullSession(Jobs[job].Session);

    /// <summary>
    /// The service status and state of a scenario in <see cref="ScenarioNames"/>; the <c>c-</c>
    /// component scenarios are their jobs alone, without the idle session.
    /// </summary>
    public static (ServiceStatus Status, UiState State) Scenario(string name) => name switch
    {
        "empty" => (ServiceStatus.Running, State(recent: true)),
        "one" or "c-active" or "c-confirm" or "c-killed" or RefusedScenario =>
            (ServiceStatus.Running, State(run: ["cargo"])),
        "several" or "kill-confirm" or "killed" =>
            (ServiceStatus.Running, State(run: Several, wait: ["npm"], idle: true)),
        "c-removed" => (ServiceStatus.Running, State(wait: ["npm"])),
        "bench-drain" =>
            (ServiceStatus.Running, State(run: ["cargo"], wait: ["npmB"], bench: "drain")),
        "bench-quiet" => (ServiceStatus.Running, State(wait: ["npmB"], bench: "quiet")),
        "bench-run" => (ServiceStatus.Running, State(wait: ["npmB"], bench: "run")),
        "bench-done" => (ServiceStatus.Running, State(wait: ["npmN"])),
        "down" => (ServiceStatus.Stopped, State(run: ["cargo"])),
        "max" => (ServiceStatus.Running, MaxState(exes: false)),
        "icons" => (ServiceStatus.Running, MaxState(exes: true)),
        "c-over" => (ServiceStatus.Running, State(run: ["py"])),
        "c-queued" => (ServiceStatus.Running, State(wait: ["npm", "ctest"])),
        "c-bench" => (ServiceStatus.Running, State(bench: "quiet")),
        _ => throw new ArgumentException($"unknown scenario {name}", nameof(name)),
    };

    /// <summary>
    /// The Kill and Remove states a scenario starts in (the prototype's <c>init</c>): Kill?
    /// with 3 s left, or Stopped or Removed at the scenario's moment. Empty for the others.
    /// </summary>
    public static IReadOnlyDictionary<long, StopView> Stops(string name) => name switch
    {
        "kill-confirm" or "c-confirm" => new Dictionary<long, StopView>
        {
            [LeaseId("cargo")] = new(StopKind.Kill, StopPhase.Confirming, SecondsLeft: 3),
        },
        "killed" or "c-killed" => new Dictionary<long, StopView>
        {
            [LeaseId("cargo")] = new(StopKind.Kill, StopPhase.Ended, EndedAtMs: NowMs),
        },
        "c-removed" => new Dictionary<long, StopView>
        {
            [LeaseId("npm")] = new(StopKind.Remove, StopPhase.Ended, EndedAtMs: NowMs),
        },
        _ => new Dictionary<long, StopView>(),
    };

    /// <param name="run">Active jobs, in grant order.</param>
    /// <param name="wait">Queued jobs, in queue order after a queued benchmark.</param>
    /// <param name="bench"><c>drain</c>, <c>quiet</c> or <c>run</c>; <c>null</c> without
    /// one.</param>
    /// <param name="idle">Adds the idle session <c>e09c7b3a</c>.</param>
    /// <param name="recent">Adds the two Recently finished runs of the <c>empty</c>
    /// scenario.</param>
    /// <param name="exes">Gives the processes real Windows executables, for their icons.</param>
    public static UiState State(
        string[]? run = null,
        string[]? wait = null,
        string? bench = null,
        bool idle = false,
        bool recent = false,
        bool exes = false)
    {
        run ??= [];
        wait ??= [];
        var active = run.Select(name => Active(name, exes)).ToList();
        var queue = new List<QueuedJob>();
        var used = run.Concat(wait).ToList();
        if (bench is not null)
        {
            used.Add("bench");
            if (bench == "run")
            {
                active.Add(Active("bench", exes, LeaseKind.BenchmarkExclusive));
            }
            else
            {
                var quiet = bench == "quiet";
                BenchmarkProgress progress = quiet
                    ? new BenchmarkProgress.Measuring
                    {
                        CpuAvgPct = 3,
                        GpuAvgPct = 0,
                        MeasuredSecs = 6,
                        NeededSecs = 10,
                    }
                    : new BenchmarkProgress.Draining { ActiveLeases = (uint)run.Length };
                queue.Add(Queued(
                    Jobs["bench"],
                    1,
                    LeaseKind.BenchmarkExclusive,
                    progress,
                    quiet ? 4UL : null));
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

        var state = Build(sessions, active, queue);
        return recent ? state with { Recent = RecentRuns() } : state;
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

    private static UiState MaxState(bool exes) =>
        State(
            run: ["cargo", "py", "vite", "go"],
            wait: ["npm", "ctest"],
            idle: true,
            exes: exes);

    private static string FullSession(string shortId) => $"{shortId}-1111-4000-8000-000000000000";

    private static JobProfile Reserve(
        float cores,
        ulong ramMb,
        float gpuPct = 0,
        ulong vramMb = 0) =>
        new()
        {
            Cores = cores,
            EstSecs = 300,
            GpuPct = gpuPct,
            RamMb = ramMb,
            VramMb = vramMb,
        };

    private static ActiveJob Active(string name, bool exes, LeaseKind kind = LeaseKind.Normal)
    {
        var job = Jobs[name];
        var exe = exes && Exes.TryGetValue(name, out var path)
            ? Environment.ExpandEnvironmentVariables(path)
            : null;
        return new ActiveJob
        {
            Lease = Lease(job, LeaseState.Active, kind),
            Process = job.Process is null
                ? null
                : new JobProcess { Bound = true, Exe = exe, Name = job.Process, Pid = job.Pid },
        };
    }

    private static QueuedJob Queued(
        Job job,
        uint position,
        LeaseKind kind = LeaseKind.Normal,
        BenchmarkProgress? progress = null,
        ulong? waitLeft = null) =>
        new()
        {
            Lease = Lease(job, LeaseState.Queued, kind),
            Position = position,
            Verdict = new Verdict
            {
                Benchmark = progress,
                Fits = false,
                Reasons = ["not enough free cores"],
                WaitEstimateSecs = waitLeft ?? job.WaitLeft,
            },
        };

    private static Lease Lease(Job job, LeaseState state, LeaseKind kind)
    {
        var active = state == LeaseState.Active;
        var grantedAtMs = NowMs - (job.Elapsed * 1000);
        return new Lease
        {
            Class = job.Class,
            Cwd = job.Cwd,
            ExpiresAtMs = NowMs + 30_000,
            GrantedAtMs = active ? grantedAtMs : null,
            Holder = new Holder
            {
                Pid = 1000 + (uint)job.Id,
                PidStartTime = 1_792_824_000,
                Session = FullSession(job.Session),
            },
            Id = job.Id,
            Kind = kind,
            Label = job.Command,
            Profile = job.Profile with { EstSecs = (ulong)job.Estimate },
            RequestedAtMs = active ? grantedAtMs - 5_000 : NowMs - (job.Waited * 1000),
            State = state,
        };
    }

    private static List<RecentRun> RecentRuns() =>
    [
        Run("cargo", endedAgoSecs: 240, ranSecs: 292),
        Run("npm", endedAgoSecs: 660, ranSecs: 130),
    ];

    private static RecentRun Run(string name, long endedAgoSecs, long ranSecs)
    {
        var job = Jobs[name];
        var endedAtMs = NowMs - (endedAgoSecs * 1000);
        return new RecentRun
        {
            Command = job.Command,
            Cwd = job.Cwd,
            Declared = job.Profile,
            EndCause = EndCause.Released,
            EndedAtMs = endedAtMs,
            LeaseId = job.Id + 100,
            Session = FullSession(job.Session),
            StartedAtMs = endedAtMs - (ranSecs * 1000),
        };
    }

    // Elapsed and Estimate are an active job's seconds since its grant and its estimate; Waited
    // and WaitLeft a queued job's seconds since its request and rhd's remaining wait.
    private sealed record Job(
        long Id,
        string Session,
        string Cwd,
        float Cores,
        ulong RamMb,
        string Command,
        JobClass? Class,
        string? Process,
        uint Pid,
        JobProfile Profile,
        long Elapsed = 0,
        long Estimate = 300,
        long Waited = 0,
        ulong? WaitLeft = null);
}
