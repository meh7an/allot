using System.ComponentModel;
using System.Runtime.CompilerServices;

using Microsoft.UI.Xaml.Controls;

namespace SettingsSpike;

public sealed partial class MainPage : Page, INotifyPropertyChanged
{
    private bool _example = true;
    private double _headroom = 15;

    public MainPage()
    {
        InitializeComponent();
        Loaded += (_, _) => FocusProbe.Focus(Microsoft.UI.Xaml.FocusState.Programmatic);
    }

    public event PropertyChangedEventHandler? PropertyChanged;

    public bool Example
    {
        get => _example;
        set => Set(ref _example, value);
    }

    public double Headroom
    {
        get => _headroom;
        set => Set(ref _headroom, value);
    }

    public string Summary(bool example, double headroom) =>
        $"Bound values: switch {(example ? "on" : "off")}, headroom {headroom:0} %";

    private void Set<T>(ref T field, T value, [CallerMemberName] string name = "")
    {
        if (!EqualityComparer<T>.Default.Equals(field, value))
        {
            field = value;
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));
        }
    }
}
