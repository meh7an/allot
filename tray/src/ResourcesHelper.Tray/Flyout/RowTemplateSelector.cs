using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

using ResourcesHelper.Tray.Core.ViewModels;

namespace ResourcesHelper.Tray.Flyout;

/// <summary>
/// Picks the template for a body row. Each group heading has its own template, because its icon
/// font and colour are theme resources that a binding could not follow.
/// </summary>
public sealed partial class RowTemplateSelector : DataTemplateSelector
{
    public DataTemplate? BenchmarkHeading { get; set; }

    public DataTemplate? RunningHeading { get; set; }

    public DataTemplate? WaitingHeading { get; set; }

    public DataTemplate? JobCard { get; set; }

    public DataTemplate? IdleLine { get; set; }

    protected override DataTemplate? SelectTemplateCore(object item) => item switch
    {
        GroupHeadingRow { Kind: JobGroupKind.Benchmark } => BenchmarkHeading,
        GroupHeadingRow { Kind: JobGroupKind.Running } => RunningHeading,
        GroupHeadingRow { Kind: JobGroupKind.Waiting } => WaitingHeading,
        JobCardRow => JobCard,
        IdleRow => IdleLine,
        _ => null,
    };

    protected override DataTemplate? SelectTemplateCore(object item, DependencyObject container) =>
        SelectTemplateCore(item);
}
