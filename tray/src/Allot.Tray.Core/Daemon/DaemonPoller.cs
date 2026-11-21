using System.Globalization;

using Allot.Tray.Core.Contract;

namespace Allot.Tray.Core.Daemon;

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

    /// <summary>
    /// How long a restart waits for the old allotd to stop answering before it starts the new one.
    /// </summary>
    public static readonly TimeSpan StopTimeout = TimeSpan.FromSeconds(10);

    private static readonly TimeSpan StopCheck = TimeSpan.FromMilliseconds(200);

    private readonly Lock _gate = new();
    private readonly PollSchedule _schedule = new();
    private TaskCompletionSource _wake = NewWake();
    private DateTimeOffset? _startingUntil;
    private volatile string? _startProblem;
    private volatile bool _flyoutOpen;

    /// <summary>
    /// Raised after every poll, on the polling thread.
    /// </summary>
    public event EventHandler<PollResult>? Updated;

    public PollResult? Latest { get; private set; }

    /// <summary>
    /// The newest state allotd served, kept while it is down.
    /// </summary>
    public UiState? LastKnown { get; private set; }

    /// <summary>
    /// Why the last Start service failed: the launch itself, or allotd not answering within
    /// <see cref="StartTimeout"/>. Cleared by the next start and once allotd answers.
    /// </summary>
    public string? StartProblem => _startProblem;

    /// <summary>
    /// The port this panel dials, which differs from <c>daemon.port</c> until allotd restarts.
    /// </summary>
    public ushort Port => client.Port;

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
    /// Launches allotd and polls quickly until it answers or <see cref="StartTimeout"/> passes.
    /// </summary>
    /// <returns><c>null</c> when allotd was launched, else why not.</returns>
    public string? StartService()
    {
        // A failed launch polls too, so whoever shows the poll shows the problem at once.
        var problem = launcher.Start();
        _startProblem = problem;
        if (problem is null)
        {
            lock (_gate)
            {
                _startingUntil = time.GetUtcNow() + StartTimeout;
                _schedule.Reset();
            }
        }

        Wake();
        return problem;
    }

    /// <summary>
    /// Asks allotd to stop, then polls at once so the icon and flyout show it stopped.
    /// </summary>
    /// <returns><c>null</c> when allotd accepted, else why not.</returns>
    public async Task<string?> StopServiceAsync(CancellationToken cancellationToken)
    {
        var problem = await client.ShutdownAsync(record: true, cancellationToken)
            .ConfigureAwait(false);
        PollNow();
        return problem;
    }

    /// <summary>
    /// Restarts allotd so a new port or token takes effect: stops it without a stop record, waits
    /// until it no longer answers, then starts it and polls it on <paramref name="port"/>.
    /// </summary>
    /// <param name="port">The port <c>config.toml</c> names now.</param>
    /// <param name="cancellationToken">Cancels the wait.</param>
    /// <returns><c>null</c> when allotd stopped and was started again, else why not.</returns>
    public async Task<string?> RestartServiceAsync(ushort port, CancellationToken cancellationToken)
    {
        var problem = await client.ShutdownAsync(record: false, cancellationToken)
            .ConfigureAwait(false);
        if (problem is not null)
        {
            PollNow();
            return problem;
        }

        // A new allotd started while the old one still holds its instance lock exits at once.
        var until = time.GetUtcNow() + StopTimeout;
        while (await client.HealthAsync(cancellationToken).ConfigureAwait(false) is not null)
        {
            if (time.GetUtcNow() >= until)
            {
                PollNow();
                return "the service did not stop within 10 s";
            }

            await Task.Delay(StopCheck, time, cancellationToken).ConfigureAwait(false);
        }

        client.UsePort(port);
        return StartService();
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

    // A launched allotd takes a moment to bind its port; until then it looks stopped.
    private PollResult WhileStarting(PollResult result)
    {
        if (result.Status != ServiceStatus.Stopped)
        {
            _startingUntil = null;
            _startProblem = null;
            return result;
        }

        if (_startingUntil is not { } until)
        {
            return result;
        }

        if (time.GetUtcNow() < until)
        {
            return result with { Status = ServiceStatus.Starting };
        }

        _startingUntil = null;
        var seconds = StartTimeout.TotalSeconds.ToString(CultureInfo.InvariantCulture);
        var detail = $"allotd did not answer within {seconds} s; see logs\\allotd.log";
        _startProblem = detail;
        return result with { Detail = detail };
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
