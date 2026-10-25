using System.Globalization;
using System.Xml.Linq;

using Allot.Tray.Core.Tests.Support;

namespace Allot.Tray.Core.Tests;

/// <summary>
/// Checks the app's <c>Themes/Tokens.xaml</c> against the values written in the design handoff,
/// in the CSS notation the handoff uses.
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
        ("AllotSurfaceBrush", "rgba(243,243,243,.86)", "rgba(36,36,36,.86)"),
        ("AllotSurfaceEdgeBrush", "rgba(0,0,0,.09)", "rgba(255,255,255,.08)"),
        ("AllotTextBrush", "#1a1a1a", "#ffffff"),
        ("AllotTextSecondaryBrush", "#5c5c5c", "#c5c5c5"),
        ("AllotTextStripBrush", "#4a4a4a", "#c5c5c5"),
        ("AllotTextSubtleBrush", "#8a8a8a", "#9a9a9a"),
        ("AllotCardBrush", "rgba(255,255,255,.72)", "rgba(255,255,255,.055)"),
        ("AllotCardBorderBrush", "rgba(0,0,0,.14)", "rgba(0,0,0,.40)"),
        ("AllotStripBrush", "rgba(0,0,0,.075)", "rgba(255,255,255,.09)"),
        ("AllotSeparatorBrush", "rgba(0,0,0,.08)", "rgba(255,255,255,.08)"),
        ("AllotChipBackgroundBrush", "rgba(0,0,0,.07)", "rgba(255,255,255,.10)"),
        ("AllotChipForegroundBrush", "#3a3a3a", "#dedede"),
        ("AllotTileBackgroundBrush", "rgba(0,0,0,.06)", "rgba(255,255,255,.07)"),
        ("AllotQueuedTileBorderBrush", "rgba(0,0,0,.3)", "rgba(255,255,255,.3)"),
        ("AllotTrackBrush", "rgba(0,0,0,.07)", "rgba(255,255,255,.08)"),
        ("AllotAccentForegroundBrush", "#ffffff", "#000000"),
        ("AllotAccentBusyBrush", "#7a9cc4", "#3f7f9e"),
        ("AllotRunningBrush", "#0f7b0f", "#6ccb5f"),
        ("AllotWaitingBrush", "#9d5d00", "#fce100"),
        ("AllotWaitingSoftBrush", "rgba(157,93,0,.18)", "rgba(252,225,0,.2)"),
        ("AllotBenchmarkBrush", "#6f3fb0", "#c9a8f0"),
        ("AllotBenchmarkForegroundBrush", "#ffffff", "#1a1a1a"),
        ("AllotBenchmarkSoftBrush", "rgba(111,63,176,.35)", "rgba(201,168,240,.35)"),
        ("AllotStepUpcomingBrush", "rgba(0,0,0,.1)", "rgba(255,255,255,.12)"),
        ("AllotDangerTextBrush", "#c42b1c", "#ff99a4"),
        ("AllotDangerFillBrush", "#c42b1c", "#c42b1c"),
        ("AllotDangerFillHoverBrush", "#a32317", "#a32317"),
        ("AllotDangerForegroundBrush", "#ffffff", "#ffffff"),
        ("AllotKillHoverBrush", "rgba(196,43,28,.14)", "rgba(196,43,28,.14)"),
        ("AllotStoppedBrush", "#8a8a8a", "#8a8a8a"),
        ("AllotPressedBrush", "rgba(0,0,0,.06)", "rgba(255,255,255,.08)"),
        ("AllotMenuBackgroundBrush", "rgba(249,249,249,.97)", "rgba(44,44,44,.97)"),
        ("AllotMenuEdgeBrush", "rgba(0,0,0,.1)", "rgba(255,255,255,.1)"),
        ("AllotMenuHoverBrush", "rgba(128,128,128,.14)", "rgba(128,128,128,.14)"),
        ("AllotMenuDisabledBrush", "#a0a0a0", "#6a6a6a"),
        ("AllotHatchStrongBrush", "rgba(0,0,0,.2)", "rgba(255,255,255,.22)"),
        ("AllotHatchWeakBrush", "rgba(0,0,0,.05)", "rgba(255,255,255,.05)"),
        ("AllotMarkerBrush", "#1a1a1a", "#ffffff"),
        ("AllotMarkerRingBrush", "rgba(255,255,255,.8)", "rgba(0,0,0,.6)"),
        ("AllotDriveBrush", "#6b6b6b", "#a8a8a8"),
        ("AllotBannerBackgroundBrush", "rgba(196,43,28,.08)", "rgba(255,153,164,.1)"),
        ("AllotBannerBorderBrush", "rgba(196,43,28,.3)", "rgba(255,153,164,.35)"),
        ("AllotBannerForegroundBrush", "#8a1e14", "#ffb3bb"),
        ("AllotTrayAmberBrush", "#c27a00", "#fce100"),
        ("AllotTrayRedBrush", "#c42b1c", "#ff6b6b"),
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
        Assert.AreEqual("{ThemeResource SystemAccentColor}", BrushColor("Light", "AllotAccentBrush"));
        Assert.AreEqual(
            "{ThemeResource SystemAccentColorLight2}",
            BrushColor("Dark", "AllotAccentBrush"));
        Assert.AreEqual(
            "{ThemeResource SystemColorHighlightColor}",
            BrushColor("HighContrast", "AllotAccentBrush"));
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
        var expected = Handoff.Select(token => token.Key).Append("AllotAccentBrush")
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
                "AllotGlyph",
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

    // The committed fonts are subsets holding only these glyphs; a glyph added here
    // needs the subset cut again: fontTools' pyftsubset with --unicodes set to every AllotGlyph
    // codepoint, run on the full fonts from microsoft/fluentui-system-icons.
    [TestMethod]
    [DataRow("FluentSystemIcons-Regular.ttf")]
    [DataRow("FluentSystemIcons-Filled.ttf")]
    public void SubsetIconFontsHoldEveryGlyph(string font)
    {
        var path = Path.Combine(AppContext.BaseDirectory, "Fonts", font);
        var glyphs = Root.Value.Elements(X + "String")
            .Where(element => ((string?)element.Attribute(X + "Key"))?.StartsWith(
                "AllotGlyph",
                StringComparison.Ordinal) == true)
            .Select(element => element.Value[0])
            .Distinct()
            .ToList();

        foreach (var glyph in glyphs)
        {
            Assert.IsTrue(FontCmap.Maps(path, glyph), $"{font} lacks U+{(int)glyph:X4}");
        }

        // The full fonts are megabytes; the subsets a few kilobytes.
        Assert.IsLessThan(64 * 1024, new FileInfo(path).Length, $"{font} is not the subset");
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
