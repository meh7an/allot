namespace Allot.Tray.Core.Notifications;

/// <summary>
/// The toasts of handoff section 9 and the update notice, each behind its own switch on the
/// Settings window's Panel page.
/// </summary>
public enum ToastKind
{
    ServiceStopped,
    RunningLong,
    BenchmarkFinished,
    JobFinished,
    KilledOutsideFlyout,
    UpdateAvailable,
}

public enum ToastButton
{
    StartService,
    OpenLogs,
    Show,
    Update,
}

/// <summary>
/// One toast to show, in the app's words; the tray app turns it into an app notification.
/// </summary>
/// <param name="Kind">Which toast it is.</param>
/// <param name="Tag">Names the toast in Notification Center; showing another toast with the same
/// tag replaces it.</param>
/// <param name="Title">The first, bold line.</param>
/// <param name="Body">The second line.</param>
/// <param name="Buttons">The buttons, primary first.</param>
/// <param name="Target">The key of the flyout row that Show or a click on the toast scrolls to
/// (<c>job:&lt;lease&gt;</c> or <c>recent</c>), or <c>null</c> for the top.</param>
public sealed record ToastMessage(
    ToastKind Kind,
    string Tag,
    string Title,
    string Body,
    IReadOnlyList<ToastButton> Buttons,
    string? Target);

/// <summary>
/// What one poll changes: toasts to show, tags of shown toasts that no longer apply, and a
/// release to tell allotd about.
/// </summary>
/// <param name="Show">New toasts, oldest event first.</param>
/// <param name="Withdraw">Tags to remove from Notification Center.</param>
/// <param name="Announced">The release this panel raised the notice for, or showed in the open
/// flyout, for <c>POST /update/announced</c>; <c>null</c> otherwise.</param>
public sealed record ToastUpdate(
    IReadOnlyList<ToastMessage> Show,
    IReadOnlyList<string> Withdraw,
    string? Announced = null)
{
    public static ToastUpdate None { get; } = new([], []);
}
