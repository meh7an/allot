namespace Allot.Tray.Core.Daemon;

/// <summary>
/// How long to wait before the next poll. Live data ticks at 1 Hz while the flyout is open; the
/// closed flyout only drives the tray icon and toasts. While allotd is unreachable the wait doubles,
/// capped lower while the flyout is open, so a service started elsewhere shows up quickly.
/// </summary>
public sealed class PollSchedule
{
    public static readonly TimeSpan OpenInterval = TimeSpan.FromSeconds(1);
    public static readonly TimeSpan ClosedInterval = TimeSpan.FromSeconds(5);
    public static readonly TimeSpan StartingInterval = TimeSpan.FromMilliseconds(500);
    public static readonly TimeSpan FirstRetry = TimeSpan.FromSeconds(1);
    public static readonly TimeSpan OpenRetryCap = TimeSpan.FromSeconds(5);
    public static readonly TimeSpan ClosedRetryCap = TimeSpan.FromSeconds(30);

    private int _failures;

    /// <param name="flyoutOpen">Whether the flyout is showing.</param>
    /// <param name="status">The status the poll just returned.</param>
    public TimeSpan Next(bool flyoutOpen, ServiceStatus status)
    {
        switch (status)
        {
            case ServiceStatus.Running:
                _failures = 0;
                return flyoutOpen ? OpenInterval : ClosedInterval;
            case ServiceStatus.Starting:
                _failures = 0;
                return StartingInterval;
            default:
                var cap = flyoutOpen ? OpenRetryCap : ClosedRetryCap;
                var retry = FirstRetry * Math.Pow(2, Math.Min(_failures, 16));
                _failures++;
                return retry < cap ? retry : cap;
        }
    }

    /// <summary>
    /// Opening the flyout or starting the service retries at once and from the shortest wait.
    /// </summary>
    public void Reset() => _failures = 0;
}
