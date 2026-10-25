using System.Collections.ObjectModel;

namespace Allot.Tray.Core.ViewModels;

/// <summary>
/// One entry of the flyout body, in display order: a group heading, a job card, the idle line,
/// the first-run card, the empty line or the Recently finished card. Rows are matched between
/// polls by <see cref="Key"/>, so a row that stays keeps its control and only its changed values
/// update.
/// </summary>
public abstract class FlyoutRow : Observable
{
    public abstract string Key { get; }

    /// <summary>
    /// Copies the values of a freshly built row with the same key.
    /// </summary>
    public abstract void Update(FlyoutRow fresh);
}

public sealed class GroupHeadingRow(JobGroupKind kind, string heading) : FlyoutRow
{
    private string _heading = heading;

    public JobGroupKind Kind { get; } = kind;

    public string Heading
    {
        get => _heading;
        private set => Set(ref _heading, value);
    }

    // Built once: Sync reads keys many times on every update.
    public override string Key { get; } = $"group:{kind}";

    public override void Update(FlyoutRow fresh) => Heading = ((GroupHeadingRow)fresh).Heading;
}

/// <summary>
/// A job card: the session strip, then the job row with its tile, command, chip, action, detail
/// line, time and edge bar.
/// </summary>
public sealed class JobCardRow : FlyoutRow
{
    private string _shortId;
    private string _project;
    private string _projectPath;
    private string _usage;
    private string _command;
    private JobCard _card;

    public JobCardRow(JobItem job)
    {
        LeaseId = job.Id;
        Key = $"job:{job.Id}";
        _shortId = job.Session.ShortId;
        _project = job.Session.Project;
        _projectPath = job.Session.ProjectPath;
        _usage = job.Session.Usage;
        _command = job.Command;
        _card = job.Card;
    }

    public long LeaseId { get; }

    public string ShortId
    {
        get => _shortId;
        private set => Set(ref _shortId, value);
    }

    public string Project
    {
        get => _project;
        private set => Set(ref _project, value);
    }

    /// <summary>
    /// The full working directory, for the project's tooltip.
    /// </summary>
    public string ProjectPath
    {
        get => _projectPath;
        private set => Set(ref _projectPath, value);
    }

    public string Usage
    {
        get => _usage;
        private set => Set(ref _usage, value);
    }

    public string Command
    {
        get => _command;
        private set => Set(ref _command, value);
    }

    /// <summary>
    /// The job row. Bindings read its members through the properties below, which raise a change
    /// only for the members that changed.
    /// </summary>
    public JobCard Card => _card;

    public string Tile => _card.Tile;

    public bool IsQueued => _card.IsQueued;

    public string? ExePath => _card.ExePath;

    public string Chip => _card.Chip;

    public string ChipTooltip => _card.ChipTooltip;

    public bool IsBenchmark => _card.IsBenchmark;

    public string Strong => _card.Strong;

    public string Detail => _card.Detail;

    public string DetailTooltip => _card.DetailTooltip;

    public bool DetailIsDanger => _card.DetailIsDanger;

    public string Time => _card.Time;

    public bool TimeIsWarning => _card.TimeIsWarning;

    public double BarFraction => _card.BarFraction;

    public EdgeBarTone BarTone => _card.BarTone;

    public CardAction Action => _card.Action;

    public string ActionName => _card.ActionName;

    public bool IsEnded => _card.IsEnded;

    public bool IsStruck => _card.IsStruck;

    public DetailIcon Icon => _card.Icon;

    public bool HasSteps => _card.Steps is not null;

    public BenchmarkStep Step => _card.Steps?.Current ?? BenchmarkStep.Drain;

    public int QuietTicks => _card.Steps?.QuietTicks ?? 0;

    public double RunFraction => _card.Steps?.RunFraction ?? 0;

    public override string Key { get; }

    public override void Update(FlyoutRow fresh)
    {
        var card = (JobCardRow)fresh;
        ShortId = card.ShortId;
        Project = card.Project;
        ProjectPath = card.ProjectPath;
        Usage = card.Usage;
        Command = card.Command;
        UpdateCard(card._card);
    }

    private void UpdateCard(JobCard fresh)
    {
        var old = _card;
        if (old == fresh)
        {
            return;
        }

        _card = fresh;
        Changed(old.Tile != fresh.Tile, nameof(Tile));
        Changed(old.IsQueued != fresh.IsQueued, nameof(IsQueued));
        Changed(old.ExePath != fresh.ExePath, nameof(ExePath));
        Changed(old.Chip != fresh.Chip, nameof(Chip));
        Changed(old.ChipTooltip != fresh.ChipTooltip, nameof(ChipTooltip));
        Changed(old.IsBenchmark != fresh.IsBenchmark, nameof(IsBenchmark));
        Changed(old.Strong != fresh.Strong, nameof(Strong));
        Changed(old.Detail != fresh.Detail, nameof(Detail));
        Changed(old.DetailTooltip != fresh.DetailTooltip, nameof(DetailTooltip));
        Changed(old.DetailIsDanger != fresh.DetailIsDanger, nameof(DetailIsDanger));
        Changed(old.Time != fresh.Time, nameof(Time));
        Changed(old.TimeIsWarning != fresh.TimeIsWarning, nameof(TimeIsWarning));
        Changed(old.BarFraction != fresh.BarFraction, nameof(BarFraction));
        Changed(old.BarTone != fresh.BarTone, nameof(BarTone));
        Changed(old.Action != fresh.Action, nameof(Action));
        Changed(old.ActionName != fresh.ActionName, nameof(ActionName));
        Changed(old.IsEnded != fresh.IsEnded, nameof(IsEnded));
        Changed(old.IsStruck != fresh.IsStruck, nameof(IsStruck));
        Changed(old.Icon != fresh.Icon, nameof(Icon));
        Changed((old.Steps is null) != (fresh.Steps is null), nameof(HasSteps));
        Changed(old.Steps?.Current != fresh.Steps?.Current, nameof(Step));
        Changed(old.Steps?.QuietTicks != fresh.Steps?.QuietTicks, nameof(QuietTicks));
        Changed(old.Steps?.RunFraction != fresh.Steps?.RunFraction, nameof(RunFraction));
        Changed(true, nameof(Card));
    }

    private void Changed(bool changed, string name)
    {
        if (changed)
        {
            Raise(name);
        }
    }
}

public sealed class IdleRow(IdleLine line) : FlyoutRow
{
    private string _label = line.Label;
    private string _ids = line.Ids;
    private string _usage = line.Usage;

    public string Label
    {
        get => _label;
        private set => Set(ref _label, value);
    }

    public string Ids
    {
        get => _ids;
        private set => Set(ref _ids, value);
    }

    public string Usage
    {
        get => _usage;
        private set => Set(ref _usage, value);
    }

    public override string Key => "idle";

    public override void Update(FlyoutRow fresh)
    {
        var idle = (IdleRow)fresh;
        Label = idle.Label;
        Ids = idle.Ids;
        Usage = idle.Usage;
    }
}

/// <summary>
/// The Recently finished heading and its card of rows, shown while nothing runs or waits.
/// </summary>
public sealed class RecentRow(IReadOnlyList<RecentLine> lines) : FlyoutRow
{
    /// <summary>
    /// Changed in place, like the Resources card's lists. Under Native AOT, handing XAML a new
    /// <c>List</c> through the binding failed with E_INVALIDARG in <c>ItemsSource</c>, which
    /// ended the app.
    /// </summary>
    public ObservableCollection<RecentLine> Lines { get; } = new(lines);

    public override string Key => "recent";

    public override void Update(FlyoutRow fresh)
    {
        var lines = ((RecentRow)fresh).Lines;
        if (lines.SequenceEqual(Lines))
        {
            return;
        }

        Lines.Clear();
        foreach (var line in lines)
        {
            Lines.Add(line);
        }
    }
}

/// <summary>
/// The first-run card with its Launch at logon switch and Done.
/// </summary>
public sealed class WelcomeRow(WelcomeCard card) : FlyoutRow
{
    private bool _launchAtLogon = card.LaunchAtLogon;

    public bool LaunchAtLogon
    {
        get => _launchAtLogon;
        private set => Set(ref _launchAtLogon, value);
    }

    public override string Key => "welcome";

    public override void Update(FlyoutRow fresh) =>
        LaunchAtLogon = ((WelcomeRow)fresh).LaunchAtLogon;
}

/// <summary>
/// The "No jobs running or waiting." line.
/// </summary>
public sealed class NothingRow : FlyoutRow
{
    public override string Key => "nothing";

    public override void Update(FlyoutRow fresh)
    {
    }
}
