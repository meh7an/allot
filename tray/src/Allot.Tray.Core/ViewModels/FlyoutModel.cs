using System.Collections.ObjectModel;

using Allot.Tray.Core.Settings;

namespace Allot.Tray.Core.ViewModels;

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
    private ServiceBanner _banner = ServiceBanner.Hidden;
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
    /// The rows come from the last state allotd served before it went away.
    /// </summary>
    public bool IsLastKnown
    {
        get => _isLastKnown;
        private set => Set(ref _isLastKnown, value);
    }

    /// <summary>
    /// The service-down banner and last known caption, or <see cref="ServiceBanner.Hidden"/>
    /// while the service runs.
    /// </summary>
    public ServiceBanner Banner
    {
        get => _banner;
        private set => Set(ref _banner, value);
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

    /// <summary>
    /// Raised by <see cref="Apply"/> once for each card whose stop ended or was refused, with
    /// its <see cref="JobCard.Announcement"/>; focus has moved on, so Narrator is told this way.
    /// </summary>
    public event Action<string>? Announced;

    public void Apply(FlyoutSnapshot snapshot)
    {
        var before = Rows.OfType<JobCardRow>()
            .ToDictionary(row => row.Key, row => row.Card.Announcement, StringComparer.Ordinal);
        Glyph = snapshot.Header.Glyph;
        Summary = snapshot.Header.Summary;
        IsLastKnown = snapshot.IsLastKnown;
        Sync(Rows, RowsOf(snapshot));

        foreach (var row in Rows.OfType<JobCardRow>())
        {
            var now = row.Card.Announcement;
            if (now.Length > 0 && before.GetValueOrDefault(row.Key) != now)
            {
                Announced?.Invoke(now);
            }
        }
    }

    /// <summary>
    /// True when <paramref name="snapshot"/> has rows that come or go, or move: applying it
    /// builds or drops controls, which a closed flyout does at once so a click doesn't wait.
    /// </summary>
    public bool ChangesRows(FlyoutSnapshot snapshot)
    {
        var fresh = RowsOf(snapshot);
        if (fresh.Count != Rows.Count)
        {
            return true;
        }

        for (var i = 0; i < fresh.Count; i++)
        {
            if (!string.Equals(fresh[i].Key, Rows[i].Key, StringComparison.Ordinal))
            {
                return true;
            }
        }

        return false;
    }

    public void ApplyBanner(ServiceBanner? banner) => Banner = banner ?? ServiceBanner.Hidden;

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

        if (snapshot.Welcome is { } welcome)
        {
            rows.Add(new WelcomeRow(welcome));
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
