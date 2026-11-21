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
/// A number box with what goes beside it: a slider for a percent, a unit picker for a duration
/// or a size, or the unit's name.
/// </summary>
internal sealed class NumberEditor
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
            // A minimum, not a width, so a long number at 200 % text widens the box (M7.1).
            MinWidth = 132,
            SpinButtonPlacementMode = NumberBoxSpinButtonPlacementMode.Inline,
            ValidationMode = NumberBoxValidationMode.Disabled,
            SmallChange = 1,
            LargeChange = 10,
            NumberFormatter = Formatter(model.Quantity, model.ShowsWholeNumbers),
        };
        _box.ValueChanged += OnBoxChanged;
        _box.PreviewKeyDown += OnKeyDown;
        var panel = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        if (model.Quantity == Quantity.Percent)
        {
            var (minimum, maximum) = model.SliderRange;
            _slider = new Slider
            {
                Width = 160,
                Minimum = minimum,
                Maximum = maximum,
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
    /// The box and its companions.
    /// </summary>
    public FrameworkElement Root { get; }

    /// <summary>
    /// The number box, which the row names after the setting.
    /// </summary>
    public FrameworkElement Field => _box;

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
    private static DecimalFormatter Formatter(Quantity quantity, bool whole) =>
        new()
        {
            IsGrouped = quantity != Quantity.Port,
            IntegerDigits = 1,
            FractionDigits = 0,
            NumberRounder = new IncrementNumberRounder
            {
                Increment = whole ? 1 : 0.01,
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
