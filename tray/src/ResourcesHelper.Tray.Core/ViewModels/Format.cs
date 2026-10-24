using System.Globalization;

using ResourcesHelper.Tray.Core.Contract;

namespace ResourcesHelper.Tray.Core.ViewModels;

/// <summary>
/// Text shared by several view models. Numbers use the invariant culture: the UI is English, and
/// a decimal comma would read as a list separator next to the handoff's middle dots.
/// </summary>
public static class Format
{
    private const int ShortIdLength = 8;

    /// <summary>
    /// The 8-character prefix the flyout shows for a session UUID.
    /// </summary>
    public static string ShortId(string session) =>
        session.Length <= ShortIdLength ? session : session[..ShortIdLength];

    /// <summary>
    /// The project folder: the last segment of a working directory, or empty without one.
    /// </summary>
    public static string Project(string? cwd)
    {
        if (string.IsNullOrEmpty(cwd))
        {
            return "";
        }

        var trimmed = cwd.TrimEnd('\\', '/');
        var separator = trimmed.LastIndexOfAny(['\\', '/']);
        return separator < 0 ? trimmed : trimmed[(separator + 1)..];
    }

    /// <summary>
    /// Live usage as <c>7.8 cores · 2.9 GB</c>.
    /// </summary>
    public static string Usage(ResourceUsage usage) =>
        string.Create(
            CultureInfo.InvariantCulture,
            $"{usage.Cores:0.0} cores · {Gigabytes(usage.RamMb)} GB");

    /// <summary>
    /// Megabytes as gigabytes with one decimal, divided by 1024 as Task Manager does.
    /// </summary>
    public static string Gigabytes(ulong megabytes) =>
        (megabytes / 1024.0).ToString("0.0", CultureInfo.InvariantCulture);
}
