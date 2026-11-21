using Allot.Tray.Core.Contract;

namespace Allot.Tray.Core.ServiceSettings;

/// <summary>
/// Where the session stands with allotd. Every state but <see cref="SessionStatus.Ready"/> keeps
/// the last values read-only.
/// </summary>
public enum SessionStatus
{
    /// <summary>
    /// No answer yet.
    /// </summary>
    Loading,

    /// <summary>
    /// allotd answered; the values can be changed unless the file is rejected.
    /// </summary>
    Ready,

    /// <summary>
    /// allotd does not answer and the stop record says the user stopped it.
    /// </summary>
    Stopped,

    /// <summary>
    /// allotd does not answer.
    /// </summary>
    Down,

    /// <summary>
    /// allotd refused this panel's key.
    /// </summary>
    Token,

    /// <summary>
    /// allotd answered something unusable; the session's detail says what.
    /// </summary>
    Failed,

    /// <summary>
    /// The running allotd is older than Settings and has no settings routes.
    /// </summary>
    Outdated,
}

/// <summary>
/// The kinds of save status.
/// </summary>
public enum SaveKind
{
    /// <summary>
    /// All changes saved.
    /// </summary>
    Saved,

    /// <summary>
    /// A save is in flight.
    /// </summary>
    Saving,

    /// <summary>
    /// Changes wait for the burst to end, or cannot be saved until they are fixed.
    /// </summary>
    Unsaved,
}

/// <summary>
/// The save status line: "All changes saved", "Saving..." or "{n} changes not saved".
/// </summary>
/// <param name="Kind">Which line.</param>
/// <param name="Count">Unsaved: how many settings are not saved.</param>
public readonly record struct SaveStatus(SaveKind Kind, int Count = 0);

/// <summary>
/// A group of settings on a page.
/// </summary>
/// <param name="Spec">The group.</param>
/// <param name="Settings">The group's settings this platform shows.</param>
public sealed record GroupModel(SettingsGroupSpec Spec, IReadOnlyList<ValueModel> Settings);

/// <summary>
/// A page of the Settings window.
/// </summary>
/// <param name="Spec">The page.</param>
/// <param name="Groups">The page's groups that have settings this platform shows.</param>
public sealed record PageModel(SettingsPageSpec Spec, IReadOnlyList<GroupModel> Groups);

/// <summary>
/// A change allotd did not write because <c>config.toml</c> changed by hand since it was read.
/// </summary>
/// <param name="Setting">The setting, which shows the file's value now.</param>
/// <param name="Changes">The changes that would write the user's value.</param>
public sealed record ConflictEdit(ValueModel Setting, IReadOnlyList<Change> Changes);

/// <summary>
/// The edits a hand edit got in the way of; Apply my change sends them again.
/// </summary>
/// <param name="Edits">The edits, in the order they were saved.</param>
public sealed record Conflict(IReadOnlyList<ConflictEdit> Edits);
