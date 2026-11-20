import AllotContract
import Foundation

/// The value of one setting in the kind its control edits. A model holds one of these so the
/// settings session can keep every model in a single list.
public enum SettingData: Sendable, Equatable {
    /// A switch.
    case on(Bool)

    /// A number in the unit of its setting; `nil` where allotd works it out.
    case number(Double?)

    /// A list of strings, such as the print-only flags.
    case list([String])

    /// Every row of a size table.
    case rows([SizeRow])

    /// The heavy-command rules.
    case rules([HeavyRule])
}

extension SettingValue {
    /// The setting's key.
    public var key: String {
        switch self {
        case .switchValue(let value): value.key
        case .numberValue(let value): value.key
        case .listValue(let value): value.key
        case .rulesValue(let value): value.key
        case .rowsValue(let value): value.key
        }
    }

    /// The value in the shape the models hold.
    public var data: SettingData {
        switch self {
        case .switchValue(let value): .on(value.value)
        case .numberValue(let value): .number(value.value)
        case .listValue(let value): .list(value.value)
        case .rulesValue(let value): .rules(value.value)
        case .rowsValue(let value): .rows(value.value)
        }
    }
}

/// A limit of a number's range.
public struct Limit: Sendable, Equatable {
    public let value: Double

    /// Whether the limit itself is out of range.
    public let exclusive: Bool

    public init(value: Double, exclusive: Bool) {
        self.value = value
        self.exclusive = exclusive
    }

    init?(_ bound: Bound?) {
        guard let bound else {
            return nil
        }
        self.init(value: bound.value, exclusive: bound.exclusive)
    }
}

/// What is wrong with a value the user entered, for the line under its field. ``service``
/// carries allotd's own reason for a refused value.
public enum ValueIssue: Sendable, Equatable {
    case required
    case notANumber
    case whole
    case range(min: Limit?, max: Limit?)
    case atLeast(label: String, value: Double, quantity: Quantity)
    case atMost(label: String, value: Double, quantity: Quantity)
    case blank
    case pattern
    case duplicate
    case nameTaken
    case service(String)
}

/// A unit a number can be shown in. `factor` is how many of the file's unit make one.
public struct NumberUnit: Sendable, Equatable {
    public let name: String
    public let factor: Double

    public init(name: String, factor: Double) {
        self.name = name
        self.factor = factor
    }

    public static let durations = [
        NumberUnit(name: "seconds", factor: 1),
        NumberUnit(name: "minutes", factor: 60),
        NumberUnit(name: "hours", factor: 3600),
    ]

    public static let memory = [
        NumberUnit(name: "MB", factor: 1),
        NumberUnit(name: "GB", factor: 1024),
    ]

    /// The largest unit that shows `value` as a whole number; the smallest for 0 or automatic.
    public static func best(_ units: [NumberUnit], _ value: Double?) -> NumberUnit {
        var best = units.first ?? NumberUnit(name: "", factor: 1)
        guard let value, value != 0 else {
            return best
        }
        for unit in units where isWhole(value / unit.factor) {
            best = unit
        }
        return best
    }
}

/// The sentence under a field for `issue`.
///
/// - Parameter number: Formats a number for the user's locale.
public func issueText(_ issue: ValueIssue, number: (Double) -> String) -> String {
    switch issue {
    case .required:
        return "Enter a value."
    case .notANumber:
        return "Enter a number."
    case .whole:
        return "Enter a whole number."
    case .range(let min, let max):
        return "Enter a number \(rangeText(min, max, number))."
    case .atLeast(let label, let value, let quantity):
        return "Must be at least \(label) (\(quantityText(value, quantity, number: number)))."
    case .atMost(let label, let value, let quantity):
        return "Must be at most \(label) (\(quantityText(value, quantity, number: number)))."
    case .blank:
        return "Enter some text."
    case .pattern:
        return "Use only the characters the help names."
    case .duplicate:
        return "The list already has this."
    case .nameTaken:
        return "Another preset or a class already has this name."
    case .service(let message):
        return message
    }
}

/// A value with its unit, such as "15 %", "3 minutes" or "2 GB". Durations take the largest unit
/// that divides them, and memory GB when it is a whole number of them.
///
/// - Parameters:
///   - value: In the setting's own unit.
///   - number: Formats a number for the user's locale.
public func quantityText(
    _ value: Double,
    _ quantity: Quantity,
    number: (Double) -> String
) -> String {
    switch quantity {
    case .percent:
        return "\(number(value)) %"
    case .runs:
        return plural(value, "run", number)
    case .days:
        return plural(value, "day", number)
    case .cores:
        return plural(value, "core", number)
    case .seconds:
        let unit = NumberUnit.best(NumberUnit.durations, value)
        return plural(value / unit.factor, String(unit.name.dropLast()), number)
    case .megabytes:
        let unit = NumberUnit.best(NumberUnit.memory, value)
        return "\(number(value / unit.factor)) \(unit.name)"
    case .port:
        return isWhole(value) && abs(value) < 1e15 ? String(Int64(value)) : String(value)
    }
}

/// Whether `value` is a finite whole number.
func isWhole(_ value: Double) -> Bool {
    value.isFinite && value == value.rounded()
}

/// "from 0 to 99", "at least 1", "above 0", "at least 0 and below 100".
private func rangeText(_ min: Limit?, _ max: Limit?, _ number: (Double) -> String) -> String {
    if let min, let max, !min.exclusive, !max.exclusive {
        return "from \(number(min.value)) to \(number(max.value))"
    }
    var parts: [String] = []
    if let min {
        parts.append("\(min.exclusive ? "above" : "at least") \(number(min.value))")
    }
    if let max {
        parts.append("\(max.exclusive ? "below" : "at most") \(number(max.value))")
    }
    return parts.joined(separator: " and ")
}

private func plural(_ value: Double, _ noun: String, _ number: (Double) -> String) -> String {
    "\(number(value)) \(value == 1 ? noun : "\(noun)s")"
}
