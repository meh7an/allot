using Allot.Tray.Core.Contract;

namespace Allot.Tray.Core.ViewModels;

/// <summary>
/// The daemon's time between polls: the latest state's <c>generated_at_ms</c> plus the time since
/// it arrived, so that elapsed and waited times tick every second without a new poll and without
/// trusting the local clock to match allotd's.
/// </summary>
public sealed class StateClock(TimeProvider time)
{
    private long _generatedAtMs;
    private DateTimeOffset _receivedAt;
    private bool _live;

    /// <summary>
    /// The current moment in the daemon's clock, or <c>null</c> while no live state is known.
    /// </summary>
    public long? NowMs
    {
        get
        {
            if (!_live)
            {
                return null;
            }

            var since = time.GetUtcNow() - _receivedAt;
            return _generatedAtMs + Math.Max(0, (long)since.TotalMilliseconds);
        }
    }

    /// <summary>
    /// Takes the state of a poll; <c>null</c> stops the clock, as last known times stand still.
    /// </summary>
    public void Received(UiState? state)
    {
        _live = state is not null;
        if (state is not null)
        {
            _generatedAtMs = state.GeneratedAtMs;
            _receivedAt = time.GetUtcNow();
        }
    }
}
