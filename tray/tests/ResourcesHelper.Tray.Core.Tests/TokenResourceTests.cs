using System.Globalization;
using System.Xml.Linq;

namespace ResourcesHelper.Tray.Core.Tests;

/// <summary>
/// Checks the app's <c>Themes/Tokens.xaml</c> against the values written in
/// <c>plan/design handoff/README.md</c>, in the CSS notation the handoff uses.
/// </summary>
[TestClass]
public sealed class TokenResourceTests
{
    private static readonly XNamespace Xaml =
        "http://schemas.microsoft.com/winfx/2006/xaml/presentation";
    private static readonly XNamespace X = "http://schemas.microsoft.com/winfx/2006/xaml";

    private static readonly string[] Themes = ["Light", "Dark", "HighContrast"];

    // Token, light, dark. From the "Design tokens" table and the values the sections give.
    private static readonly (string Key, string Light, string Dark)[] Handoff =
    [
        ("RhSurfaceBrush", "rgba(243,243,243,.86)", "rgba(36,36,36,.86)"),
        ("RhSurfaceEdgeBrush", "rgba(0,0,0,.09)", "rgba(255,255,255,.08)"),
        ("RhTextBrush", "#1a1a1a", "#ffffff"),
        ("RhTextSecondaryBrush", "#5c5c5c", "#c5c5c5"),
        ("RhTextStripBrush", "#4a4a4a", "#c5c5c5"),
        ("RhTextSubtleBrush", "#8a8a8a", "#9a9a9a"),
        ("RhCardBrush", "rgba(255,255,255,.72)", "rgba(255,255,255,.055)"),
        ("RhCardBorderBrush", "rgba(0,0,0,.14)", "rgba(0,0,0,.40)"),
        ("RhStripBrush", "rgba(0,0,0,.075)", "rgba(255,255,255,.09)"),
        ("RhSeparatorBrush", "rgba(0,0,0,.08)", "rgba(255,255,255,.08)"),
        ("RhChipBackgroundBrush", "rgba(0,0,0,.07)", "rgba(255,255,255,.10)"),
        ("RhChipForegroundBrush", "#3a3a3a", "#dedede"),
        ("RhTileBackgroundBrush", "rgba(0,0,0,.06)", "rgba(255,255,255,.07)"),
        ("RhQueuedTileBorderBrush", "rgba(0,0,0,.3)", "rgba(255,255,255,.3)"),
        ("RhTrackBrush", "rgba(0,0,0,.07)", "rgba(255,255,255,.08)"),
        ("RhAccentForegroundBrush", "#ffffff", "#000000"),
        ("RhAccentBusyBrush", "#7a9cc4", "#3f7f9e"),
        ("RhRunningBrush", "#0f7b0f", "#6ccb5f"),
        ("RhWaitingBrush", "#9d5d00", "#fce100"),
        ("RhWaitingSoftBrush", "rgba(157,93,0,.18)", "rgba(252,225,0,.2)"),
        ("RhBenchmarkBrush", "#6f3fb0", "#c9a8f0"),
        ("RhBenchmarkForegroundBrush", "#ffffff", "#1a1a1a"),
        ("RhBenchmarkSoftBrush", "rgba(111,63,176,.35)", "rgba(201,168,240,.35)"),
        ("RhStepUpcomingBrush", "rgba(0,0,0,.1)", "rgba(255,255,255,.12)"),
        ("RhDangerTextBrush", "#c42b1c", "#ff99a4"),
        ("RhDangerFillBrush", "#c42b1c", "#c42b1c"),
        ("RhDangerFillHoverBrush", "#a32317", "#a32317"),
        ("RhDangerForegroundBrush", "#ffffff", "#ffffff"),
        ("RhKillHoverBrush", "rgba(196,43,28,.14)", "rgba(196,43,28,.14)"),
        ("RhStoppedBrush", "#8a8a8a", "#8a8a8a"),
        ("RhPressedBrush", "rgba(0,0,0,.06)", "rgba(255,255,255,.08)"),
        ("RhMenuBackgroundBrush", "rgba(249,249,249,.97)", "rgba(44,44,44,.97)"),
        ("RhMenuEdgeBrush", "rgba(0,0,0,.1)", "rgba(255,255,255,.1)"),
        ("RhMenuHoverBrush", "rgba(128,128,128,.14)", "rgba(128,128,128,.14)"),
        ("RhMenuDisabledBrush", "#a0a0a0", "#6a6a6a"),
        ("RhHatchStrongBrush", "rgba(0,0,0,.2)", "rgba(255,255,255,.22)"),
        ("RhHatchWeakBrush", "rgba(0,0,0,.05)", "rgba(255,255,255,.05)"),
        ("RhMarkerBrush", "#1a1a1a", "#ffffff"),
        ("RhMarkerRingBrush", "rgba(255,255,255,.8)", "rgba(0,0,0,.6)"),
        ("RhDriveBrush", "#6b6b6b", "#a8a8a8"),
        ("RhBannerBackgroundBrush", "rgba(196,43,28,.08)", "rgba(255,153,164,.1)"),
        ("RhBannerBorderBrush", "rgba(196,43,28,.3)", "rgba(255,153,164,.35)"),
        ("RhBannerForegroundBrush", "#8a1e14", "#ffb3bb"),
        ("RhTrayAmberBrush", "#c27a00", "#fce100"),
        ("RhTrayRedBrush", "#c42b1c", "#ff6b6b"),
    ];

    private static readonly Lazy<XElement> Root = new(() => XDocument.Load(
        Path.Combine(AppContext.BaseDirectory, "Themes", "Tokens.xaml")).Root!);

    public static IEnumerable<object[]> HandoffTokens =>
        Handoff.Select(token => new object[] { token.Key, token.Light, token.Dark });

    [TestMethod]
    [DynamicData(nameof(HandoffTokens))]
    public void BrushHasTheHandoffColor(string key, string light, string dark)
    {
        Assert.AreEqual(ToXamlColor(light), BrushColor("Light", key), $"{key} (light)");
        Assert.AreEqual(ToXamlColor(dark), BrushColor("Dark", key), $"{key} (dark)");
    }

    [TestMethod]
    public void AccentFollowsTheSystemAccent()
    {
        Assert.AreEqual("{ThemeResource SystemAccentColor}", BrushColor("Light", "RhAccentBrush"));
        Assert.AreEqual(
            "{ThemeResource SystemAccentColorLight2}",
            BrushColor("Dark", "RhAccentBrush"));
        Assert.AreEqual(
            "{ThemeResource SystemColorHighlightColor}",
            BrushColor("HighContrast", "RhAccentBrush"));
    }

    [TestMethod]
    public void EveryThemeDefinesTheSameBrushes()
    {
        var keys = Themes.Select(theme => Keys(theme).Order(StringComparer.Ordinal).ToArray())
            .ToList();

        CollectionAssert.AreEqual(keys[0], keys[1], "Light and Dark differ");
        CollectionAssert.AreEqual(keys[0], keys[2], "Light and HighContrast differ");
    }

    [TestMethod]
    public void EveryHandoffTokenIsDefined()
    {
        var expected = Handoff.Select(token => token.Key).Append("RhAccentBrush")
            .Order(StringComparer.Ordinal)
            .ToArray();

        CollectionAssert.AreEqual(expected, Keys("Light").Order(StringComparer.Ordinal).ToArray());
    }

    [TestMethod]
    public void HighContrastUsesOnlySystemColors()
    {
        foreach (var key in Keys("HighContrast"))
        {
            StringAssert.StartsWith(
                BrushColor("HighContrast", key),
                "{ThemeResource SystemColor",
                key);
        }
    }

    [TestMethod]
    public void IconGlyphsArePrivateUseCodepoints()
    {
        var glyphs = Root.Value.Elements(X + "String")
            .Where(element => ((string?)element.Attribute(X + "Key"))?.StartsWith(
                "RhGlyph",
                StringComparison.Ordinal) == true)
            .ToList();

        Assert.HasCount(21, glyphs);
        foreach (var glyph in glyphs)
        {
            var value = glyph.Value;
            Assert.AreEqual(1, value.Length, (string?)glyph.Attribute(X + "Key"));
            Assert.IsTrue(value[0] is >= '' and <= '', value);
        }
    }

    private static IEnumerable<string> Keys(string theme) =>
        Theme(theme).Elements(Xaml + "SolidColorBrush")
            .Select(brush => (string)brush.Attribute(X + "Key")!);

    private static string BrushColor(string theme, string key) =>
        Theme(theme).Elements(Xaml + "SolidColorBrush")
            .Where(brush => (string?)brush.Attribute(X + "Key") == key)
            .Select(brush => (string)brush.Attribute("Color")!)
            .SingleOrDefault() ?? $"<{key} missing>";

    private static XElement Theme(string name) =>
        Root.Value.Element(Xaml + "ResourceDictionary.ThemeDictionaries")!
            .Elements(Xaml + "ResourceDictionary")
            .Single(dictionary => (string?)dictionary.Attribute(X + "Key") == name);

    // #rrggbb or rgba(r,g,b,a) to XAML's #AARRGGBB, alpha rounded half away from zero.
    private static string ToXamlColor(string css)
    {
        if (css.StartsWith('#'))
        {
            return "#FF" + css[1..].ToUpperInvariant();
        }

        var parts = css["rgba(".Length..^1].Split(',');
        var alpha = (int)Math.Round(
            double.Parse(parts[3], CultureInfo.InvariantCulture) * 255,
            MidpointRounding.AwayFromZero);
        var rgb = parts[..3].Select(part => int.Parse(part, CultureInfo.InvariantCulture));
        return "#" + string.Concat(new[] { alpha }.Concat(rgb).Select(value =>
            value.ToString("X2", CultureInfo.InvariantCulture)));
    }
}
