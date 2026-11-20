using Allot.Tray.Core.Contract;

namespace Allot.Tray.Core.ServiceSettings;

/// <summary>
/// Where a value model lives: a Settings page, saved by the <see cref="SettingsSession"/>, or the
/// draft of an edit dialog, kept until its Save.
/// </summary>
public interface IValueHost
{
    /// <summary>
    /// Whether another switch is on now, edits included; <c>null</c> when there is no such switch.
    /// </summary>
    bool? SwitchOf(string key);

    /// <summary>
    /// The number another setting or field shows now, edits included; <c>null</c> when there is
    /// no such number, or it is automatic.
    /// </summary>
    double? NumberOf(string key);

    /// <summary>
    /// The rows of another size table, edits included; <c>null</c> when there is none.
    /// </summary>
    IReadOnlyList<SizeRow>? RowsOf(string key);

    /// <summary>
    /// The label of another setting or field, for issues that name it.
    /// </summary>
    string LabelOf(string key);

    /// <summary>
    /// The user changed <paramref name="model"/>.
    /// </summary>
    /// <param name="model">The model that changed.</param>
    /// <param name="changes">What saves it; empty while the model has an issue or went back to
    /// its saved value.</param>
    /// <param name="now">Saves without waiting for the burst to end.</param>
    void Edited(ValueModel model, IReadOnlyList<Change> changes, bool now);
}
