import AllotContract
import Foundation

/// A confirmation, such as Reset Page or Stop Service.
public struct Confirmation: Sendable, Equatable {
    public let title: String

    /// What happens, such as what goes back to its default.
    public let body: String

    /// The primary button, in title case.
    public let button: String

    public init(title: String, body: String, button: String) {
        self.title = title
        self.body = body
        self.button = button
    }
}

/// The Settings window's fixed sentences: the save status, the reset confirmations and notices,
/// and a saved or pending value as the user reads it.
public enum SettingsText {
    public static let nothingToReset = "Nothing to reset"

    public static let loading = "Loading settings…"

    /// A number for `locale`, grouped, with at most two decimals.
    public static func numbers(_ locale: Locale) -> @Sendable (Double) -> String {
        { value in
            value.formatted(
                .number.locale(locale).precision(.fractionLength(0...2)).grouping(.automatic))
        }
    }

    /// "All changes saved", "Saving…", "1 change not saved".
    public static func saveStatus(_ status: SaveStatus) -> String {
        switch status {
        case .saved:
            "All changes saved"
        case .saving:
            "Saving…"
        case .unsaved(let count):
            count == 1 ? "1 change not saved" : "\(count) changes not saved"
        }
    }

    /// The default of a setting with its unit, such as "15 %" or "Off"; empty without one.
    @MainActor
    public static func defaultOf(_ model: ValueModel, number: (Double) -> String) -> String {
        switch model.spec.control {
        case .`switch`(let control):
            return onOff(control.default)
        case .number(let control):
            guard let value = control.default else {
                return ""
            }
            if let off = control.off, value == off.value {
                return "Off"
            }
            return quantityText(value, control.quantity, number: number)
        case .stringList(let control):
            return control.default.joined(separator: ", ")
        default:
            return ""
        }
    }

    /// The value a change would write, for the conflict banner: "15 %", "On", "the default".
    @MainActor
    public static func changeValue(
        _ model: ValueModel,
        _ change: Change,
        number: (Double) -> String
    ) -> String {
        switch change {
        case .setSwitch(let set):
            return onOff(set.value)
        case .setNumber(let set):
            if let duration = model as? DurationModel, let off = duration.off,
                set.value == off.value
            {
                return "Off"
            }
            if let numberModel = model as? NumberModel {
                return quantityText(set.value, numberModel.quantity, number: number)
            }
            return number(set.value)
        case .setList(let set):
            return set.value.joined(separator: ", ")
        default:
            return "the default"
        }
    }

    /// The notice after a field's reset: "Headroom reset to 15 %."
    @MainActor
    public static func fieldReset(_ model: ValueModel, number: (Double) -> String) -> String {
        let value = defaultOf(model, number: number)
        return value.isEmpty ? "\(model.label) reset." : "\(model.label) reset to \(value)."
    }

    /// The accessible name of a field's reset button: "Reset Headroom to 15 %".
    @MainActor
    public static func resetButtonName(_ model: ValueModel, number: (Double) -> String) -> String {
        let value = defaultOf(model, number: number)
        return value.isEmpty ? "Reset \(model.label)" : "Reset \(model.label) to \(value)"
    }

    /// Reset Page's confirmation for the settings on a page that differ from their default.
    ///
    /// - Parameter pageLabel: The page, such as Admission.
    @MainActor
    public static func resetPage(
        _ pageLabel: String,
        changed: [ValueModel],
        number: (Double) -> String
    ) -> Confirmation {
        resetPage(
            pageLabel,
            lines: changed.map { model in
                let value = defaultOf(model, number: number)
                return value.isEmpty ? model.label : "\(model.label): \(value)"
            })
    }

    /// Reset Page's confirmation, from one line per changed setting with its default.
    public static func resetPage(_ pageLabel: String, lines: [String]) -> Confirmation {
        let count = settings(lines.count)
        return Confirmation(
            title: "Reset \(count) on \(pageLabel)?",
            body: "These go back to their defaults:\n\(lines.joined(separator: "\n"))",
            button: "Reset \(titled(count))")
    }

    /// Reset All Service Settings' confirmation.
    public static func resetAll(_ changed: Int) -> Confirmation {
        Confirmation(
            title: changed == 1
                ? "Reset the 1 changed setting?" : "Reset all \(changed) changed settings?",
            body: "config.toml goes back to the defaults. Your rules and presets are replaced by "
                + "the built-in ones, and presets you added are deleted. Comments next to these "
                + "keys are removed too. This can't be undone.",
            button: "Reset \(titled(settings(changed)))")
    }

    public static func onOff(_ on: Bool) -> String {
        on ? "On" : "Off"
    }

    /// "a", "a and b", "a, b and c".
    public static func joined(_ parts: [String]) -> String {
        guard parts.count >= 2, let last = parts.last else {
            return parts.joined()
        }
        return "\(parts.dropLast().joined(separator: ", ")) and \(last)"
    }

    private static func settings(_ count: Int) -> String {
        count == 1 ? "1 setting" : "\(count) settings"
    }

    // Buttons are title case on macOS: "Reset 3 Settings".
    private static func titled(_ text: String) -> String {
        text.replacingOccurrences(of: "setting", with: "Setting")
    }
}
