import AllotKit
import SwiftUI

/// The rows of one of allotd's settings: one row, or two for a duration with an off switch
/// (DESIGN.md section 3).
struct SettingRows: View {
    let setting: ValueModel
    let window: SettingsWindowModel

    var body: some View {
        let locked = window.session.readOnly || !setting.enabled
        if let duration = setting as? DurationModel, let off = duration.off {
            SettingRow(
                label: off.label, help: off.help, key: setting.key,
                notes: notes, reset: resetButton
            ) {
                let isOn = Binding(get: { duration.isOn }, set: { duration.setOn($0) })
                Toggle(off.label, isOn: isOn)
                    .toggleStyle(.switch)
                    .labelsHidden()
                    .rowHint()
            }
            .disabled(locked)
            SettingRow(label: setting.label, help: setting.spec.help, key: setting.key) {
                NumberEditor(model: duration)
            }
            .padding(.leading, 20)
            .disabled(locked || !duration.isOn)
        } else if let list = setting as? StringListModel {
            // Chips need the row's width, so they go under the label and help. The print-only
            // flags are the one list on a page.
            VStack(alignment: .leading, spacing: 0) {
                SettingRow(
                    label: setting.label, help: setting.spec.help, key: setting.key,
                    notes: notes, reset: resetButton, standalone: true
                ) {
                    EmptyView()
                }
                ChipsEditor(
                    model: list, entryName: "New print-only flag", placeholder: "--dry-run",
                    addLabel: "Add Flag",
                    hint: RowText.hint(help: setting.spec.help, key: setting.key, notes: notes))
                    .padding(.bottom, 6)
            }
            .disabled(locked)
        } else {
            SettingRow(
                label: setting.label, help: setting.spec.help, key: setting.key,
                notes: notes, reset: resetButton
            ) {
                editor
            }
            .disabled(locked)
        }
    }

    @ViewBuilder private var editor: some View {
        if let toggle = setting as? SwitchModel {
            Toggle(setting.label, isOn: Binding(get: { toggle.isOn }, set: { toggle.set($0) }))
                .toggleStyle(.switch)
                .labelsHidden()
                .rowHint()
        } else if let number = setting as? NumberModel {
            NumberEditor(model: number)
        }
    }

    // The lines under the help: an issue, why the setting is disabled, or a pending restart.
    private var notes: [RowNote] {
        var notes: [RowNote] = []
        if let issue = setting.issue {
            notes.append(.issue(issueText(issue, number: window.number)))
        }
        if let disabled = setting.disabledNote {
            notes.append(.info(disabled))
        }
        if let restart = SettingsBanners.restartNote(window.session, key: setting.key) {
            notes.append(.info(restart))
        }
        return notes
    }

    private var resetButton: ResetButton? {
        guard setting.canReset, !window.session.readOnly else {
            return nil
        }
        let name = SettingsText.resetButtonName(setting, number: window.number)
        return ResetButton(name: name) {
            let message = SettingsText.fieldReset(setting, number: window.number)
            let undo = FieldReset.reset(setting)
            window.showNotice(message, undo: undo)
        }
    }
}

/// A line under a row's help.
enum RowNote: Hashable {
    case issue(String)
    case info(String)
}

/// A row's reset button (DESIGN.md section 3): shown only while the value differs from its
/// default.
struct ResetButton {
    let name: String
    let action: @MainActor () -> Void
}

/// A row: label and help on the left with the key at the end of the help (D4), the control and
/// its reset button on the right, and any notes under the help.
struct SettingRow<Control: View>: View {
    // Keeps the controls lined up whether or not a row shows its reset button.
    private static var resetWidth: CGFloat {
        22
    }

    let label: String
    let help: String
    var key: String?
    var notes: [RowNote] = []
    var reset: ResetButton?

    /// Whether the row has no control of its own, so its text is what a screen reader reads.
    /// Otherwise the control carries the label and, as its hint, the help, key and errors.
    var standalone = false
    @ViewBuilder let control: Control

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            texts
            Spacer(minLength: 16)
            control
                .environment(
                    \.settingHint, RowText.hint(help: help, key: key, notes: notes))
            Group {
                if let reset {
                    Button {
                        reset.action()
                    } label: {
                        Image(systemName: "arrow.counterclockwise")
                            .accessibilityHidden(true)
                    }
                    .buttonStyle(.borderless)
                    .help(reset.name)
                    .accessibilityLabel(reset.name)
                } else {
                    Color.clear
                        .accessibilityHidden(true)
                }
            }
            .frame(width: Self.resetWidth)
        }
        .padding(.vertical, 6)
    }

    // The label, help with the key, and notes. A control's row hides them from a screen reader,
    // since the control says the same as its name and hint.
    @ViewBuilder private var texts: some View {
        let column = VStack(alignment: .leading, spacing: 3) {
            Text(label)
                .font(SettingsFont.body)
                .fixedSize(horizontal: false, vertical: true)
            if !help.isEmpty || key != nil {
                helpText
                    .font(SettingsFont.callout)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
            ForEach(notes, id: \.self) { note in
                switch note {
                case .issue(let text):
                    HStack(alignment: .firstTextBaseline, spacing: 4) {
                        Image(systemName: "exclamationmark.circle.fill")
                            .accessibilityHidden(true)
                        Text(text)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                    .font(SettingsFont.callout)
                    .foregroundStyle(.red)
                    .accessibilityElement(children: .ignore)
                    .accessibilityLabel("Error: \(text)")
                case .info(let text):
                    Text(text)
                        .font(SettingsFont.callout)
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
        }
        if standalone {
            column.accessibilityElement(children: .combine)
        } else {
            column.accessibilityHidden(true)
        }
    }

    private var helpText: Text {
        guard let key else {
            return Text(help)
        }
        return Text(help) + Text("  ") + Text(key).font(SettingsFont.captionMono)
    }
}

/// A number with what goes with its quantity: a slider for a percent, a stepper for counts and
/// days, a unit picker for durations and sizes, and nothing for the port.
struct NumberEditor: View {
    let model: NumberModel

    /// Whether the number box takes focus when the editor appears, such as the first field of
    /// a dialog.
    var autofocus = false

    var body: some View {
        HStack(spacing: 6) {
            switch model.quantity {
            case .percent:
                Slider(value: sliderValue, in: sliderRange, step: 1)
                    .frame(width: 140)
                    .accessibilityLabel("\(model.label) slider")
                    .rowHint()
                NumberField(model: model, autofocus: autofocus)
                Text("%")
                    .font(SettingsFont.body)
                    .accessibilityHidden(true)
            case .runs, .days, .cores:
                NumberField(model: model, autofocus: autofocus)
                Stepper(model.label, onIncrement: { step(1) }, onDecrement: { step(-1) })
                    .labelsHidden()
                    .accessibilityLabel("\(model.label) stepper")
                Text(countUnit)
                    .font(SettingsFont.body)
                    .accessibilityHidden(true)
            case .seconds, .megabytes:
                NumberField(model: model, autofocus: autofocus)
                Picker(model.label, selection: unit) {
                    ForEach(model.units, id: \.name) { option in
                        Text(option.name).tag(option.name)
                    }
                }
                .labelsHidden()
                .fixedSize()
                .accessibilityLabel("\(model.label) unit")
            case .port:
                NumberField(model: model, grouped: false, autofocus: autofocus)
            }
        }
    }

    private var countUnit: String {
        switch model.quantity {
        case .runs: "runs"
        case .days: "days"
        default: "cores"
        }
    }

    private var sliderValue: Binding<Double> {
        let range = sliderRange
        return Binding(
            get: { min(max(model.shown ?? range.lowerBound, range.lowerBound), range.upperBound) },
            set: { value in
                let whole = value.rounded()
                if whole != model.number {
                    model.commit(whole)
                }
            })
    }

    // The range the spec allows, in whole steps.
    private var sliderRange: ClosedRange<Double> {
        let low = model.control.min.map { $0.exclusive ? $0.value + 1 : $0.value } ?? 0
        let high = model.control.max.map { $0.exclusive ? $0.value - 1 : $0.value } ?? 100
        return low...max(low, high)
    }

    private var unit: Binding<String> {
        Binding(get: { model.unit.name }, set: { model.setUnit($0) })
    }

    private func step(_ delta: Double) {
        model.commit((model.shown ?? 0) + delta)
    }
}

/// A number box that commits on Return, on leaving the field and on a stepper's step, never per
/// keystroke; Escape brings back the saved value.
struct NumberField: View {
    let model: NumberModel
    var grouped = true

    /// Takes focus when the box appears.
    var autofocus = false

    @State private var text = ""
    @FocusState private var focused: Bool

    var body: some View {
        TextField(model.label, text: $text)
            .labelsHidden()
            .font(SettingsFont.body)
            .multilineTextAlignment(.trailing)
            .frame(width: (model.quantity == .port ? 72 : 64) * SettingsFont.scale)
            .rowHint()
            .focused($focused)
            .onSubmit { commit() }
            .onExitCommand {
                model.undo()
                text = shownText
            }
            .onChange(of: focused) {
                if !focused {
                    commit()
                }
            }
            .onChange(of: model.value) {
                // A value the user typed that cannot be saved stays as typed.
                if !focused && model.issue == nil {
                    text = shownText
                }
            }
            .onChange(of: model.unit) {
                text = shownText
            }
            .onAppear {
                text = shownText
                if autofocus {
                    Task { @MainActor in
                        focused = true
                    }
                }
            }
    }

    private var shownText: String {
        guard let shown = model.shown, shown.isFinite else {
            return ""
        }
        return grouped ? NumberText.format(shown) : NumberText.plain(shown)
    }

    private func commit() {
        guard text != shownText else {
            return
        }
        model.commit(NumberText.parse(text) ?? .nan)
        if model.issue == nil {
            text = shownText
        }
    }
}

/// Numbers in the user's locale for the number boxes.
enum NumberText {
    static func format(_ value: Double) -> String {
        value.formatted(.number.precision(.fractionLength(0...2)))
    }

    /// Never grouped, as the port is.
    static func plain(_ value: Double) -> String {
        value.formatted(.number.precision(.fractionLength(0...2)).grouping(.never))
    }

    /// What the user typed, read in their locale with or without grouping; `nil` when it is
    /// not a number.
    static func parse(_ text: String) -> Double? {
        let formatter = NumberFormatter()
        formatter.numberStyle = .decimal
        formatter.isLenient = true
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : formatter.number(from: trimmed)?.doubleValue
    }
}
