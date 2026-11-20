namespace Allot.Tray.Core.ServiceSettings;

/// <summary>
/// A unit a number can be shown in.
/// </summary>
/// <param name="Name">The unit as the user reads it: <c>minutes</c>, <c>GB</c>.</param>
/// <param name="Factor">How many of the file's unit make one.</param>
public sealed record ValueUnit(string Name, double Factor)
{
    /// <summary>
    /// A duration is in seconds in the file, and shown in seconds, minutes or hours.
    /// </summary>
    public static readonly IReadOnlyList<ValueUnit> Durations =
        [new("seconds", 1), new("minutes", 60), new("hours", 3600)];

    /// <summary>
    /// A size is in megabytes in the file, and shown in MB or GB.
    /// </summary>
    public static readonly IReadOnlyList<ValueUnit> Memory = [new("MB", 1), new("GB", 1024)];

    /// <summary>
    /// The name of one, such as <c>minute</c>; for the plural unit names of durations.
    /// </summary>
    public string Singular => Name[..^1];

    /// <summary>
    /// The largest unit that shows <paramref name="value"/> as a whole number; the smallest for
    /// 0 or automatic.
    /// </summary>
    /// <param name="units">The units, smallest first.</param>
    /// <param name="value">In the file's unit.</param>
    public static ValueUnit Best(IReadOnlyList<ValueUnit> units, double? value)
    {
        var best = units.Count > 0 ? units[0] : new ValueUnit(string.Empty, 1);
        if (value is not { } number || number == 0)
        {
            return best;
        }

        foreach (var unit in units)
        {
            if (IsWhole(number / unit.Factor))
            {
                best = unit;
            }
        }

        return best;
    }

    internal static bool IsWhole(double value) =>
        double.IsFinite(value) && Math.Truncate(value) == value;
}
