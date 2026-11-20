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
    /// The live sentence at the end of a group, a polite live region.
    /// </summary>
    public static TextBlock LiveSentence()
    {
        var text = Text(string.Empty, "SettingsLiveTextStyle");
        AutomationProperties.SetLiveSetting(
            text,
            AutomationLiveSetting.Polite);
        return text;
    }

    /// <summary>
    /// Sets a live sentence's text, announcing it once when it changed.
    /// </summary>
    public static void SetLive(TextBlock text, string value)
    {
        if (text.Text == value)
        {
            return;
        }

        text.Text = value;
        text.Visibility = Shows(value.Length > 0);
        Announce(text);
    }

    /// <summary>
    /// Tells screen readers that a live region's text changed.
    /// </summary>
    public static void Announce(FrameworkElement element) =>
        FrameworkElementAutomationPeer.FromElement(element)
            ?.RaiseAutomationEvent(AutomationEvents.LiveRegionChanged);

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
