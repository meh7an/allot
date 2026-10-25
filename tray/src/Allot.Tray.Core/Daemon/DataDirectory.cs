namespace Allot.Tray.Core.Daemon;

/// <summary>
/// Locations of the per-user files under <c>%USERPROFILE%\.allot\</c>, as in
/// <c>allot_shared::Paths</c>. Not under AppData: Windows redirects writes there from MSIX-packaged
/// apps (the Claude desktop app and everything it starts), so other processes would not see
/// them.
/// </summary>
public sealed record DataDirectory(string Root)
{
    private const string AppDirectory = ".allot";

    /// <exception cref="InvalidOperationException">USERPROFILE is unset or empty.</exception>
    public static DataDirectory FromEnvironment()
    {
        var home = Environment.GetEnvironmentVariable("USERPROFILE");
        if (string.IsNullOrEmpty(home))
        {
            throw new InvalidOperationException("USERPROFILE is not set");
        }

        return Under(home);
    }

    /// <summary>
    /// The install rooted at <c>&lt;home&gt;\.allot</c>.
    /// </summary>
    public static DataDirectory Under(string home) => new(Path.Combine(home, AppDirectory));

    /// <summary>
    /// The bearer token clients send; allotd writes it on first start.
    /// </summary>
    public string TokenFile => Path.Combine(Root, "token");

    /// <summary>
    /// The daemon config, which holds the port. Optional; the daemon runs on defaults without it.
    /// </summary>
    public string ConfigFile => Path.Combine(Root, "config.toml");

    public string DaemonExe => Path.Combine(Root, "bin", "allotd.exe");

    public string LogDirectory => Path.Combine(Root, "logs");

    /// <summary>
    /// The tray app's own settings.
    /// </summary>
    public string SettingsFile => Path.Combine(Root, "tray.json");
}
