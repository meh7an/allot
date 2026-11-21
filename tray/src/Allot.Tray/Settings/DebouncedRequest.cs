using Microsoft.UI.Dispatching;

namespace Allot.Tray.Settings;

/// <summary>
/// A request sent a pause after the last trigger, with a counter so an answer that arrives after
/// a newer request began is dropped.
/// </summary>
internal sealed class DebouncedRequest
{
    private readonly DispatcherQueueTimer _timer;
    private int _request;

    /// <param name="dispatcher">The UI thread's queue, which the timer ticks on.</param>
    /// <param name="pause">How long after the last <see cref="Restart"/> the request runs.</param>
    /// <param name="run">Sends the request.</param>
    public DebouncedRequest(DispatcherQueue dispatcher, TimeSpan pause, Func<Task> run)
    {
        _timer = dispatcher.CreateTimer();
        _timer.Interval = pause;
        _timer.IsRepeating = false;
        _timer.Tick += (_, _) => _ = run();
    }

    /// <summary>
    /// Runs the request after the pause, unless another trigger comes first.
    /// </summary>
    public void Restart()
    {
        _timer.Stop();
        _timer.Start();
    }

    /// <summary>
    /// Cancels a request that has not started.
    /// </summary>
    public void Stop() => _timer.Stop();

    /// <summary>
    /// Starts a request; the number goes to <see cref="IsCurrent"/> when its answer arrives.
    /// </summary>
    public int Begin() => ++_request;

    /// <summary>
    /// Whether no newer request began since <paramref name="request"/>.
    /// </summary>
    public bool IsCurrent(int request) => request == _request;
}
