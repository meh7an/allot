using System.Text.Json;
using System.Text.Json.Serialization;

namespace SettingsSpike;

/// <summary>
/// Where the Settings window was: the top-left corner in physical pixels (screen coordinates)
/// and the size in DIPs, so a restore onto a monitor with another scale keeps the same size on
/// screen. The rect is the restored one even while maximized.
/// </summary>
internal sealed record SavedPlacement
{
    public int X { get; set; }

    public int Y { get; set; }

    public double WidthDip { get; set; }

    public double HeightDip { get; set; }

    public bool Maximized { get; set; }
}

[JsonSourceGenerationOptions(WriteIndented = true)]
[JsonSerializable(typeof(SavedPlacement))]
internal sealed partial class PlacementJson : JsonSerializerContext
{
}

/// <summary>
/// <c>%LOCALAPPDATA%\AllotSettingsSpike\window.json</c>; the real tray's <c>tray.json</c> is never
/// read or written.
/// </summary>
internal static class PlacementStore
{
    private static readonly string Path = System.IO.Path.Combine(SpikeLog.Folder, "window.json");

    public static SavedPlacement? Load()
    {
        try
        {
            using var stream = File.OpenRead(Path);
            return JsonSerializer.Deserialize(stream, PlacementJson.Default.SavedPlacement);
        }
        catch (Exception error) when (error is IOException or JsonException)
        {
            return null;
        }
    }

    public static void Save(SavedPlacement placement)
    {
        Directory.CreateDirectory(SpikeLog.Folder);
        File.WriteAllText(Path, JsonSerializer.Serialize(placement, PlacementJson.Default.SavedPlacement));
    }

    public static void Forget() => File.Delete(Path);
}
