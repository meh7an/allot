using System.Text.Json;

namespace ResourcesHelper.Tray.Core.Settings;

/// <summary>
/// Reads and writes <c>tray.json</c>. A missing or unreadable file gives the defaults, and the
/// next save replaces it; members the file lacks keep their defaults, and unknown ones are
/// ignored, so older and newer builds can share the file.
/// </summary>
public sealed class SettingsStore(string path)
{
    public string FilePath { get; } = path;

    public TraySettings Load()
    {
        try
        {
            using var stream = File.OpenRead(FilePath);
            return JsonSerializer.Deserialize(stream, SettingsJsonContext.Default.TraySettings)
                ?? new TraySettings();
        }
        catch (Exception error) when (error is IOException
            or UnauthorizedAccessException
            or JsonException)
        {
            return new TraySettings();
        }
    }

    /// <summary>
    /// Writes a temporary file beside <see cref="FilePath"/> and renames it over the old one, so a
    /// crash mid-write never leaves half a file.
    /// </summary>
    public void Save(TraySettings settings)
    {
        if (Path.GetDirectoryName(FilePath) is { Length: > 0 } directory)
        {
            _ = Directory.CreateDirectory(directory);
        }

        var temporary = FilePath + ".tmp";
        using (var stream = File.Create(temporary))
        {
            JsonSerializer.Serialize(stream, settings, SettingsJsonContext.Default.TraySettings);
            stream.WriteByte((byte)'\n');
        }

        File.Move(temporary, FilePath, overwrite: true);
    }
}
