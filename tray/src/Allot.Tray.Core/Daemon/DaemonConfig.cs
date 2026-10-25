using System.Globalization;

namespace Allot.Tray.Core.Daemon;

/// <summary>
/// The one value the tray app needs from <c>config.toml</c>: <c>[daemon] port</c>.
/// </summary>
public static class DaemonConfig
{
    /// <summary>
    /// allotd's port when the config does not set one, as in <c>DaemonConfig::default</c>.
    /// </summary>
    public const ushort DefaultPort = 47823;

    public static ushort ReadPort(DataDirectory data)
    {
        try
        {
            return PortFrom(File.ReadAllText(data.ConfigFile));
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            return DefaultPort;
        }
    }

    /// <summary>
    /// Finds the port as either <c>port</c> under <c>[daemon]</c> or a top-level
    /// <c>daemon.port</c>. Not a TOML parser: allotd validates the whole file and refuses to start on
    /// a bad one, so anything this cannot read falls back to the default.
    /// </summary>
    public static ushort PortFrom(string toml)
    {
        var table = "";
        foreach (var rawLine in toml.Split('\n'))
        {
            var line = StripComment(rawLine).Trim();
            if (line.Length == 0)
            {
                continue;
            }

            if (line.StartsWith('[') && line.EndsWith(']'))
            {
                table = line.Trim('[', ']').Trim();
                continue;
            }

            var equals = line.IndexOf('=', StringComparison.Ordinal);
            if (equals < 0)
            {
                continue;
            }

            var key = line[..equals].Trim();
            var isPort = (table == "daemon" && key == "port")
                || (table.Length == 0 && key == "daemon.port");
            if (isPort)
            {
                return ParsePort(line[(equals + 1)..].Trim());
            }
        }

        return DefaultPort;
    }

    // The port is an integer, so a '#' always starts a comment.
    private static string StripComment(string line)
    {
        var hash = line.IndexOf('#', StringComparison.Ordinal);
        return hash < 0 ? line : line[..hash];
    }

    // TOML allows underscores between digits, as in the 47_823 allotd's own default uses.
    private static ushort ParsePort(string value)
    {
        var digits = value.Replace("_", "", StringComparison.Ordinal);
        var parsed = ushort.TryParse(
            digits,
            NumberStyles.None,
            CultureInfo.InvariantCulture,
            out var port);
        return parsed && port != 0
            ? port
            : DefaultPort;
    }
}
