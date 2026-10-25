using Microsoft.Win32;

using ResourcesHelper.Tray.Core.Startup;

namespace ResourcesHelper.Tray.Startup;

/// <summary>
/// The Launch at logon values under HKCU. A Debug build uses keys of its own that Windows never
/// reads, so trying the switch leaves the real autostart alone.
/// </summary>
internal sealed class RegistryAutostartStore : IAutostartStore
{
#if DEBUG
    private const string RunKey = @"Software\ResourcesHelper\DebugRun";
    private const string ApprovedKey = @"Software\ResourcesHelper\DebugStartupApproved";
#else
    private const string RunKey = @"Software\Microsoft\Windows\CurrentVersion\Run";
    private const string ApprovedKey =
        @"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
#endif

    /// <summary>
    /// Where the Run entry lives, for the log.
    /// </summary>
    public static string Location => $@"HKCU\{RunKey}";

    public string? ReadCommand()
    {
        using var key = Registry.CurrentUser.OpenSubKey(RunKey);
        return key?.GetValue(Autostart.ValueName) as string;
    }

    public byte[]? ReadApproval()
    {
        using var key = Registry.CurrentUser.OpenSubKey(ApprovedKey);
        return key?.GetValue(Autostart.ValueName) as byte[];
    }

    public void WriteCommand(string command)
    {
        using var key = Registry.CurrentUser.CreateSubKey(RunKey);
        key.SetValue(Autostart.ValueName, command, RegistryValueKind.String);
    }

    public void DeleteCommand() => Delete(RunKey);

    public void DeleteApproval() => Delete(ApprovedKey);

    private static void Delete(string path)
    {
        using var key = Registry.CurrentUser.OpenSubKey(path, writable: true);
        key?.DeleteValue(Autostart.ValueName, throwOnMissingValue: false);
    }
}
