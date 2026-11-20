using Allot.Tray.Core.Contract;

namespace Allot.Tray.Core.Daemon;

/// <summary>
/// The settings routes of allotd: <see cref="DaemonClient"/>, or a fake in tests. Only
/// cancellation throws.
/// </summary>
public interface ISettingsClient
{
    /// <summary>
    /// <c>GET /settings</c>: every setting with its spec, value and the file's revision.
    /// </summary>
    Task<SettingsReply<SettingsState>> SettingsAsync(CancellationToken cancellationToken);

    /// <summary>
    /// <c>PATCH /settings</c>: writes the changes together or not at all.
    /// </summary>
    /// <returns>The new state.</returns>
    Task<SettingsReply<SettingsState>> ChangeSettingsAsync(
        SettingsPatch patch,
        CancellationToken cancellationToken);

    /// <summary>
    /// <c>POST /settings/preview</c>: what fits with unsaved changes on top of the file.
    /// </summary>
    Task<SettingsReply<FitPreview>> PreviewSettingsAsync(
        IReadOnlyList<Change> changes,
        CancellationToken cancellationToken);

    /// <summary>
    /// <c>POST /classify</c>: how the hook treats a command line under the file's rules, or
    /// under unsaved changes to them.
    /// </summary>
    Task<SettingsReply<CommandTest>> ClassifyAsync(
        ClassifyRequest request,
        CancellationToken cancellationToken);
}
