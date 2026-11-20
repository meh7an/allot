using Allot.Tray.Core.Contract;
using Allot.Tray.Core.ServiceSettings;

using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Input;

using Windows.Globalization.NumberFormatting;
using Windows.System;

namespace Allot.Tray.Settings;

/// <summary>
/// What every editor on a page shares: the session, the number format and the Undo toast.
/// </summary>
/// <param name="Session">The settings session.</param>
/// <param name="Number">Formats a number for the user's locale.</param>
/// <param name="Toast">Shows a brief notice, with an Undo action or none.</param>
/// <param name="Confirm">Asks in a dialog; true when the user chose the primary button.</param>
internal sealed record PageContext(
    SettingsSession Session,
    Func<double, string> Number,
    Action<string, Action?> Toast,
    Func<Confirmation, Task<bool>> Confirm);

/// <summary>
/// The rows of one allotd setting, each control kept in step with its view model (DESIGN.md
/// section 3). A number commits on Enter, on leaving the field or on a spin step; Escape shows the
/// saved value again.
/// </summary>
internal static class ValueEditors
{
    /// <summary>
    /// The rows of <paramref name="model"/>: one, or two for a duration with an off switch.
    /// </summary>
    public static IReadOnlyList<SettingRow> For(ValueModel model, PageContext context) =>
        model switch
        {
            SwitchModel switchModel => [SwitchRow(switchModel, context)],
            DurationModel { Off: { } off } duration => OffDurationRows(duration, off, context),
            NumberModel number => [NumberRow(number, context, model.Key)],
            _ => [],
        };

    private static SettingRow SwitchRow(SwitchModel model, PageContext context)
    {
        var toggle = new ToggleSwitch();
        var row = new SettingRow(model.Label, model.Spec.Help, model.Key, toggle);
        var syncing = false;
        toggle.Toggled += (_, _) =>
        {
            if (!syncing && toggle.IsOn != model.Value)
            {
                model.Turn(toggle.IsOn);
            }
        };
        Follow(model, context, () =>
        {
            syncing = true;
            toggle.IsOn = model.Value;
            syncing = false;
            toggle.IsEnabled = Editable(model, context);
            ShowState(row, model, context);
        });
        return row;
    }

    // "End leases left idle" with the duration indented under it; the switch row holds the key
    // and the reset button, the duration row the line under the value.
    private static List<SettingRow> OffDurationRows(
        DurationModel model,
        OffSwitch off,
        PageContext context)
    {
        var toggle = new ToggleSwitch();
        var switchRow = new SettingRow(off.Label, off.Help, model.Key, toggle);
        var editor = new NumberEditor(model);
        var durationRow = new SettingRow(model.Label, model.Spec.Help, null, editor.Root)
        {
            Indented = true,
        };
        var syncing = false;
        toggle.Toggled += (_, _) =>
        {
            if (!syncing && toggle.IsOn != model.IsOn)
            {
                model.SetOn(toggle.IsOn);
            }
        };
        Follow(model, context, () =>
        {
            syncing = true;
            toggle.IsOn = model.IsOn;
            syncing = false;
            toggle.IsEnabled = Editable(model, context);
            editor.Sync(Editable(model, context) && model.IsOn);
            switchRow.SetReset(
                SettingsText.ResetButtonName(model, context.Number),
                ResetAction(model, context));
            durationRow.SetNote(Note(model, context));
        });
        return [switchRow, durationRow];
    }

    private static SettingRow NumberRow(NumberModel model, PageContext context, string key)
    {
        var editor = new NumberEditor(model);
        var row = new SettingRow(model.Label, model.Spec.Help, key, editor.Root);
        Follow(model, context, () =>
        {
            editor.Sync(Editable(model, context));
            ShowState(row, model, context);
        });
        return row;
    }

    // The model changes on edits and refreshes; the session on everything else that bears on a row
    // (read-only states, a switch another setting requires, a restart).
    private static void Follow(ValueModel model, PageContext context, Action sync)
    {
        model.PropertyChanged += (_, _) => sync();
        context.Session.PropertyChanged += (_, _) => sync();
        sync();
    }

    private static bool Editable(ValueModel model, PageContext context) =>
        !context.Session.ReadOnly && model.Enabled;

    private static void ShowState(SettingRow row, ValueModel model, PageContext context)
    {
        var name = SettingsText.ResetButtonName(model, context.Number);
        row.SetReset(name, ResetAction(model, context));
        row.SetNote(Note(model, context));
    }

    private static Action? ResetAction(ValueModel model, PageContext context)
    {
        if (!model.CanReset || context.Session.ReadOnly)
        {
            return null;
        }

        return () =>
        {
            var undo = FieldReset.Reset(model);
            context.Toast(SettingsText.FieldReset(model, context.Number), undo);
        };
    }

    private static RowNote? Note(ValueModel model, PageContext context)
    {
        if (model.Issue is { } issue)
        {
            var undo = issue.Kind == IssueKind.Service ? model.Undo : (Action?)null;
            return new RowNote(RowNoteKind.Error, ValueText.IssueText(issue, context.Number), undo);
        }

        if (model.DisabledNote is { } disabled)
        {
            return new RowNote(RowNoteKind.Info, disabled);
        }

        return SettingsBanners.RestartNote(context.Session, model.Key) is { } restart
            ? new RowNote(RowNoteKind.Caution, restart)
            : null;
    }

    /// <summary>
    /// A number box with what goes beside it: a slider for a percent, a unit picker for a duration
    /// or a size, or the unit's name.
    /// </summary>
    private sealed class NumberEditor
    {
        private readonly NumberModel _model;
        private readonly NumberBox _box;
        private readonly Slider? _slider;
        private readonly ComboBox? _units;
        private bool _syncing;

        public NumberEditor(NumberModel model)
        {
            _model = model;
            _box = new NumberBox
            {
                Width = 132,
                SpinButtonPlacementMode = NumberBoxSpinButtonPlacementMode.Inline,
                ValidationMode = NumberBoxValidationMode.Disabled,
                SmallChange = 1,
                LargeChange = 10,
                NumberFormatter = Formatter(model.Quantity, model.Control.Integer),
            };
            _box.ValueChanged += OnBoxChanged;
            _box.PreviewKeyDown += OnKeyDown;
            var panel = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
            if (model.Quantity == Quantity.Percent)
            {
                _slider = new Slider
                {
                    Width = 160,
                    Minimum = model.Control.Min?.Value ?? 0,
                    Maximum = model.Control.Max?.Value ?? 100,
                    StepFrequency = 1,
                    VerticalAlignment = VerticalAlignment.Center,
                };
                AutomationProperties.SetName(_slider, $"{model.Label} slider");
                _slider.ValueChanged += OnSliderChanged;
                panel.Children.Add(_slider);
            }

            panel.Children.Add(_box);
            if (model.Units.Count > 0)
            {
                _units = new ComboBox { MinWidth = 108 };
                AutomationProperties.SetName(_units, $"{model.Label} unit");
                foreach (var unit in model.Units)
                {
                    _units.Items.Add(unit.Name);
                }

                _units.SelectionChanged += OnUnitChanged;
                panel.Children.Add(_units);
            }
            else if (UnitName(model.Quantity) is { Length: > 0 } unitName)
            {
                panel.Children.Add(new TextBlock
                {
                    Text = unitName,
                    VerticalAlignment = VerticalAlignment.Center,
                });
            }

            Root = panel;
        }

        /// <summary>
        /// The box and its companions; the row names the box after the setting.
        /// </summary>
        public FrameworkElement Root { get; }

        /// <summary>
        /// Shows the model's value, unit and enabled state without reporting them back.
        /// </summary>
        public void Sync(bool enabled)
        {
            _syncing = true;
            var shown = _model.Shown ?? double.NaN;
            if (!Same(_box.Value, shown))
            {
                _box.Value = shown;
            }

            if (_slider is not null && double.IsFinite(shown))
            {
                _slider.Value = Math.Clamp(shown, _slider.Minimum, _slider.Maximum);
            }

            if (_units is not null)
            {
                _units.SelectedIndex = IndexOf(_model.Units, _model.Unit);
                _units.IsEnabled = enabled;
            }

            _box.IsEnabled = enabled;
            if (_slider is not null)
            {
                _slider.IsEnabled = enabled;
            }

            _syncing = false;
        }

        private static bool Same(double left, double right) =>
            left.Equals(right) || (double.IsNaN(left) && double.IsNaN(right));

        private static int IndexOf(IReadOnlyList<ValueUnit> units, ValueUnit unit)
        {
            for (var i = 0; i < units.Count; i++)
            {
                if (units[i].Name == unit.Name)
                {
                    return i;
                }
            }

            return 0;
        }

        private static string UnitName(Quantity quantity) => quantity switch
        {
            Quantity.Percent => "%",
            Quantity.Runs => "runs",
            Quantity.Days => "days",
            Quantity.Cores => "cores",
            _ => string.Empty,
        };

        // Grouped digits in the user's format, except the port; at most two decimals.
        private static DecimalFormatter Formatter(Quantity quantity, bool integer) =>
            new()
            {
                IsGrouped = quantity != Quantity.Port,
                IntegerDigits = 1,
                FractionDigits = 0,
                NumberRounder = new IncrementNumberRounder
                {
                    Increment = integer ? 1 : 0.01,
                    RoundingAlgorithm = RoundingAlgorithm.RoundHalfAwayFromZero,
                },
            };

        private void OnBoxChanged(NumberBox sender, NumberBoxValueChangedEventArgs args)
        {
            if (!_syncing && !Same(args.NewValue, _model.Shown ?? double.NaN))
            {
                _model.Commit(args.NewValue);
            }
        }

        private void OnSliderChanged(object sender, RangeBaseValueChangedEventArgs args)
        {
            if (!_syncing && !Same(args.NewValue, _model.Shown ?? double.NaN))
            {
                _model.Commit(args.NewValue);
            }
        }

        private void OnUnitChanged(object sender, SelectionChangedEventArgs args)
        {
            if (!_syncing && _units?.SelectedItem is string name)
            {
                _model.SetUnit(name);
            }
        }

        private void OnKeyDown(object sender, KeyRoutedEventArgs args)
        {
            if (args.Key == VirtualKey.Escape && (_model.Dirty || _model.Issue is not null))
            {
                _model.Undo();
                args.Handled = true;
            }
        }
    }
}
