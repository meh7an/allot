using System.Collections.Concurrent;
using System.ComponentModel;
using System.Runtime.CompilerServices;

namespace ResourcesHelper.Tray.Core.ViewModels;

/// <summary>
/// Raises <see cref="PropertyChanged"/> only for values that really changed, so a 1 Hz poll
/// with nothing new touches no binding.
/// </summary>
public abstract class Observable : INotifyPropertyChanged
{
    // One event argument per property name, shared: the names are a fixed set, and a 1 Hz
    // update raises dozens of changes.
    private static readonly ConcurrentDictionary<string, PropertyChangedEventArgs> Args =
        new(StringComparer.Ordinal);

    private static long s_raised;

    public event PropertyChangedEventHandler? PropertyChanged;

    /// <summary>
    /// Property changes raised by every observable so far, for the measure log.
    /// </summary>
    public static long Raised => Interlocked.Read(ref s_raised);

    /// <returns>True when the value changed.</returns>
    protected bool Set<T>(ref T field, T value, [CallerMemberName] string? name = null)
    {
        if (EqualityComparer<T>.Default.Equals(field, value))
        {
            return false;
        }

        field = value;
        Raise(name);
        return true;
    }

    protected void Raise(string? name)
    {
        _ = Interlocked.Increment(ref s_raised);
        if (PropertyChanged is { } handler)
        {
            var args = name is null
                ? new PropertyChangedEventArgs(null)
                : Args.GetOrAdd(name, static key => new PropertyChangedEventArgs(key));
            handler(this, args);
        }
    }
}
