import AllotContract
import Foundation

/// The Commands page's sentences: the rule list's status and rows, its confirmations and notices,
/// and the lines of Test a command (DESIGN.md section 4, Commands).
public enum RulesText {
    public static let builtIn = "Built-in rules."

    public static let yours = "Your rules. They replace the built-in list."

    public static let empty =
        "No rules. No command counts as heavy, so the hook never asks for a lease."

    public static let anyArguments = "any arguments"

    public static let noCommand = "No command to test in this line."

    public static let draftNotReady = "Enter a program to test this rule."

    /// The status line above the list.
    @MainActor
    public static func status(_ rules: RuleListModel) -> String {
        rules.canReset ? yours : builtIn
    }

    /// A rule's arguments in its row: "build · test · run", or "any arguments".
    public static func arguments(_ rule: HeavyRule) -> String {
        rule.args.isEmpty ? anyArguments : rule.args.joined(separator: " · ")
    }

    /// A rule's row as VoiceOver reads it: "Rule 1: cargo, build · test, CPU heavy".
    public static func rowName(
        _ index: Int,
        _ rule: HeavyRule,
        classLabel: (JobClass) -> String
    ) -> String {
        "Rule \(index + 1): \(rule.program), \(arguments(rule)), \(classLabel(rule.`class`))"
    }

    /// The accessible name of a rule's menu button.
    public static func moreOptionsName(_ index: Int) -> String {
        "More options for rule \(index + 1)"
    }

    /// The notice after a delete, with Undo.
    public static func deleted(_ index: Int) -> String {
        "Rule \(index + 1) deleted."
    }

    /// The rule sheet's title.
    @MainActor
    public static func dialogTitle(_ draft: RuleDraft) -> String {
        draft.index.map { "Edit rule \($0 + 1)" } ?? "Add rule"
    }

    /// Use Built-in Rules' confirmation.
    ///
    /// - Parameters:
    ///   - current: The rules in the list now.
    ///   - builtIn: The built-in rules.
    public static func useBuiltIn(current: Int, builtIn: Int) -> Confirmation {
        let yours = current == 1 ? "Your 1 rule is" : "Your \(current) rules are"
        let ones = builtIn == 1 ? "built-in one" : "\(builtIn) built-in ones"
        return Confirmation(
            title: "Use the built-in rules?",
            body: "\(yours) replaced by the \(ones). This can't be undone.",
            button: "Use Built-in Rules")
    }

    /// The label of a class from the Classes table's spec, such as "CPU heavy".
    public static func classLabels(
        _ classes: SettingControl.SizeTable?
    ) -> @Sendable (JobClass) -> String {
        let rows = classes?.rows ?? []
        return { jobClass in
            rows.first { $0.name == jobClass.rawValue }?.label ?? jobClass.rawValue
        }
    }

    /// One line per simple command of the tested line, and one more naming the combined lease
    /// when more than one command is heavy.
    ///
    /// - Parameters:
    ///   - test: allotd's answer.
    ///   - rules: The rules it was tested against, for the matched rule's text.
    ///   - classLabel: Names a class, such as "CPU heavy".
    ///   - number: Formats a number for the user's locale.
    public static func testLines(
        _ test: CommandTest,
        rules: [HeavyRule],
        classLabel: (JobClass) -> String,
        number: (Double) -> String
    ) -> [String] {
        if test.commands.isEmpty {
            return [noCommand]
        }
        var lines = test.commands.map { result -> String in
            switch result.kind {
            case .heavy:
                heavyLine(result, rules: rules, classLabel: classLabel, number: number)
            case .printOnly:
                "Print-only: \(result.flag ?? "") only prints and exits, so no lease is needed."
            case .notHeavy:
                "Not heavy: no rule matches \(result.command)."
            }
        }
        let heavy = test.commands.filter { $0.kind == .heavy }.count
        if heavy > 1, let lease = test.lease {
            let combined = leaseText(lease, classLabel: classLabel, number: number)
            lines.append("The whole line needs one \(combined).")
        }
        return lines
    }

    /// The rules that matched a command of the tested line, from 0, for the "Matches" mark.
    public static func matched(_ test: CommandTest?) -> Set<Int> {
        var matched = Set<Int>()
        for result in test?.commands ?? [] where result.kind == .heavy {
            if let index = result.ruleIndex {
                matched.insert(Int(index))
            }
        }
        return matched
    }

    private static func heavyLine(
        _ result: CommandResult,
        rules: [HeavyRule],
        classLabel: (JobClass) -> String,
        number: (Double) -> String
    ) -> String {
        let needs = result.size.map {
            "Heavy: needs a \(leaseText($0, classLabel: classLabel, number: number))."
        } ?? "Heavy."
        guard let index = result.ruleIndex.map(Int.init), rules.indices.contains(index) else {
            return needs
        }
        let rule = rules[index]
        let args = rule.args.isEmpty ? anyArguments : alternatives(rule.args)
        return "\(needs) Rule \(index + 1): \(rule.program) with \(args)."
    }

    // "CPU heavy lease (8 cores, 4 GB memory)"; the GPU parts only when the class asks for them.
    private static func leaseText(
        _ size: ClassSize,
        classLabel: (JobClass) -> String,
        number: (Double) -> String
    ) -> String {
        let profile = size.profile
        func text(_ value: Double, _ quantity: Quantity) -> String {
            quantityText(value, quantity, number: number)
        }
        var parts = [
            text(double(profile.cores), .cores),
            "\(text(Double(profile.ramMb), .megabytes)) memory",
        ]
        if profile.gpuPct > 0 {
            parts.append("\(text(double(profile.gpuPct), .percent)) of the GPU")
        }
        if profile.vramMb > 0 {
            parts.append("\(text(Double(profile.vramMb), .megabytes)) video memory")
        }
        return "\(classLabel(size.`class`)) lease (\(parts.joined(separator: ", ")))"
    }

    // "build, test, run or bench".
    private static func alternatives(_ parts: [String]) -> String {
        guard parts.count >= 2, let last = parts.last else {
            return parts.joined()
        }
        return "\(parts.dropLast().joined(separator: ", ")) or \(last)"
    }
}
