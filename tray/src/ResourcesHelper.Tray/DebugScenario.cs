#if DEBUG
namespace ResourcesHelper.Tray;

/// <summary>
/// <c>rh-tray.exe --scenario &lt;name&gt;</c> in a Debug build shows a handoff scenario from
/// <see cref="Core.Samples.Handoff"/> instead of polling rhd, beside the real tray app, for
/// comparing the flyout against the handoff's state board. Release builds have no such switch.
/// </summary>
internal static class DebugScenario
{
    /// <summary>
    /// The scenario named on the command line, or <c>null</c>.
    /// </summary>
    public static string? Name { get; } = Find(Environment.GetCommandLineArgs());

    private static string? Find(string[] args)
    {
        var index = Array.IndexOf(args, "--scenario");
        return index >= 0 && index + 1 < args.Length ? args[index + 1] : null;
    }
}
#endif
