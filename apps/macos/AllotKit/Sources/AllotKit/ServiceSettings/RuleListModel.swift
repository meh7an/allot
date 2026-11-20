import AllotContract
import Foundation
import Observation

/// The heavy-command rules: an ordered list edited one rule at a time in a dialog, reordered
/// and deleted from each row's menu, and written whole on every change.
@MainActor
public final class RuleListModel: ValueModel {
    public let control: SettingControl.RuleList

    public init(spec: SettingSpec, host: (any ValueHost)?, value: [HeavyRule]) {
        guard case .ruleList(let control) = spec.control else {
            preconditionFailure("\(spec.key) is not a rule list")
        }
        self.control = control
        super.init(spec: spec, host: host, value: .rules(value))
    }

    /// The rules, in matching order.
    public var ruleList: [HeavyRule] {
        if case .rules(let rules) = value {
            return rules
        }
        return []
    }

    public override var defaultValue: SettingData? {
        .rules(control.default)
    }

    /// Names the rule, from 1, and its field for `classifier.rules[<i>].<field>`.
    public override func describeProblem(_ problemKey: String, _ message: String) -> String {
        guard let (index, fieldKey) = Self.parse(String(problemKey.dropFirst(key.count))) else {
            return super.describeProblem(problemKey, message)
        }
        let field = control.fields.first { $0.key == fieldKey }
        let subject = field.map { " \($0.label)" } ?? ""
        return "Rule \(index + 1):\(subject) \(message)"
    }

    /// A draft of the rule at `index` for its edit dialog.
    public func edit(_ index: Int) -> RuleDraft? {
        guard ruleList.indices.contains(index) else {
            return nil
        }
        return RuleDraft(fields: control.fields, rule: ruleList[index], index: index)
    }

    /// A draft of a new rule, added at the end.
    public func add() -> RuleDraft {
        RuleDraft(fields: control.fields, rule: nil, index: nil)
    }

    /// Writes the list with the draft's rule in its place.
    ///
    /// - Returns: Whether it was handed over to be written.
    @discardableResult
    public func save(_ draft: RuleDraft) -> Bool {
        guard draft.check(), let rule = draft.rule() else {
            return false
        }
        var rules = ruleList
        if let index = draft.index, rules.indices.contains(index) {
            rules[index] = rule
        } else {
            rules.append(rule)
        }
        write(rules)
        return true
    }

    /// Moves a rule to position `to`; rules match in list order.
    public func move(from: Int, to: Int) {
        var rules = ruleList
        guard rules.indices.contains(from) else {
            return
        }
        let rule = rules.remove(at: from)
        guard to >= 0, to <= rules.count else {
            return
        }
        rules.insert(rule, at: to)
        write(rules)
    }

    public func remove(at index: Int) {
        guard ruleList.indices.contains(index) else {
            return
        }
        var rules = ruleList
        rules.remove(at: index)
        write(rules)
    }

    private func write(_ rules: [HeavyRule]) {
        apply(.rules(rules), issue: nil, now: true, change: Contract.setRules(rules))
    }

    /// `[<index>]` or `[<index>].<field>`.
    private static func parse(_ text: String) -> (index: Int, field: String?)? {
        guard text.hasPrefix("["), let close = text.firstIndex(of: "]") else {
            return nil
        }
        let digits = text[text.index(after: text.startIndex)..<close]
        guard !digits.isEmpty, digits.allSatisfy({ $0.isASCII && $0.isNumber }),
            let index = Int(digits)
        else {
            return nil
        }
        let tail = text[text.index(after: close)...]
        if tail.isEmpty {
            return (index, nil)
        }
        let field = tail.dropFirst()
        guard tail.hasPrefix("."), !field.isEmpty,
            field.allSatisfy({ $0.isASCII && ($0.isLetter || $0.isNumber || $0 == "_") })
        else {
            return nil
        }
        return (index, String(field))
    }
}

/// The fields of one heavy-command rule in its edit dialog, kept until Save.
@MainActor
@Observable
public final class RuleDraft {
    public private(set) var program: String

    public private(set) var programIssue: ValueIssue?

    /// The class's value, such as `cpu_heavy`; `nil` until one is picked.
    public private(set) var ruleClass: String?

    public private(set) var classIssue: ValueIssue?

    /// The rule's place in the list, or `nil` for a new rule.
    public let index: Int?

    /// The rule's argument patterns, as chips.
    public let args: StringListModel

    /// Called after every change of the draft, so the dialog can refresh its Save button.
    @ObservationIgnored public var onChange: (() -> Void)?

    private let fields: [SettingSpec]
    private let host: DraftHost

    init(fields: [SettingSpec], rule: HeavyRule?, index: Int?) {
        let host = DraftHost()
        self.fields = fields
        self.index = index
        self.host = host
        program = rule?.program ?? ""
        ruleClass = rule?.`class`.rawValue ?? Self.choice(in: fields)?.default
        args = StringListModel(
            spec: Self.spec(in: fields, "args"), host: host, value: rule?.args ?? [])
        host.onEdited = { [weak self] in self?.onChange?() }
    }

    /// The program field's spec.
    public var programSpec: SettingSpec {
        Self.spec(in: fields, "program")
    }

    /// The class field's spec.
    public var classSpec: SettingSpec {
        Self.spec(in: fields, "class")
    }

    public var classOptions: [ChoiceOption] {
        Self.choice(in: fields)?.options ?? []
    }

    public func setProgram(_ text: String) {
        program = text.trimmingCharacters(in: .whitespacesAndNewlines)
        programIssue = currentProgramIssue()
        onChange?()
    }

    public func setClass(_ value: String) {
        ruleClass = value
        classIssue = nil
        onChange?()
    }

    /// - Returns: Whether the rule can be saved.
    public func check() -> Bool {
        programIssue = currentProgramIssue()
        classIssue = ruleClass == nil ? .required : nil
        onChange?()
        return programIssue == nil && classIssue == nil
    }

    /// The rule as drafted; `nil` while no class is picked.
    public func rule() -> HeavyRule? {
        guard let value = ruleClass, let jobClass = JobClass(rawValue: value) else {
            return nil
        }
        return Contract.rule(program: program, args: args.entries, class: jobClass)
    }

    private func currentProgramIssue() -> ValueIssue? {
        if program.isEmpty {
            return .blank
        }
        if case .text(let control) = programSpec.control, let pattern = control.pattern,
            program.range(of: pattern, options: .regularExpression) == nil
        {
            return .pattern
        }
        return nil
    }

    private static func choice(in fields: [SettingSpec]) -> SettingControl.Choice? {
        if case .choice(let control) = spec(in: fields, "class").control {
            return control
        }
        return nil
    }

    private static func spec(in fields: [SettingSpec], _ key: String) -> SettingSpec {
        guard let field = fields.first(where: { $0.key == key }) else {
            preconditionFailure("a rule has no field \(key)")
        }
        return field
    }
}
