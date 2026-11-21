using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Allot.Tray.Settings;

/// <summary>
/// Building blocks of the Settings window's code-built pages: cards, headings and text in the
/// styles of <c>SettingsStyles.xaml</c>.
/// </summary>
internal static class SettingsUi
{
    public static GridLength Star { get; } = new(1, GridUnitType.Star);

    public static FontFamily SymbolFont =>
        (FontFamily)Application.Current.Resources["SymbolThemeFontFamily"];

    public static Style Style(string key) => (Style)Application.Current.Resources[key];

    public static Visibility Shows(bool shown) => shown ? Visibility.Visible : Visibility.Collapsed;

    /// <summary>
    /// Hides an icon from screen readers, where text next to it already says what it means.
    /// </summary>
    public static T Decorative<T>(T element)
        where T : UIElement
    {
        AutomationProperties.SetAccessibilityView(element, AccessibilityView.Raw);
        return element;
    }

    public static TextBlock Text(string text, string style) =>
        new() { Text = text, Style = Style(style), TextWrapping = TextWrapping.WrapWholeWords };

    /// <summary>
    /// A group's heading above its card.
    /// </summary>
    public static TextBlock Heading(string text)
    {
        var heading = Text(text, "SettingsGroupHeadingStyle");
        AutomationProperties.SetHeadingLevel(
            heading,
            AutomationHeadingLevel.Level2);
        return heading;
    }

    /// <summary>
    /// A card holding <paramref name="rows"/>, with a separator between rows.
    /// </summary>
    public static Border Card(IEnumerable<UIElement> rows)
    {
        var panel = new StackPanel();
        foreach (var row in rows)
        {
            if (panel.Children.Count > 0)
            {
                panel.Children.Add(new Border { Style = Style("SettingsSeparatorStyle") });
            }

            panel.Children.Add(row);
        }

        return new Border { Style = Style("SettingsCardStyle"), Child = panel };
    }

    /// <summary>
    /// The live sentence at the end of a group. Screen readers read it where it stands; it is not
    /// announced when it changes (Mehran at M7.1: only errors and outcomes are).
    /// </summary>
    public static TextBlock LiveSentence() => Text(string.Empty, "SettingsLiveTextStyle");

    /// <summary>
    /// Sets a live sentence's text; it hides while empty.
    /// </summary>
    /// <returns>Whether the text changed.</returns>
    public static bool SetLive(TextBlock text, string value)
    {
        if (text.Text == value)
        {
            return false;
        }

        text.Text = value;
        text.Visibility = Shows(value.Length > 0);
        return true;
    }

    /// <summary>
    /// Speaks <paramref name="text"/> to screen readers without moving focus. A newer text from
    /// the same window replaces one not yet spoken.
    /// </summary>
    /// <param name="element">Any element of the window, which raises the notification.</param>
    /// <param name="text">What to say.</param>
    public static void Announce(FrameworkElement element, string text)
    {
        var peer = FrameworkElementAutomationPeer.FromElement(element)
            ?? FrameworkElementAutomationPeer.CreatePeerForElement(element);
        peer?.RaiseNotificationEvent(
            AutomationNotificationKind.ActionCompleted,
            AutomationNotificationProcessing.ImportantMostRecent,
            text,
            "AllotSettings");
    }

    /// <summary>
    /// Code text, such as a TOML key, a program or a flag.
    /// </summary>
    public static FontFamily Mono { get; } = new("Cascadia Mono, Consolas");

    /// <summary>
    /// A borderless button showing a Segoe Fluent glyph, such as reset or delete.
    /// </summary>
    /// <param name="glyph">The glyph's character.</param>
    /// <param name="name">The accessible name and tooltip, such as "Delete preset render".</param>
    /// <param name="onClick">What it does.</param>
    /// <param name="size">The button's width and height.</param>
    public static Button IconButton(string glyph, string name, Action onClick, double size = 32)
    {
        var button = new Button
        {
            Width = size,
            Height = size,
            Padding = new Thickness(0),
            VerticalAlignment = VerticalAlignment.Center,
            Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent),
            BorderThickness = new Thickness(0),
            Content = new FontIcon
            {
                Glyph = glyph,
                FontSize = size > 24 ? 14 : 10,
                IsTextScaleFactorEnabled = false,
                FontFamily = SymbolFont,
            },
        };
        AutomationProperties.SetName(button, name);
        ToolTipService.SetToolTip(button, name);
        button.Click += (_, _) => onClick();
        return button;
    }

    /// <summary>
    /// A button; the first of a group uses the accent style.
    /// </summary>
    public static Button Button(string text, Action onClick, bool accent = false)
    {
        var button = new Button { Content = text };
        if (accent)
        {
            button.Style = Style("AccentButtonStyle");
        }

        button.Click += (_, _) => onClick();
        return button;
    }
}
