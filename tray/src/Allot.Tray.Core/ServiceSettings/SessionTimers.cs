namespace Allot.Tray.Core.ServiceSettings;

/// <summary>
/// The timers of the <see cref="SettingsSession"/>: the burst that ends 600 ms after the last
/// change, and the refresh every 5 s. A fake fires them by hand in tests.
/// </summary>
public interface ISessionTimers
{
    /// <summary>
    /// Calls <paramref name="callback"/> once after <paramref name="delay"/>, on the thread the
    /// session lives on. Disposing the result before then cancels it.
    /// </summary>
    IDisposable Schedule(TimeSpan delay, Action callback);
}

/// <summary>
/// Timers on a <see cref="TimeProvider"/> that call back on a <see cref="SynchronizationContext"/>,
/// such as the UI thread's, since the session and its models are not thread-safe.
/// </summary>
/// <param name="time">The clock the timers run on.</param>
/// <param name="context">Where callbacks run; <c>null</c> runs them on the timer's thread.</param>
public sealed class TimeProviderSessionTimers(TimeProvider time, SynchronizationContext? context)
    : ISessionTimers
{
    /// <inheritdoc/>
    public IDisposable Schedule(TimeSpan delay, Action callback) =>
        time.CreateTimer(_ => Run(callback), null, delay, Timeout.InfiniteTimeSpan);

    private void Run(Action callback)
    {
        if (context is null)
        {
            callback();
        }
        else
        {
            context.Post(static state => ((Action)state!)(), callback);
        }
    }
}
