using System.Globalization;

using ResourcesHelper.Tray.Core.Contract;

namespace ResourcesHelper.Tray.Core.Daemon;

/// <summary>
/// Polls the daemon on the <see cref="PollSchedule"/> and keeps the last state it served, which
/// the service-down view shows dimmed.
/// </summary>
public sealed class DaemonPoller(DaemonClient client, IServiceLauncher launcher, TimeProvider time)
{
    /// <summary>
    /// How long Start service reports <see cref="ServiceStatus.Starting"/> before giving up.
    /// </summary>
    public static readonly TimeSpan StartTimeout = TimeSpan.FromSeconds(10);

    private readonly Lock _gate = new();
    private readonly PollSchedule _schedule = new();
    private TaskCompletionSource _wake = NewWake();
    private DateTimeOffset? _startingUntil;
    private volatile bool _flyoutOpen;

    /// <summary>
    /// Raised after every poll, on the polling thread.
    /// </summary>
    public event EventHandler<PollResult>? Updated;

    public PollResult? Latest { get; private set; }

    /// <summary>
    /// The newest state rhd served, kept while it is down.
    /// </summary>
    public UiState? LastKnown { get; private set; }

    /// <summary>
    /// Switches between the open and closed cadence; opening also polls at once.
    /// </summary>
    public void SetFlyoutOpen(bool open)
    {
        _flyoutOpen = open;
        if (open)
        {
            PollNow();
        }
    }

    /// <summary>
    /// Launches rhd and polls quickly until it answers or <see cref="StartTimeout"/> passes.
    /// </summary>
    /// <returns><c>null</c> when rhd was launched, else why not.</returns>
    public string? StartService()
    {
        if (launcher.Start() is { } problem)
        {
            return problem;
        }

        lock (_gate)
        {
            _startingUntil = time.GetUtcNow() + StartTimeout;
            _schedule.Reset();
        }

        Wake();
        return null;
    }

    /// <summary>
    /// Asks rhd to stop, then polls at once so the icon and flyout show it stopped.
    /// </summary>
    /// <returns><c>null</c> when rhd accepted, else why not.</returns>
    public async Task<string?> StopServiceAsync(CancellationToken cancellationToken)
    {
        var problem = await client.ShutdownAsync(cancellationToken).ConfigureAwait(false);
        PollNow();
        return problem;
    }

    /// <summary>
    /// Polls at once and restarts the backoff.
    /// </summary>
    public void PollNow()
    {
        lock (_gate)
        {
            _schedule.Reset();
        }

        Wake();
    }

    /// <summary>
    /// Polls until <paramref name="cancellationToken"/> is cancelled, then throws
    /// <see cref="OperationCanceledException"/>.
    /// </summary>
    public async Task RunAsync(CancellationToken cancellationToken)
    {
        while (true)
        {
            var delay = await PollOnceAsync(cancellationToken).ConfigureAwait(false);
            await WaitAsync(delay, cancellationToken).ConfigureAwait(false);
        }
    }

    /// <returns>How long to wait before the next poll.</returns>
    public async Task<TimeSpan> PollOnceAsync(CancellationToken cancellationToken)
    {
        var result = await client.PollAsync(cancellationToken).ConfigureAwait(false);
        TimeSpan delay;
        lock (_gate)
        {
            result = WhileStarting(result);
            delay = _schedule.Next(_flyoutOpen, result.Status);
        }

        if (result.State is { } state)
        {
            LastKnown = state;
        }

        Latest = result;
        Updated?.Invoke(this, result);
        return delay;
    }

    private static TaskCompletionSource NewWake() =>
        new(TaskCreationOptions.RunContinuationsAsynchronously);

    // A launched rhd takes a moment to bind its port; until then it looks stopped.
    private PollResult WhileStarting(PollResult result)
    {
        if (_startingUntil is not { } until)
        {
            return result;
        }

        if (result.Status != ServiceStatus.Stopped)
        {
            _startingUntil = null;
            return result;
        }

        if (time.GetUtcNow() < until)
        {
            return result with { Status = ServiceStatus.Starting };
        }

        _startingUntil = null;
        var seconds = StartTimeout.TotalSeconds.ToString(CultureInfo.InvariantCulture);
        return result with { Detail = $"rhd did not answer within {seconds} s; see logs\\rhd.log" };
    }

    private void Wake() => Volatile.Read(ref _wake).TrySetResult();

    private async Task WaitAsync(TimeSpan delay, CancellationToken cancellationToken)
    {
        var wake = Volatile.Read(ref _wake);
        using var sleep = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        _ = await Task.WhenAny(Task.Delay(delay, time, sleep.Token), wake.Task)
            .ConfigureAwait(false);
        await sleep.CancelAsync().ConfigureAwait(false);
        if (wake.Task.IsCompleted)
        {
            _ = Interlocked.CompareExchange(ref _wake, NewWake(), wake);
        }

        cancellationToken.ThrowIfCancellationRequested();
    }
}
