using Microsoft.UI.Xaml.Controls;

namespace SettingsSpike;

public sealed partial class AdmissionPage : Page
{
    public AdmissionPage()
    {
        InitializeComponent();
    }

    // A collection expression here builds a compiler-made read-only type that CsWinRT cannot
    // marshal under Native AOT: setting ItemsSource then throws ArgumentException ("Value does
    // not fall within the expected range"). A List works.
    public List<ClassRow> Classes { get; } = new()
    {
        new() { Name = "cpu_heavy", Cores = "8 cores", MinCores = "min 4" },
        new() { Name = "cpu_medium", Cores = "4 cores", MinCores = "min 2" },
        new() { Name = "gpu_heavy", Cores = "2 cores", MinCores = "min 1" },
    };
}

/// <summary>
/// Sample rows; the real table comes from the settings spec (M1.1). Plain setters: the XAML
/// compiler's generated type info assigns them, so "init" does not compile (CS8852).
/// </summary>
public sealed partial class ClassRow
{
    public string Name { get; set; } = "";

    public string Cores { get; set; } = "";

    public string MinCores { get; set; } = "";
}
