import AllotContract
import AllotKit
import SwiftUI

/// The Commands page's rule groups: Test a command above the heavy-command rules (DESIGN.md
/// D6), and the rules as a numbered list with a menu per rule, Option-Up and Option-Down to move
/// the focused rule and Delete to delete it, then Add Rule… and Use Built-in Rules…. Every
/// change writes the whole list at once.
struct RuleListSections: View {
    let window: SettingsWindowModel
    let rules: RuleListModel
    let title: String

    @State private var matched: Set<Int> = []
    @State private var sheet: RuleSheet?
    @FocusState private var focusedRule: Int?

    var body: some View {
        let editable = !window.session.readOnly
        let list = rules.ruleList
        SettingsSection(title: "Test a command") {
            CommandTestBox(
                window: window, help: CommandTestBox.pageHelp,
                scope: TestScope(changes: [], rules: list)
            ) { found in
                matched = found
            }
            .padding(.vertical, 6)
        }
        SettingsSection(title: title) {
            (Text(RulesText.status(rules)) + Text("  ")
                + Text(rules.key).font(SettingsFont.captionMono))
                .font(SettingsFont.callout)
                .fixedSize(horizontal: false, vertical: true)
                .foregroundStyle(.secondary)
                .padding(.vertical, 6)
            if list.isEmpty {
                Text(RulesText.empty)
                    .fixedSize(horizontal: false, vertical: true)
                    .padding(.vertical, 6)
            } else {
                VStack(alignment: .leading, spacing: 0) {
                    ForEach(list.indices, id: \.self) { index in
                        Divider()
                        ruleRow(index, list[index], count: list.count, editable: editable)
                    }
                }
                .accessibilityElement(children: .contain)
                .accessibilityLabel("Rules, checked in order")
            }
            Divider()
            HStack(spacing: 8) {
                Button("Add Rule…") {
                    sheet = RuleSheet(draft: rules.add())
                }
                Button("Use Built-in Rules…") {
                    Task { await useBuiltIn() }
                }
                .disabled(!rules.canReset)
            }
            .disabled(!editable)
            .padding(.vertical, 6)
        }
        .sheet(item: $sheet) { shown in
            RuleSheetView(window: window, rules: rules, draft: shown.draft) {
                sheet = nil
            }
        }
    }

    private func ruleRow(_ index: Int, _ rule: HeavyRule, count: Int, editable: Bool) -> some View {
        let classLabel = classLabels(window)
        let isMatch = matched.contains(index)
        let name = RulesText.rowName(index, rule, classLabel: classLabel)
        // The row's label says all of it, with "matches" while it matches; the texts inside are
        // hidden from a screen reader, and the menu stays its own named control.
        return HStack(spacing: 12) {
            Text("\(index + 1)")
                .foregroundStyle(.secondary)
                .frame(width: 24 * SettingsFont.scale, alignment: .trailing)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 2) {
                Text(rule.program)
                    .font(SettingsFont.mono)
                Text(RulesText.arguments(rule))
                    .font(SettingsFont.calloutMono)
                    .foregroundStyle(.secondary)
            }
            .accessibilityHidden(true)
            Spacer(minLength: 8)
            if isMatch {
                // Text, not only the tint: "Matches".
                Text("Matches")
                    .font(SettingsFont.captionStrong)
                    .padding(EdgeInsets(top: 2, leading: 6, bottom: 2, trailing: 6))
                    .background(Color.accentColor.opacity(0.18), in: Capsule())
                    .accessibilityHidden(true)
            }
            Text("→ \(classLabel(rule.`class`))")
                .accessibilityHidden(true)
            Menu {
                Button("Edit…") {
                    edit(index)
                }
                Button("Move Up") {
                    move(index, to: index - 1)
                }
                .disabled(index == 0)
                Button("Move Down") {
                    move(index, to: index + 1)
                }
                .disabled(index == count - 1)
                Divider()
                Button("Delete") {
                    delete(index)
                }
            } label: {
                Image(systemName: "ellipsis.circle")
                    .accessibilityHidden(true)
            }
            .menuStyle(.button)
            .buttonStyle(.borderless)
            .menuIndicator(.hidden)
            .fixedSize()
            .disabled(!editable)
            .accessibilityLabel(RulesText.moreOptionsName(index))
        }
        .padding(EdgeInsets(top: 6, leading: 4, bottom: 6, trailing: 4))
        .background(
            isMatch ? Color.accentColor.opacity(0.08) : Color.clear,
            in: RoundedRectangle(cornerRadius: 6))
        .focusable(editable)
        .focusEffectDisabled()
        .focused($focusedRule, equals: index)
        // Our own focus ring, so a focused rule shows it whatever the system does for a custom
        // focusable row.
        .overlay {
            RoundedRectangle(cornerRadius: 6)
                .strokeBorder(Color.accentColor, lineWidth: 2)
                .opacity(focusedRule == index ? 1 : 0)
                .allowsHitTesting(false)
                .accessibilityHidden(true)
        }
        // Only while the rule has focus, so Delete in a text box never deletes a rule.
        .onKeyPress(keys: [.upArrow, .downArrow, .delete, .deleteForward], phases: .down) { press in
            ruleKey(press, index)
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel(isMatch ? "\(name), matches" : name)
        .accessibilityActions {
            if editable {
                Button("Edit…") {
                    edit(index)
                }
                Button("Move Up") {
                    move(index, to: index - 1)
                }
                Button("Move Down") {
                    move(index, to: index + 1)
                }
                Button("Delete") {
                    delete(index)
                }
            }
        }
    }

    private func ruleKey(_ press: KeyPress, _ index: Int) -> KeyPress.Result {
        // Arrow keys also carry the numeric-pad and function flags; only these four count.
        let held = press.modifiers.intersection([.shift, .control, .option, .command])
        switch press.key {
        case .upArrow where held == .option:
            move(index, to: index - 1)
        case .downArrow where held == .option:
            move(index, to: index + 1)
        case .delete where held.isEmpty, .deleteForward where held.isEmpty:
            delete(index)
        default:
            return .ignored
        }
        return .handled
    }

    private func edit(_ index: Int) {
        if let draft = rules.edit(index) {
            sheet = RuleSheet(draft: draft)
        }
    }

    private func move(_ index: Int, to: Int) {
        guard !window.session.readOnly, rules.ruleList.indices.contains(to) else {
            return
        }
        rules.move(from: index, to: to)
        focusedRule = to
    }

    private func delete(_ index: Int) {
        guard !window.session.readOnly, rules.ruleList.indices.contains(index) else {
            return
        }
        let old = rules.ruleList
        rules.remove(at: index)
        window.showNotice(RulesText.deleted(index)) {
            rules.replace(old)
        }
        let left = rules.ruleList.count
        focusedRule = left == 0 ? nil : min(index, left - 1)
    }

    private func useBuiltIn() async {
        let confirmation = RulesText.useBuiltIn(
            current: rules.ruleList.count, builtIn: rules.control.default.count)
        if await window.confirm(confirmation) {
            rules.reset()
            await window.session.flush()
        }
    }
}

/// The rule sheet being shown.
private struct RuleSheet: Identifiable {
    let id = UUID()
    let draft: RuleDraft
}

/// The edit sheet of a heavy-command rule, and Add Rule (DESIGN.md section 4, Commands): program,
/// arguments as chips and the lease class, with a test box that tries the rule before Save. Save
/// writes the whole list at once.
private struct RuleSheetView: View {
    private static let testHelp =
        "Shows how the hook would treat this command with this rule saved."

    let window: SettingsWindowModel
    let rules: RuleListModel
    let draft: RuleDraft
    let done: @MainActor () -> Void

    @State private var program = ""
    @FocusState private var programFocused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(RulesText.dialogTitle(draft))
                .font(SettingsFont.headline)
                .accessibilityAddTraits(.isHeader)
            VStack(alignment: .leading, spacing: 0) {
                SettingRow(
                    label: draft.programSpec.label, help: draft.programSpec.help,
                    notes: notes(draft.programIssue)
                ) {
                    TextField(draft.programSpec.label, text: $program)
                        .labelsHidden()
                        .font(SettingsFont.mono)
                        .autocorrectionDisabled()
                        .frame(width: 200 * SettingsFont.scale)
                        .rowHint()
                        .focused($programFocused)
                        .onChange(of: program) {
                            draft.setProgram(program)
                        }
                }
                Divider()
                SettingRow(
                    label: draft.args.label, help: draft.args.spec.help,
                    notes: notes(draft.args.issue), standalone: true
                ) {
                    EmptyView()
                }
                ChipsEditor(
                    model: draft.args, entryName: "New argument pattern", placeholder: "build",
                    addLabel: "Add Argument",
                    hint: RowText.hint(
                        help: draft.args.spec.help, key: nil, notes: notes(draft.args.issue)))
                    .padding(.bottom, 6)
                Divider()
                SettingRow(
                    label: draft.classSpec.label, help: draft.classSpec.help,
                    notes: notes(draft.classIssue)
                ) {
                    Picker(draft.classSpec.label, selection: ruleClass) {
                        ForEach(draft.classOptions, id: \.value) { option in
                            Text(option.label).tag(Optional(option.value))
                        }
                    }
                    .labelsHidden()
                    .fixedSize()
                    .rowHint()
                }
                Divider()
                CommandTestBox(window: window, help: Self.testHelp, scope: scope) { _ in }
                    .padding(.vertical, 6)
            }
            HStack {
                Spacer()
                Button("Cancel", role: .cancel) {
                    done()
                }
                .keyboardShortcut(.cancelAction)
                Button("Save") {
                    if rules.save(draft) {
                        done()
                    }
                }
                .buttonStyle(.borderedProminent)
            }
        }
        .padding(20)
        .font(SettingsFont.body)
        .frame(width: 560)
        .onAppear {
            program = draft.program
            // The first field takes focus.
            Task { @MainActor in
                programFocused = true
            }
        }
    }

    private var ruleClass: Binding<String?> {
        Binding(
            get: { draft.ruleClass },
            set: { value in
                if let value {
                    draft.setClass(value)
                }
            })
    }

    // The list with this rule saved; `nil` while the rule is not complete.
    private var scope: TestScope? {
        guard let changes = rules.proposal(draft) else {
            return nil
        }
        let proposed = changes.compactMap { change -> [HeavyRule]? in
            if case .setRules(let set) = change {
                return set.rules
            }
            return nil
        }
        return TestScope(changes: changes, rules: proposed.first ?? [])
    }

    private func notes(_ issue: ValueIssue?) -> [RowNote] {
        issue.map { [.issue(issueText($0, number: window.number))] } ?? []
    }
}

/// The rules a command line is tested against: changes beyond the page's, such as the list with
/// a sheet's draft, and the list they make, for naming the matched rule.
struct TestScope: Equatable {
    let changes: [Change]
    let rules: [HeavyRule]
}

/// Test a command (DESIGN.md D6): a command line, then one line per simple command saying how the
/// hook would treat it. On the Commands page it tests the rules on screen; in the rule sheet, the
/// list with the rule being edited. The Mac's commands reach the hook through Bash, so it always
/// tests as Bash.
private struct CommandTestBox: View {
    static let pageHelp = "Shows how the hook would treat this command with the rules below."

    // Typing tests after this pause, not on every keystroke.
    private static let pause = Duration.milliseconds(300)

    let window: SettingsWindowModel
    let help: String

    /// The rules to test against now; `nil` while a draft rule is not complete.
    let scope: TestScope?

    /// Called with the rules that matched, from 0, after each test; empty when none did.
    let matched: @MainActor (Set<Int>) -> Void

    @State private var line = ""
    @State private var result = ""

    var body: some View {
        let request = TestRequest(line: line, scope: scope, status: window.session.status)
        VStack(alignment: .leading, spacing: 4) {
            Text("Command line")
            Text(help)
                .font(SettingsFont.callout)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
                .accessibilityHidden(true)
            TextField("Command line", text: $line, prompt: Text("cargo build --release"))
                .labelsHidden()
                .font(SettingsFont.mono)
                .autocorrectionDisabled()
                .accessibilityHint(help)
            if !result.isEmpty {
                Text(result)
                    .font(SettingsFont.callout)
                    .foregroundStyle(.secondary)
                    .textSelection(.enabled)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .task(id: request) {
            try? await Task.sleep(for: Self.pause)
            guard !Task.isCancelled else {
                return
            }
            await run(request)
        }
    }

    private func run(_ request: TestRequest) async {
        guard !request.line.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            show("", nil)
            return
        }
        guard request.status == .ready else {
            show(LiveText.serviceStopped, nil)
            return
        }
        guard let scope = request.scope else {
            show(RulesText.draftNotReady, nil)
            return
        }
        let reply = try? await window.session.classify(
            request.line, shell: .bash, changes: scope.changes)
        guard !Task.isCancelled, let reply else {
            return
        }
        switch reply {
        case .value(let test):
            let lines = RulesText.testLines(
                test, rules: scope.rules, classLabel: classLabels(window), number: window.number)
            show(lines.joined(separator: "\n"), test)
        case .failure(let failure):
            show("Can't test this now: \(failure.message)", nil)
        }
    }

    private func show(_ text: String, _ test: CommandTest?) {
        if text != result && !text.isEmpty {
            AccessibilityNotification.Announcement(text).post()
        }
        result = text
        matched(RulesText.matched(test))
    }
}

/// What a test runs on; a change to any part tests again.
private struct TestRequest: Equatable {
    let line: String
    let scope: TestScope?
    let status: SessionStatus
}

/// Names each class as the Classes table does, such as "CPU heavy".
@MainActor
private func classLabels(_ window: SettingsWindowModel) -> @Sendable (JobClass) -> String {
    RulesText.classLabels((window.session.setting("classes") as? SizeTableModel)?.control)
}
