using System.Globalization;

using Allot.Tray.Core.Daemon;
using Allot.Tray.Core.ServiceSettings;

using Microsoft.UI.Dispatching;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Allot.Tray.Settings;

/// <summary>
/// What the Settings window works with.
/// </summary>
/// <param name="Session">allotd's settings.</param>
/// <param name="Service">Start, Stop and Restart service, and the latest poll.</param>
/// <param name="Data">The install, for the config file and the port this panel dials.</param>
/// <param name="Panel">The Panel page's stores.</param>
/// <param name="OpenLogs">Opens the log folder.</param>
internal sealed record SettingsServices(
    SettingsSession Session,
    ServiceControl Service,
    DataDirectory Data,
    PanelHost Panel,
    Action OpenLogs);

/// <summary>
/// The Settings window. Built once and hidden on close by its <see cref="SettingsHost"/>, so the
/// page, scroll and unsaved edits survive a close; values are fetched again on every show.
/// </summary>
public sealed partial class SettingsWindow : Window
{
    private static readonly TimeSpan NoticeTime = TimeSpan.FromSeconds(10);

    private readonly SettingsServices _services;
    private readonly PageContext _context;
    private readonly PanelPage _panel;
    private readonly Dictionary<string, AllotdPage> _pages = new(StringComparer.Ordinal);
    private readonly FitPreviews _fits;
    private readonly AdmissionPreview _preview;
    private readonly DispatcherQueueTimer _noticeTimer;
    private ContentDialog? _dialog;
    private Action? _noticeUndo;
    private string _pageId = SettingsBanners.PanelPage;
    private string _saveStatus = string.Empty;
    private bool _visible;

    internal SettingsWindow(SettingsServices services)
    {
        _services = services;
        InitializeComponent();
        Hwnd = Microsoft.UI.Win32Interop.GetWindowFromWindowId(AppWindow.Id);
        Presenter = OverlappedPresenter.Create();
        AppWindow.SetPresenter(Presenter);
        ExtendsContentIntoTitleBar = true;
        SetTitleBar(AppTitleBar);
        AppWindow.SetIcon(Path.Combine(AppContext.BaseDirectory, "Assets", "AppIcon.ico"));

        _context = new PageContext(
            services.Session,
            SettingsText.Numbers(CultureInfo.CurrentCulture),
            ShowNotice,
            ConfirmAsync,
            ShowDialogAsync);
        _noticeTimer = DispatcherQueue.CreateTimer();
        _noticeTimer.Interval = NoticeTime;
        _noticeTimer.IsRepeating = false;
        _noticeTimer.Tick += (_, _) => Notice.IsOpen = false;
        Notice.ActionButton = SettingsUi.Button("Undo", OnNoticeUndo);
        Notice.Closed += (_, _) => _noticeTimer.Stop();

        _panel = new PanelPage(services.Panel, ConfirmAsync);
        _fits = new FitPreviews(services.Session, DispatcherQueue);
        _preview = new AdmissionPreview(_context, _fits);
        BuildPages();

        services.Session.PropertyChanged += (_, _) => ShowSessionState();
        Activated += OnActivated;
        Nav.SelectedItem = Nav.MenuItems[0];
        ShowSessionState();
    }

    internal nint Hwnd { get; }

    internal OverlappedPresenter Presenter { get; }

    /// <summary>
    /// Shows <paramref name="pageId"/>, or keeps the page shown last.
    /// </summary>
    internal void SelectPage(string? pageId)
    {
        if (pageId is null)
        {
            return;
        }

        foreach (var item in Nav.MenuItems.OfType<NavigationViewItem>())
        {
            if (item.Tag as string == pageId)
            {
                Nav.SelectedItem = item;
            }
        }
    }

    /// <summary>
    /// The window was shown or hidden: fetching runs only while it shows, and hiding sends what
    /// is pending and cancels an open dialog.
    /// </summary>
    internal void SetVisible(bool visible)
    {
        _visible = visible;
        _services.Session.SetVisible(visible);
        _fits.SetActive(visible && ShowsFits(_pageId));
        if (visible)
        {
            _panel.Sync();
        }
        else
        {
            _dialog?.Hide();
            Notice.IsOpen = false;
        }
    }

    private void BuildPages()
    {
        var admission = Page("admission", "Admission");
        admission.Live["keep_free"] = _preview.HeadroomText;
        admission.Live["learn"] = () => EstimatesText();
        admission.After["learn"] = [_preview.Card];
        _fits.Updated += admission.Refresh;

        var sizes = Page("sizes", "Sizes");
        sizes.Cards["classes"] = group => SizeTable(group);
        sizes.Cards["presets"] = group => SizeTable(group);

        var commands = Page("commands", "Commands");
        commands.Cards["rules"] = group =>
        {
            var card = new RuleListCard(_context, (RuleListModel)group.Settings[0]);
            return new StackPanel
            {
                Children = { card.TestCard, SettingsUi.Heading(group.Spec.Label), card.Rules },
            };
        };

        var benchmarks = Page("benchmarks", "Benchmarks");
        benchmarks.Live["quiet"] = () => BenchmarksText();

        var leases = Page("leases", "Leases");
        leases.Live["history"] = () => HistoryText();

        var service = new AllotdPage("service", "Service", _context) { OffersResetAll = true };
        _pages[service.Id] = service;
        service.Top.Add(ServiceCards.Status(
            _services.Service,
            _context,
            () => DaemonConfig.ReadPort(_services.Data),
            _services.OpenLogs));
        service.End.Add(ServiceCards.Files(_services.Data));

        foreach (var page in _pages.Values)
        {
            page.ShowTop();
        }
    }

    private AllotdPage Page(string id, string title)
    {
        var page = new AllotdPage(id, title, _context);
        _pages[id] = page;
        return page;
    }

    // The fit preview runs while a page that shows it is open.
    private static bool ShowsFits(string pageId) => pageId is "admission" or "sizes";

    private StackPanel SizeTable(GroupModel group)
    {
        var table = new SizeTableCard(_context, _fits, (SizeTableModel)group.Settings[0]);
        return new StackPanel { Children = { SettingsUi.Heading(group.Spec.Label), table.Root } };
    }

    private string EstimatesText()
    {
        var session = _context.Session;
        var lower = (session.Setting("estimates.lower") as SwitchModel)?.Value ?? true;
        return LiveText.Estimates(
            lower,
            Number("estimates.min_runs"),
            Number("estimates.margin_pct"),
            _context.Number);
    }

    private string BenchmarksText() =>
        LiveText.Benchmarks(
            Number("benchmark.quiet_cpu_pct"),
            Number("benchmark.quiet_gpu_pct"),
            Number("benchmark.quiet_secs"),
            Number("benchmark.max_quiet_wait_secs"),
            _context.Number);

    private string HistoryText() =>
        LiveText.History(
            Number("retention.lease_days"),
            Number("retention.history_days"),
            Number("retention.keep_runs_per_job"),
            Number("retention.max_job_runs"),
            _context.Number);

    // The value on screen; a field with an issue still shows the user's number.
    private double Number(string key) =>
        (_context.Session.Setting(key) as NumberModel)?.Value ?? 0;

    private void OnNavSelectionChanged(
        NavigationView sender,
        NavigationViewSelectionChangedEventArgs args)
    {
        _pageId = (args.SelectedItem as NavigationViewItem)?.Tag as string
            ?? SettingsBanners.PanelPage;
        PageHost.Content = _pages.TryGetValue(_pageId, out var page) ? page.Root : _panel.Root;
        _fits.SetActive(_visible && ShowsFits(_pageId));
        ShowBanners();
    }

    private void OnActivated(object sender, WindowActivatedEventArgs args)
    {
        // Values are fetched on focus too, so a hand edit shows without waiting for the next tick.
        if (_visible && args.WindowActivationState != WindowActivationState.Deactivated)
        {
            _ = _services.Session.RefreshAsync();
        }
    }

    private void ShowSessionState()
    {
        var session = _services.Session;
        var status = session.Status == SessionStatus.Loading
            ? "Loading settings…"
            : SettingsText.SaveStatus(session.SaveStatus);
        if (status != _saveStatus)
        {
            _saveStatus = status;
            SaveStatusText.Text = status;
            SettingsUi.Announce(SaveStatusText);
        }

        ShowBanners();
    }

    private void ShowBanners()
    {
        var banners = SettingsBanners.For(
            _services.Session,
            _pageId,
            DaemonConfig.ReadPort(_services.Data),
            _context.Number);
        Banners.Children.Clear();
        foreach (var banner in banners)
        {
            Banners.Children.Add(BannerBar(banner));
        }

        Banners.Visibility = SettingsUi.Shows(banners.Count > 0);
    }

    private InfoBar BannerBar(SettingsBanner banner)
    {
        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        for (var i = 0; i < banner.Actions.Count; i++)
        {
            var action = banner.Actions[i];
            buttons.Children.Add(SettingsUi.Button(
                SettingsBanners.Label(action),
                () => Run(action),
                accent: i == 0));
        }

        return new InfoBar
        {
            IsOpen = true,
            IsClosable = false,
            Severity = banner.Severity switch
            {
                BannerSeverity.Error => InfoBarSeverity.Error,
                BannerSeverity.Warning => InfoBarSeverity.Warning,
                _ => InfoBarSeverity.Informational,
            },
            Message = banner.Message,
            Content = buttons,
        };
    }

    private void Run(BannerAction action)
    {
        var session = _services.Session;
        switch (action)
        {
            case BannerAction.StartService:
                _services.Service.Start();
                break;
            case BannerAction.RestartService:
                _ = RestartAsync();
                break;
            case BannerAction.TryNow:
            case BannerAction.CheckAgain:
                _ = session.RefreshAsync();
                break;
            case BannerAction.OpenLogs:
                _services.OpenLogs();
                break;
            case BannerAction.OpenFile:
                ServiceCards.OpenFile(_services.Data.ConfigFile);
                break;
            case BannerAction.ApplyMine:
                session.ApplyMine();
                break;
            case BannerAction.KeepFileValues:
                session.KeepFileValues();
                break;
            case BannerAction.SaveAgain:
                _ = session.FlushAsync();
                break;
        }
    }

    private async Task RestartAsync()
    {
        if (await _services.Service.RestartAsync().ConfigureAwait(true) is { } problem)
        {
            ShowNotice($"The service did not restart: {problem}", null);
        }

        await _services.Session.RefreshAsync().ConfigureAwait(true);
    }

    private void ShowNotice(string message, Action? undo)
    {
        _noticeUndo = undo;
        Notice.Message = message;
        Notice.ActionButton.Visibility = SettingsUi.Shows(undo is not null);
        Notice.IsOpen = true;
        _noticeTimer.Stop();
        _noticeTimer.Start();
    }

    private void OnNoticeUndo()
    {
        var undo = _noticeUndo;
        _noticeUndo = null;
        Notice.IsOpen = false;
        undo?.Invoke();
    }

    private async Task<bool> ConfirmAsync(Confirmation confirmation)
    {
        var dialog = new ContentDialog
        {
            Title = confirmation.Title,
            Content = new TextBlock
            {
                Text = confirmation.Body,
                TextWrapping = TextWrapping.WrapWholeWords,
            },
            PrimaryButtonText = confirmation.Button,
            CloseButtonText = "Cancel",
            DefaultButton = ContentDialogButton.Close,
        };
        return await ShowDialogAsync(dialog).ConfigureAwait(true) == ContentDialogResult.Primary;
    }

    // One dialog at a time; SetVisible(false) hides the open one.
    private async Task<ContentDialogResult> ShowDialogAsync(ContentDialog dialog)
    {
        if (_dialog is not null || Content.XamlRoot is null)
        {
            return ContentDialogResult.None;
        }

        dialog.XamlRoot = Content.XamlRoot;
        _dialog = dialog;
        try
        {
            return await dialog.ShowAsync();
        }
        finally
        {
            _dialog = null;
        }
    }
}
