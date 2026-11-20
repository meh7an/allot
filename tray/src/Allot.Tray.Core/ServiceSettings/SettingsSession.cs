using System.Text.Json;

using Allot.Tray.Core.Contract;
using Allot.Tray.Core.Daemon;
using Allot.Tray.Core.ViewModels;

namespace Allot.Tray.Core.ServiceSettings;

/// <summary>
/// The Settings window's allotd settings: one view model per setting, fetched from
/// <c>GET /settings</c> and saved through <c>PATCH /settings</c> with the file's revision. Single
/// values save as one burst 600 ms after the last change; tables, rules and chips save at once.
/// The Panel page's own settings are not here: they stay in the tray's own store.
/// </summary>
/// <remarks>
/// The session and its models belong to the thread that created them, the UI thread. Awaits
/// therefore keep the caller's context (no <c>ConfigureAwait(false)</c>), and the
/// <see cref="ISessionTimers"/> call back on that thread.
/// </remarks>
public sealed class SettingsSession : Observable, IValueHost, IDisposable
{
    /// <summary>
    /// Single values save this long after the last change, as one <c>PATCH</c>.
    /// </summary>
    public static readonly TimeSpan SaveDelay = TimeSpan.FromMilliseconds(600);

    /// <summary>
    /// While the window is visible, values are fetched again this often.
    /// </summary>
    public static readonly TimeSpan RefreshInterval = TimeSpan.FromSeconds(5);

    private readonly ISettingsClient _client;
    private readonly ISessionTimers _timers;
    private readonly Platform _platform;
    private readonly Func<CancellationToken, Task<string?>> _readStopRecord;
    private readonly CancellationTokenSource _lifetime = new();
    private readonly Dictionary<string, ValueModel> _models = new(StringComparer.Ordinal);
    private readonly OrderedDictionary<string, Pending> _pending = new(StringComparer.Ordinal);
    private string? _revision;
    private IDisposable? _saveTimer;
    private IDisposable? _refreshTimer;
    private bool _saving;
    private bool _visible;
    private bool _disposed;

    /// <param name="client">The settings routes: a <see cref="DaemonClient"/>.</param>
    /// <param name="timers">The burst and refresh timers.</param>
    /// <param name="platform">The platform of this panel; settings for others are left out.</param>
    /// <param name="readStopRecord">The text of <c>~/.allot/stopped.json</c>, or <c>null</c> when
    /// there is none; see <see cref="ReadStopRecordAsync"/>.</param>
    public SettingsSession(
        ISettingsClient client,
        ISessionTimers timers,
        Platform platform,
        Func<CancellationToken, Task<string?>> readStopRecord)
    {
        _client = client;
        _timers = timers;
        _platform = platform;
        _readStopRecord = readStopRecord;
    }

    public SessionStatus Status { get; private set; } = SessionStatus.Loading;

    /// <summary>
    /// Why allotd did not answer, in its words or the client's.
    /// </summary>
    public string? Detail { get; private set; }

    /// <summary>
    /// The stop record, while <see cref="Status"/> is <see cref="SessionStatus.Stopped"/>.
    /// </summary>
    public StopRecord? StopRecord { get; private set; }

    /// <summary>
    /// Why allotd rejects <c>config.toml</c>, if it does.
    /// </summary>
    public string? FileError { get; private set; }

    /// <summary>
    /// Keys whose saved value takes effect only once allotd restarts.
    /// </summary>
    public IReadOnlyList<string> PendingRestart { get; private set; } = [];

    /// <summary>
    /// The port allotd listens on, which differs from <c>daemon.port</c> until a restart.
    /// </summary>
    public uint? ListeningPort { get; private set; }

    /// <summary>
    /// The pages, with the settings this platform shows.
    /// </summary>
    public IReadOnlyList<PageModel> Pages { get; private set; } = [];

    /// <summary>
    /// The edits a hand edit of <c>config.toml</c> got in the way of, until the user decides.
    /// </summary>
    public Conflict? Conflict { get; private set; }

    /// <summary>
    /// Why the last save was not written though allotd answered, such as a held file; the
    /// changes stay unsaved until the next save.
    /// </summary>
    public string? WriteProblem { get; private set; }

    /// <summary>
    /// Whether allotd's settings cannot be changed now.
    /// </summary>
    public bool ReadOnly => Status != SessionStatus.Ready || FileError is not null;

    public SaveStatus SaveStatus
    {
        get
        {
            if (_saving)
            {
                return new SaveStatus(SaveKind.Saving);
            }

            var count = _pending.Count;
            foreach (var model in _models.Values)
            {
                if (model.Issue is not null && !_pending.ContainsKey(model.Key))
                {
                    count++;
                }
            }

            return count == 0
                ? new SaveStatus(SaveKind.Saved)
                : new SaveStatus(SaveKind.Unsaved, count);
        }
    }

    /// <summary>
    /// The text of the stop record file, or <c>null</c> when there is none.
    /// </summary>
    public static async Task<string?> ReadStopRecordAsync(
        string path,
        CancellationToken cancellationToken)
    {
        try
        {
            return await File.ReadAllTextAsync(path, cancellationToken).ConfigureAwait(false);
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            return null;
        }
    }

    /// <summary>
    /// The stop record, as allotd reads it: a file that is not a record still means the user
    /// stopped allotd, from the CLI, since only its presence counts.
    /// </summary>
    public static StopRecord? ParseStopRecord(string? text)
    {
        if (text is null)
        {
            return null;
        }

        try
        {
            return JsonSerializer.Deserialize(text, ContractJsonContext.Default.StopRecord)
                ?? CliStop();
        }
        catch (JsonException)
        {
            return CliStop();
        }
    }

    public ValueModel? Setting(string key) => _models.GetValueOrDefault(key);

    /// <summary>
    /// The settings on a page whose saved value differs from the default, for Reset page.
    /// </summary>
    public IReadOnlyList<ValueModel> ChangedOn(string pageId) =>
        ChangedIn(Pages.Where(page => page.Spec.Id == pageId));

    /// <summary>
    /// Every changed setting on every page, for Reset all.
    /// </summary>
    public IReadOnlyList<ValueModel> ChangedSettings() => ChangedIn(Pages);

    /// <summary>
    /// Fetches the values again: on show, on focus and every 5 s while visible. Changed values
    /// update in place; edits not yet saved stay.
    /// </summary>
    public async Task RefreshAsync()
    {
        if (_disposed)
        {
            return;
        }

        try
        {
            var reply = await _client.SettingsAsync(_lifetime.Token);
            if (_disposed)
            {
                return;
            }

            if (reply.Value is { } state)
            {
                Take(state);
                if (_pending.Count > 0 && !ReadOnly)
                {
                    Schedule(TimeSpan.Zero);
                }
            }
            else if (reply.Failure is { } failure)
            {
                await UnansweredAsync(failure);
            }

            Notify();
        }
        catch (OperationCanceledException) when (_disposed)
        {
            // The session was disposed while the request was out.
        }
    }

    /// <summary>
    /// Starts or stops fetching every 5 s. Hiding the window sends a pending burst first.
    /// </summary>
    public void SetVisible(bool visible)
    {
        _visible = visible;
        ClearRefresh();
        if (visible)
        {
            Tick();
        }
        else if (_pending.Count > 0)
        {
            _ = FlushAsync();
        }
    }

    /// <summary>
    /// Sends the pending changes now instead of after the burst.
    /// </summary>
    public async Task FlushAsync()
    {
        try
        {
            await FlushCoreAsync();
        }
        catch (OperationCanceledException) when (_disposed)
        {
            // The session was disposed while the request was out.
        }
    }

    /// <summary>
    /// Sends the changes a hand edit got in the way of again, over the file's values.
    /// </summary>
    public void ApplyMine()
    {
        if (Conflict is not { } conflict)
        {
            return;
        }

        Conflict = null;
        foreach (var edit in conflict.Edits)
        {
            _pending[edit.Setting.Key] = new Pending(
                edit.Setting, edit.Changes, edit.Setting.Generation);
        }

        _ = FlushAsync();
    }

    /// <summary>
    /// Drops the changes a hand edit got in the way of; the file's values stay.
    /// </summary>
    public void KeepFileValues()
    {
        Conflict = null;
        Notify();
    }

    /// <summary>
    /// Resets every changed setting on a page, or on every page, in one write. The window asks
    /// first.
    /// </summary>
    /// <param name="pageId"><c>null</c> for Reset all.</param>
    public void ResetChanged(string? pageId)
    {
        var settings = pageId is null ? ChangedSettings() : ChangedOn(pageId);
        foreach (var model in settings)
        {
            model.Reset();
        }

        _ = FlushAsync();
    }

    /// <summary>
    /// What fits with the shown values, unsaved edits included.
    /// </summary>
    public Task<SettingsReply<FitPreview>> PreviewAsync(
        CancellationToken cancellationToken = default) =>
        _client.PreviewSettingsAsync(Unsaved(), cancellationToken);

    /// <summary>
    /// How the hook treats <paramref name="line"/>, under the shown values and
    /// <paramref name="changes"/>, such as a rule list edited in a dialog.
    /// </summary>
    public Task<SettingsReply<CommandTest>> ClassifyAsync(
        string line,
        Shell shell,
        IReadOnlyList<Change>? changes = null,
        CancellationToken cancellationToken = default)
    {
        var all = Unsaved();
        if (changes is not null)
        {
            all.AddRange(changes);
        }

        var request = new ClassifyRequest { Line = line, Shell = shell, Changes = all };
        return _client.ClassifyAsync(request, cancellationToken);
    }

    /// <summary>
    /// Stops the timers; answers that arrive later are dropped.
    /// </summary>
    public void Dispose()
    {
        if (_disposed)
        {
            return;
        }

        _disposed = true;
        ClearSave();
        ClearRefresh();
        _lifetime.Cancel();
        _lifetime.Dispose();
    }

    bool? IValueHost.SwitchOf(string key) => (Setting(key) as SwitchModel)?.Value;

    double? IValueHost.NumberOf(string key) => (Setting(key) as NumberModel)?.Value;

    IReadOnlyList<SizeRow>? IValueHost.RowsOf(string key) =>
        (Setting(key) as SizeTableModel)?.Value;

    string IValueHost.LabelOf(string key) => Setting(key)?.Label ?? key;

    void IValueHost.Edited(ValueModel model, IReadOnlyList<Change> changes, bool now)
    {
        if (changes.Count == 0)
        {
            _ = _pending.Remove(model.Key);
        }
        else
        {
            _pending[model.Key] = new Pending(model, changes, model.Generation);
        }

        if (now)
        {
            _ = FlushAsync();
            return;
        }

        if (changes.Count > 0)
        {
            Schedule(SaveDelay);
        }

        Notify();
    }

    private static StopRecord CliStop() => new() { By = StoppedBy.Cli, StoppedAtMs = 0 };

    private static List<ValueModel> ChangedIn(IEnumerable<PageModel> pages)
    {
        var settings = new List<ValueModel>();
        foreach (var group in pages.SelectMany(page => page.Groups))
        {
            settings.AddRange(group.Settings.Where(model => model.CanReset));
        }

        return settings;
    }

    // Whether the setting `key` owns a problem reported for `problemKey`: the key itself, a row
    // of a table (presets.big.min_cores) or a rule (classifier.rules[2].program).
    private static bool Owns(string key, string problemKey) =>
        problemKey == key
        || problemKey.StartsWith($"{key}.", StringComparison.Ordinal)
        || problemKey.StartsWith($"{key}[", StringComparison.Ordinal);

    private async Task FlushCoreAsync()
    {
        ClearSave();
        if (_disposed
            || _saving
            || _pending.Count == 0
            || ReadOnly
            || _revision is not { } revision)
        {
            Notify();
            return;
        }

        var batch = _pending.Values.ToList();
        _pending.Clear();
        _saving = true;
        Notify();
        var patch = new SettingsPatch
        {
            Revision = revision,
            Changes = [.. batch.SelectMany(entry => entry.Changes)],
        };
        SettingsReply<SettingsState> reply;
        try
        {
            reply = await _client.ChangeSettingsAsync(patch, _lifetime.Token);
        }
        finally
        {
            _saving = false;
        }

        if (_disposed)
        {
            return;
        }

        if (reply.Value is { } state)
        {
            WriteProblem = null;
            foreach (var entry in batch)
            {
                entry.Model.MarkSaved(entry.Generation);
            }

            Take(state);
        }
        else if (reply.Failure is { } failure)
        {
            await RefusedAsync(batch, failure);
        }

        if (_pending.Count > 0 && _saveTimer is null)
        {
            Schedule(SaveDelay);
        }

        Notify();
    }

    private void Take(SettingsState state)
    {
        Status = SessionStatus.Ready;
        Detail = null;
        StopRecord = null;
        _revision = state.Revision;
        FileError = state.FileError;
        PendingRestart = state.PendingRestart;
        ListeningPort = state.ListeningPort;
        var values = new Dictionary<string, SettingValue>(StringComparer.Ordinal);
        foreach (var entry in state.Values)
        {
            if (ValueModels.KeyOf(entry) is { } key)
            {
                values[key] = entry;
            }
        }

        if (_models.Count == 0)
        {
            Pages = Build(state, values);
            return;
        }

        foreach (var model in _models.Values)
        {
            if (values.TryGetValue(model.Key, out var value))
            {
                _ = model.Take(value);
            }
        }
    }

    private List<PageModel> Build(SettingsState state, Dictionary<string, SettingValue> values)
    {
        var pages = new List<PageModel>();
        foreach (var page in state.Spec.Pages)
        {
            var groups = new List<GroupModel>();
            foreach (var group in page.Groups)
            {
                var settings = new List<ValueModel>();
                foreach (var spec in group.Settings)
                {
                    if ((spec.Platforms.Count == 0 || spec.Platforms.Contains(_platform))
                        && values.TryGetValue(spec.Key, out var value)
                        && ValueModels.Create(spec, this, value) is { } model)
                    {
                        settings.Add(model);
                        _models[model.Key] = model;
                    }
                }

                if (settings.Count > 0)
                {
                    groups.Add(new GroupModel(group, settings));
                }
            }

            pages.Add(new PageModel(page, groups));
        }

        return pages;
    }

    private async Task UnansweredAsync(SettingsFailure failure)
    {
        Detail = failure.Message;
        switch (failure.Kind)
        {
            case SettingsFailureKind.Unreachable:
                var text = await _readStopRecord(_lifetime.Token);
                StopRecord = ParseStopRecord(text);
                Status = StopRecord is null ? SessionStatus.Down : SessionStatus.Stopped;
                break;
            case SettingsFailureKind.Token:
                Status = SessionStatus.Token;
                break;
            default:
                Status = SessionStatus.Failed;
                break;
        }
    }

    private async Task RefusedAsync(List<Pending> batch, SettingsFailure failure)
    {
        switch (failure.Kind)
        {
            case SettingsFailureKind.Conflict:
                Conflict = new Conflict(
                    [.. batch.Select(entry => new ConflictEdit(entry.Model, entry.Changes))]);
                UndoAll(batch);
                await RefreshAsync();
                return;
            case SettingsFailureKind.Invalid:
                if (Route(batch, failure))
                {
                    Schedule(TimeSpan.Zero);
                    return;
                }

                // The file itself is broken, so nothing can be saved until it is fixed by hand.
                UndoAll(batch);
                await RefreshAsync();
                return;
            case SettingsFailureKind.Token:
            case SettingsFailureKind.Unreachable:
                Restore(batch);
                await UnansweredAsync(failure);
                return;
            default:
                WriteProblem = failure.Message;
                Restore(batch);
                return;
        }
    }

    // Shows each refused value's reason under its field, and sends the rest of the batch again.
    // Returns whether any problem belonged to a change in the batch.
    private bool Route(List<Pending> batch, SettingsFailure failure)
    {
        var refused = new HashSet<ValueModel>();
        foreach (var problem in failure.Problems)
        {
            var entry = batch.FirstOrDefault(candidate => Owns(candidate.Model.Key, problem.Key));
            if (entry is not null)
            {
                entry.Model.Refuse(entry.Model.DescribeProblem(problem.Key, problem.Message));
                _ = refused.Add(entry.Model);
            }
        }

        if (refused.Count == 0)
        {
            return false;
        }

        Restore(batch.Where(entry => !refused.Contains(entry.Model)));
        return true;
    }

    private static void UndoAll(List<Pending> batch)
    {
        foreach (var entry in batch)
        {
            entry.Model.Undo();
        }
    }

    // Puts changes back in the burst, unless the user changed the same setting since.
    private void Restore(IEnumerable<Pending> batch)
    {
        foreach (var entry in batch)
        {
            _ = _pending.TryAdd(entry.Model.Key, entry);
        }
    }

    private List<Change> Unsaved() =>
        [.. _pending.Values.SelectMany(entry => entry.Changes)];

    private void Schedule(TimeSpan delay)
    {
        ClearSave();
        _saveTimer = _timers.Schedule(delay, OnSaveTimer);
    }

    private void OnSaveTimer()
    {
        _saveTimer = null;
        if (!_disposed)
        {
            _ = FlushAsync();
        }
    }

    private void Tick()
    {
        _refreshTimer = _timers.Schedule(RefreshInterval, OnRefreshTimer);
        _ = RefreshAsync();
    }

    private void OnRefreshTimer()
    {
        _refreshTimer = null;
        if (_visible && !_disposed)
        {
            Tick();
        }
    }

    private void ClearSave()
    {
        _saveTimer?.Dispose();
        _saveTimer = null;
    }

    private void ClearRefresh()
    {
        _refreshTimer?.Dispose();
        _refreshTimer = null;
    }

    private void Notify() => Raise(null);

    private sealed record Pending(ValueModel Model, IReadOnlyList<Change> Changes, int Generation);
}
