using Allot.Tray.Core;
using Allot.Tray.Core.Startup;
using Allot.Tray.Interop;
using Allot.Tray.Startup;

using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.Windows.AppLifecycle;

namespace Allot.Tray;

public static class Program
{
    [STAThread]
    private static void Main()
    {
        WinRT.ComWrappersSupport.InitializeComWrappers();

        var command = TrayCommand.Parse(Environment.GetCommandLineArgs()[1..]);
        switch (command.Action)
        {
            case TrayAction.SetLaunchAtLogon:
                Environment.ExitCode = SetLaunchAtLogon(command.LaunchAtLogon);
                return;
            case TrayAction.Quit:
                // The running app reads the switch from the handed-over launch and quits; with
                // none running there is nothing to do.
                _ = RedirectToRunningInstance();
                return;
            case TrayAction.Invalid:
                Environment.ExitCode = TrayCommand.UsageExitCode;
                return;
            case TrayAction.Run:
                break;
        }

#if DEBUG
        // A Debug build showing a handoff scenario runs beside the real tray app.
        if (DebugScenario.Name is null && RedirectToRunningInstance())
        {
            return;
        }
#else
        if (RedirectToRunningInstance())
        {
            return;
        }
#endif

        Application.Start(p =>
        {
            var context = new DispatcherQueueSynchronizationContext(
                DispatcherQueue.GetForCurrentThread());
            SynchronizationContext.SetSynchronizationContext(context);
            _ = new App();
        });
    }

    /// <summary>
    /// Writes or deletes the Run entry for this exe, the way the first-run card and the tray
    /// menu do; <c>allot install</c> and <c>allot uninstall</c> call it.
    /// </summary>
    /// <returns>The exit code: 0, or 1 when the registry refused.</returns>
    private static int SetLaunchAtLogon(bool on)
    {
        try
        {
            new Autostart(new RegistryAutostartStore(), Environment.ProcessPath ?? "").Set(on);
            return 0;
        }
        catch (Exception error) when (error is UnauthorizedAccessException or IOException
            or System.Security.SecurityException)
        {
            return 1;
        }
    }

    /// <summary>
    /// Hands this launch to the instance that owns the key, if another one does.
    /// </summary>
    /// <returns>True when this process should exit.</returns>
    private static bool RedirectToRunningInstance()
    {
        var owner = AppInstance.FindOrRegisterForKey(TrayIdentity.InstanceKey);
        if (owner.IsCurrent)
        {
            return false;
        }

        // This launch holds the foreground right; without passing it on, the running instance
        // can't bring its flyout to the front.
        _ = NativeMethods.AllowSetForegroundWindow(owner.ProcessId);
        var args = AppInstance.GetCurrent().GetActivatedEventArgs();
        // Awaiting the redirect on the STA main thread can deadlock; a pool thread does the
        // call while this blocking wait keeps pumping COM.
        Task.Run(() => owner.RedirectActivationToAsync(args).AsTask()).Wait();
        return true;
    }
}
