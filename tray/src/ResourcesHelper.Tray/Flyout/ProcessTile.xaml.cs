using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace ResourcesHelper.Tray.Flyout;

/// <summary>
/// A job row's process tile. It shows <see cref="Text"/> until the icon of
/// <see cref="ExePath"/> arrives from <see cref="ExeIcons"/>, and asks again at the new size
/// when the display scale changes.
/// </summary>
public sealed partial class ProcessTile : UserControl
{
    public static readonly DependencyProperty TextProperty = DependencyProperty.Register(
        nameof(Text), typeof(string), typeof(ProcessTile), new PropertyMetadata("", OnChanged));

    public static readonly DependencyProperty IsQueuedProperty = DependencyProperty.Register(
        nameof(IsQueued),
        typeof(bool),
        typeof(ProcessTile),
        new PropertyMetadata(false, OnChanged));

    public static readonly DependencyProperty ExePathProperty = DependencyProperty.Register(
        nameof(ExePath),
        typeof(string),
        typeof(ProcessTile),
        new PropertyMetadata(null, OnChanged));

    private const double IconSize = 16;

    private XamlRoot? _root;
    private string? _shownKey;

    public ProcessTile()
    {
        InitializeComponent();
        Loaded += (_, _) => Attach();
        Unloaded += (_, _) => Detach();
    }

    /// <summary>
    /// The initials, or <c>#N</c> for a queued job.
    /// </summary>
    public string Text
    {
        get => (string)GetValue(TextProperty);
        set => SetValue(TextProperty, value);
    }

    public bool IsQueued
    {
        get => (bool)GetValue(IsQueuedProperty);
        set => SetValue(IsQueuedProperty, value);
    }

    public string? ExePath
    {
        get => (string?)GetValue(ExePathProperty);
        set => SetValue(ExePathProperty, value);
    }

    private static void OnChanged(DependencyObject sender, DependencyPropertyChangedEventArgs e) =>
        ((ProcessTile)sender).Refresh();

    private void Attach()
    {
        _root = XamlRoot;
        if (_root is not null)
        {
            _root.Changed += OnRootChanged;
        }

        Refresh();
    }

    private void Detach()
    {
        if (_root is not null)
        {
            _root.Changed -= OnRootChanged;
            _root = null;
        }
    }

    private void OnRootChanged(XamlRoot sender, XamlRootChangedEventArgs args) => Refresh();

    private void Refresh()
    {
        Initials.Text = Text;
        Fill.Visibility = IsQueued ? Visibility.Collapsed : Visibility.Visible;
        QueuedEdge.Visibility = IsQueued ? Visibility.Visible : Visibility.Collapsed;

        var path = IsQueued ? null : ExePath;
        if (string.IsNullOrEmpty(path) || XamlRoot is null)
        {
            ShowIcon(null, null);
            return;
        }

        var size = (int)Math.Round(IconSize * XamlRoot.RasterizationScale);
        var key = $"{size}|{path}";
        if (key != _shownKey)
        {
            _ = LoadAsync(path, size, key);
        }
    }

    private async Task LoadAsync(string path, int size, string key)
    {
        _shownKey = key;
        var icon = await ExeIcons.GetAsync(path, size);

        // A newer request, or a recycled tile, may have replaced this one meanwhile.
        if (_shownKey == key)
        {
            ShowIcon(icon, key);
        }
    }

    private void ShowIcon(ImageSource? icon, string? key)
    {
        _shownKey = key;
        Icon.Source = icon;
        Icon.Visibility = icon is null ? Visibility.Collapsed : Visibility.Visible;
        var initials = icon is null ? Visibility.Visible : Visibility.Collapsed;
        Initials.Visibility = initials;
        if (!IsQueued)
        {
            Fill.Visibility = initials;
        }
    }
}
