using Microsoft.UI.Xaml;

namespace SettingsSpike;

public sealed partial class BoundWindow : Window
{
    public BoundWindow()
    {
        InitializeComponent();
    }

    public string Caption => "A Window with x:Bind in its own XAML";
}
