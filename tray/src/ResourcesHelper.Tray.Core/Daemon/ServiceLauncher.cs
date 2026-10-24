using System.ComponentModel;
using System.Diagnostics;

namespace ResourcesHelper.Tray.Core.Daemon;

public interface IServiceLauncher
{
    /// <summary>
    /// Starts rhd without waiting for it to answer.
    /// </summary>
    /// <returns><c>null</c> when it was started, else why not.</returns>
    string? Start();
}

/// <summary>
/// Starts the installed <c>bin\rhd.exe</c>. rhd is a GUI-subsystem binary, so it gets no console,
/// and Windows does not end it with the tray app.
/// </summary>
public sealed class ServiceLauncher(DataDirectory data) : IServiceLauncher
{
    public string? Start()
    {
        var exe = data.DaemonExe;
        if (!File.Exists(exe))
        {
            return $"{exe} is missing; run rh install";
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
