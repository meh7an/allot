using System.Diagnostics;

using ResourcesHelper.Tray.Core.Logging;

namespace ResourcesHelper.Tray;

/// <summary>
/// The app-wide <see cref="TrayLog"/>, set up once at launch. Lines written before that only reach
/// the debugger.
/// </summary>
internal static class AppLog
{
    private static TrayLog? s_log;

    public static void Initialize(TrayLog log) => s_log = log;

    public static void Write(string message)
    {
        if (s_log is { } log)
        {
            log.Write(message);
        }
        else
        {
            Debug.WriteLine(message);
        }
    }
}
