using Allot.Tray.Core.Contract;

namespace Allot.Tray.Core.ServiceSettings;

/// <summary>
/// What is wrong with a value the user entered.
/// </summary>
public enum IssueKind
{
    /// <summary>
    /// A number the setting cannot do without is missing.
    /// </summary>
    Required,

    /// <summary>
    /// The text is not a finite number.
    /// </summary>
    NotANumber,

    /// <summary>
    /// The setting takes whole numbers only.
    /// </summary>
    Whole,

    /// <summary>
    /// Outside the setting's own range, <see cref="ValueIssue.Min"/> and
    /// <see cref="ValueIssue.Max"/>.
    /// </summary>
    Range,

    /// <summary>
    /// Below another setting's value, named by <see cref="ValueIssue.Label"/>.
    /// </summary>
    AtLeast,

    /// <summary>
    /// Above another setting's value, named by <see cref="ValueIssue.Label"/>.
    /// </summary>
    AtMost,

    /// <summary>
    /// Text that is empty.
    /// </summary>
    Blank,

    /// <summary>
    /// Text that does not match the pattern the spec names.
    /// </summary>
    Pattern,

    /// <summary>
    /// A list entry the list already has.
    /// </summary>
    Duplicate,

    /// <summary>
    /// A row name another row or a class already has.
    /// </summary>
    NameTaken,

    /// <summary>
    /// allotd's own reason for a refused value, in <see cref="ValueIssue.Message"/>.
    /// </summary>
    Service,
}

/// <summary>
/// What is wrong with a value the user entered, for the line under its field; the sentences are
/// <see cref="ValueText.IssueText"/>. Only the members of its <see cref="Kind"/> are set.
/// </summary>
/// <param name="Kind">What is wrong.</param>
/// <param name="Min">Range: the lower end of the range.</param>
/// <param name="Max">Range: the upper end of the range.</param>
/// <param name="Label">AtLeast and AtMost: the label of the setting compared with.</param>
/// <param name="Value">AtLeast and AtMost: that setting's value.</param>
/// <param name="Quantity">AtLeast and AtMost: what that value counts.</param>
/// <param name="Message">Service: allotd's reason.</param>
public sealed record ValueIssue(
    IssueKind Kind,
    Bound? Min = null,
    Bound? Max = null,
    string? Label = null,
    double Value = 0,
    Quantity Quantity = Quantity.Percent,
    string? Message = null)
{
    public static readonly ValueIssue Required = new(IssueKind.Required);

    public static readonly ValueIssue NotANumber = new(IssueKind.NotANumber);

    public static readonly ValueIssue Whole = new(IssueKind.Whole);

    public static readonly ValueIssue Blank = new(IssueKind.Blank);

    public static readonly ValueIssue Pattern = new(IssueKind.Pattern);

    public static readonly ValueIssue Duplicate = new(IssueKind.Duplicate);

    public static readonly ValueIssue NameTaken = new(IssueKind.NameTaken);

    public static ValueIssue Range(Bound? min, Bound? max) =>
        new(IssueKind.Range, Min: min, Max: max);

    public static ValueIssue AtLeast(string label, double value, Quantity quantity) =>
        new(IssueKind.AtLeast, Label: label, Value: value, Quantity: quantity);

    public static ValueIssue AtMost(string label, double value, Quantity quantity) =>
        new(IssueKind.AtMost, Label: label, Value: value, Quantity: quantity);

    public static ValueIssue Service(string message) =>
        new(IssueKind.Service, Message: message);
}
