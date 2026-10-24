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
    private const int InitialsLength = 2;

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

    /// <summary>
    /// A reservation as <c>8 cores · 4 GB</c> or <c>2 cores · 4 GB · GPU 80% · 4 GB VRAM</c>.
    /// Cores always show; RAM, GPU and VRAM only when reserved.
    /// </summary>
    public static string Reservation(JobProfile profile)
    {
        var parts = new List<string>(4) { Cores(profile.Cores) };
        if (profile.RamMb > 0)
        {
            parts.Add($"{Size(profile.RamMb)} GB");
        }

        if (profile.GpuPct > 0)
        {
            parts.Add(string.Create(CultureInfo.InvariantCulture, $"GPU {profile.GpuPct:0}%"));
        }

        if (profile.VramMb > 0)
        {
            parts.Add($"{Size(profile.VramMb)} GB VRAM");
        }

        return string.Join(" · ", parts);
    }

    /// <summary>
    /// <c>1 core</c>, <c>8 cores</c>, <c>2.5 cores</c>.
    /// </summary>
    public static string Cores(float cores)
    {
        var number = cores.ToString("0.#", CultureInfo.InvariantCulture);
        return number == "1" ? "1 core" : $"{number} cores";
    }

    /// <summary>
    /// A duration as <c>3:06</c>, or <c>1:02:03</c> from one hour.
    /// </summary>
    public static string Duration(long seconds)
    {
        seconds = Math.Max(0, seconds);
        var hours = seconds / 3600;
        var minutes = seconds / 60 % 60;
        var rest = seconds % 60;
        return hours > 0
            ? string.Create(CultureInfo.InvariantCulture, $"{hours}:{minutes:00}:{rest:00}")
            : string.Create(CultureInfo.InvariantCulture, $"{seconds / 60}:{rest:00}");
    }

    /// <summary>
    /// A rough wait as <c>2 min</c> or <c>1 h 20 min</c>, rounded to whole minutes and never
    /// less than one.
    /// </summary>
    public static string Minutes(ulong seconds)
    {
        var minutes = Math.Max(1, (long)Math.Round(seconds / 60.0, MidpointRounding.AwayFromZero));
        if (minutes < 60)
        {
            return $"{minutes} min";
        }

        var hours = minutes / 60;
        var rest = minutes % 60;
        return rest == 0 ? $"{hours} h" : $"{hours} h {rest} min";
    }

    /// <summary>
    /// How long ago something ended: <c>just now</c>, <c>4 min ago</c>, <c>2 h ago</c>,
    /// <c>3 d ago</c>.
    /// </summary>
    public static string Ago(long seconds) => seconds switch
    {
        < 60 => "just now",
        < 3600 => $"{seconds / 60} min ago",
        < 86_400 => $"{seconds / 3600} h ago",
        _ => $"{seconds / 86_400} d ago",
    };

    /// <summary>
    /// The two-letter placeholder for a process tile: <c>cargo.exe</c> gives <c>CA</c>. Paths,
    /// quotes and the <c>.exe</c> suffix are ignored; empty when nothing is left.
    /// </summary>
    public static string Initials(string program)
    {
        var name = program.Trim().Trim('"');
        var separator = name.LastIndexOfAny(['\\', '/']);
        if (separator >= 0)
        {
            name = name[(separator + 1)..];
        }

        if (name.EndsWith(".exe", StringComparison.OrdinalIgnoreCase))
        {
            name = name[..^4];
        }

        var letters = name.Where(char.IsLetterOrDigit).Take(InitialsLength).ToArray();
        return new string(letters).ToUpperInvariant();
    }

    // Whole gigabytes without decimals, others with one: 4 GB, 1.5 GB.
    private static string Size(ulong megabytes) =>
        (megabytes / 1024.0).ToString("0.#", CultureInfo.InvariantCulture);
}
