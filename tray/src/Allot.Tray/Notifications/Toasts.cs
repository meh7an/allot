using System.Runtime.InteropServices;

using Allot.Tray.Core.Notifications;

using Microsoft.UI.Dispatching;
using Microsoft.Windows.AppLifecycle;
using Microsoft.Windows.AppNotifications;
using Microsoft.Windows.AppNotifications.Builder;

namespace Allot.Tray.Notifications;

/// <summary>
/// What a click on a toast asks for.
/// </summary>
/// <param name="Action">The clicked button's action, or empty for a click on the toast
/// body.</param>
/// <param name="Target">The flyout row to scroll to, or <c>null</c>.</param>
internal sealed record ToastAction(string Action, string? Target);

/// <summary>
/// Registration, activation and showing of app notifications, which work unpackaged. The
/// rules for when a toast shows live in <see cref="ToastRules"/>.
/// </summary>
internal static class Toasts
{
    public const string StartService = "start-service";
    public const string OpenLogs = "open-logs";
    public const string Show = "show";

    private const string ActionKey = "action";
    private const string TargetKey = "target";

    // Every toast of the app, so a withdrawal names only ours.
    private const string Group = "allot";

    /// <param name="dispatcher">The UI thread's queue; clicks arrive on a background thread.</param>
    /// <param name="onAction">Runs on the UI thread with the click.</param>
    public static void Register(DispatcherQueue dispatcher, Action<ToastAction> onAction)
    {
        var manager = AppNotificationManager.Default;
        // Attached before Register, or a click starts a second process instead of reaching
        // this one.
        manager.NotificationInvoked += (source, args) =>
        {
            var action = ActionOf(args.Arguments);
            AppLog.Write($"toast invoked: action '{action.Action}', target '{action.Target}'");
            _ = dispatcher.TryEnqueue(() => onAction(action));
        };

        var icon = new Uri(Path.Combine(AppContext.BaseDirectory, "Assets", "AppIcon.ico"));
        try
        {
            manager.Register("Allot", icon);
        }
        catch (Exception e) when (e is COMException or IOException)
        {
            // Toasts are optional; the tray must keep working without them. A missing file
            // (0x80070002) arrives as FileNotFoundException, not COMException.
            AppLog.Write($"toast register failed: 0x{e.HResult:X8} {e.Message}");
        }
    }

    public static void Unregister() => AppNotificationManager.Default.Unregister();

    /// <param name="toast">The toast to show.</param>
    /// <param name="logo">The PNG of the tray icon's current state.</param>
    public static void Display(ToastMessage toast, string logo)
    {
        var builder = new AppNotificationBuilder()
            .SetAppLogoOverride(new Uri(logo))
            .AddText(toast.Title)
            .AddText(toast.Body);
        if (toast.Target is { } target)
        {
            _ = builder.AddArgument(TargetKey, target);
        }

        foreach (var button in toast.Buttons)
        {
            _ = builder.AddButton(ButtonFor(button, toast.Target));
        }

        var notification = builder.BuildNotification();
        notification.Tag = toast.Tag;
        notification.Group = Group;
        try
        {
            AppNotificationManager.Default.Show(notification);
            AppLog.Write($"toast shown: {toast.Kind} '{toast.Tag}'");
        }
        catch (COMException e)
        {
            AppLog.Write($"toast {toast.Kind} failed: 0x{e.HResult:X8} {e.Message}");
        }
    }

    /// <summary>
    /// Removes a shown toast from Notification Center, if it is still there.
    /// </summary>
    public static async Task WithdrawAsync(string tag)
    {
        try
        {
            await AppNotificationManager.Default.RemoveByTagAndGroupAsync(tag, Group);
            AppLog.Write($"toast withdrawn: '{tag}'");
        }
        catch (COMException e)
        {
            AppLog.Write($"toast withdraw '{tag}' failed: 0x{e.HResult:X8} {e.Message}");
        }
    }

    /// <returns>
    /// The click when a toast launched or redirected to this process, otherwise null.
    /// </returns>
    public static ToastAction? ActionOf(AppActivationArguments activation) =>
        activation.Data is AppNotificationActivatedEventArgs args ? ActionOf(args.Arguments) : null;

    private static ToastAction ActionOf(IDictionary<string, string> arguments) =>
        new(
            arguments.TryGetValue(ActionKey, out var action) ? action : string.Empty,
            arguments.TryGetValue(TargetKey, out var target) ? target : null);

    private static AppNotificationButton ButtonFor(ToastButton button, string? target)
    {
        var (text, action) = button switch
        {
            ToastButton.StartService => ("Start service", StartService),
            ToastButton.OpenLogs => ("Open logs", OpenLogs),
            ToastButton.Show => ("Show", Show),
            _ => throw new ArgumentOutOfRangeException(nameof(button)),
        };
        var result = new AppNotificationButton(text).AddArgument(ActionKey, action);
        return target is null ? result : result.AddArgument(TargetKey, target);
    }
}
