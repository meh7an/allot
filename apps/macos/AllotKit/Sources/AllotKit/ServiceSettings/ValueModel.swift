import AllotContract
import Foundation
import Observation

// The view models of the Settings window's values, one per kind of value, shared by every page
// and by the class, preset and rule dialogs. They hold no I/O: a page's host, the settings
// session, saves what they report; a dialog's draft keeps it until Save.

/// Where a value model lives: a Settings page, saved by the settings session, or the draft of
/// an edit dialog, kept until its Save.
@MainActor
public protocol ValueHost: AnyObject {
    /// The value another setting or field shows now, edits included; `nil` when there is none.
    func valueOf(_ key: String) -> SettingData?

    /// The label of another setting or field, for issues that name it.
    func labelOf(_ key: String) -> String

    /// The user changed `model`. `changes` saves it, and is empty while the model has an issue
    /// or went back to its saved value; `now` saves without waiting for the burst to end.
    func edited(_ model: ValueModel, changes: [Change], now: Bool)
}

/// One value of the Settings window, with its saved value, the value shown, and an issue when
/// the shown value cannot be saved.
@MainActor
@Observable
public class ValueModel {
    public let spec: SettingSpec

    /// What is wrong with the shown value, if anything.
    public internal(set) var issue: ValueIssue?

    /// Bumped on every edit, so a save that answers late marks only the edit it carried as
    /// saved.
    public private(set) var generation = 0

    /// The value allotd last reported.
    public private(set) var saved: SettingData

    /// The value the field shows: the saved one, or the user's edit.
    public private(set) var value: SettingData

    /// Whether the shown value is the user's edit, not yet saved.
    public private(set) var dirty = false

    // Subclasses cannot repeat @Observable, so state of their own that a view shows, such as a
    // number's unit, reads and bumps this to be observed.
    var shownRevision = 0

    // The host owns the models, so the models must not own it back.
    @ObservationIgnored weak var host: (any ValueHost)?

    public init(spec: SettingSpec, host: (any ValueHost)?, value: SettingData) {
        self.spec = spec
        self.host = host
        saved = value
        self.value = value
    }

    public var key: String {
        spec.key
    }

    public var label: String {
        spec.label
    }

    /// The value while the key is not in the file, or `nil` when the setting has none.
    public var defaultValue: SettingData? {
        nil
    }

    /// Whether the row shows its reset button: the saved value differs from the default.
    public var canReset: Bool {
        guard let fallback = defaultValue else {
            return false
        }
        return saved != fallback
    }

    /// Whether the switch this setting needs is on; a disabled setting keeps its value.
    public var enabled: Bool {
        guard let requires = spec.requires else {
            return true
        }
        return host?.valueOf(requires.key) != .on(false)
    }

    /// Why the setting is disabled, or `nil` while it is enabled.
    public var disabledNote: String? {
        enabled ? nil : spec.requires?.note
    }

    /// Takes the value allotd reports. An edit not yet saved stays on screen; a hand edit to the
    /// same key then meets it as a conflict when it is saved.
    public func load(_ saved: SettingData) {
        self.saved = saved
        if !dirty {
            value = saved
            loaded()
        }
    }

    /// Removes the key from the file, so its default applies again.
    public func reset() {
        guard let fallback = defaultValue else {
            return
        }
        show(fallback, issue: nil)
        host?.edited(self, changes: [Contract.reset(key)], now: false)
    }

    /// Shows the saved value again and drops the edit and its issue.
    public func undo() {
        issue = nil
        value = saved
        dirty = false
        generation += 1
        loaded()
        host?.edited(self, changes: [], now: false)
    }

    /// Marks the edit of `generation` as saved; a later edit stays dirty.
    public func markSaved(_ generation: Int) {
        if generation == self.generation && issue == nil {
            dirty = false
        }
    }

    /// Shows allotd's reason for refusing the value; the value stays for the user to fix.
    public func refuse(_ message: String) {
        issue = .service(message)
    }

    /// allotd's reason for a refused value as a sentence that names it, such as "Headroom must
    /// be at least 0 and below 100".
    ///
    /// - Parameters:
    ///   - problemKey: The dotted key allotd reported, this setting's or inside it.
    ///   - message: What the value must be.
    public func describeProblem(_ problemKey: String, _ message: String) -> String {
        "\(label) \(message)"
    }

    /// Shows `value` as the user's edit.
    func show(_ value: SettingData, issue: ValueIssue?) {
        self.value = value
        self.issue = issue
        dirty = true
        generation += 1
    }

    /// Shows `value` and reports it to the host: saved with `change` when it has no issue.
    func apply(
        _ value: SettingData,
        issue: ValueIssue?,
        now: Bool,
        change: @autoclosure () -> Change
    ) {
        show(value, issue: issue)
        host?.edited(self, changes: issue == nil ? [change()] : [], now: now)
    }

    /// Shows `issue` without changing the value.
    func flag(_ issue: ValueIssue?) {
        self.issue = issue
    }

    /// Called after the shown value was replaced by the saved one.
    func loaded() {}
}

/// An on/off setting.
@MainActor
public final class SwitchModel: ValueModel {
    public init(spec: SettingSpec, host: (any ValueHost)?, value: Bool) {
        super.init(spec: spec, host: host, value: .on(value))
    }

    public var isOn: Bool {
        value == .on(true)
    }

    public override var defaultValue: SettingData? {
        if case .`switch`(let control) = spec.control {
            return .on(control.default)
        }
        return nil
    }

    public func set(_ on: Bool) {
        apply(.on(on), issue: nil, now: false, change: Contract.setSwitch(key, on))
    }
}

/// A number in the unit of its setting: percent, runs, days, cores or a port. Durations and
/// memory sizes add a unit the user picks. `nil` is a value only where the setting can be
/// automatic.
@MainActor
public class NumberModel: ValueModel {
    private var storedUnit = NumberUnit(name: "", factor: 1)

    /// The unit the value is shown in.
    public internal(set) var unit: NumberUnit {
        get {
            _ = shownRevision
            return storedUnit
        }
        set {
            storedUnit = newValue
            shownRevision += 1
        }
    }

    public let control: SettingControl.Number

    /// The units the user can pick from, smallest first; empty when the number has only its
    /// own.
    public let units: [NumberUnit]

    /// - Parameter units: The units the value can be shown in, smallest first.
    public init(
        spec: SettingSpec,
        host: (any ValueHost)?,
        value: Double?,
        units: [NumberUnit] = []
    ) {
        guard case .number(let control) = spec.control else {
            preconditionFailure("\(spec.key) is not a number")
        }
        self.control = control
        self.units = units
        super.init(spec: spec, host: host, value: .number(value))
        unit = NumberUnit.best(units, value)
    }

    public var quantity: Quantity {
        control.quantity
    }

    /// The value, or `nil` while automatic.
    public var number: Double? {
        if case .number(let number) = value {
            return number
        }
        return nil
    }

    public override var defaultValue: SettingData? {
        if let fallback = control.default {
            return .number(fallback)
        }
        return control.automatic != nil ? .number(nil) : nil
    }

    /// The value in the picked unit, or `nil` while automatic.
    public var shown: Double? {
        number.map { $0 / unit.factor }
    }

    /// Whether the value is worked out by allotd, as `automatic` describes.
    public var automatic: Bool {
        number == nil
    }

    /// Shows the value in another unit; the value itself does not change.
    public func setUnit(_ name: String) {
        if let found = units.first(where: { $0.name == name }) {
            unit = found
        }
    }

    /// Commits a number the user entered in the picked unit: on Enter, on leaving the field, or
    /// on a spin step.
    public func commit(_ shown: Double) {
        // The file holds whole megabytes and seconds; a value entered in a larger unit is
        // rounded to them.
        let value = units.isEmpty ? shown : (shown * unit.factor + 0.5).rounded(.down)
        apply(
            .number(value), issue: check(value), now: false,
            change: Contract.setNumber(key, value))
    }

    /// Leaves the value to allotd, or gives it the number shown before.
    ///
    /// - Parameters:
    ///   - automatic: Whether allotd works the value out.
    ///   - value: The number to give it when `automatic` is false.
    public func setAutomatic(_ automatic: Bool, value: Double? = nil) {
        if automatic {
            apply(.number(nil), issue: nil, now: false, change: Contract.reset(key))
        } else if let value {
            commit(value / unit.factor)
        }
    }

    /// Checks the shown value again, after a value it is compared with changed.
    public func recheck() {
        if case .service = issue {
            return
        }
        if let number {
            flag(check(number))
        }
    }

    /// What is wrong with `value` for this setting, or `nil`.
    public func check(_ value: Double) -> ValueIssue? {
        // Sizes go out as 32-bit floats, so a value that overflows one cannot be sent either.
        guard value.isFinite, Float(value).isFinite else {
            return .notANumber
        }
        if control.integer && !isWhole(value) {
            return .whole
        }
        if Self.outside(value, control.min, control.max) {
            return .range(min: Limit(control.min), max: Limit(control.max))
        }
        if let minKey = control.minKey, let least = other(minKey), value < least {
            let label = host?.labelOf(minKey) ?? minKey
            return .atLeast(label: label, value: least, quantity: quantity)
        }
        if let maxKey = control.maxKey, let most = other(maxKey), value > most {
            let label = host?.labelOf(maxKey) ?? maxKey
            return .atMost(label: label, value: most, quantity: quantity)
        }
        return nil
    }

    override func loaded() {
        unit = NumberUnit.best(units, number)
    }

    private func other(_ key: String) -> Double? {
        if case .number(let number)? = host?.valueOf(key) {
            return number
        }
        return nil
    }

    private static func outside(_ value: Double, _ min: Bound?, _ max: Bound?) -> Bool {
        let low = min.map { $0.exclusive ? value <= $0.value : value < $0.value } ?? false
        let high = max.map { $0.exclusive ? value >= $0.value : value > $0.value } ?? false
        return low || high
    }
}

/// A duration in seconds, shown in seconds, minutes or hours. Where one value turns the feature
/// off (`off` in the spec), a switch in front of it writes that value.
@MainActor
public final class DurationModel: NumberModel {
    // A duration turned back on with neither an earlier value nor a default.
    private static let minuteSecs = 60.0

    private var lastOn: Double?

    public init(spec: SettingSpec, host: (any ValueHost)?, value: Double?) {
        super.init(spec: spec, host: host, value: value, units: NumberUnit.durations)
    }

    /// The switch in front of the duration, or `nil` when it has none.
    public var off: OffSwitch? {
        control.off
    }

    /// Whether the feature is on; always true without an off switch.
    public var isOn: Bool {
        guard let off = control.off else {
            return true
        }
        return number != off.value
    }

    /// Turns the feature off by writing the off value, or on again with the duration it had.
    public func setOn(_ on: Bool) {
        guard let off = control.off, on != isOn else {
            return
        }
        if !on {
            lastOn = number
            apply(
                .number(off.value), issue: nil, now: false,
                change: Contract.setNumber(key, off.value))
            return
        }
        let value = lastOn ?? control.default ?? Self.minuteSecs
        commit(value / unit.factor)
    }

    override func loaded() {
        super.loaded()
        if let off = control.off, number == off.value {
            unit = NumberUnit.best(NumberUnit.durations, control.default)
        }
    }
}

/// A size in megabytes, shown in MB or GB; the file always gets MB.
@MainActor
public final class MemoryModel: NumberModel {
    public init(spec: SettingSpec, host: (any ValueHost)?, value: Double?) {
        super.init(spec: spec, host: host, value: value, units: NumberUnit.memory)
    }
}

/// A list of strings shown as chips, such as the print-only flags; saved on add or remove.
@MainActor
public final class StringListModel: ValueModel {
    public init(spec: SettingSpec, host: (any ValueHost)?, value: [String]) {
        super.init(spec: spec, host: host, value: .list(value))
    }

    /// The entries, in file order.
    public var entries: [String] {
        if case .list(let entries) = value {
            return entries
        }
        return []
    }

    public override var defaultValue: SettingData? {
        if case .stringList(let control) = spec.control {
            return .list(control.default)
        }
        return nil
    }

    /// Adds an entry; refuses a blank one or one the list already has.
    ///
    /// - Returns: Whether it was added.
    @discardableResult
    public func add(_ text: String) -> Bool {
        let entry = text.trimmingCharacters(in: .whitespacesAndNewlines)
        if entry.isEmpty {
            flag(.blank)
            return false
        }
        if entries.contains(entry) {
            flag(.duplicate)
            return false
        }
        save(entries + [entry])
        return true
    }

    /// The entry text changed: a blank or duplicate flag was about the text before, so it goes.
    public func entryChanged() {
        if issue == .blank || issue == .duplicate {
            flag(nil)
        }
    }

    /// Writes `entries` as they are, such as the list before a reset for Undo.
    public func replace(_ entries: [String]) {
        save(entries)
    }

    public func remove(at index: Int) {
        if entries.indices.contains(index) {
            var rest = entries
            rest.remove(at: index)
            save(rest)
        }
    }

    private func save(_ entries: [String]) {
        apply(.list(entries), issue: nil, now: true, change: Contract.setList(key, entries))
    }
}

/// The view model of a setting, by its control and quantity.
///
/// - Parameter value: The saved value, in the shape the control edits.
/// - Returns: `nil` for a text or choice, which live only in a rule's dialog, and for a value
///   of another shape than the control edits.
@MainActor
public func makeValueModel(
    _ spec: SettingSpec,
    host: any ValueHost,
    value: SettingData
) -> ValueModel? {
    switch (spec.control, value) {
    case (.`switch`, .on(let on)):
        return SwitchModel(spec: spec, host: host, value: on)
    case (.number(let control), .number(let number)):
        switch control.quantity {
        case .seconds:
            return DurationModel(spec: spec, host: host, value: number)
        case .megabytes:
            return MemoryModel(spec: spec, host: host, value: number)
        default:
            return NumberModel(spec: spec, host: host, value: number)
        }
    case (.stringList, .list(let entries)):
        return StringListModel(spec: spec, host: host, value: entries)
    case (.sizeTable, .rows(let rows)):
        return SizeTableModel(spec: spec, host: host, value: rows)
    case (.ruleList, .rules(let rules)):
        return RuleListModel(spec: spec, host: host, value: rules)
    default:
        return nil
    }
}
