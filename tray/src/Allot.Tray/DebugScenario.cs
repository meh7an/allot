#if DEBUG
using System.Security;
using System.Security.Principal;

using Allot.Tray.Core.Daemon;
using Allot.Tray.Core.Samples;
using Allot.Tray.Core.ViewModels;

using Microsoft.Win32;

namespace Allot.Tray;

/// <summary>
/// <c>allot-tray.exe --scenario &lt;name&gt;</c> in a Debug build shows a handoff scenario from
/// <see cref="Core.Samples.Handoff"/> instead of polling allotd, beside the real tray app, for
/// comparing the flyout against the handoff's state board. Release builds have no such switch.
/// </summary>
internal static class DebugScenario
{
    /// <summary>
    /// The scenario named on the command line, or <c>null</c>.
    /// </summary>
    public static string? Name { get; } = Find(Environment.GetCommandLineArgs(), "--scenario");

    /// <summary>
    /// <c>--settings &lt;page&gt;</c> beside <c>--scenario</c>: opens Settings on that page at
    /// start, for checks that read the window's UI Automation tree (M7.1). A scenario instance
    /// has no single-instance key, so nothing else can open it from outside.
    /// </summary>
    public static string? SettingsPage { get; } =
        Find(Environment.GetCommandLineArgs(), "--settings");

    /// <summary>
    /// Whether USERPROFILE is the user's own profile folder, as Windows records it, so the
    /// scenario instance would reach the real ~/.allot. Unknown counts as real.
    /// </summary>
    public static bool UsesRealHome()
    {
        var home = Environment.GetEnvironmentVariable("USERPROFILE");
        string? real = null;
        try
        {
            var sid = WindowsIdentity.GetCurrent().User?.Value;
            using var key = Registry.LocalMachine.OpenSubKey(
                $@"SOFTWARE\Microsoft\Windows NT\CurrentVersion\ProfileList\{sid}");
            if (key?.GetValue("ProfileImagePath") is string path)
            {
                real = Environment.ExpandEnvironmentVariables(path);
            }
        }
        catch (Exception error) when (error is SecurityException
            or UnauthorizedAccessException
            or IOException)
        {
            AppLog.Write($"reading the profile folder failed: {error.Message}");
        }

        return string.IsNullOrEmpty(home)
            || real is null
            || string.Equals(
                Path.TrimEndingDirectorySeparator(Path.GetFullPath(home)),
                Path.TrimEndingDirectorySeparator(Path.GetFullPath(real)),
                StringComparison.OrdinalIgnoreCase);
    }

    private static string? Find(string[] args, string option)
    {
        var index = Array.IndexOf(args, option);
        return index >= 0 && index + 1 < args.Length ? args[index + 1] : null;
    }

    /// <summary>
    /// A fake allotd for Kill and Remove: it answers after 300 ms and ends the job, except in
    /// <see cref="Handoff.RefusedScenario"/>, where it refuses the kill as allotd refuses a
    /// protected process.
    /// </summary>
    /// <param name="scenario">The scenario shown now.</param>
    /// <param name="clock">The scenario's daemon clock, for the end time.</param>
    /// <param name="ended">Takes the job out of the scenario's state, as allotd stops listing
    /// it.</param>
    public sealed class Stopper(Func<string> scenario, StateClock clock, Action<long> ended)
        : IJobStopper
    {
        private static readonly TimeSpan Delay = TimeSpan.FromMilliseconds(300);

        public async Task<StopResult> KillAsync(long lease, CancellationToken cancellationToken)
        {
            await Task.Delay(Delay, cancellationToken).ConfigureAwait(true);
            if (scenario() == Handoff.RefusedScenario)
            {
                return StopResult.Refused(Handoff.RefusedReason);
            }

            return End(lease);
        }

        public async Task<StopResult> RemoveAsync(long lease, CancellationToken cancellationToken)
        {
            await Task.Delay(Delay, cancellationToken).ConfigureAwait(true);
            return End(lease);
        }

        private StopResult End(long lease)
        {
            var at = clock.NowMs ?? Handoff.NowMs;
            ended(lease);
            return new StopResult(true, "", at);
        }
    }
}
#endif
