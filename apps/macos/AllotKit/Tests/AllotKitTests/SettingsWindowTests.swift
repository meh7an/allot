import AllotContract
import Foundation
import Testing

@testable import AllotKit

private let english = SettingsText.numbers(Locale(identifier: "en_US"))

private struct Loaded {
    let session: SettingsSession
    let client: FakeSettings
    let timers: FakeTimers
}

/// A session loaded from the settings-state fixture, changed by `edit`.
@MainActor
private func loaded(
    stopRecord: String? = nil,
    _ edit: (inout [String: Any]) -> Void = { _ in }
) async throws -> Loaded {
    let client = FakeSettings(state: try fixtureState(edit))
    let timers = FakeTimers()
    let session = SettingsSession(
        client: client, timers: timers, platform: .macos, readStopRecord: { stopRecord })
    try await session.refresh()
    return Loaded(session: session, client: client, timers: timers)
}

/// Sets the value of `key` in a settings-state object.
private func setValue(_ object: inout [String: Any], _ key: String, _ value: Any) {
    var values = object["values"] as? [[String: Any]] ?? []
    if let index = values.firstIndex(where: { $0["key"] as? String == key }) {
        values[index]["value"] = value
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

private func counts(_ full: Int, _ shrunk: Int, more: Bool = false) throws -> Fits {
    try decoded(Fits.self, ["full": full, "shrunk": shrunk, "more": more])
}

@Suite struct SettingsTextTests {
    @Test func saveStatusReadsAsTheDesignSays() {
        #expect(SettingsText.saveStatus(.saved) == "All changes saved")
        #expect(SettingsText.saveStatus(.saving) == "Saving…")
        #expect(SettingsText.saveStatus(.unsaved(count: 1)) == "1 change not saved")
        #expect(SettingsText.saveStatus(.unsaved(count: 3)) == "3 changes not saved")
    }

    @Test func numbersAreGroupedWithAtMostTwoDecimals() {
        #expect(english(5000) == "5,000")
        #expect(english(3.3) == "3.3")
        #expect(english(2.456) == "2.46")
        #expect(english(15) == "15")
    }

    @Test func joinedListsReadAsASentence() {
        #expect(SettingsText.joined(["a"]) == "a")
        #expect(SettingsText.joined(["a", "b"]) == "a and b")
        #expect(SettingsText.joined(["a", "b", "c"]) == "a, b and c")
    }

    @MainActor
    @Test func resetTextsNameTheDefault() async throws {
        let session = try await loaded().session
        let port = try setting(session, "daemon.port", NumberModel.self)
        let days = try setting(session, "retention.lease_days", NumberModel.self)
        let idle = try setting(session, "leases.idle_release_secs", DurationModel.self)

        #expect(SettingsText.fieldReset(port, number: english) == "Port reset to 47823.")
        #expect(SettingsText.resetButtonName(port, number: english) == "Reset Port to 47823")
        #expect(SettingsText.fieldReset(idle, number: english) == "Idle for reset to 3 minutes.")

        let page = SettingsText.resetPage("Leases", changed: [days, port], number: english)
        #expect(page.title == "Reset 2 settings on Leases?")
        #expect(
            page.body
                == "These go back to their defaults:\nKeep ended leases for: 30 days\nPort: 47823")
        #expect(page.button == "Reset 2 Settings")

        #expect(SettingsText.resetAll(1).title == "Reset the 1 changed setting?")
        #expect(SettingsText.resetAll(1).button == "Reset 1 Setting")
        #expect(SettingsText.resetAll(12).title == "Reset all 12 changed settings?")
    }
}

@MainActor
@Suite struct SettingsBannersTests {
    @Test func thePanelPageShowsNoServiceBanner() async throws {
        let session = try await loaded().session

        #expect(SettingsBanners.banners(
            session, pageId: SettingsBanners.panelPage, port: 47823, number: english).isEmpty)
    }

    @Test func stoppedByYouOffersStartService() async throws {
        let parts = try await loaded(stopRecord: #"{"by": "panel", "stopped_at_ms": 1}"#)
        parts.client.queueRead(failure(.unreachable))
        try await parts.session.refresh()

        let banners = SettingsBanners.banners(
            parts.session, pageId: "admission", port: 47823, number: english)

        #expect(banners.count == 1)
        #expect(banners[0].message == "The service is stopped. Start it to change these settings.")
        #expect(banners[0].actions == [.startService])
        #expect(parts.session.readOnly)
    }

    @Test func notAnsweringNamesThePortThisPanelDials() async throws {
        let parts = try await loaded()
        parts.client.queueRead(failure(.unreachable))
        try await parts.session.refresh()

        let banner = try #require(SettingsBanners.banners(
            parts.session, pageId: "leases", port: 47823, number: english).first)

        #expect(banner.severity == .warning)
        #expect(
            banner.message
                == "The service isn't answering on port 47823. Trying again every 5 s.")
        #expect(banner.actions == [.tryNow, .openLogs])
    }

    @Test func aRefusedKeyOffersRestartService() async throws {
        let parts = try await loaded()
        parts.client.queueRead(failure(.token))
        try await parts.session.refresh()

        let banner = try #require(SettingsBanners.banners(
            parts.session, pageId: "service", port: 47823, number: english).first)

        #expect(banner.severity == .error)
        #expect(
            banner.message
                == "The service refused this panel's key. Restart the service to make a new one.")
        #expect(banner.actions == [.restartService])
    }

    @Test func anOutdatedServiceOffersRestartAndLogsButNoTryNow() async throws {
        let parts = try await loaded()
        parts.client.queueRead(failure(.outdated, message: "the service answered 404"))
        try await parts.session.refresh()

        let banner = try #require(SettingsBanners.banners(
            parts.session, pageId: "admission", port: 47823, number: english).first)

        #expect(parts.session.status == .outdated)
        #expect(banner.severity == .error)
        #expect(
            banner.message
                == "The service running now is older than this app and has no settings. Restart "
                + "it to run the version installed with this app, or update it with allot install.")
        #expect(banner.actions == [.restartService, .openLogs])
    }

    @Test func anOutdatedServiceOffersTheUpdateTheAppCarries() async throws {
        let parts = try await loaded()
        parts.client.queueRead(failure(.outdated, message: "the service answered 404"))
        try await parts.session.refresh()
        let update = try ServiceUpdate(
            installed: .version(#require(ServiceVersion("0.5.0"))),
            bundled: #require(ServiceVersion("0.6.0")))

        let banners = SettingsBanners.banners(
            parts.session, pageId: "service", port: 47823, number: english, update: update)

        #expect(banners.count == 1)
        #expect(
            banners.first?.message
                == "The service running now is older than this app and has no settings. Update "
                + "it to the version that comes with this app.")
        #expect(banners.first?.actions == [.updateService, .openLogs])
    }

    @Test func theServicePageAloneOffersTheUpdate() async throws {
        let session = try await loaded().session
        let update = try ServiceUpdate(
            installed: .version(#require(ServiceVersion("0.5.0"))),
            bundled: #require(ServiceVersion("0.6.0")))

        let service = SettingsBanners.banners(
            session, pageId: "service", port: 47823, number: english, update: update)
        let leases = SettingsBanners.banners(
            session, pageId: "leases", port: 47823, number: english, update: update)

        #expect(service.count == 1)
        #expect(service.first?.severity == .informational)
        #expect(
            service.first?.message
                == "The installed service is version 0.5.0. This app comes with 0.6.0.")
        #expect(service.first?.actions == [.updateService])
        #expect(leases.isEmpty)
    }

    @Test func aMissingServiceIsInstalledAndAFailureSaysWhy() async throws {
        let session = try await loaded().session
        var update = try ServiceUpdate(
            installed: .missing, bundled: #require(ServiceVersion("0.6.0")))
        update.problem = "allot install exited with 1; see app-update.log in the logs"

        let banner = try #require(SettingsBanners.banners(
            session, pageId: "service", port: 47823, number: english, update: update).first)

        #expect(banner.severity == .error)
        #expect(
            banner.message
                == "The service isn't installed. This app comes with 0.6.0. The last update "
                + "failed: allot install exited with 1; see app-update.log in the logs.")
        #expect(banner.actions == [.installService])
    }

    @Test func anUpdateInProgressHasNoButton() async throws {
        let session = try await loaded().session
        var update = try ServiceUpdate(
            installed: .unreadable, bundled: #require(ServiceVersion("0.6.0")))
        update.installing = true

        let banner = try #require(SettingsBanners.banners(
            session, pageId: "service", port: 47823, number: english, update: update).first)

        #expect(banner.actions.isEmpty)
    }

    @Test func aDetailThatStartsInLowercaseStartsItsSentenceInUppercase() async throws {
        let parts = try await loaded()
        parts.client.queueRead(failure(.failed, message: "the service answered 404"))
        try await parts.session.refresh()

        let banner = try #require(SettingsBanners.banners(
            parts.session, pageId: "admission", port: 47823, number: english).first)

        #expect(
            banner.message
                == "The service sent an answer Settings can't use. The service answered 404.")
        #expect(SettingsBanners.sentence(" closed it") == "Closed it.")
        #expect(SettingsBanners.sentence("Already. Fine.") == "Already. Fine.")
        #expect(SettingsBanners.sentence("config.toml is open") == "config.toml is open.")
        #expect(
            SettingsBanners.sentence("allotd was not found", capitalize: false)
                == "allotd was not found.")
        #expect(SettingsBanners.sentence(nil) == "")
    }

    @Test func aNewPortWaitsForARestartOnTheServicePageOnly() async throws {
        let session = try await loaded().session

        let service = SettingsBanners.banners(
            session, pageId: SettingsBanners.servicePage, port: 47823, number: english)
        let admission = SettingsBanners.banners(
            session, pageId: "admission", port: 47823, number: english)

        #expect(service.map(\.message) == ["Restart the service to use port 47999."])
        #expect(service.first?.actions == [.restartService])
        #expect(admission.isEmpty)
        #expect(
            SettingsBanners.restartNote(session, key: "daemon.port")
                == "Restart needed. The service still listens on 47823.")
        #expect(SettingsBanners.restartNote(session, key: "hook.fail_open") == nil)
    }

    @Test func aRejectedFileIsAnErrorBannerAndKeepsThePagesReadOnly() async throws {
        let session = try await loaded { $0["file_error"] = "expected a number at line 3" }
            .session

        let banner = try #require(SettingsBanners.banners(
            session, pageId: "admission", port: 47823, number: english).first)

        #expect(session.readOnly)
        #expect(banner.severity == .error)
        #expect(
            banner.message
                == "config.toml has an error. Expected a number at line 3. The service keeps "
                + "using its last good settings. Fix the file to change settings here.")
        #expect(banner.actions == [.openFile, .checkAgain])
    }

    @Test func aHandEditConflictNamesTheChangeThatWasNotSaved() async throws {
        let parts = try await loaded()
        parts.client.queueWrite(failure(.conflict))
        try setting(parts.session, "estimates.margin_pct", NumberModel.self).commit(15)

        await fire(parts.timers)

        let banner = try #require(SettingsBanners.banners(
            parts.session, pageId: "admission", port: 47823, number: english).first)
        #expect(banner.severity == .warning)
        #expect(
            banner.message
                == "config.toml changed outside Settings. Showing the file's values. Your "
                + "change to Safety margin (15 %) was not saved.")
        #expect(banner.actions == [.applyMine, .keepFileValues])
    }

    @Test func aHeldFileKeepsTheChangesAndOffersTryAgain() async throws {
        let held = "config.toml is open in another program that blocks changes."
        let parts = try await loaded()
        parts.client.queueWrite(failure(.held, message: held))
        try setting(parts.session, "admission.headroom_pct", NumberModel.self).commit(20)

        await fire(parts.timers)

        let banner = try #require(SettingsBanners.banners(
            parts.session, pageId: "admission", port: 47823, number: english).first)
        #expect(banner.message == "Your changes are not saved yet. \(held)")
        #expect(banner.actions == [.saveAgain])
        #expect(parts.session.saveStatus == .unsaved(count: 1))
    }

    @Test func bannerButtonsAreTitleCase() {
        #expect(BannerAction.startService.label == "Start Service")
        #expect(BannerAction.keepFileValues.label == "Keep File's Values")
        #expect(BannerAction.saveAgain.label == "Try Again")
    }
}

@Suite struct LiveTextTests {
    private func kept(gpu: Bool) throws -> KeptFree {
        var object: [String: Any] = [
            "cores": 2.4, "total_cores": 16, "ram_mb": 4915.2, "total_ram_mb": 32768,
        ]
        if gpu {
            object["gpu_pct"] = 15
            object["vram_mb"] = 1228.8
            object["total_vram_mb"] = 8192
        }
        return try decoded(KeptFree.self, object)
    }

    @Test func headroomSentenceNamesWhatIsKeptFree() throws {
        #expect(
            LiveText.headroom(try kept(gpu: true), number: english)
                == "Keeps 2.4 of 16 cores, 4.8 of 32 GB memory, 15 % of the GPU and 1.2 of 8 GB "
                + "video memory free.")
        #expect(
            LiveText.headroom(try kept(gpu: false), number: english)
                == "Keeps 2.4 of 16 cores and 4.8 of 32 GB memory free.")
    }

    @Test func estimatesSentenceFollowsMarginAndRuns() {
        #expect(
            LiveText.estimates(lower: true, minRuns: 3, marginPct: 10, number: english)
                == "A command that asks for 8 cores but used about 3 in its last 3 runs is "
                + "reserved 3.3 cores. Requests never go below 0.25 cores, 256 MB, or their "
                + "minimum cores, and run time never lowers.")
        #expect(
            LiveText.estimates(lower: true, minRuns: 1, marginPct: 0, number: english)
                .contains("in its last run is reserved 3 cores."))
        #expect(
            LiveText.estimates(lower: false, minRuns: 3, marginPct: 10, number: english)
                == LiveText.loweringOff)
    }

    @Test func fitsReadAtFullSizeAndShrunk() throws {
        #expect(LiveText.fits(try counts(0, 0), minCores: 4, number: english) == "None fit")
        #expect(LiveText.fits(try counts(0, 1), minCores: 4, number: english) == "1 at 4 cores")
        #expect(
            LiveText.fits(try counts(1, 2), minCores: 4, number: english)
                == "1 at full size, 2 more at 4 cores")
        #expect(
            LiveText.fits(try counts(1, 0), minCores: 1, number: english) == "1 at full size")
        #expect(
            LiveText.fits(try counts(99, 0, more: true), minCores: 1, number: english)
                == "at least 99 at full size")
        #expect(
            LiveText.fits(try counts(2, 99, more: true), minCores: 0.5, number: english)
                == "2 at full size, at least 99 more at 0.5 cores")
    }

    @Test func theFixturePreviewFillsEveryClass() throws {
        let preview = try JSONDecoder().decode(
            FitPreview.self, from: Fixtures.data("fit-preview.json"))
        let cpu = try #require(preview.classes.first { $0.name == "cpu_heavy" })

        #expect(
            LiveText.fits(cpu.now, minCores: Double(cpu.minCores), number: english)
                == "1 at 4 cores")
        #expect(
            LiveText.fits(cpu.idle, minCores: Double(cpu.minCores), number: english)
                == "1 at full size, 1 more at 4 cores")
        let caption = LiveText.fitsCaption(37.6, number: english)
        #expect(caption.hasPrefix("Based on this machine now (38 %"))
    }

    @Test func benchmarksSentenceJoinsEqualLimits() {
        #expect(
            LiveText.benchmarks(
                cpuPct: 10, gpuPct: 10, quietSecs: 10, maxWaitSecs: 60, number: english)
                == "A benchmark starts once CPU and GPU stay at or below 10 % for 10 seconds. If "
                + "that takes longer than 1 minute, it starts anyway, marked not quiet.")
        #expect(
            LiveText.benchmarks(
                cpuPct: 10, gpuPct: 20, quietSecs: 90, maxWaitSecs: 600, number: english)
                == "A benchmark starts once CPU stays at or below 10 % and GPU at or below 20 % "
                + "for 90 seconds. If that takes longer than 10 minutes, it starts anyway, "
                + "marked not quiet.")
    }

    @Test func historySentenceGroupsLargeCounts() {
        #expect(
            LiveText.history(
                leaseDays: 30, historyDays: 90, keepRuns: 20, maxRuns: 5000, number: english)
                == "Keeps ended leases for 30 days and job runs for 90 days. Each command keeps "
                + "at least its newest 20 runs, and never more than 5,000 runs are stored.")
    }
}

@Suite struct ServiceRowTests {
    @Test func serviceRowFollowsThePollAndTheStopRecord() {
        let running = ServiceRow.from(.running, session: .ready, version: "0.5.0", port: 47823)
        #expect(running.text == "Running · version 0.5.0 · port 47823")
        #expect(running.buttonText == "Stop Service…")
        #expect(!running.opensLogs)

        let stopped = ServiceRow.from(.stopped, session: .stopped, version: nil, port: 47823)
        #expect(stopped.text == ServiceRow.stoppedByYou)
        #expect(stopped.button == .start)
        #expect(!stopped.opensLogs)

        let down = ServiceRow.from(.stopped, session: .down, version: nil, port: 47823)
        #expect(down.text == "Not answering on port 47823.")
        #expect(down.buttonText == "Start Service")
        #expect(down.opensLogs)

        let starting = ServiceRow.from(.starting, session: .down, version: nil, port: 47823)
        #expect(starting.button == .starting)
        #expect(starting.buttonText == "Starting…")
    }

    @Test func serviceRowShowsWhyTheLastStartFailed() {
        let failed = ServiceRow.from(
            .stopped, session: .down, version: nil, port: 47823,
            startProblem: "allotd was not found")

        #expect(failed.text == "The service did not start. allotd was not found.")
        #expect(failed.button == .start)
        #expect(failed.opensLogs)
    }

    @Test func theStopWarningSaysTheStopLasts() throws {
        let state = try Fixtures.state("busy")
        let text = try #require(ServiceMenu.stopConfirmation(state))

        #expect(ServiceMenu.confirmTitle == "Stop the service?")
        #expect(
            text.hasSuffix(
                "Jobs keep running, and new heavy commands run without leases until you start "
                    + "the service again."))
    }
}

@MainActor
@Suite struct FieldResetTests {
    @Test func resetThenUndoWritesTheOldNumberBack() async throws {
        let session = try await loaded { setValue(&$0, "leases.heartbeat_ttl_secs", 120) }
            .session
        let model = try setting(session, "leases.heartbeat_ttl_secs", DurationModel.self)

        let undo = try #require(FieldReset.reset(model))

        #expect(model.number == 60)
        undo()
        #expect(model.number == 120)
        #expect(model.issue == nil)
    }

    @Test func resetThenUndoTurnsAnOffDurationOffAgain() async throws {
        let session = try await loaded { setValue(&$0, "leases.idle_release_secs", 0) }.session
        let model = try setting(session, "leases.idle_release_secs", DurationModel.self)

        let undo = try #require(FieldReset.reset(model))

        #expect(model.isOn)
        undo()
        #expect(!model.isOn)
        #expect(model.number == 0)
    }

    @Test func resetThenUndoTurnsASwitchBack() async throws {
        let session = try await loaded { setValue(&$0, "hook.fail_open", false) }.session
        let model = try setting(session, "hook.fail_open", SwitchModel.self)

        let undo = try #require(FieldReset.reset(model))

        #expect(model.isOn)
        undo()
        #expect(!model.isOn)
    }
}

@MainActor
@Suite struct FitPreviewsTests {
    @Test func activePreviewsAskWithTheUnsavedEdits() async throws {
        let parts = try await loaded()
        let fits = FitPreviews(session: parts.session, timers: parts.timers)
        fits.setActive(true)
        await settle()

        #expect(parts.client.previews.count == 1)
        #expect(fits.unavailable == nil)
    }

    @Test func inactivePreviewsAskNothing() async throws {
        let parts = try await loaded()
        let fits = FitPreviews(session: parts.session, timers: parts.timers)

        fits.sessionChanged()
        await settle()

        #expect(parts.client.previews.isEmpty)
        #expect(parts.timers.delays.isEmpty)
    }

    @Test func aChangeAsksAgainAfterTheSaveDelay() async throws {
        let parts = try await loaded()
        let fits = FitPreviews(session: parts.session, timers: parts.timers)
        fits.setActive(true)
        await settle()

        fits.sessionChanged()

        #expect(parts.timers.delays == [SettingsSession.saveDelay])
    }

    @Test func aStoppedServiceExplainsWhyNothingShows() async throws {
        let parts = try await loaded(stopRecord: #"{"by": "cli", "stopped_at_ms": 1}"#)
        parts.client.queueRead(failure(.unreachable))
        try await parts.session.refresh()
        let fits = FitPreviews(session: parts.session, timers: parts.timers)

        #expect(fits.unavailable == LiveText.serviceStopped)
        #expect(fits.fit("classes", "cpu_heavy") == nil)
    }
}

@Suite struct PanelTogglesTests {
    @Test func panelTogglesWriteTheirOwnSetting() {
        var settings = TraySettings()
        for toggle in PanelToggles.meters(sharesMemory: false) + PanelToggles.notifications {
            let before = settings
            settings[keyPath: toggle.key] = !toggle.defaultValue

            #expect(settings != before, "\(toggle.label)")
            settings[keyPath: toggle.key] = toggle.defaultValue
        }

        #expect(settings == TraySettings())
    }

    @Test func driveActivityNeedsDrives() throws {
        let activity = try #require(
            PanelToggles.meters(sharesMemory: false).first { $0.label == "Drive activity" })
        var settings = TraySettings()

        #expect(activity.isEnabled(in: settings))
        settings.meters.drives = false
        #expect(!activity.isEnabled(in: settings))
    }

    @Test func vramReadsGpuMemoryWhereTheGpuSharesMemory() {
        #expect(PanelToggles.meters(sharesMemory: true).map(\.label).contains("GPU memory"))
        #expect(PanelToggles.meters(sharesMemory: false).map(\.label).contains("VRAM"))
    }

    @Test func panelResetPageResetsOnlyItsOwnSettings() {
        var settings = TraySettings()
        settings.meters.cpu = false
        settings.notifications.jobFinished = true
        settings.textSize = .larger
        settings.resourcesExpanded = true
        settings.firstRunDone = true

        #expect(
            PanelToggles.changedLines(settings, sharesMemory: false)
                == ["CPU: On", "Job finished: Off", "Text size: Default"])
        PanelToggles.reset(&settings)

        #expect(PanelToggles.changedLines(settings, sharesMemory: false).isEmpty)
        #expect(settings.meters == MeterToggles())
        #expect(settings.notifications == NotificationToggles())
        #expect(settings.textSize == .standard)
        #expect(settings.resourcesExpanded)
        #expect(settings.firstRunDone)
    }
}
