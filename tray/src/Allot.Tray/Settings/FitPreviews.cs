using Allot.Tray.Core.Contract;
using Allot.Tray.Core.ServiceSettings;

using Microsoft.UI.Dispatching;

namespace Allot.Tray.Settings;

/// <summary>
/// allotd's fit preview for the values on screen, unsaved edits included: asked 600 ms after the
/// last change and with every refresh, while a page that shows it (Admission, Sizes) is open in a
/// visible window.
/// </summary>
internal sealed class FitPreviews
{
    /// <summary>
    /// Why a live part shows nothing yet, before the first preview arrives.
    /// </summary>
    public const string NotYet = "Can't work this out yet. Trying again in a moment.";

    private readonly SettingsSession _session;
    private readonly DispatcherQueueTimer _debounce;
    private int _request;
    private bool _active;

    public FitPreviews(SettingsSession session, DispatcherQueue dispatcher)
    {
        _session = session;
        _debounce = dispatcher.CreateTimer();
        _debounce.Interval = SettingsSession.SaveDelay;
        _debounce.IsRepeating = false;
        _debounce.Tick += (_, _) => _ = RequestAsync();
        session.PropertyChanged += (_, _) =>
        {
            if (_active)
            {
                _debounce.Stop();
                _debounce.Start();
            }
        };
    }

    /// <summary>
    /// Raised when a preview arrived, or the service's state changed what can be shown.
    /// </summary>
    public event Action? Updated;

    /// <summary>
    /// The latest preview, or <c>null</c> before the first one.
    /// </summary>
    public FitPreview? Latest { get; private set; }

    /// <summary>
    /// Why no preview can be shown now, or <c>null</c> when <see cref="Latest"/> applies.
    /// </summary>
    public string? Unavailable => _session.Status switch
    {
        SessionStatus.Ready or SessionStatus.Loading => null,
        _ => LiveText.ServiceStopped,
    };

    /// <summary>
    /// The fit of a class or preset by name, or <c>null</c> while none is known.
    /// </summary>
    public SizeFit? Of(string table, string name)
    {
        if (Unavailable is not null || Latest is not { } preview)
        {
            return null;
        }

        var fits = table == "classes" ? preview.Classes : preview.Presets;
        return fits.FirstOrDefault(fit => fit.Name == name);
    }

    /// <summary>
    /// Starts or stops previewing.
    /// </summary>
    public void SetActive(bool active)
    {
        _active = active;
        _debounce.Stop();
        if (active)
        {
            _ = RequestAsync();
        }
    }

    private async Task RequestAsync()
    {
        if (!_active || _session.Status != SessionStatus.Ready)
        {
            Updated?.Invoke();
            return;
        }

        var request = ++_request;
        var reply = await _session.PreviewAsync().ConfigureAwait(true);
        if (request != _request)
        {
            return;
        }

        if (reply.Value is { } preview)
        {
            Latest = preview;
        }
        else
        {
            AppLog.Write($"settings: fit preview failed: {reply.Failure?.Message}");
        }

        Updated?.Invoke();
    }
}
