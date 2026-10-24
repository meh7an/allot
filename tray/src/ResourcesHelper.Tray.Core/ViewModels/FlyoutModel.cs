using System.Collections.ObjectModel;

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

    public ObservableCollection<FlyoutRow> Rows { get; } = [];

    public void Apply(FlyoutSnapshot snapshot)
    {
        Glyph = snapshot.Header.Glyph;
        Summary = snapshot.Header.Summary;
        IsLastKnown = snapshot.IsLastKnown;
        Sync(Rows, RowsOf(snapshot));
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
