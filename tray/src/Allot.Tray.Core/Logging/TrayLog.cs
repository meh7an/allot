using System.Globalization;
using System.Text;

namespace Allot.Tray.Core.Logging;

/// <summary>
/// The tray app's log, <c>logs\tray.log</c> beside allotd's. Once the file reaches
/// <paramref name="maxBytes"/> it becomes <c>tray.log.1</c> (replacing the older one) and a new
/// file starts, so the log never takes more than twice that. Logging never throws: a log that
/// can't be written must not take the tray app down.
/// </summary>
/// <param name="path">The log file.</param>
/// <param name="maxBytes">The size at which the file rotates.</param>
/// <param name="time">The clock for the timestamps.</param>
public sealed class TrayLog(string path, long maxBytes, TimeProvider time)
{
    public const long DefaultMaxBytes = 1024 * 1024;

    private static readonly UTF8Encoding Utf8 = new(encoderShouldEmitUTF8Identifier: false);

    private readonly Lock _gate = new();

    public string FilePath { get; } = path;

    public string RotatedPath => FilePath + ".1";

    public static TrayLog In(string directory) =>
        new(Path.Combine(directory, "tray.log"), DefaultMaxBytes, TimeProvider.System);

    public void Write(string message)
    {
        var line = string.Create(
            CultureInfo.InvariantCulture,
            $"{time.GetLocalNow():yyyy-MM-dd HH:mm:ss.fff} {message}\n");
        lock (_gate)
        {
            try
            {
                RotateIfFull();
                if (Path.GetDirectoryName(FilePath) is { Length: > 0 } directory)
                {
                    _ = Directory.CreateDirectory(directory);
                }

                File.AppendAllText(FilePath, line, Utf8);
            }
            catch (Exception error) when (error is IOException or UnauthorizedAccessException)
            {
                System.Diagnostics.Debug.WriteLine($"tray log failed: {error.Message}: {line}");
            }
        }
    }

    private void RotateIfFull()
    {
        var file = new FileInfo(FilePath);
        if (file.Exists && file.Length >= maxBytes)
        {
            File.Move(FilePath, RotatedPath, overwrite: true);
        }
    }
}
