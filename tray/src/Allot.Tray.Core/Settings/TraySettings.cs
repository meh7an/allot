using System.Text.Json;
using System.Text.Json.Serialization;

namespace Allot.Tray.Core.Settings;

// Every member is { get; set; } with its default: the System.Text.Json source generator sets
// init-only members in its object initializer even when the JSON lacks them, which would turn a
// missing toggle into false instead of its default.

/// <summary>
/// What the tray app remembers in <c>tray.json</c>. Launch at logon is not here: it lives in the
/// autostart entry itself.
/// </summary>
public sealed record TraySettings
{
    public MeterToggles Meters { get; set; } = new();

    public NotificationToggles Notifications { get; set; } = new();

    /// <summary>
    /// Whether the Resources card shows its meter rows; collapsed shows the summary line.
    /// </summary>
    public bool ResourcesExpanded { get; set; }

    /// <summary>
    /// The first-run card was dismissed with Done.
    /// </summary>
    public bool FirstRunDone { get; set; }

    /// <summary>
    /// The first-run card has shown once: the flyout opened by itself and Launch at logon was
    /// turned on, which neither does again.
    /// </summary>
    public bool FirstRunShown { get; set; }
}

/// <summary>
/// The ⋯ › Meters check items; all on by default.
/// </summary>
public sealed record MeterToggles
{
    public bool Cpu { get; set; } = true;

    public bool Ram { get; set; } = true;

    public bool Gpu { get; set; } = true;

    public bool Vram { get; set; } = true;

    public bool Drives { get; set; } = true;

    /// <summary>
    /// Read and write rates on the drive rows; disabled in the menu while Drives is off.
    /// </summary>
    public bool DriveActivity { get; set; } = true;
}

/// <summary>
/// The ⋯ › Notifications check items.
/// </summary>
public sealed record NotificationToggles
{
    public bool ServiceStopped { get; set; } = true;

    public bool RunningLong { get; set; } = true;

    public bool BenchmarkFinished { get; set; } = true;

    public bool JobFinished { get; set; }

    public bool KilledOutsideFlyout { get; set; }
}

[JsonSourceGenerationOptions(
    PropertyNamingPolicy = JsonKnownNamingPolicy.SnakeCaseLower,
    WriteIndented = true,
    IndentSize = 2,
    NewLine = "\n",
    ReadCommentHandling = JsonCommentHandling.Skip,
    AllowTrailingCommas = true,
    RespectNullableAnnotations = true)]
[JsonSerializable(typeof(TraySettings))]
internal sealed partial class SettingsJsonContext : JsonSerializerContext
{
}
