namespace SettingsSpike;

/// <summary>
/// Appends to <c>%LOCALAPPDATA%\AllotSettingsSpike\spike.log</c>, away from <c>~\.allot</c>, and
/// tells the launcher, which shows the last lines.
/// </summary>
internal static class SpikeLog
{
    public static string Folder { get; } = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
        "AllotSettingsSpike");

    public static string File { get; } = Path.Combine(Folder, "spike.log");

    /// <summary>
    /// Raised on the writing thread with the stamped line.
    /// </summary>
    public static event Action<string>? Written;

    public static void Write(string message)
    {
        var line = $"{DateTime.Now:HH:mm:ss.fff} [{Environment.ProcessId}] {message}";
        try
        {
            Directory.CreateDirectory(Folder);
            System.IO.File.AppendAllText(File, line + "\n");
        }
        catch (IOException)
        {
            // A second spike process may hold the file for a moment; the line still shows.
        }

        Written?.Invoke(line);
    }
}
