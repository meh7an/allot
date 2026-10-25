using System.ComponentModel;
using System.Diagnostics;

namespace Allot.Tray.Core.Daemon;

public interface IServiceLauncher
{
    /// <summary>
    /// Starts allotd without waiting for it to answer.
    /// </summary>
    /// <returns><c>null</c> when it was started, else why not.</returns>
    string? Start();
}

/// <summary>
/// Starts the installed <c>bin\allotd.exe</c>. allotd is a GUI-subsystem binary, so it gets no console,
/// and Windows does not end it with the tray app.
/// </summary>
public sealed class ServiceLauncher(DataDirectory data) : IServiceLauncher
{
    public string? Start()
    {
        var exe = data.DaemonExe;
        if (!File.Exists(exe))
        {
            return $"{exe} is missing; run allot install";
        }

        var start = new ProcessStartInfo(exe)
        {
            UseShellExecute = false,
            CreateNoWindow = true,
            WorkingDirectory = Path.GetDirectoryName(exe),
        };
        try
        {
            using var process = Process.Start(start);
            return process is null ? $"{exe} did not start" : null;
        }
        catch (Win32Exception error)
        {
            return $"{exe}: {error.Message}";
        }
    }
}
