import AllotContract
import Foundation
import Testing

@testable import AllotKit

private struct Harness {
    let session: SettingsSession
    let client: FakeSettings
    let timers: FakeTimers
}

/// A session loaded from the settings-state fixture.
@MainActor
private func loaded(
    platform: Platform = .linux,
    stopRecord: String? = nil,
    state: SettingsState? = nil
) async throws -> Harness {
    let client = FakeSettings(state: try state ?? fixtureState())
    let timers = FakeTimers()
    let session = SettingsSession(
        client: client, timers: timers, platform: platform, readStopRecord: { stopRecord })
    try await session.refresh()
    return Harness(session: session, client: client, timers: timers)
}

/// The fixture's state with a preset of the user's own, `render`.
private func stateWithRender() throws -> SettingsState {
    try fixtureState { object in
        var values = object["values"] as? [[String: Any]] ?? []
        if let index = values.firstIndex(where: { $0["key"] as? String == "presets" }) {
            var presets = values[index]
            var rows = presets["value"] as? [[String: Any]] ?? []
            rows.append([
                "name": "render",
                "profile": [
                    "cores": 6, "ram_mb": 8192, "gpu_pct": 40, "vram_mb": 4096, "est_secs": 900,
                ],
            ])
            presets["value"] = rows
            values[index] = presets
        }
        object["values"] = values
    }
}

/// Lets a save in flight finish, fires the burst timer, and lets that save finish.
@MainActor
private func fire(_ timers: FakeTimers) async {
    await settle()
    timers.fireAll()
    await settle()
}

@MainActor
private func model<Model: ValueModel>(
    _ session: SettingsSession,
    _ key: String,
    _ type: Model.Type
) throws -> Model {
    try #require(session.setting(key) as? Model)
}

@MainActor
@Suite struct SettingsSessionTests {
    @Test func theFixtureLoadsEverySettingWithItsViewModel() async throws {
        let (session, _, _) = try await loadedParts()

        #expect(session.status == .ready)
        #expect(!session.readOnly)
        #expect(session.pendingRestart == ["daemon.port"])
        #expect(
            session.pages.map { $0.spec.id }
                == ["admission", "sizes", "commands", "benchmarks", "leases", "service"])
        _ = try model(session, "estimates.lower", SwitchModel.self)
        _ = try model(session, "admission.headroom_pct", NumberModel.self)
        _ = try model(session, "leases.idle_release_secs", DurationModel.self)
        _ = try model(session, "classifier.light_flags", StringListModel.self)
        _ = try model(session, "presets", SizeTableModel.self)
        _ = try model(session, "classifier.rules", RuleListModel.self)
        #expect(session.saveStatus == .saved)
    }

    @Test func theLinuxGroupShowsOnlyOnLinux() async throws {
        let linux = try await loaded(platform: .linux)
        let macos = try await loaded(platform: .macos)

        func groups(_ session: SettingsSession) -> [String]? {
            session.pages.first { $0.spec.id == "service" }?.groups.map { $0.spec.id }
        }
        #expect(groups(linux.session) == ["connection", "linux"])
        #expect(groups(macos.session) == ["connection"])
        #expect(macos.session.setting("linux.scopes") == nil)
    }

    @Test func resetShowsOnlyForValuesThatDifferFromTheDefault() async throws {
        let (session, _, _) = try await loadedParts()

        #expect(session.setting("daemon.port")?.canReset == true)
        #expect(session.setting("admission.headroom_pct")?.canReset == false)
        #expect(session.setting("classes")?.canReset == true)
        #expect(session.setting("presets")?.canReset == false)
        #expect(session.changedOn("service").map { $0.key } == ["daemon.port"])
    }

    @Test func singleValuesSaveAsOneBurst600MsAfterTheLastChange() async throws {
        let (session, client, timers) = try await loadedParts()
        let lower = try model(session, "estimates.lower", SwitchModel.self)
        let headroom = try model(session, "admission.headroom_pct", NumberModel.self)

        lower.set(false)
        headroom.commit(20)
        headroom.commit(25)

        #expect(timers.delays == [SettingsSession.saveDelay])
        #expect(session.saveStatus == .unsaved(count: 2))
        await fire(timers)

        let patch = try #require(client.patches.first)
        #expect(client.patches.count == 1)
        #expect(patch.revision == "69fd99abe4f654c7")
        #expect(
            try jsonObject(patch.changes)
                == jsonObject(
                    """
                    [{"kind": "set_switch", "key": "estimates.lower", "value": false},
                     {"kind": "set_number", "key": "admission.headroom_pct", "value": 25}]
                    """))
        #expect(session.saveStatus == .saved)
        #expect(!headroom.dirty)
    }

    @Test func aValueOutsideItsRangeShowsWhyAndIsNotSent() async throws {
        let (session, client, timers) = try await loadedParts()
        let headroom = try model(session, "admission.headroom_pct", NumberModel.self)

        headroom.commit(100)

        #expect(timers.delays.isEmpty)
        #expect(headroom.number == 100)
        #expect(
            issueText(try #require(headroom.issue), number: plain)
                == "Enter a number at least 0 and below 100.")
        #expect(session.saveStatus == .unsaved(count: 1))
        headroom.undo()
        #expect(headroom.number == 15)
        #expect(session.saveStatus == .saved)
        #expect(client.patches.isEmpty)
    }

    @Test func aValueBelowTheSettingItDependsOnNamesThatSetting() async throws {
        let (session, _, _) = try await loadedParts()
        let wait = try model(session, "benchmark.max_quiet_wait_secs", DurationModel.self)

        wait.setUnit("seconds")
        wait.commit(5)

        #expect(wait.issue == .atLeast(label: "Quiet for", value: 10, quantity: .seconds))
        #expect(
            issueText(try #require(wait.issue), number: plain)
                == "Must be at least Quiet for (10 seconds).")
    }

    @Test func allotdsRefusalLandsUnderItsFieldAndTheRestIsSentAgain() async throws {
        let (session, client, timers) = try await loadedParts()
        let problem = try fieldError(
            "admission.headroom_pct", "must be at least 0 and below 100")
        client.queueWrite(failure(.invalid, problems: [problem]))
        try model(session, "admission.headroom_pct", NumberModel.self).commit(50)
        try model(session, "estimates.lower", SwitchModel.self).set(false)

        await fire(timers)
        await fire(timers)

        let headroom = try model(session, "admission.headroom_pct", NumberModel.self)
        #expect(headroom.issue == .service("Headroom must be at least 0 and below 100"))
        #expect(headroom.number == 50)
        #expect(client.patches.count == 2)
        #expect(
            try jsonObject(client.patches[1].changes)
                == jsonObject(#"[{"kind": "set_switch", "key": "estimates.lower", "value": false}]"#))
        #expect(session.saveStatus == .unsaved(count: 1))
    }

    @Test func aHandEditInBetweenShowsTheFilesValuesAndCanApplyTheChangeAgain() async throws {
        let (session, client, timers) = try await loadedParts()
        let margin = try model(session, "estimates.margin_pct", NumberModel.self)
        client.queueWrite(failure(.conflict, revision: "f38fd8b70ab492ce"))
        client.queueRead(.value(try fixtureState { $0["revision"] = "f38fd8b70ab492ce" }))
        let change = #"[{"kind": "set_number", "key": "estimates.margin_pct", "value": 15}]"#

        margin.commit(15)
        await fire(timers)

        #expect(margin.number == 10)
        let conflict = try #require(session.conflict)
        #expect(conflict.edits.count == 1)
        #expect(try jsonObject(conflict.edits[0].changes) == jsonObject(change))
        session.applyMine()
        await fire(timers)
        #expect(session.conflict == nil)
        #expect(client.patches.count == 2)
        #expect(client.patches[1].revision == "f38fd8b70ab492ce")
        #expect(try jsonObject(client.patches[1].changes) == jsonObject(change))
    }

    @Test func aHeldFileKeepsTheChangesUnsavedUntilTheNextSave() async throws {
        let (session, client, timers) = try await loadedParts()
        client.queueWrite(failure(.held, message: "config.toml is open in another program"))

        try model(session, "hook.fail_open", SwitchModel.self).set(false)
        await fire(timers)

        #expect(session.writeProblem == "config.toml is open in another program")
        #expect(session.saveStatus == .unsaved(count: 1))
        await fire(timers)
        #expect(client.patches.count == 2)
        #expect(session.writeProblem == nil)
    }

    @Test func aFailedWriteOtherThanHeldWaitsForTryAgainOrTheNextEdit() async throws {
        let (session, client, timers) = try await loadedParts()
        client.queueWrite(failure(.failed, message: "config.toml is read-only"))

        try model(session, "hook.fail_open", SwitchModel.self).set(false)
        await fire(timers)
        try await session.refresh()

        #expect(session.writeProblem == "config.toml is read-only")
        #expect(timers.delays.isEmpty)
        await fire(timers)
        #expect(client.patches.count == 1)
        #expect(session.saveStatus == .unsaved(count: 1))

        await session.flush()

        #expect(client.patches.count == 2)
        #expect(session.writeProblem == nil)
    }

    @Test func changesAllotdDidNotAnswerGoOutAgainOnceARefreshFindsItBack() async throws {
        let (session, client, timers) = try await loadedParts()
        client.queueWrite(failure(.unreachable, message: "Connection refused"))

        try model(session, "hook.fail_open", SwitchModel.self).set(false)
        await fire(timers)
        await fire(timers)
        #expect(client.patches.count == 1)
        #expect(session.saveStatus == .unsaved(count: 1))

        try await session.refresh()
        await fire(timers)
        #expect(client.patches.count == 2)
    }

    @Test func aRefreshSentBeforeASaveDoesNotBringBackTheOldRevision() async throws {
        let (session, client, timers) = try await loadedParts()
        client.holdReads = true
        let refresh = Task { try await session.refresh() }
        await settle()
        client.queueWrite(.value(try fixtureState { $0["revision"] = "after-save" }))

        try model(session, "hook.fail_open", SwitchModel.self).set(false)
        await fire(timers)
        client.completeReads()
        try await refresh.value
        try model(session, "estimates.lower", SwitchModel.self).set(false)
        await fire(timers)

        #expect(client.patches[1].revision == "after-save")
    }

    @Test func aRefusalNoBatchedSettingOwnsIsShownBeforeTheEditsAreUndone() async throws {
        let (session, client, timers) = try await loadedParts()
        let headroom = try model(session, "admission.headroom_pct", NumberModel.self)
        client.queueWrite(
            failure(
                .invalid, message: "config.toml is not valid TOML",
                problems: [try fieldError("classifier.nowhere", "unknown")]))

        headroom.commit(50)
        await fire(timers)

        #expect(session.writeProblem == "config.toml is not valid TOML")
        #expect(headroom.number == 15)
        await session.flush()
        #expect(session.writeProblem == nil)
    }

    @Test func aNumberTooLargeForAFloatIsRefusedAsNotANumber() async throws {
        let (session, _, _) = try await loadedParts()
        let presets = try model(session, "presets", SizeTableModel.self)
        let draft = presets.add()

        draft.field("cores")?.commit(Double("9999999999999999999999999999999999999999") ?? 0)

        #expect(draft.field("cores")?.issue == .notANumber)
    }

    @Test func aBlankChipFlagGoesWhenTheEntryChangesAndNeverCountsAsUnsaved() async throws {
        let (session, _, _) = try await loadedParts()
        let flags = try model(session, "classifier.light_flags", StringListModel.self)

        #expect(flags.add(" ") == false)
        #expect(session.saveStatus == .saved)
        flags.entryChanged()
        #expect(flags.issue == nil)
        #expect(flags.add("--help") == false)
        #expect(flags.issue == .duplicate)
        flags.entryChanged()
        #expect(flags.issue == nil)
    }

    @Test func aRejectedFileMakesThePagesReadOnly() async throws {
        let (session, client, _) = try await loadedParts()
        client.queueRead(.value(try fixtureState { $0["file_error"] = "expected `=`" }))

        try await session.refresh()

        #expect(session.readOnly)
        #expect(session.fileError == "expected `=`")
    }

    @Test func noAnswerReadsTheStopRecordToTellAStopFromAFailure() async throws {
        let record = String(decoding: try Fixtures.data("stop-record.json"), as: UTF8.self)
        let stopped = try await loaded(stopRecord: record)
        let down = try await loaded()
        for harness in [stopped, down] {
            harness.client.queueRead(failure(.unreachable, message: "Connection refused"))
            try await harness.session.refresh()
        }

        #expect(stopped.session.status == .stopped)
        #expect(stopped.session.stopRecord?.by == .panel)
        #expect(down.session.status == .down)
        #expect(down.session.detail == "Connection refused")
        #expect(down.session.readOnly)
        #expect(down.session.setting("daemon.port")?.value == .number(47999))
    }

    @Test func aTokenRefusalIsItsOwnStatusAndAnUnusableAnswerFails() async throws {
        let token = try await loaded()
        let failed = try await loaded()
        token.client.queueRead(failure(.token))
        failed.client.queueRead(failure(.failed, message: "boom"))

        try await token.session.refresh()
        try await failed.session.refresh()

        #expect(token.session.status == .token)
        #expect(failed.session.status == .failed)
        #expect(failed.session.detail == "boom")
    }

    @Test func aStopRecordThatIsNotOneStillCountsAsAStop() throws {
        let record = try #require(SettingsSession.parseStopRecord("not json"))

        #expect(record.by == .cli)
        #expect(record.stoppedAtMs == 0)
        #expect(SettingsSession.parseStopRecord(nil) == nil)
    }

    @Test func showingTheWindowFetchesNowAndEvery5SHidingItSaves() async throws {
        let (session, client, timers) = try await loadedParts()

        session.setVisible(true)
        #expect(timers.delays == [SettingsSession.refreshInterval])
        try model(session, "hook.fail_open", SwitchModel.self).set(false)
        session.setVisible(false)
        await fire(timers)

        #expect(client.patches.count == 1)
        #expect(timers.delays.isEmpty)
    }

    @Test func durationsShowTheirLargestWholeUnitAndSaveSeconds() async throws {
        let (session, client, timers) = try await loadedParts()
        let idle = try model(session, "leases.idle_release_secs", DurationModel.self)

        #expect(idle.unit.name == "minutes")
        #expect(idle.shown == 3)
        idle.setUnit("seconds")
        #expect(idle.shown == 180)
        idle.setUnit("minutes")
        idle.commit(5)
        #expect(idle.number == 300)
        idle.setOn(false)
        #expect(!idle.isOn)
        idle.setOn(true)
        await fire(timers)

        let patch = try #require(client.patches.first)
        #expect(
            try jsonObject(patch.changes)
                == jsonObject(
                    #"[{"kind": "set_number", "key": "leases.idle_release_secs", "value": 300}]"#))
    }

    @Test func turningADurationOffWritesItsOffValue() async throws {
        let (session, client, timers) = try await loadedParts()

        try model(session, "leases.idle_release_secs", DurationModel.self).setOn(false)
        await fire(timers)

        let patch = try #require(client.patches.first)
        #expect(
            try jsonObject(patch.changes)
                == jsonObject(
                    #"[{"kind": "set_number", "key": "leases.idle_release_secs", "value": 0}]"#))
    }

    @Test func chipsSaveOnAddAndRemoveNeverBlankOrTwice() async throws {
        let (session, client, timers) = try await loadedParts()
        let flags = try model(session, "classifier.light_flags", StringListModel.self)

        #expect(flags.add("  ") == false)
        #expect(flags.issue == .blank)
        #expect(flags.add("--help") == false)
        #expect(flags.issue == .duplicate)
        #expect(flags.add(" --dry-run ") == true)
        await fire(timers)

        let patch = try #require(client.patches.first)
        #expect(
            try jsonObject(patch.changes)
                == jsonObject(
                    """
                    [{"kind": "set_list", "key": "classifier.light_flags",
                      "value": ["--help", "-h", "--version", "-V", "/?", "--dry-run"]}]
                    """))
    }

    @Test func aNewPresetNeedsAFreeNameAndItsSizesThenSavesAtOnce() async throws {
        let (session, client, timers) = try await loadedParts()
        let presets = try model(session, "presets", SizeTableModel.self)
        let draft = presets.add()

        #expect(draft.canRename)
        draft.setName("Render")
        #expect(draft.nameIssue == .pattern)
        draft.setName("light")
        #expect(draft.nameIssue == .nameTaken)
        draft.setName("render")
        #expect(presets.save(draft) == false)
        #expect(draft.field("cores")?.issue == .required)
        #expect(draft.field("min_cores")?.issue == nil)

        draft.field("cores")?.commit(6)
        let ram = try #require(draft.field("ram_mb") as? MemoryModel)
        ram.setUnit("GB")
        ram.commit(1.5)
        #expect(ram.number == 1536)
        #expect(presets.save(draft))
        await fire(timers)

        // An automatic minimum is left out of the JSON, which allotd reads as automatic.
        let patch = try #require(client.patches.first)
        #expect(
            try jsonObject(patch.changes)
                == jsonObject(
                    """
                    [{"kind": "set_row", "table": "presets", "name": "render",
                      "profile": {"cores": 6, "ram_mb": 1536, "gpu_pct": 0, "vram_mb": 0,
                                  "est_secs": 300}}]
                    """))
    }

    @Test func aMinimumAboveTheCoresIsRefusedInTheDialog() async throws {
        let (session, _, _) = try await loadedParts()
        let classes = try model(session, "classes", SizeTableModel.self)
        let draft = try #require(classes.edit("mixed"))

        #expect(!draft.canRename)
        draft.field("min_cores")?.commit(8)

        #expect(
            draft.field("min_cores")?.issue
                == .atMost(label: "Cores", value: 6, quantity: .cores))
        #expect(classes.save(draft) == false)
        draft.field("cores")?.commit(8)
        #expect(draft.field("min_cores")?.issue == nil)
    }

    @Test func renamingAPresetRemovesTheOldRowInTheSameWrite() async throws {
        let (session, client, timers) = try await loadedParts(state: try stateWithRender())
        let presets = try model(session, "presets", SizeTableModel.self)
        let draft = try #require(presets.edit("render"))

        #expect((draft.field("ram_mb") as? MemoryModel)?.unit.name == "GB")
        draft.setName("render_big")
        #expect(presets.save(draft))
        #expect(
            presets.sizeRows.map { $0.name }.filter { $0.hasPrefix("render") } == ["render_big"])
        await fire(timers)

        let patch = try #require(client.patches.first)
        let sent = try #require(try jsonObject(patch.changes) as? [[String: Any]])
        let pairs = sent.map { "\($0["kind"] as? String ?? ""):\($0["name"] as? String ?? "")" }
        #expect(pairs == ["reset_row:render", "set_row:render_big"])
    }

    @Test func deletingAUserPresetAndResettingABuiltInOneAreRowResets() async throws {
        let (session, client, timers) = try await loadedParts(state: try stateWithRender())
        let presets = try model(session, "presets", SizeTableModel.self)

        #expect(presets.rows.first { $0.row.name == "render" }?.builtIn == false)
        presets.remove("render")
        await fire(timers)

        let patch = try #require(client.patches.first)
        #expect(
            try jsonObject(patch.changes)
                == jsonObject(#"[{"kind": "reset_row", "table": "presets", "name": "render"}]"#))
        #expect(
            presets.describeProblem("presets.render.min_cores", "must be above 0")
                == "render: Minimum cores must be above 0")
    }

    @Test func rulesAreWrittenWholeOnEveryEditMoveAndDelete() async throws {
        let (session, client, timers) = try await loadedParts()
        let rules = try model(session, "classifier.rules", RuleListModel.self)
        func programs() -> [String] {
            rules.ruleList.map { $0.program }
        }

        rules.move(from: 0, to: 2)
        #expect(programs() == ["pnpm", "msbuild", "cargo", "cmake", "python*", "py"])
        rules.remove(at: 5)
        let draft = try #require(rules.edit(1))
        draft.setProgram(" ")
        #expect(rules.save(draft) == false)
        #expect(draft.programIssue == .blank)
        draft.setProgram("ninja")
        #expect(draft.args.add("-j"))
        draft.setClass("mixed")
        #expect(rules.save(draft))
        await fire(timers)

        let last = try #require(client.patches.last)
        #expect(
            try jsonObject(last.changes)
                == jsonObject(
                    """
                    [{"kind": "set_rules", "rules": [
                      {"program": "pnpm", "args": ["build*", "test*"], "class": "cpu_heavy"},
                      {"program": "ninja", "args": ["-j"], "class": "mixed"},
                      {"program": "cargo", "args": ["build", "test", "run", "bench"],
                       "class": "cpu_heavy"},
                      {"program": "cmake", "args": ["--build"], "class": "cpu_heavy"},
                      {"program": "python*", "args": ["train*.py"], "class": "gpu_heavy"}]}]
                    """))
        #expect(
            rules.describeProblem("classifier.rules[2].program", "must not be blank")
                == "Rule 3: Program must not be blank")
    }

    @Test func resetPageRemovesEveryChangedKeyOnItInOneWrite() async throws {
        let (session, client, timers) = try await loadedParts()

        session.resetChanged("service")
        await fire(timers)

        let patch = try #require(client.patches.first)
        #expect(
            try jsonObject(patch.changes)
                == jsonObject(#"[{"kind": "reset", "key": "daemon.port"}]"#))
        #expect(session.setting("daemon.port")?.value == .number(47999))
    }

    @Test func aPreviewSendsTheUnsavedChanges() async throws {
        let (session, client, _) = try await loadedParts()
        try model(session, "admission.headroom_pct", NumberModel.self).commit(30)

        _ = try await session.preview()

        let sent = try #require(client.previews.first)
        #expect(client.previews.count == 1)
        #expect(
            try jsonObject(sent)
                == jsonObject(
                    #"[{"kind": "set_number", "key": "admission.headroom_pct", "value": 30}]"#))
    }

    @Test func valuesReadWithTheirUnit() {
        #expect(quantityText(15, .percent, number: plain) == "15 %")
        #expect(quantityText(180, .seconds, number: plain) == "3 minutes")
        #expect(quantityText(1, .seconds, number: plain) == "1 second")
        #expect(quantityText(2048, .megabytes, number: plain) == "2 GB")
        #expect(quantityText(1000, .megabytes, number: plain) == "1000 MB")
        #expect(quantityText(1, .cores, number: plain) == "1 core")
        #expect(quantityText(47823, .port, number: { _ in "47,823" }) == "47823")
        #expect(
            issueText(
                .range(
                    min: Limit(value: 1, exclusive: false),
                    max: Limit(value: 65535, exclusive: false)),
                number: plain) == "Enter a number from 1 to 65535.")
        #expect(
            issueText(.range(min: Limit(value: 0, exclusive: true), max: nil), number: plain)
                == "Enter a number above 0.")
    }

    private func loadedParts(
        state: SettingsState? = nil
    ) async throws -> (SettingsSession, FakeSettings, FakeTimers) {
        let harness = try await loaded(state: state)
        return (harness.session, harness.client, harness.timers)
    }
}
