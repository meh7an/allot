using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.Windows.AppLifecycle;

namespace SettingsSpike;

/// <summary>
/// Single instance under its own key, so the spike never hands off to the real
/// <c>allot-tray.exe</c>. A second launch opens Settings in the running spike, as a future
/// <c>allot settings</c> or a toast button would; <c>--launcher</c> shows the stand-in flyout
/// instead, and <c>--no-grant</c> skips AllowSetForegroundWindow to see what happens without it.
/// </summary>
public static class Program
{
    public const string InstanceKey = "Allot.SettingsSpike";

    [STAThread]
    private static void Main()
    {
        WinRT.ComWrappersSupport.InitializeComWrappers();
        if (RedirectToRunningInstance())
        {
            return;
        }

        Application.Start(p =>
        {
            var context = new DispatcherQueueSynchronizationContext(
                DispatcherQueue.GetForCurrentThread());
            SynchronizationContext.SetSynchronizationContext(context);
            _ = new App();
        });
    }

    private static bool RedirectToRunningInstance()
    {
        var owner = AppInstance.FindOrRegisterForKey(InstanceKey);
        if (owner.IsCurrent)
        {
            return false;
        }

        if (!Environment.GetCommandLineArgs().Contains("--no-grant"))
        {
            var granted = NativeMethods.AllowSetForegroundWindow(owner.ProcessId);
            SpikeLog.Write($"second launch: AllowSetForegroundWindow({owner.ProcessId}) = {granted}");
        }

        var args = AppInstance.GetCurrent().GetActivatedEventArgs();
        // Awaiting the redirect on the STA main thread can deadlock (as in the tray).
        Task.Run(() => owner.RedirectActivationToAsync(args).AsTask()).Wait();
        return true;
    }
}
