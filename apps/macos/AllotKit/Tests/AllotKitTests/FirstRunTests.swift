import Testing

@testable import AllotKit

@MainActor
@Suite struct FirstRunTests {
    @Test func cardShowsUntilDoneWhileNoSessionEverConnected() throws {
        let state = try Fixtures.state("first-run")
        let notDone = FirstRunState(isDone: false, login: .on)

        let shown = PanelSnapshot.from(status: .running, state: state, firstRun: notDone)
        let done = PanelSnapshot.from(
            status: .running, state: state, firstRun: FirstRunState(isDone: true, login: .on))

        #expect(shown.welcome == WelcomeCard(login: .on))
        #expect(!shown.showNothing)
        #expect(done.welcome == nil)
        #expect(done.showNothing)
    }

    @Test func noCardOnceASessionConnectedOrForLastKnownState() throws {
        let firstRun = FirstRunState(isDone: false, login: .off)

        let connected = PanelSnapshot.from(
            status: .running, state: try Fixtures.state("macos"), firstRun: firstRun)
        let down = PanelSnapshot.from(
            status: .stopped, state: try Fixtures.state("first-run"), firstRun: firstRun)

        #expect(connected.welcome == nil)
        #expect(down.welcome == nil)
    }

    @Test func firstShowingHappensOnce() throws {
        let snapshot = PanelSnapshot.from(
            status: .running, state: try Fixtures.state("first-run"),
            firstRun: FirstRunState(isDone: false, login: .off))
        var settings = TraySettings()

        #expect(snapshot.showsFirstRunForTheFirstTime(settings))
        settings.firstRunShown = true
        #expect(!snapshot.showsFirstRunForTheFirstTime(settings))
    }

    @Test func modelShowsTheLoginItemAndDone() throws {
        let model = PanelModel()
        model.loginItem = MemoryLoginItem(.off)
        var saved: [TraySettings] = []
        model.onSettingsChanged = { saved.append($0) }
        let state = try Fixtures.state("first-run")
        model.apply(PollResult(status: .running, state: state), lastKnown: state)

        try model.setLaunchAtLogin(true)
        let afterLogin = model.snapshot.welcome
        model.finishFirstRun()

        #expect(afterLogin == WelcomeCard(login: .on))
        #expect(model.login == .on)
        #expect(model.snapshot.welcome == nil)
        #expect(saved.map(\.firstRunDone) == [true])
    }

    @Test func translocatedLoginItemCannotBeTurnedOn() throws {
        let model = PanelModel()
        model.loginItem = MemoryLoginItem(.translocated)

        try model.setLaunchAtLogin(true)

        #expect(model.login == .translocated)
    }
}

@MainActor
@Suite struct PanelModelSettingsTests {
    @Test func resourcesToggleAndMeterChangesRebuildTheCardAndSave() throws {
        let model = PanelModel()
        var saved: [TraySettings] = []
        model.onSettingsChanged = { saved.append($0) }
        let state = try Fixtures.state("macos")
        model.apply(PollResult(status: .running, state: state), lastKnown: state)
        let collapsed = model.resources

        model.toggleResources()
        model.updateSettings { $0.meters.cpu = false }

        #expect(collapsed.isVisible)
        #expect(!collapsed.isExpanded)
        #expect(model.resources.isExpanded)
        #expect(model.resources.meters.map(\.id) == ["ram", "gpu:0"])
        #expect(saved.count == 2)
        #expect(saved.last?.resourcesExpanded == true)
    }

    @Test func unchangedSettingsAreNotSaved() {
        let model = PanelModel()
        var saves = 0
        model.onSettingsChanged = { _ in saves += 1 }

        model.updateSettings { $0.meters.cpu = true }

        #expect(saves == 0)
    }

    @Test func downServiceShowsTheBannerAndHidesTheCard() throws {
        let model = PanelModel()
        let state = try Fixtures.state("macos")
        model.apply(PollResult(status: .running, state: state), lastKnown: state)

        model.apply(PollResult(status: .stopped), lastKnown: state, startProblem: "refused")

        #expect(model.banner?.button == .start)
        #expect(model.banner?.problem == "refused")
        #expect(!model.resources.isVisible)
    }

    @Test func killNotesItsLeaseBeforeSending() throws {
        let clock = TestClock()
        let model = PanelModel(now: clock.source)
        var noted: [Int64] = []
        model.onKillSent = { noted.append($0) }
        let state = try Fixtures.state("macos")
        model.apply(PollResult(status: .running, state: state), lastKnown: state)

        model.pressKill(71)
        #expect(noted.isEmpty)
        // Past the double-press guard.
        clock.advance(.milliseconds(500))
        model.pressKill(71)

        #expect(noted == [71])
    }
}

@MainActor
@Suite struct ScenarioPlayerSettingsTests {
    @Test func presetsSetTheCardAndFirstRunResetsItsFlags() throws {
        var settings = TraySettings()
        settings.firstRunDone = true
        settings.firstRunShown = true
        let model = PanelModel(settings: settings)
        let player = ScenarioPlayer(model: model)

        try player.show("res-expanded")
        let expanded = model.settings
        try player.show("first-run")

        #expect(expanded.resourcesExpanded)
        #expect(!expanded.meters.vram)
        #expect(!expanded.meters.driveActivity)
        #expect(!model.settings.firstRunDone)
        #expect(!model.settings.firstRunShown)
        #expect(model.snapshot.welcome != nil)
    }

    @Test func downScenarioStartsThenShowsTheStartedOne() async throws {
        let model = PanelModel()
        let player = ScenarioPlayer(model: model)
        try player.show(Handoff.downScenario)

        player.startService()
        let starting = model.banner?.button
        try await Task.sleep(for: ScenarioPlayer.startDelay + .milliseconds(300))

        #expect(starting == .starting)
        #expect(model.banner == nil)
        #expect(model.snapshot.header.summary == "1 running")
    }
}
