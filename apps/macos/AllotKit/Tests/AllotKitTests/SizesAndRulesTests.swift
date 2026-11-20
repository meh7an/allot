import AllotContract
import Foundation
import Testing

@testable import AllotKit

// The Sizes and Commands pages' view models (M5.2, ported from M4.2's tests): table cells and
// Fits now, the edit sheets' titles, warnings and proposals, Undo after a row reset or a rule
// delete, and Test a command.

private let english = SettingsText.numbers(Locale(identifier: "en_US"))

private struct Loaded {
    let session: SettingsSession
    let client: FakeSettings
    let timers: FakeTimers
}

/// A session loaded from the settings-state fixture, changed by `edit`; writes answer with the
/// same state, so it stays the saved one.
@MainActor
private func loaded(_ edit: (inout [String: Any]) -> Void = { _ in }) async throws -> Loaded {
    let client = FakeSettings(state: try fixtureState(edit))
    let timers = FakeTimers()
    let session = SettingsSession(
        client: client, timers: timers, platform: .macos, readStopRecord: { nil })
    try await session.refresh()
    return Loaded(session: session, client: client, timers: timers)
}

/// Replaces the value of `key` in a settings-state object.
private func setValue(_ object: inout [String: Any], _ key: String, _ value: Any) {
    var values = object["values"] as? [[String: Any]] ?? []
    if let index = values.firstIndex(where: { $0["key"] as? String == key }) {
        values[index]["value"] = value
    }
    object["values"] = values
}

/// The fixture with a preset the user added, `render`.
private func withRender(_ object: inout [String: Any]) {
    var values = object["values"] as? [[String: Any]] ?? []
    if let index = values.firstIndex(where: { $0["key"] as? String == "presets" }) {
        var rows = values[index]["value"] as? [[String: Any]] ?? []
        rows.append([
            "name": "render",
            "profile": [
                "cores": 6, "ram_mb": 8192, "gpu_pct": 40, "vram_mb": 4096, "est_secs": 900,
            ],
        ])
        values[index]["value"] = rows
    }
    object["values"] = values
}

@MainActor
private func setting<Model: ValueModel>(
    _ session: SettingsSession,
    _ key: String,
    _ type: Model.Type
) throws -> Model {
    try #require(session.setting(key) as? Model)
}

/// Lets a save in flight finish, fires the burst timer, and lets that save finish.
@MainActor
private func fire(_ timers: FakeTimers) async {
    await settle()
    timers.fireAll()
    await settle()
}

private func decoded<Value: Decodable>(
    _ type: Value.Type,
    _ object: [String: Any]
) throws -> Value {
    try JSONDecoder().decode(type, from: JSONSerialization.data(withJSONObject: object))
}

private func counts(_ full: Int, _ shrunk: Int, more: Bool = false) -> [String: Any] {
    ["full": full, "shrunk": shrunk, "more": more]
}

private func fit(_ full: Int, _ shrunk: Int, more: Bool = false) throws -> SizeFit {
    try decoded(
        SizeFit.self,
        [
            "name": "cpu_heavy", "min_cores": 4, "now": counts(full, shrunk, more: more),
            "idle": counts(0, 0),
        ])
}

private func profile(
    _ cores: Double,
    _ ramMb: Int,
    _ gpuPct: Double,
    _ vramMb: Int
) -> [String: Any] {
    ["cores": cores, "ram_mb": ramMb, "gpu_pct": gpuPct, "vram_mb": vramMb, "est_secs": 300]
}

private func size(_ jobClass: String, _ profile: [String: Any]) -> [String: Any] {
    ["class": jobClass, "min_cores": 1, "profile": profile]
}

private func heavy(_ command: String, _ ruleIndex: Int, _ size: [String: Any]) -> [String: Any] {
    ["command": command, "kind": "heavy", "rule_index": ruleIndex, "size": size]
}

/// A change as the tests compare it: its kind, and the row's name for row changes.
private func describe(_ change: Change) -> String {
    switch change {
    case .setRow(let row): "set_row \(row.name)"
    case .resetRow(let row): "reset_row \(row.name)"
    case .setNumber: "set_number"
    case .setRules: "set_rules"
    default: "other"
    }
}

private func labels(_ jobClass: JobClass) -> String {
    switch jobClass {
    case .cpuHeavy: "CPU heavy"
    case .gpuHeavy: "GPU heavy"
    case .mixed: "Mixed"
    case .light: "Light"
    }
}

@Suite struct SizesTextTests {
    @Test func sizeCellsCarryTheirUnitsAndTheAutomaticMinimum() {
        let row = Contract.sizeRow(
            name: "big",
            profile: Contract.profile(
                cores: 8, ramMb: 4096, gpuPct: 50, vramMb: 2048, estSecs: 300),
            minCores: nil)

        #expect(
            SizesText.cells(row, number: english)
                == SizeCells(
                    cores: "8", minCores: "Automatic (4)", memory: "4 GB", gpu: "50 %",
                    videoMemory: "2 GB", runTime: "5 minutes"))

        let other = SizesText.cells(
            Contract.sizeRow(
                name: "big",
                profile: Contract.profile(
                    cores: 8, ramMb: 1000, gpuPct: 0, vramMb: 0, estSecs: 45),
                minCores: 6),
            number: english)
        #expect(other.minCores == "6")
        #expect(other.memory == "1,000 MB")
        #expect(other.runTime == "45 seconds")
    }

    @Test func theAutomaticMinimumIsHalfTheCoresButAtLeastOne() {
        #expect(SizesText.automaticMinCores(8) == 4)
        #expect(SizesText.automaticMinCores(1.5) == 1)
        #expect(SizesText.automaticMinCores(1) == 1)
        #expect(SizesText.automaticMinCores(0.5) == 0.5)
        #expect(SizesText.automaticMinCores(0) == 0)
    }

    @Test func fitsNowReadsShortInTheTable() throws {
        #expect(SizesText.fitsNow(nil, number: english) == SizesText.fitsUnknown)
        #expect(SizesText.fitsNow(try fit(0, 0), number: english) == "None")
        #expect(SizesText.fitsNow(try fit(3, 0), number: english) == "3")
        #expect(SizesText.fitsNow(try fit(0, 2), number: english) == "2 shrunk")
        #expect(SizesText.fitsNow(try fit(1, 2), number: english) == "1, 2 more shrunk")
        #expect(SizesText.fitsNow(try fit(99, 0, more: true), number: english) == "at least 99")
        #expect(
            SizesText.fitsNow(try fit(2, 99, more: true), number: english)
                == "2, at least 99 more shrunk")
    }

    @Test func theDialogLineNamesNowAndWithEveryLeaseEnded() throws {
        let line = try decoded(
            SizeFit.self,
            ["name": "cpu_heavy", "min_cores": 4, "now": counts(1, 2), "idle": counts(2, 0)])
        #expect(
            SizesText.dialogFits(line, number: english)
                == "With these values, now: 1 at full size, 2 more at 4 cores. With every lease "
                + "ended: 2 at full size.")

        let none = try decoded(
            SizeFit.self,
            ["name": "cpu_heavy", "min_cores": 4, "now": counts(0, 0), "idle": counts(2, 0)])
        let noneLine = SizesText.dialogFits(none, number: english)
        #expect(noneLine.hasPrefix("With these values, now: none fit."))
    }

    @Test func deletingAPresetIsConfirmedInTitleCase() {
        #expect(
            SizesText.deletePreset("render")
                == Confirmation(
                    title: "Delete preset render?",
                    body: "Sessions that ask for it by name get an error. You can add it again "
                        + "later.",
                    button: "Delete Preset"))
    }
}

@MainActor
@Suite struct SizeTableTests {
    @Test func dialogTitlesNameTheClassOrPreset() async throws {
        let session = try await loaded(withRender).session
        let classes = try setting(session, "classes", SizeTableModel.self)
        let presets = try setting(session, "presets", SizeTableModel.self)

        #expect(
            SizesText.dialogTitle(classes, try #require(classes.edit("cpu_heavy")))
                == "Edit CPU heavy")
        #expect(
            SizesText.dialogTitle(presets, try #require(presets.edit("render")))
                == "Edit preset render")
        #expect(SizesText.dialogTitle(presets, presets.add()) == SizesText.addPresetTitle)
    }

    @Test func renamingAPresetWarnsAboutSessionsThatAskForTheOldName() async throws {
        let session = try await loaded(withRender).session
        let presets = try setting(session, "presets", SizeTableModel.self)
        let draft = try #require(presets.edit("render"))

        #expect(SizesText.renameWarning(draft) == nil)
        draft.setName("render_big")
        #expect(
            SizesText.renameWarning(draft)
                == "Sessions that ask for render get an error until they use render_big.")
        #expect(SizesText.renameWarning(try #require(presets.edit("cpu_small"))) == nil)
    }

    @Test func rowActionsNameTheirRowAndSayWhatHappened() async throws {
        let session = try await loaded(withRender).session
        let classes = try setting(session, "classes", SizeTableModel.self)
        let presets = try setting(session, "presets", SizeTableModel.self)
        let cpu = try #require(classes.rows.first { $0.row.name == "cpu_heavy" })
        let small = try #require(presets.rows.first { $0.row.name == "cpu_small" })
        let render = try #require(presets.rows.first { $0.row.name == "render" })

        #expect(SizesText.editName(classes, cpu) == "Edit CPU heavy")
        #expect(SizesText.resetName(classes, cpu) == "Reset CPU heavy to its built-in size")
        #expect(SizesText.rowRemoved(classes, cpu) == "CPU heavy reset to its built-in size.")
        #expect(
            SizesText.rowRemoved(presets, small) == "Preset cpu_small reset to its built-in size.")
        #expect(SizesText.deleteName(render) == "Delete preset render")
        #expect(SizesText.rowRemoved(presets, render) == "Preset render deleted.")
    }

    @Test func deletingAPresetThenUndoWritesItBack() async throws {
        let parts = try await loaded(withRender)
        let presets = try setting(parts.session, "presets", SizeTableModel.self)
        let old = try #require(presets.sizeRows.first { $0.name == "render" })

        presets.remove("render")
        #expect(!presets.sizeRows.contains { $0.name == "render" })
        await fire(parts.timers)
        presets.restore(old)
        await fire(parts.timers)

        let patches = parts.client.patches
        #expect(patches.count == 2)
        #expect(patches.first?.changes == [Contract.resetRow(table: "presets", name: "render")])
        #expect(patches.last?.changes == [Contract.setRow(table: "presets", row: old)])
    }

    @Test func aRowProposalFollowsTheDialogAndWaitsForAValidRow() async throws {
        let parts = try await loaded(withRender)
        let presets = try setting(parts.session, "presets", SizeTableModel.self)
        let draft = try #require(presets.edit("render"))
        try #require(draft.field("cores")).commit(10)

        let proposal = try #require(presets.proposal(draft))
        #expect(proposal.count == 1)
        guard case .setRow(let row)? = proposal.first else {
            Issue.record("expected a set_row, got \(proposal)")
            return
        }
        #expect(row.profile.cores == 10)

        draft.setName("render_big")
        #expect(
            presets.proposal(draft)?.map(describe) == ["reset_row render", "set_row render_big"])

        draft.setName("Render")
        #expect(!draft.ready)
        #expect(presets.proposal(draft) == nil)

        let added = presets.add()
        added.setName("fresh")
        // A new preset needs its cores and memory first, and a proposal shows no issue.
        #expect(presets.proposal(added) == nil)
        #expect(added.field("cores")?.issue == nil)
        #expect(parts.client.patches.isEmpty)
    }

    @Test func thePreviewAddsTheDialogsChangesAfterTheUnsavedOnes() async throws {
        let parts = try await loaded()
        try setting(parts.session, "admission.headroom_pct", NumberModel.self).commit(30)
        let classes = try setting(parts.session, "classes", SizeTableModel.self)
        let draft = try #require(classes.edit("mixed"))
        try #require(draft.field("cores")).commit(12)

        _ = try await parts.session.preview(classes.proposal(draft) ?? [])

        #expect(parts.client.previews.last?.map(describe) == ["set_number", "set_row mixed"])
    }
}

@MainActor
@Suite struct StringListUndoTests {
    @Test func resettingPrintOnlyFlagsThenUndoWritesTheOldListBack() async throws {
        let parts = try await loaded { object in
            setValue(&object, "classifier.light_flags", ["--help", "--dry-run"])
        }
        let flags = try setting(parts.session, "classifier.light_flags", StringListModel.self)
        #expect(flags.canReset)
        let saved = flags.entries

        let undo = try #require(FieldReset.reset(flags))
        await fire(parts.timers)
        undo()
        await fire(parts.timers)

        let patches = parts.client.patches
        #expect(patches.count == 2)
        #expect(patches.first?.changes == [Contract.reset("classifier.light_flags")])
        #expect(patches.last?.changes == [Contract.setList("classifier.light_flags", saved)])
    }
}

@MainActor
@Suite struct RuleListTests {
    @Test func deletingARuleThenUndoWritesTheWholeListBack() async throws {
        let parts = try await loaded()
        let rules = try setting(parts.session, "classifier.rules", RuleListModel.self)
        let old = rules.ruleList

        rules.remove(at: 0)
        #expect(rules.ruleList.first?.program == "pnpm")
        await fire(parts.timers)
        rules.replace(old)
        await fire(parts.timers)

        #expect(parts.client.patches.last?.changes == [Contract.setRules(old)])
    }

    @Test func aRuleProposalPutsTheDraftInItsPlaceOrAtTheEnd() async throws {
        let session = try await loaded().session
        let rules = try setting(session, "classifier.rules", RuleListModel.self)
        let count = rules.ruleList.count

        let edited = try #require(rules.edit(1))
        edited.setProgram("bun")
        let replaced = try proposedRules(rules, edited)
        #expect(replaced.count == count)
        #expect(replaced[1].program == "bun")

        let added = rules.add()
        // A new rule needs its program first, and a proposal shows no issue.
        #expect(rules.proposal(added) == nil)
        #expect(added.programIssue == nil)
        added.setProgram("ninja")
        added.setClass("mixed")
        let appended = try proposedRules(rules, added)
        #expect(appended.count == count + 1)
        #expect(appended.last?.`class` == .mixed)
    }

    @Test func ruleListTextsFollowTheList() async throws {
        let session = try await loaded().session
        let rules = try setting(session, "classifier.rules", RuleListModel.self)
        let list = rules.ruleList
        let classLabel = RulesText.classLabels(
            try setting(session, "classes", SizeTableModel.self).control)

        #expect(RulesText.status(rules) == RulesText.builtIn)
        #expect(RulesText.arguments(list[0]) == "build · test · run · bench")
        #expect(RulesText.arguments(list[2]) == RulesText.anyArguments)
        #expect(
            RulesText.rowName(0, list[0], classLabel: classLabel)
                == "Rule 1: cargo, build · test · run · bench, CPU heavy")
        #expect(RulesText.moreOptionsName(2) == "More options for rule 3")
        #expect(RulesText.deleted(2) == "Rule 3 deleted.")
        #expect(RulesText.dialogTitle(try #require(rules.edit(1))) == "Edit rule 2")
        #expect(RulesText.dialogTitle(rules.add()) == "Add rule")

        let yours = try await loaded { object in
            setValue(
                &object, "classifier.rules",
                [["program": "ninja", "args": [String](), "class": "cpu_heavy"] as [String: Any]])
        }.session
        #expect(
            RulesText.status(try setting(yours, "classifier.rules", RuleListModel.self))
                == RulesText.yours)
        #expect(
            RulesText.useBuiltIn(current: 5, builtIn: 6)
                == Confirmation(
                    title: "Use the built-in rules?",
                    body: "Your 5 rules are replaced by the 6 built-in ones. This can't be "
                        + "undone.",
                    button: "Use Built-in Rules"))
        #expect(
            RulesText.useBuiltIn(current: 1, builtIn: 6).body
                == "Your 1 rule is replaced by the 6 built-in ones. This can't be undone.")
    }

    @Test func theFixtureTestReadsOneLinePerCommand() async throws {
        let session = try await loaded().session
        let rules = try setting(session, "classifier.rules", RuleListModel.self)
        let classLabel = RulesText.classLabels(
            try setting(session, "classes", SizeTableModel.self).control)
        let test = try JSONDecoder().decode(
            CommandTest.self, from: try Fixtures.data("command-test.json"))

        let lines = RulesText.testLines(
            test, rules: rules.ruleList, classLabel: classLabel, number: english)

        #expect(
            lines == [
                "Heavy: needs a CPU heavy lease (8 cores, 4 GB memory). Rule 1: cargo with build, "
                    + "test, run or bench.",
                "Print-only: --help only prints and exits, so no lease is needed.",
                "Not heavy: no rule matches cargo fmt.",
            ])
        #expect(RulesText.matched(test) == [0])
        #expect(RulesText.matched(nil).isEmpty)
    }

    private func proposedRules(_ rules: RuleListModel, _ draft: RuleDraft) throws -> [HeavyRule] {
        let proposal = try #require(rules.proposal(draft))
        guard proposal.count == 1, case .setRules(let set)? = proposal.first else {
            Issue.record("expected one set_rules, got \(proposal)")
            return []
        }
        return set.rules
    }
}

@Suite struct CommandTestTextTests {
    @Test func twoHeavyCommandsNameTheCombinedLease() throws {
        let rules = [
            Contract.rule(program: "cargo", args: [], class: .cpuHeavy),
            Contract.rule(program: "python*", args: ["train*.py"], class: .gpuHeavy),
        ]
        let test = try decoded(
            CommandTest.self,
            [
                "lease": size("mixed", profile(6, 8192, 50, 2048)),
                "commands": [
                    heavy("cargo build", 0, size("cpu_heavy", profile(8, 4096, 0, 0))),
                    heavy("python train.py", 1, size("gpu_heavy", profile(2, 4096, 50, 2048))),
                ],
            ])

        let lines = RulesText.testLines(test, rules: rules, classLabel: labels, number: english)

        #expect(
            lines == [
                "Heavy: needs a CPU heavy lease (8 cores, 4 GB memory). Rule 1: cargo with any "
                    + "arguments.",
                "Heavy: needs a GPU heavy lease (2 cores, 4 GB memory, 50 % of the GPU, 2 GB "
                    + "video memory). Rule 2: python* with train*.py.",
                "The whole line needs one Mixed lease (6 cores, 8 GB memory, 50 % of the GPU, 2 "
                    + "GB video memory).",
            ])
        #expect(RulesText.matched(test) == [0, 1])
    }

    @Test func aLineWithNoCommandSaysSo() throws {
        let test = try decoded(CommandTest.self, ["commands": [Any]()])

        #expect(
            RulesText.testLines(test, rules: [], classLabel: labels, number: english)
                == [RulesText.noCommand])
    }
}
