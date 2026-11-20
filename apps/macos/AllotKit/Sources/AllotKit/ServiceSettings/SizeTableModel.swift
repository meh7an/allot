import AllotContract
import Foundation
import Observation

/// One row of a size table as a page lists it.
public struct SizeRowView: Sendable, Equatable {
    public let row: SizeRow
    public let label: String

    /// Built-in rows can be edited and reset, never renamed or deleted.
    public let builtIn: Bool

    /// A built-in row whose values differ from its defaults; always true for a row the user
    /// added.
    public let changed: Bool
}

/// A size table, classes or presets: rows edited one at a time in a dialog, each Save written
/// at once.
@MainActor
public final class SizeTableModel: ValueModel {
    public let control: SettingControl.SizeTable

    public init(spec: SettingSpec, host: (any ValueHost)?, value: [SizeRow]) {
        guard case .sizeTable(let control) = spec.control else {
            preconditionFailure("\(spec.key) is not a size table")
        }
        self.control = control
        super.init(spec: spec, host: host, value: .rows(value))
    }

    /// The rows, classes in class order and presets by name.
    public var sizeRows: [SizeRow] {
        if case .rows(let rows) = value {
            return rows
        }
        return []
    }

    public override var defaultValue: SettingData? {
        .rows(control.rows.map(Self.builtInRow))
    }

    public override var canReset: Bool {
        rows.contains { $0.changed }
    }

    /// Whether the user can add, rename and delete rows.
    public var canAdd: Bool {
        control.newRowName != nil
    }

    public var rows: [SizeRowView] {
        sizeRows.map { row in
            let spec = builtIn(row.name)
            return SizeRowView(
                row: row,
                label: spec?.label ?? row.name,
                builtIn: spec != nil,
                changed: spec.map { !Self.matches(row, $0) } ?? true)
        }
    }

    /// A draft of the row `name` for its edit dialog.
    ///
    /// - Returns: `nil` when the table has no such row.
    public func edit(_ name: String) -> RowDraft? {
        guard let row = sizeRows.first(where: { $0.name == name }) else {
            return nil
        }
        return RowDraft(table: self, row: row, builtIn: builtIn(name) != nil)
    }

    /// A draft of a new row, with the defaults a new row starts with.
    public func add() -> RowDraft {
        RowDraft(table: self, row: nil, builtIn: false)
    }

    /// Writes the draft's row, renaming it when its name changed. Nothing is written while the
    /// draft has an issue.
    ///
    /// - Returns: Whether it was handed over to be written.
    @discardableResult
    public func save(_ draft: RowDraft) -> Bool {
        guard draft.check() else {
            return false
        }
        let row = draft.row()
        var changes: [Change] = []
        if let original = draft.original, original != row.name {
            changes.append(Contract.resetRow(table: key, name: original))
        }
        changes.append(Contract.setRow(table: key, row: row))
        let rest = sizeRows.filter { $0.name != draft.original && $0.name != row.name }
        write(sorted(rest + [row]), changes)
        return true
    }

    /// Deletes a row the user added; a built-in row is reset instead.
    public func remove(_ name: String) {
        let rows: [SizeRow]
        if let spec = builtIn(name) {
            rows = sizeRows.map { $0.name == name ? Self.builtInRow(spec) : $0 }
        } else {
            rows = sizeRows.filter { $0.name != name }
        }
        write(rows, [Contract.resetRow(table: key, name: name)])
    }

    /// Names the row and field of `presets.<name>.<field>`, or the name itself for
    /// `presets.<name>`.
    public override func describeProblem(_ problemKey: String, _ message: String) -> String {
        let inside = String(problemKey.dropFirst(key.count + 1))
        if inside.isEmpty {
            return super.describeProblem(problemKey, message)
        }
        let name: String
        var fieldKey: String?
        if let dot = inside.firstIndex(of: ".") {
            name = String(inside[..<dot])
            fieldKey = String(inside[inside.index(after: dot)...])
        } else {
            name = inside
        }
        let field = control.fields.first { $0.key == fieldKey }
        let label = field?.label ?? control.newRowName?.label ?? "Name"
        return "\(builtIn(name)?.label ?? name): \(label) \(message)"
    }

    /// The names a new or renamed row cannot take: every other row's, and every class's.
    ///
    /// - Parameter original: The row's own name, which it may keep.
    public func takenNames(original: String?) -> [String] {
        var names = sizeRows.map(\.name).filter { $0 != original }
        if key != "classes", case .rows(let classes)? = host?.valueOf("classes") {
            names += classes.map(\.name)
        }
        return names
    }

    private func write(_ rows: [SizeRow], _ changes: [Change]) {
        show(.rows(rows), issue: nil)
        host?.edited(self, changes: changes, now: true)
    }

    private func builtIn(_ name: String) -> SizeRowSpec? {
        control.rows.first { $0.name == name }
    }

    /// Classes keep the spec's order; presets go by name, as allotd lists them.
    private func sorted(_ rows: [SizeRow]) -> [SizeRow] {
        if key == "classes" {
            let order = control.rows.map(\.name)
            return rows.sorted {
                (order.firstIndex(of: $0.name) ?? -1) < (order.firstIndex(of: $1.name) ?? -1)
            }
        }
        return rows.sorted { $0.name < $1.name }
    }

    private static func builtInRow(_ spec: SizeRowSpec) -> SizeRow {
        Contract.sizeRow(
            name: spec.name, profile: spec.default, minCores: spec.defaultMinCores.map(double))
    }

    private static func matches(_ row: SizeRow, _ spec: SizeRowSpec) -> Bool {
        row.profile == spec.default && row.minCores == spec.defaultMinCores
    }
}

/// The fields of a class or preset in its edit dialog, kept until Save.
@MainActor
@Observable
public final class RowDraft {
    /// The name as the user typed it, trimmed.
    public private(set) var name: String

    public private(set) var nameIssue: ValueIssue?

    /// The row's name before the edit, or `nil` for a new row.
    public let original: String?

    public let builtIn: Bool

    /// The row's number fields, in the order of the spec.
    @ObservationIgnored public private(set) var fields: [NumberModel] = []

    /// Called after every change of the draft, so the dialog can refresh its Save button.
    @ObservationIgnored public var onChange: (() -> Void)?

    private let table: SizeTableModel
    private let host = DraftHost()

    init(table: SizeTableModel, row: SizeRow?, builtIn: Bool) {
        self.table = table
        original = row?.name
        name = row?.name ?? ""
        self.builtIn = builtIn
        host.lookup = { [weak self] key in self?.field(key)?.value }
        host.labeler = { [weak self] key in self?.field(key)?.label ?? key }
        host.onEdited = { [weak self] in self?.edited() }
        fields = table.control.fields.compactMap { (spec: SettingSpec) -> NumberModel? in
            let value: Double?
            if let row {
                value = Self.rowValue(row, spec.key)
            } else if case .number(let number) = spec.control {
                value = number.default
            } else {
                value = nil
            }
            return makeValueModel(spec, host: host, value: .number(value)) as? NumberModel
        }
    }

    /// Whether the name can be edited: rows the user added, in a table that allows them.
    public var canRename: Bool {
        table.canAdd && !builtIn
    }

    /// The name field's label, help and pattern, when the table allows names.
    public var nameSpec: RowName? {
        table.control.newRowName
    }

    public func field(_ key: String) -> NumberModel? {
        fields.first { $0.key == key }
    }

    public func setName(_ text: String) {
        name = text.trimmingCharacters(in: .whitespacesAndNewlines)
        nameIssue = currentNameIssue()
        onChange?()
    }

    /// Checks every field and the name, showing each issue.
    ///
    /// - Returns: Whether the row can be saved.
    public func check() -> Bool {
        nameIssue = currentNameIssue()
        for field in fields {
            if field.number == nil {
                field.issue = field.control.automatic == nil ? .required : nil
            } else {
                field.recheck()
            }
        }
        onChange?()
        return nameIssue == nil && fields.allSatisfy { $0.issue == nil }
    }

    public func row() -> SizeRow {
        func number(_ key: String) -> Double {
            field(key)?.number ?? 0
        }
        let profile = Contract.profile(
            cores: number("cores"),
            ramMb: number("ram_mb"),
            gpuPct: number("gpu_pct"),
            vramMb: number("vram_mb"),
            estSecs: number("est_secs"))
        return Contract.sizeRow(
            name: name, profile: profile, minCores: field("min_cores")?.number)
    }

    private func currentNameIssue() -> ValueIssue? {
        guard let spec = nameSpec, canRename else {
            return nil
        }
        if name.isEmpty {
            return .blank
        }
        if name.range(of: spec.pattern, options: .regularExpression) == nil {
            return .pattern
        }
        return table.takenNames(original: original).contains(name) ? .nameTaken : nil
    }

    private func edited() {
        for field in fields {
            field.recheck()
        }
        onChange?()
    }

    private static func rowValue(_ row: SizeRow, _ key: String) -> Double? {
        switch key {
        case "min_cores": row.minCores.map(double)
        case "cores": double(row.profile.cores)
        case "ram_mb": Double(row.profile.ramMb)
        case "gpu_pct": double(row.profile.gpuPct)
        case "vram_mb": Double(row.profile.vramMb)
        case "est_secs": Double(row.profile.estSecs)
        default: nil
        }
    }
}

/// The host of a draft's fields: they see each other, and any edit tells the draft.
@MainActor
final class DraftHost: ValueHost {
    var lookup: @MainActor (String) -> SettingData? = { _ in nil }
    var labeler: @MainActor (String) -> String = { $0 }
    var onEdited: @MainActor () -> Void = {}

    func valueOf(_ key: String) -> SettingData? {
        lookup(key)
    }

    func labelOf(_ key: String) -> String {
        labeler(key)
    }

    func edited(_ model: ValueModel, changes: [Change], now: Bool) {
        onEdited()
    }
}

/// A `Float` of the contract as the number the user typed: `0.1` stays `0.1`, not
/// `0.10000000149`.
func double(_ value: Float) -> Double {
    Double("\(value)") ?? Double(value)
}
