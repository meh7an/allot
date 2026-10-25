namespace ResourcesHelper.Tray.Core.Startup;

public enum TrayAction
{
    /// <summary>
    /// Start the app, or open the flyout of the one already running.
    /// </summary>
    Run,

    /// <summary>
    /// Ask the running app to quit; start nothing.
    /// </summary>
    Quit,

    /// <summary>
    /// Turn Launch at logon on or off for this exe, then exit.
    /// </summary>
    SetLaunchAtLogon,

    /// <summary>
    /// A switch of ours with a wrong value; exit with an error.
    /// </summary>
    Invalid,
}

/// <summary>
/// What a launch of the tray exe asks for. <c>rh install</c> and <c>rh uninstall</c> pass the
/// switches; any other command line starts the app as before.
/// </summary>
/// <param name="Action">What to do.</param>
/// <param name="LaunchAtLogon">The value for <see cref="TrayAction.SetLaunchAtLogon"/>.</param>
public sealed record TrayCommand(TrayAction Action, bool LaunchAtLogon = false)
{
    public const string QuitSwitch = "--quit";
    public const string LaunchAtLogonSwitch = "--launch-at-logon";

    /// <summary>
    /// The exit code of a command line this app does not understand.
    /// </summary>
    public const int UsageExitCode = 2;

    /// <summary>
    /// Reads the arguments after the exe path. A switch must come first and alone, with its
    /// value.
    /// </summary>
    public static TrayCommand Parse(IReadOnlyList<string> args)
    {
        if (args.Count == 0)
        {
            return new(TrayAction.Run);
        }

        return args[0] switch
        {
            QuitSwitch when args.Count == 1 => new(TrayAction.Quit),
            LaunchAtLogonSwitch when args.Count == 2 => args[1].ToUpperInvariant() switch
            {
                "ON" => new(TrayAction.SetLaunchAtLogon, LaunchAtLogon: true),
                "OFF" => new(TrayAction.SetLaunchAtLogon, LaunchAtLogon: false),
                _ => new(TrayAction.Invalid),
            },
            QuitSwitch or LaunchAtLogonSwitch => new(TrayAction.Invalid),
            _ => new(TrayAction.Run),
        };
    }

    /// <summary>
    /// Whether a launch handed to the running app asked it to quit. The handed-over command line
    /// is one string, which may or may not start with the exe path.
    /// </summary>
    public static bool AsksToQuit(string? commandLine) =>
        commandLine is not null
        && commandLine
            .Split(' ', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries)
            .Contains(QuitSwitch);
}
