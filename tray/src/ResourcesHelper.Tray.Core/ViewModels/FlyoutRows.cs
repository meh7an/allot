namespace ResourcesHelper.Tray.Core.ViewModels;

/// <summary>
/// One entry of the flyout body, in display order: a group heading, a job card or the idle
/// line. Rows are matched between polls by <see cref="Key"/>, so a row that stays keeps its
/// control and only its changed values update.
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

    public override string Key => $"group:{Kind}";

    public override void Update(FlyoutRow fresh) => Heading = ((GroupHeadingRow)fresh).Heading;
}

/// <summary>
/// A job card: the session strip and the command line.
/// </summary>
public sealed class JobCardRow : FlyoutRow
{
    private string _shortId;
    private string _project;
    private string _projectPath;
    private string _usage;
    private string _command;
    private bool _isQueued;

    public JobCardRow(JobItem job)
    {
        LeaseId = job.Lease.Id;
        _shortId = job.Session.ShortId;
        _project = job.Session.Project;
        _projectPath = job.Session.ProjectPath;
        _usage = job.Session.Usage;
        _command = job.Lease.Label;
        _isQueued = job.Queued is not null;
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

    public bool IsQueued
    {
        get => _isQueued;
        private set => Set(ref _isQueued, value);
    }

    public override string Key => $"job:{LeaseId}";

    public override void Update(FlyoutRow fresh)
    {
        var card = (JobCardRow)fresh;
        ShortId = card.ShortId;
        Project = card.Project;
        ProjectPath = card.ProjectPath;
        Usage = card.Usage;
        Command = card.Command;
        IsQueued = card.IsQueued;
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
