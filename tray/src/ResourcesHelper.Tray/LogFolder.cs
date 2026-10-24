using System.ComponentModel;
using System.Diagnostics;

namespace ResourcesHelper.Tray;

/// <summary>
/// Open logs, from the tray menu, the More options menu and the service toasts.
/// </summary>
internal static class LogFolder
{
    /// <summary>
    /// Opens the folder that holds rhd's and the tray app's logs in Explorer, creating it first
    /// so a fresh install still opens somewhere useful.
    /// </summary>
    public static void Open(string directory)
    {
        try
        {
            _ = Directory.CreateDirectory(directory);
            using var explorer = Process.Start(
                new ProcessStartInfo("explorer.exe") { ArgumentList = { directory } });
        }
        catch (Exception error) when (error is IOException
            or UnauthorizedAccessException
            or Win32Exception)
        {
            AppLog.Write($"opening {directory} failed: {error.Message}");
        }
    }
}
