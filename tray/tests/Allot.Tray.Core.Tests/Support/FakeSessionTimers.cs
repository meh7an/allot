using Allot.Tray.Core.ServiceSettings;

namespace Allot.Tray.Core.Tests.Support;

/// <summary>
/// Timers that fire only when a test says so.
/// </summary>
internal sealed class FakeSessionTimers : ISessionTimers
{
    private readonly List<PendingTimer> _pending = [];

    /// <summary>
    /// The delays of the timers that have neither fired nor been cancelled.
    /// </summary>
    public List<TimeSpan> Delays => [.. _pending.Select(timer => timer.Delay)];

    public IDisposable Schedule(TimeSpan delay, Action callback)
    {
        var timer = new PendingTimer(this, delay, callback);
        _pending.Add(timer);
        return timer;
    }

    /// <summary>
    /// Fires every pending timer; a timer a callback schedules waits for the next call.
    /// </summary>
    public void FireAll()
    {
        var timers = _pending.ToList();
        _pending.Clear();
        foreach (var timer in timers)
        {
            timer.Fire();
        }
    }

    private sealed class PendingTimer(FakeSessionTimers owner, TimeSpan delay, Action callback)
        : IDisposable
    {
        public TimeSpan Delay => delay;

        public void Fire() => callback();

        public void Dispose() => _ = owner._pending.Remove(this);
    }
}
