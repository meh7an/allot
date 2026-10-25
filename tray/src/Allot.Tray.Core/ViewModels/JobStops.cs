using Allot.Tray.Core.Contract;
using Allot.Tray.Core.Daemon;

namespace Allot.Tray.Core.ViewModels;

public enum StopPress
{
    /// <summary>
    /// Nothing happens: the double-press guard, a stop already under way, or an unknown job.
    /// </summary>
    Ignored,

    /// <summary>
    /// The first press of Kill: the card asks Kill?.
    /// </summary>
    Confirming,

    /// <summary>
    /// The caller sends the request to allotd and reports back with
    /// <see cref="JobStops.Finished"/>.
    /// </summary>
    Send,
}

/// <summary>
/// The Kill and Remove state of every card (handoff section 4): the two-step Kill? confirm, the
/// request in flight, and the Stopped, Removed or refused line that holds before the card goes.
/// It keeps the last seen copy of a job whose stop is under way or done, because allotd stops
/// listing the lease the moment it ends, while the card still holds for
/// <see cref="EndedFor"/>.
/// </summary>
public sealed class JobStops(TimeProvider time)
{
    /// <summary>
    /// How long Kill? waits for the second press.
    /// </summary>
    public static readonly TimeSpan ConfirmFor = TimeSpan.FromSeconds(4);

    /// <summary>
    /// A second press this soon after the first is ignored, so a double-click can't kill.
    /// </summary>
    public static readonly TimeSpan DoublePress = TimeSpan.FromMilliseconds(300);

    /// <summary>
    /// How long Stopped or Removed shows before the card collapses.
    /// </summary>
    public static readonly TimeSpan EndedFor = TimeSpan.FromSeconds(4);

    /// <summary>
    /// How long a refusal shows before the card reads normally again.
    /// </summary>
    public static readonly TimeSpan FailedFor = TimeSpan.FromSeconds(6);

    private static readonly TimeSpan Second = TimeSpan.FromSeconds(1);

    private readonly Dictionary<long, Stop> _stops = [];
    private UiState? _seen;

    /// <summary>
    /// The cards with a stop, as they read now; call <see cref="Expire"/> first.
    /// </summary>
    public IReadOnlyDictionary<long, StopView> Views =>
        _stops.ToDictionary(pair => pair.Key, pair => ViewOf(pair.Value));

    /// <summary>
    /// The time until a card's text next changes by itself (a countdown second, or a hold
    /// running out), or <c>null</c> when none will.
    /// </summary>
    public TimeSpan? NextChange
    {
        get
        {
            TimeSpan? next = null;
            foreach (var stop in _stops.Values)
            {
                var age = Age(stop);
                TimeSpan? due = stop.Phase switch
                {
                    StopPhase.Confirming => Second - TimeSpan.FromTicks(age.Ticks % Second.Ticks),
                    StopPhase.Ended => EndedFor - age,
                    StopPhase.Failed => FailedFor - age,
                    _ => null,
                };
                if (due is { } soon && (next is null || soon < next))
                {
                    next = soon < TimeSpan.Zero ? TimeSpan.Zero : soon;
                }
            }

            return next;
        }
    }

    /// <summary>
    /// Takes a poll's live state: copies of the jobs with a stop are refreshed, and a confirm or
    /// refusal is dropped once its job is gone or has moved between running and waiting.
    /// </summary>
    public void Observe(UiState state)
    {
        var before = _seen;
        _seen = state;
        foreach (var (lease, stop) in _stops.ToList())
        {
            var held = Find(state, lease);
            if (held is not null && KindOf(held) == stop.Kind)
            {
                _stops[lease] = stop with { Held = held };
            }
            else if (stop.Phase is StopPhase.Confirming or StopPhase.Failed)
            {
                _ = _stops.Remove(lease);
            }
        }

        if (before is not null)
        {
            HoldOutsideEnds(before, state);
        }
    }

    /// <summary>
    /// A press of a running job's Kill, or of its Kill? confirm.
    /// </summary>
    public StopPress PressKill(long lease)
    {
        _ = Expire();
        if (_stops.TryGetValue(lease, out var stop))
        {
            switch (stop.Phase)
            {
                case StopPhase.Confirming when Age(stop) <= DoublePress:
                    return StopPress.Ignored;
                case StopPhase.Confirming:
                    _stops[lease] = stop with { Phase = StopPhase.Sending, Since = Now };
                    return StopPress.Send;
                case StopPhase.Sending or StopPhase.Ended:
                    return StopPress.Ignored;
                default:
                    break;
            }
        }

        if (Find(_seen, lease) is not { Active: not null } held)
        {
            return StopPress.Ignored;
        }

        _stops[lease] = new Stop(StopKind.Kill, StopPhase.Confirming, Now, held);
        return StopPress.Confirming;
    }

    /// <summary>
    /// A press of a queued request's Remove.
    /// </summary>
    /// <returns>True when the caller sends the request to allotd.</returns>
    public bool PressRemove(long lease)
    {
        _ = Expire();
        if (_stops.TryGetValue(lease, out var stop) && stop.Phase != StopPhase.Failed)
        {
            return false;
        }

        if (Find(_seen, lease) is not { Queued: not null } held)
        {
            return false;
        }

        _stops[lease] = new Stop(StopKind.Remove, StopPhase.Sending, Now, held);
        return true;
    }

    /// <summary>
    /// Esc on Kill?, or focus leaving it.
    /// </summary>
    /// <returns>True when a confirm was cancelled.</returns>
    public bool Cancel(long lease) =>
        _stops.TryGetValue(lease, out var stop)
        && stop.Phase == StopPhase.Confirming
        && _stops.Remove(lease);

    /// <summary>
    /// Cancels every confirm: the flyout closed, or the service went away.
    /// </summary>
    /// <returns>True when any was cancelled.</returns>
    public bool CancelConfirms()
    {
        var confirming = _stops.Where(pair => pair.Value.Phase == StopPhase.Confirming)
            .Select(pair => pair.Key)
            .ToList();
        foreach (var lease in confirming)
        {
            _ = _stops.Remove(lease);
        }

        return confirming.Count > 0;
    }

    /// <summary>
    /// allotd's answer to a request <see cref="PressKill"/> or <see cref="PressRemove"/> sent.
    /// </summary>
    public void Finished(long lease, StopResult result)
    {
        if (!_stops.TryGetValue(lease, out var stop) || stop.Phase != StopPhase.Sending)
        {
            return;
        }

        _stops[lease] = result.Ended
            ? stop with
            {
                Phase = StopPhase.Ended,
                Since = Now,
                Message = result.Text,
                EndedAtMs = result.EndedAtMs,
            }
            : stop with { Phase = StopPhase.Failed, Since = Now, Message = result.Text };
    }

    /// <summary>
    /// Drops confirms past their countdown and holds past their time.
    /// </summary>
    /// <returns>True when any was dropped.</returns>
    public bool Expire()
    {
        var done = _stops.Where(pair => pair.Value.Phase switch
            {
                StopPhase.Confirming => Age(pair.Value) >= ConfirmFor,
                StopPhase.Ended => Age(pair.Value) >= EndedFor,
                StopPhase.Failed => Age(pair.Value) >= FailedFor,
                _ => false,
            })
            .Select(pair => pair.Key)
            .ToList();
        foreach (var lease in done)
        {
            _ = _stops.Remove(lease);
        }

        return done.Count > 0;
    }

    /// <summary>
    /// <paramref name="state"/> with the last seen copy of every job whose stop is under way or
    /// done put back where it was, once allotd no longer lists it.
    /// </summary>
    public UiState Merge(UiState state)
    {
        var missing = _stops.Values
            .Where(stop => stop.Phase is StopPhase.Sending or StopPhase.Ended)
            .Where(stop => Find(state, stop.Held.LeaseId) is null)
            .OrderBy(stop => stop.Held.Index)
            .ToList();
        if (missing.Count == 0)
        {
            return state;
        }

        var active = state.Active.ToList();
        var queue = state.Queue.ToList();
        foreach (var stop in missing)
        {
            if (stop.Held.Active is { } job)
            {
                active.Insert(Math.Min(stop.Held.Index, active.Count), job);
            }
            else if (stop.Held.Queued is { } request)
            {
                queue.Insert(Math.Min(stop.Held.Index, queue.Count), request);
            }
        }

        return state with { Active = active, Queue = queue };
    }

    private DateTimeOffset Now => time.GetUtcNow();

    // A running job allotd stopped listing without a stop of this flyout holds its card too, read
    // from allotd's ended list: released (Finished), killed elsewhere, or ended by allotd. Removals,
    // queued requests and benchmarks (their Finished card comes from recent runs) are not held,
    // and neither is anything an older allotd, which sends no ended list, does not explain.
    private void HoldOutsideEnds(UiState before, UiState state)
    {
        for (var i = 0; i < before.Active.Count; i++)
        {
            var job = before.Active[i];
            var lease = job.Lease;
            if (_stops.ContainsKey(lease.Id)
                || lease.Kind == LeaseKind.BenchmarkExclusive
                || Find(state, lease.Id) is not null)
            {
                continue;
            }

            var ended = state.Ended.FirstOrDefault(end => end.LeaseId == lease.Id);
            StopKind? kind = ended?.Cause switch
            {
                EndCause.Released => StopKind.Finished,
                EndCause.Killed => StopKind.KilledOutside,
                EndCause.Expired => StopKind.EndedByRhd,
                _ => null,
            };
            if (ended is null || kind is not { } held)
            {
                continue;
            }

            var last = new Held(lease.Id, job, null, i);
            _stops[lease.Id] = new Stop(held, StopPhase.Ended, Now, last)
            {
                Message = held == StopKind.EndedByRhd ? ended.Reason : "",
                EndedAtMs = ended.EndedAtMs,
            };
        }
    }

    private static Held? Find(UiState? state, long lease)
    {
        if (state is null)
        {
            return null;
        }

        for (var i = 0; i < state.Active.Count; i++)
        {
            if (state.Active[i].Lease.Id == lease)
            {
                return new Held(lease, state.Active[i], null, i);
            }
        }

        for (var i = 0; i < state.Queue.Count; i++)
        {
            if (state.Queue[i].Lease.Id == lease)
            {
                return new Held(lease, null, state.Queue[i], i);
            }
        }

        return null;
    }

    private static StopKind KindOf(Held held) =>
        held.Active is null ? StopKind.Remove : StopKind.Kill;

    private TimeSpan Age(Stop stop) => Now - stop.Since;

    private StopView ViewOf(Stop stop)
    {
        var left = stop.Phase == StopPhase.Confirming
            ? Math.Max(1, ConfirmFor.Seconds - (int)(Age(stop).Ticks / Second.Ticks))
            : 0;
        return new StopView(stop.Kind, stop.Phase, left, stop.Message, stop.EndedAtMs);
    }

    /// <param name="LeaseId">The job's lease.</param>
    /// <param name="Active">The running job as last seen, or <c>null</c>.</param>
    /// <param name="Queued">The waiting request as last seen, or <c>null</c>.</param>
    /// <param name="Index">Its place in allotd's active or queue list.</param>
    private sealed record Held(long LeaseId, ActiveJob? Active, QueuedJob? Queued, int Index);

    /// <param name="Kind">Kill or Remove.</param>
    /// <param name="Phase">Where it stands.</param>
    /// <param name="Since">When it entered the phase, in local time.</param>
    /// <param name="Held">The job as last seen.</param>
    private sealed record Stop(StopKind Kind, StopPhase Phase, DateTimeOffset Since, Held Held)
    {
        public string Message { get; init; } = "";

        public long? EndedAtMs { get; init; }
    }
}
