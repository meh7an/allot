using System.Runtime.InteropServices;

using Microsoft.UI.Dispatching;
using Microsoft.Windows.AppLifecycle;
using Microsoft.Windows.AppNotifications;

namespace ResourcesHelper.Tray.Notifications;

/// <summary>
/// Registration and activation for app notifications, which work unpackaged.
/// </summary>
internal static class Toasts
{
    public const string StartService = "start-service";
    public const string OpenLogs = "open-logs";

    private const string ActionKey = "action";

    /// <param name="dispatcher">The UI thread's queue; clicks arrive on a background thread.</param>
    /// <param name="onAction">
    /// Runs on the UI thread with the clicked button's action, or an empty string for a click on
    /// the toast body.
    /// </param>
    public static void Register(DispatcherQueue dispatcher, Action<string> onAction)
    {
        var manager = AppNotificationManager.Default;
        // Attached before Register, or a click starts a second process instead of reaching
        // this one.
        manager.NotificationInvoked += (source, args) =>
        {
            var action = ActionOf(args.Arguments);
            AppLog.Write($"toast invoked: action '{action}'");
            _ = dispatcher.TryEnqueue(() => onAction(action));
        };

        var icon = new Uri(Path.Combine(AppContext.BaseDirectory, "Assets", "AppIcon.ico"));
        try
        {
            manager.Register("Resources Helper", icon);
        }
        catch (COMException e)
        {
            // Toasts are optional; the tray must keep working without them.
            AppLog.Write($"toast register failed: 0x{e.HResult:X8} {e.Message}");
        }
    }

    public static void Unregister() => AppNotificationManager.Default.Unregister();

    /// <returns>
    /// The action when a toast click launched or redirected to this process, otherwise null.
    /// </returns>
    public static string? ActionOf(AppActivationArguments activation) =>
        activation.Data is AppNotificationActivatedEventArgs args ? ActionOf(args.Arguments) : null;

    private static string ActionOf(IDictionary<string, string> arguments) =>
        arguments.TryGetValue(ActionKey, out var action) ? action : string.Empty;
}
