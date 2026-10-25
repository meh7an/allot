using ResourcesHelper.Tray.Core.Contract;
using ResourcesHelper.Tray.Core.Daemon;
using ResourcesHelper.Tray.Core.Settings;

namespace ResourcesHelper.Tray.Core.Notifications;

/// <summary>
/// Decides from each poll which toasts to show (handoff section 9). Everything is kept in memory:
/// the first live state after the tray app starts is a baseline whose jobs, runs and notices never
/// toast, so a restart repeats nothing, and an event from while the app was not running is not
/// shown. An event that comes while its toggle is off or the flyout is open counts as seen
/// without a toast.
/// </summary>
public sealed class ToastRules(TimeProvider time)
{
    /// <summary>
    /// A job is running long at this multiple of its estimate.
    /// </summary>
    public const double LongFactor = 1.5;

    /// <summary>
    /// A service stop seen on a single poll is a restart (rh install, for one), not a stop.
    /// </summary>
    public const int StoppedPolls = 2;

    /// <summary>
    /// Finishes within this time of a window's first toast join its toast.
    /// </summary>
    public static readonly TimeSpan MergeWindow = TimeSpan.FromSeconds(60);

    /// <summary>
    /// How long after the user chose Stop service a stop counts as theirs.
    /// </summary>
    public static readonly TimeSpan ExpectedStopWindow = TimeSpan.FromSeconds(15);

    private static readonly NotificationToggles Silent = new()
    {
        ServiceStopped = false,
        RunningLong = false,
        BenchmarkFinished = false,
        JobFinished = false,
        KilledOutsideFlyout = false,
    };

    private readonly HashSet<long> _long = [];
    private readonly HashSet<RunKey> _runs = [];
    private readonly HashSet<long> _notices = [];
    private readonly HashSet<long> _localKills = [];
    private readonly List<RecentRun> _merged = [];
    private DateTimeOffset _mergeStart;
    private int _mergeWindows;
    private bool _baselined;
    private bool _wasRunning;
    private int _stoppedPolls;
    private bool _stopShown;
    private DateTimeOffset? _stopChosenAt;

    /// <summary>
    /// The flyout is about to kill this lease, so its notice is not a kill from outside. Called
    /// before the request, so a poll that sees the notice first cannot race it.
    /// </summary>
    public void NoteLocalKill(long lease) => _localKills.Add(lease);

    /// <summary>
    /// The user chose Stop service, so the stop that follows does not toast.
    /// </summary>
    public void ExpectStop() => _stopChosenAt = time.GetUtcNow();

    /// <param name="status">The poll's service status.</param>
    /// <param name="live">The state rhd served, or <c>null</c> while it is not running.</param>
    /// <param name="lastKnown">The newest state rhd served, for the session count after a
    /// stop.</param>
    /// <param name="toggles">The ⋯ › Notifications toggles.</param>
    /// <param name="flyoutOpen">The flyout is showing, so the user sees events already.</param>
    public ToastUpdate Observe(
        ServiceStatus status,
        UiState? live,
        UiState? lastKnown,
        NotificationToggles toggles,
        bool flyoutOpen)
    {
        var allowed = flyoutOpen ? Silent : toggles;
        var show = new List<ToastMessage>();
        var withdraw = new List<string>();
        ObserveService(status, lastKnown, allowed, show, withdraw);
        if (live is not null)
        {
            if (_baselined)
            {
                ObserveJobs(live, allowed, show);
            }
            else
            {
                Baseline(live);
            }
        }

        return show.Count == 0 && withdraw.Count == 0 ? ToastUpdate.None : new(show, withdraw);
    }

    private void ObserveService(
        ServiceStatus status,
        UiState? lastKnown,
        NotificationToggles allowed,
        List<ToastMessage> show,
        List<string> withdraw)
    {
        if (status == ServiceStatus.Running)
        {
            _wasRunning = true;
            _stoppedPolls = 0;
            if (_stopShown)
            {
                _stopShown = false;
                withdraw.Add(ToastText.ServiceTag);
            }

            return;
        }

        // Starting and Incompatible change nothing: neither is a running service going away.
        if (status != ServiceStatus.Stopped || !_wasRunning || ++_stoppedPolls < StoppedPolls)
        {
            return;
        }

        _wasRunning = false;
        var chosen = _stopChosenAt is { } at && time.GetUtcNow() - at <= ExpectedStopWindow;
        _stopChosenAt = null;
        if (!chosen && allowed.ServiceStopped)
        {
            show.Add(ToastText.ServiceStopped(lastKnown?.Sessions.Count ?? 0));
            _stopShown = true;
        }
    }

    private void Baseline(UiState live)
    {
        _baselined = true;
        foreach (var job in live.Active.Where(job => IsLong(job, live.GeneratedAtMs)))
        {
            _ = _long.Add(job.Lease.Id);
        }

        _runs.UnionWith(live.Recent.Select(RunKey.Of));
        _notices.UnionWith(live.Notices.Select(notice => notice.LeaseId));
    }

    private void ObserveJobs(UiState live, NotificationToggles allowed, List<ToastMessage> show)
    {
        var sessions = new Dictionary<string, UiSession>(StringComparer.Ordinal);
        foreach (var session in live.Sessions)
        {
            _ = sessions.TryAdd(session.Session, session);
        }

        // Jobs, then runs and notices, each oldest first: rhd lists runs and notices newest
        // first.
        _long.IntersectWith(live.Active.Select(job => job.Lease.Id));
        foreach (var job in live.Active)
        {
            if (IsLong(job, live.GeneratedAtMs) && _long.Add(job.Lease.Id) && allowed.RunningLong)
            {
                var session = sessions.GetValueOrDefault(job.Lease.Holder.Session);
                show.Add(ToastText.RunningLong(job, session, live.GeneratedAtMs));
            }
        }

        var finished = new List<RecentRun>();
        foreach (var run in live.Recent.Reverse())
        {
            // A killed run has the kill's toast or the flyout's own Stopped card.
            if (!_runs.Add(RunKey.Of(run)) || run.EndCause is not (null or EndCause.Released))
            {
                continue;
            }

            if (run.Noise is { } noise)
            {
                if (allowed.BenchmarkFinished && run.LeaseId is not null)
                {
                    show.Add(ToastText.BenchmarkFinished(run, noise));
                }
            }
            else if (allowed.JobFinished)
            {
                finished.Add(run);
            }
        }

        _runs.IntersectWith(live.Recent.Select(RunKey.Of));
        if (finished.Count > 0)
        {
            show.Add(Finished(finished, sessions));
        }

        foreach (var notice in live.Notices.Reverse())
        {
            if (_notices.Add(notice.LeaseId)
                && notice.Cause == EndCause.Killed
                && !_localKills.Contains(notice.LeaseId)
                && allowed.KilledOutsideFlyout)
            {
                show.Add(ToastText.KilledOutsideFlyout(notice));
            }
        }

        _notices.IntersectWith(live.Notices.Select(notice => notice.LeaseId));
    }

    // The first finish opens a window; later ones within it replace its toast with the count.
    private ToastMessage Finished(List<RecentRun> runs, Dictionary<string, UiSession> sessions)
    {
        var now = time.GetUtcNow();
        if (_merged.Count == 0 || now - _mergeStart >= MergeWindow)
        {
            _merged.Clear();
            _mergeStart = now;
            _mergeWindows++;
        }

        _merged.AddRange(runs);
        var tag = $"done-{_mergeWindows}";
        if (_merged.Count > 1)
        {
            return ToastText.JobsFinished(_merged, tag);
        }

        var run = _merged[0];
        var session = run.Session is null ? null : sessions.GetValueOrDefault(run.Session);
        return ToastText.JobFinished(run, session, tag);
    }

    private static bool IsLong(ActiveJob job, long nowMs)
    {
        var lease = job.Lease;
        var elapsedMs = nowMs - (lease.GrantedAtMs ?? lease.RequestedAtMs);
        return lease.Profile.EstSecs > 0 && elapsedMs >= LongFactor * lease.Profile.EstSecs * 1000;
    }

    // Runs from before v0.2 have no lease; their times tell them apart.
    private readonly record struct RunKey(long? Lease, long StartedAtMs, long EndedAtMs)
    {
        public static RunKey Of(RecentRun run) => new(run.LeaseId, run.StartedAtMs, run.EndedAtMs);
    }
}
