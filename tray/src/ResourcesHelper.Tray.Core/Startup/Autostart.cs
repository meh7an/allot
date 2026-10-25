namespace ResourcesHelper.Tray.Core.Startup;

/// <summary>
/// The two registry values behind Launch at logon: the command under the Run key, and the
/// state Task Manager's Startup apps keeps under <c>StartupApproved\Run</c>.
/// </summary>
public interface IAutostartStore
{
    string? ReadCommand();

    byte[]? ReadApproval();

    void WriteCommand(string command);

    void DeleteCommand();

    void DeleteApproval();
}

/// <summary>
/// Launch at logon as an HKCU Run entry that starts the running tray exe. Turning it off in Task
/// Manager keeps the entry but marks it disabled, so that reads as off too; turning it on here
/// drops that mark.
/// </summary>
/// <param name="store">The registry, or a fake.</param>
/// <param name="exePath">The tray exe the entry starts.</param>
public sealed class Autostart(IAutostartStore store, string exePath)
{
    /// <summary>
    /// The value name under both keys, which Startup apps shows.
    /// </summary>
    public const string ValueName = "Resources Helper";

    public string Command { get; } = $"\"{exePath}\"";

    public bool IsOn =>
        !string.IsNullOrWhiteSpace(store.ReadCommand()) && !IsDisabled(store.ReadApproval());

    public void Set(bool on)
    {
        if (on)
        {
            store.WriteCommand(Command);
        }
        else
        {
            store.DeleteCommand();
        }

        store.DeleteApproval();
    }

    // Startup apps writes 02 (enabled) or 03 (disabled) as the first byte, then the time it
    // changed; no value at all means enabled.
    private static bool IsDisabled(byte[]? approval) =>
        approval is { Length: > 0 } && (approval[0] & 1) == 1;
}
