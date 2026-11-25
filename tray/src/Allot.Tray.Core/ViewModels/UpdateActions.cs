using Allot.Tray.Core.Contract;
using Allot.Tray.Core.Daemon;

namespace Allot.Tray.Core.ViewModels;

public enum UpdateAction
{
    None,
    Checking,
    Starting,
}

/// <summary>
/// What a press of Update or Check now is still doing, or why it failed.
/// </summary>
/// <param name="Pending">The press allotd has not answered, or whose result no poll shows yet.</param>
/// <param name="Problem">Why the last press was refused, or <c>null</c>.</param>
public sealed record UpdatePress(UpdateAction Pending, string? Problem)
{
    public static UpdatePress None { get; } = new(UpdateAction.None, null);
}

/// <summary>
/// Update and Check now, shared by the flyout, its update toast and the Service page. One press
/// at a time, on the UI thread.
/// </summary>
/// <param name="client">allotd's update routes.</param>
/// <param name="pollNow">Asks for a poll, which shows what a press changed.</param>
/// <param name="time">The clock.</param>
/// <param name="log">The tray's log.</param>
public sealed class UpdateActions(
    IUpdateClient client,
    Action pollNow,
    TimeProvider time,
    Action<string> log)
{
    /// <summary>
    /// How long an accepted Update stays "Updating" while the polls still say what they said
    /// before it, as one already under way does.
    /// </summary>
    public static readonly TimeSpan StartHold = TimeSpan.FromSeconds(5);

    private UpdateState? _state;
    private UpdateState? _pressedIn;
    private DateTimeOffset? _acceptedAt;

    public UpdatePress Press { get; private set; } = UpdatePress.None;

    /// <summary>
    /// Raised when a press or allotd's answer to it changes <see cref="Press"/>; a poll passed to
    /// <see cref="Observe"/> does not raise it.
    /// </summary>
    public event Action? Changed;

    /// <summary>
    /// Takes a poll's update status before it is shown: a refusal or an accepted Update gives way
    /// once allotd's state moves on.
    /// </summary>
    public void Observe(UpdateStatus? update)
    {
        _state = update?.State;
        var moved = _state != _pressedIn;
        var held = _acceptedAt is { } at && time.GetUtcNow() - at < StartHold;
        var over = Press.Problem is not null ? moved : _acceptedAt is not null && (moved || !held);
        if (over)
        {
            _acceptedAt = null;
            Press = UpdatePress.None;
        }
    }

    public Task CheckAsync(CancellationToken cancellationToken) =>
        RunAsync(UpdateAction.Checking, "check", client.CheckForUpdateAsync, cancellationToken);

    public Task StartAsync(CancellationToken cancellationToken) =>
        RunAsync(UpdateAction.Starting, "update", client.StartUpdateAsync, cancellationToken);

    private async Task RunAsync(
        UpdateAction action,
        string name,
        Func<CancellationToken, Task<string?>> send,
        CancellationToken cancellationToken)
    {
        if (Press.Pending != UpdateAction.None)
        {
            return;
        }

        _pressedIn = _state;
        _acceptedAt = null;
        Set(new UpdatePress(action, null));
        var problem = await send(cancellationToken).ConfigureAwait(true);
        log(problem is null ? $"{name} asked for" : $"{name} refused: {problem}");
        if (problem is not null)
        {
            Set(new UpdatePress(UpdateAction.None, problem));
        }
        else if (action == UpdateAction.Starting)
        {
            _acceptedAt = time.GetUtcNow();
        }
        else
        {
            Set(UpdatePress.None);
        }

        pollNow();
    }

    private void Set(UpdatePress press)
    {
        Press = press;
        Changed?.Invoke();
    }
}
