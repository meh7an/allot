using System.Globalization;

using ResourcesHelper.Tray.Core.Contract;
using ResourcesHelper.Tray.Core.ViewModels;

namespace ResourcesHelper.Tray.Core.Notifications;

/// <summary>
/// The wording of each toast (handoff section 9).
/// </summary>
public static class ToastText
{
    /// <summary>
    /// The one Service stopped toast, withdrawn once the service is back.
    /// </summary>
    public const string ServiceTag = "service";

    // Row keys of FlyoutRows: JobCardRow and RecentRow.
    private const string RecentTarget = "recent";

    private static readonly ToastButton[] ShowOnly = [ToastButton.Show];

    public static ToastMessage ServiceStopped(int sessions) =>
        new(
            ToastKind.ServiceStopped,
            ServiceTag,
            "Service stopped",
            $"Heavy commands now run unchecked. {Connected(sessions)}",
            [ToastButton.StartService, ToastButton.OpenLogs],
            null);

    /// <param name="job">The job past 1.5 times its estimate.</param>
    /// <param name="session">Its session, whose folder stands in when the lease has none.</param>
    /// <param name="nowMs">The state's time.</param>
    public static ToastMessage RunningLong(ActiveJob job, UiSession? session, long nowMs)
    {
        var lease = job.Lease;
        var elapsed = Math.Max(0, (nowMs - (lease.GrantedAtMs ?? lease.RequestedAtMs)) / 1000);
        var times = $"{Format.Duration(elapsed)} so far against a "
            + $"~{Format.Duration((long)lease.Profile.EstSecs)} estimate";
        return new ToastMessage(
            ToastKind.RunningLong,
            $"long-{lease.Id}",
            $"{lease.Label} is running long",
            Joined(times, lease.Holder.Session, lease.Cwd, session),
            ShowOnly,
            JobTarget(lease.Id));
    }

    /// <param name="run">The finished benchmark run.</param>
    /// <param name="noise">Its background load.</param>
    public static ToastMessage BenchmarkFinished(RecentRun run, NoiseReport noise)
    {
        var forced = noise.QuietAtStart ? "" : "not quiet at start · ";
        return new ToastMessage(
            ToastKind.BenchmarkFinished,
            $"bench-{run.LeaseId}",
            $"Benchmark finished in {Format.Duration(RanSecs(run))}",
            string.Create(
                CultureInfo.InvariantCulture,
                $"Background noise avg {noise.AvgCpuPct:0.0}%, peak {noise.PeakCpuPct:0.0}% CPU"
                    + $" · {forced}{run.Command}"),
            [],
            run.LeaseId is { } id ? JobTarget(id) : null);
    }

    /// <param name="run">The finished run.</param>
    /// <param name="session">Its session, whose folder stands in when the run has none.</param>
    /// <param name="tag">The tag of this merge window.</param>
    public static ToastMessage JobFinished(RecentRun run, UiSession? session, string tag) =>
        new(
            ToastKind.JobFinished,
            tag,
            $"{run.Command} finished",
            Joined(Format.Duration(RanSecs(run)), run.Session ?? "", run.Cwd, session),
            ShowOnly,
            RecentTarget);

    /// <summary>
    /// Several finishes within the merge window: <c>3 jobs finished</c> with the first two
    /// commands.
    /// </summary>
    /// <param name="runs">The window's runs, oldest first; at least two.</param>
    /// <param name="tag">The tag of this merge window.</param>
    public static ToastMessage JobsFinished(IReadOnlyList<RecentRun> runs, string tag)
    {
        var commands = runs.Count == 2
            ? $"{runs[0].Command} and {runs[1].Command}"
            : $"{runs[0].Command}, {runs[1].Command} and {runs.Count - 2} more";
        return new ToastMessage(
            ToastKind.JobFinished,
            tag,
            $"{runs.Count} jobs finished",
            commands,
            ShowOnly,
            RecentTarget);
    }

    public static ToastMessage KilledOutsideFlyout(Notice notice) =>
        new(
            ToastKind.KilledOutsideFlyout,
            $"killed-{notice.LeaseId}",
            $"{notice.Command} was stopped",
            "Killed from the command line · Claude was notified",
            [],
            null);

    private static string JobTarget(long lease) => $"job:{lease}";

    private static string Connected(int sessions) => sessions switch
    {
        0 => "No Claude sessions are connected.",
        1 => "1 Claude session is connected.",
        _ => $"{sessions} Claude sessions are connected.",
    };

    private static long RanSecs(RecentRun run) =>
        Math.Max(0, (run.EndedAtMs - run.StartedAtMs) / 1000);

    // <first> · <session> · <project>, leaving out what is unknown. The job's own folder comes
    // first, as the toast is about that job.
    private static string Joined(string first, string session, string cwd, UiSession? live)
    {
        var parts = new List<string>(3) { first };
        if (session.Length > 0)
        {
            parts.Add(Format.ShortId(session));
        }

        var project = Format.Project(cwd.Length > 0 ? cwd : live?.Cwd);
        if (project.Length > 0)
        {
            parts.Add(project);
        }

        return string.Join(" · ", parts);
    }
}
