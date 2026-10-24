using System.Collections.ObjectModel;

using ResourcesHelper.Tray.Core.Settings;

namespace ResourcesHelper.Tray.Core.ViewModels;

/// <summary>
/// The observable state the flyout binds to. Each poll's <see cref="FlyoutSnapshot"/> is applied
/// in place: rows are matched by key, so the window animates only rows that really enter or
/// leave, and unchanged values raise no change notification.
/// </summary>
public sealed class FlyoutModel : Observable
{
    private HeaderGlyph _glyph = HeaderGlyph.Error;
    private string _summary = "Starting service…";
    private bool _isLastKnown = true;
    private string _metersCount = MenuCounts.Meters(new MeterToggles());
    private string _notificationsCount = MenuCounts.Notifications(new NotificationToggles());

    public HeaderGlyph Glyph
    {
        get => _glyph;
        private set => Set(ref _glyph, value);
    }

    public string Summary
    {
        get => _summary;
        private set => Set(ref _summary, value);
    }

    /// <summary>
    /// The rows come from the last state rhd served before it went away.
    /// </summary>
    public bool IsLastKnown
    {
        get => _isLastKnown;
        private set => Set(ref _isLastKnown, value);
    }

    /// <summary>
    /// The trailing count of ⋯ › Meters, for example <c>4 of 5</c>.
    /// </summary>
    public string MetersCount
    {
        get => _metersCount;
        private set => Set(ref _metersCount, value);
    }

    /// <summary>
    /// The trailing count of ⋯ › Notifications.
    /// </summary>
    public string NotificationsCount
    {
        get => _notificationsCount;
        private set => Set(ref _notificationsCount, value);
    }

    public ObservableCollection<FlyoutRow> Rows { get; } = [];

    /// <summary>
    /// The Resources card docked below the rows.
    /// </summary>
    public ResourcesModel Resources { get; } = new();

    public void Apply(FlyoutSnapshot snapshot)
    {
        Glyph = snapshot.Header.Glyph;
        Summary = snapshot.Header.Summary;
        IsLastKnown = snapshot.IsLastKnown;
        Sync(Rows, RowsOf(snapshot));
    }

    public void ApplyMenu(TraySettings settings)
    {
        MetersCount = MenuCounts.Meters(settings.Meters);
        NotificationsCount = MenuCounts.Notifications(settings.Notifications);
    }

    /// <summary>
    /// Makes <paramref name="rows"/> equal to <paramref name="fresh"/> by key with the fewest
    /// removes, moves and inserts; rows that stay are updated in place.
    /// </summary>
    public static void Sync(ObservableCollection<FlyoutRow> rows, IReadOnlyList<FlyoutRow> fresh)
    {
        var wanted = fresh.Select(row => row.Key).ToHashSet(StringComparer.Ordinal);
        for (var i = rows.Count - 1; i >= 0; i--)
        {
            if (!wanted.Contains(rows[i].Key))
            {
                rows.RemoveAt(i);
            }
        }

        for (var i = 0; i < fresh.Count; i++)
        {
            var row = fresh[i];
            var existing = IndexOf(rows, row.Key, i);
            if (existing < 0)
            {
                rows.Insert(i, row);
                continue;
            }

            if (existing != i)
            {
                rows.Move(existing, i);
            }

            rows[i].Update(row);
        }
    }

    private static List<FlyoutRow> RowsOf(FlyoutSnapshot snapshot)
    {
        var rows = new List<FlyoutRow>();
        foreach (var group in snapshot.Groups)
        {
            rows.Add(new GroupHeadingRow(group.Kind, group.Heading));
            rows.AddRange(group.Jobs.Select(job => new JobCardRow(job)));
        }

        if (snapshot.Idle is { } idle)
        {
            rows.Add(new IdleRow(idle));
        }

        if (snapshot.ShowNothing)
        {
            rows.Add(new NothingRow());
        }

        if (snapshot.Recent.Count > 0)
        {
            rows.Add(new RecentRow(snapshot.Recent));
        }

        return rows;
    }

    private static int IndexOf(ObservableCollection<FlyoutRow> rows, string key, int from)
    {
        for (var i = from; i < rows.Count; i++)
        {
            if (string.Equals(rows[i].Key, key, StringComparison.Ordinal))
            {
                return i;
            }
        }

        return -1;
    }
}
