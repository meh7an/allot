using Microsoft.UI;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace SettingsSpike;

public sealed partial class SettingsWindow : Window
{
    private readonly SettingsHost _host;
    private readonly TaskCompletionSource _loaded = new();
    private double _scale;

    internal SettingsWindow(int number, SettingsHost host)
    {
        Number = number;
        _host = host;
        InitializeComponent();

        Hwnd = Win32Interop.GetWindowFromWindowId(AppWindow.Id);
        // The first selftest run saw "AppWindow.Presenter is OverlappedPresenter" fail (minimum
        // size and placement were skipped silently); later runs, AOT and JIT, saw it hold. Owning
        // the presenter, as the tray's flyout does, removes the dependency; both are logged.
        SpikeLog.Write(
            $"#{Number} default presenter: kind {AppWindow.Presenter.Kind}, "
            + $"is OverlappedPresenter {AppWindow.Presenter is OverlappedPresenter}");
        Presenter = OverlappedPresenter.Create();
        AppWindow.SetPresenter(Presenter);
        AppTitleBar.Title = TitleText;
        ExtendsContentIntoTitleBar = true;
        SetTitleBar(AppTitleBar);
        AppWindow.SetIcon(Path.Combine(AppContext.BaseDirectory, "Assets", "AppIcon.ico"));
        Activated += OnActivated;
        Nav.SelectedItem = Nav.MenuItems[0];
    }

    public int Number { get; }

    public string TitleText => $"Allot Settings #{Number}";

    public nint Hwnd { get; }

    public OverlappedPresenter Presenter { get; }

    /// <summary>
    /// Completes when the content first loads.
    /// </summary>
    public Task ContentLoaded => _loaded.Task;

    /// <summary>
    /// The page types the selftest walks through, by NavigationView tag.
    /// </summary>
    public string? PageTag => (Nav.SelectedItem as NavigationViewItem)?.Tag as string;

    public Type? PageType => ContentFrame.Content?.GetType();

    public int PageCount => Nav.MenuItems.Count;

    public void SelectPage(int index) => Nav.SelectedItem = Nav.MenuItems[index];

    public int BackdropCount => BackdropBox.Items.Count;

    public void SelectBackdrop(int index) => BackdropBox.SelectedIndex = index;

    public string BackdropName => SystemBackdrop switch
    {
        MicaBackdrop { Kind: Microsoft.UI.Composition.SystemBackdrops.MicaKind.BaseAlt } => "Mica Alt",
        MicaBackdrop => "Mica",
        DesktopAcrylicBackdrop => "Acrylic",
        _ => "none",
    };

    /// <summary>
    /// Sets the presenter's minimum size for the window's current scale. Logs what it held
    /// before, which shows whether WinUI rescales it on a DPI change by itself.
    /// </summary>
    public void ApplyMinimumSize()
    {
        var presenter = Presenter;
        var scale = NativeMethods.GetDpiForWindow(Hwnd) / 96.0;
        var width = (int)Math.Round(SettingsHost.MinimumSize.Width * scale);
        var height = (int)Math.Round(SettingsHost.MinimumSize.Height * scale);
        SpikeLog.Write(
            $"#{Number} presenter cast now {AppWindow.Presenter is OverlappedPresenter}; "
            + $"minimum size at scale {scale:0.##}: was {presenter.PreferredMinimumWidth}x"
            + $"{presenter.PreferredMinimumHeight} px, now {width}x{height} px");
        presenter.PreferredMinimumWidth = width;
        presenter.PreferredMinimumHeight = height;
    }

    public void Refresh()
    {
        var windowDpi = NativeMethods.GetDpiForWindow(Hwnd);
        var display = DisplayArea.GetFromWindowId(AppWindow.Id, DisplayAreaFallback.Nearest);
        var monitorScale = SettingsHost.MonitorScale(display);
        var size = AppWindow.Size;
        var position = AppWindow.Position;
        var scale = windowDpi / 96.0;
        var work = display.WorkArea;
        var minimum = $"{Presenter.PreferredMinimumWidth}x{Presenter.PreferredMinimumHeight}";
        Diagnostics.Text =
            $"scale {scale:0.##} (window {windowDpi} dpi, monitor {monitorScale * 96:0} dpi, XAML {Root.XamlRoot?.RasterizationScale:0.##})\n"
            + $"{size.Width}x{size.Height} px at {position.X},{position.Y} = {size.Width / scale:0}x{size.Height / scale:0} DIP, min {minimum} px\n"
            + $"display {display.DisplayId.Value}, work {work.Width}x{work.Height} at {work.X},{work.Y}; backdrop {BackdropName}";
    }

    /// <summary>
    /// Drops the handlers that point back at the window, so a closed one can be collected.
    /// </summary>
    internal void Detach()
    {
        Activated -= OnActivated;
        if (Root.XamlRoot is { } root)
        {
            root.Changed -= OnXamlRootChanged;
        }
    }

    private void OnRootLoaded(object sender, RoutedEventArgs e)
    {
        _scale = Root.XamlRoot.RasterizationScale;
        Root.XamlRoot.Changed += OnXamlRootChanged;
        Refresh();
        _host.OnContentLoaded(this);
        _ = _loaded.TrySetResult();
    }

    private void OnXamlRootChanged(XamlRoot sender, XamlRootChangedEventArgs args)
    {
        if (sender.RasterizationScale != _scale)
        {
            var size = AppWindow.Size;
            SpikeLog.Write(
                $"#{Number} scale {_scale:0.##} -> {sender.RasterizationScale:0.##}, "
                + $"window now {size.Width}x{size.Height} px");
            _scale = sender.RasterizationScale;
            ApplyMinimumSize();
        }

        Refresh();
    }

    private void OnActivated(object sender, WindowActivatedEventArgs args)
    {
        if (args.WindowActivationState != WindowActivationState.Deactivated)
        {
            SpikeLog.Write($"#{Number} activated ({args.WindowActivationState})");
        }
    }

    private void OnNavSelectionChanged(NavigationView sender, NavigationViewSelectionChangedEventArgs args)
    {
        var page = (args.SelectedItem as NavigationViewItem)?.Tag as string == "admission"
            ? typeof(AdmissionPage)
            : typeof(MainPage);
        _ = ContentFrame.Navigate(page);
    }

    private void OnBackdropChanged(object sender, SelectionChangedEventArgs e)
    {
        SystemBackdrop = BackdropBox.SelectedIndex switch
        {
            0 => new MicaBackdrop(),
            1 => new MicaBackdrop { Kind = Microsoft.UI.Composition.SystemBackdrops.MicaKind.BaseAlt },
            2 => new DesktopAcrylicBackdrop(),
            _ => null,
        };
        // With no backdrop the transparent root would show black.
        Root.Background = SystemBackdrop is null
            ? (Brush)Application.Current.Resources["SolidBackgroundFillColorBaseBrush"]
            : null;
        if (Root.IsLoaded)
        {
            Refresh();
        }
    }

    private void OnShowLauncherClick(object sender, RoutedEventArgs e) => _host.ShowLauncher?.Invoke();

    private void OnQuitClick(object sender, RoutedEventArgs e) => _host.Quit?.Invoke();
}
