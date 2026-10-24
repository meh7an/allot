using System.ComponentModel;
using System.Runtime.CompilerServices;

namespace ResourcesHelper.Tray.Core.ViewModels;

/// <summary>
/// Raises <see cref="PropertyChanged"/> only for values that really changed, so a 1 Hz poll
/// with nothing new touches no binding.
/// </summary>
public abstract class Observable : INotifyPropertyChanged
{
    public event PropertyChangedEventHandler? PropertyChanged;

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

    protected void Raise(string? name) =>
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));
}
