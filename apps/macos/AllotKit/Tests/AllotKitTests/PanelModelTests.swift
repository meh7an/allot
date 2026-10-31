import Testing

@testable import AllotKit

@MainActor
@Suite struct PanelModelTests {
    @Test func newModelShowsStartingUntilTheFirstPoll() {
        let model = PanelModel()

        #expect(model.snapshot.header.summary == "Starting service…")
        #expect(model.menuBar.icon == .down)
        #expect(model.mode == .lightDismiss)
    }

    @Test func applyShowsThePoll() throws {
        let model = PanelModel()
        let state = try Fixtures.state("macos")

        model.apply(PollResult(status: .running, state: state), lastKnown: state)

        #expect(model.snapshot.header.summary == "1 running · 1 waiting")
        #expect(model.menuBar.icon == .queued)
    }

    @Test func stoppedPollKeepsTheLastKnownJobsDimmed() throws {
        let model = PanelModel()
        let state = try Fixtures.state("macos")
        model.apply(PollResult(status: .running, state: state), lastKnown: state)

        model.apply(PollResult(status: .stopped, detail: "refused"), lastKnown: state)

        #expect(model.snapshot.isLastKnown)
        #expect(model.snapshot.groups.count == 2)
        #expect(model.menuBar == MenuBarStatus(icon: .down, summary: "Service stopped"))
    }

    @Test func stayOpenThenPinThenStayOpenOffUnpins() {
        let model = PanelModel()

        model.toggleStayOpen()
        #expect(model.mode == .stayOpen)
        model.togglePinned()
        #expect(model.mode == .pinned)
        #expect(model.mode.isStayOpen && model.mode.isPinned)
        model.toggleStayOpen()
        #expect(model.mode == .lightDismiss)
    }

    @Test func pinDoesNothingInLightDismissMode() {
        let model = PanelModel()

        model.togglePinned()

        #expect(model.mode == .lightDismiss)
    }

    @Test func unpinKeepsStayOpen() {
        let model = PanelModel()
        model.toggleStayOpen()
        model.togglePinned()

        model.togglePinned()

        #expect(model.mode == .stayOpen)
    }

    @Test func reopeningResetsToLightDismiss() {
        let model = PanelModel()
        model.toggleStayOpen()
        model.togglePinned()

        model.resetForOpen()

        #expect(model.mode == .lightDismiss)
    }

    @Test func killTwiceSendsThenHoldsStopped() async throws {
        let clock = TestClock()
        let model = PanelModel(now: clock.source)
        let stopper = FakeStopper(StopResult(ended: true, text: "", endedAtMs: Handoff.nowMs))
        model.stopper = stopper
        var answered: [Int64] = []
        model.onStopAnswered = { lease, _, _ in answered.append(lease) }
        let state = try Handoff.state(run: ["cargo"])
        model.apply(PollResult(status: .running, state: state), lastKnown: state)
        let cargo = Handoff.leaseId("cargo")

        #expect(model.pressKill(cargo) == nil)
        #expect(model.isConfirming)
        #expect(model.snapshot.groups[0].jobs[0].card.action == .confirm)
        clock.advance(.seconds(1))
        await model.pressKill(cargo)?.value

        #expect(await stopper.killed == [cargo])
        #expect(answered == [cargo])
        let card = model.snapshot.groups[0].jobs[0].card
        #expect(card.strong == "Stopped")
        #expect(model.snapshot.header.summary == "Idle")
    }

    @Test func stoppedCardStaysAfterAllotdDropsTheLeaseThenGoes() async throws {
        let clock = TestClock()
        let model = PanelModel(now: clock.source)
        model.stopper = FakeStopper(StopResult(ended: true, text: "", endedAtMs: Handoff.nowMs))
        let state = try Handoff.state(run: ["cargo"])
        model.apply(PollResult(status: .running, state: state), lastKnown: state)
        model.pressKill(Handoff.leaseId("cargo"))
        clock.advance(.seconds(1))
        await model.pressKill(Handoff.leaseId("cargo"))?.value

        let after = try Handoff.state()
        model.apply(PollResult(status: .running, state: after), lastKnown: after)
        #expect(model.snapshot.groups.first?.jobs.first?.card.strong == "Stopped")
        #expect(model.nextStopChange == .seconds(4))

        clock.advance(.seconds(4))
        model.refresh()
        #expect(model.snapshot.groups.isEmpty)
        #expect(model.snapshot.showNothing)
    }

    @Test func refusedRemoveSaysWhy() async throws {
        let model = PanelModel(now: TestClock().source)
        model.stopper = FakeStopper(.refused("the job no longer exists"))
        let state = try Handoff.state(wait: ["npm"])
        model.apply(PollResult(status: .running, state: state), lastKnown: state)

        await model.pressRemove(Handoff.leaseId("npm"))?.value

        let card = model.snapshot.groups[0].jobs[0].card
        #expect(card.detail == "Not removed: the job no longer exists")
        #expect(card.detailIsDanger)
        #expect(card.action == .remove)
    }

    @Test func frozenStopsShowUntilTheFirstPress() throws {
        let model = PanelModel(now: TestClock().source)
        let state = try Handoff.state(run: ["cargo"])
        model.frozenStops = Handoff.stops("c-killed")
        model.apply(PollResult(status: .running, state: state), lastKnown: state)
        #expect(model.snapshot.groups[0].jobs[0].card.strong == "Stopped")

        model.pressKill(Handoff.leaseId("cargo"))

        #expect(model.frozenStops.isEmpty)
        #expect(model.snapshot.groups[0].jobs[0].card.action == .confirm)
    }

    @Test func cancelConfirmsTurnsKillBack() throws {
        let model = PanelModel(now: TestClock().source)
        let state = try Handoff.state(run: ["cargo"])
        model.apply(PollResult(status: .running, state: state), lastKnown: state)
        model.pressKill(Handoff.leaseId("cargo"))

        #expect(model.cancelConfirms())
        #expect(!model.cancelConfirms())
        #expect(model.snapshot.groups[0].jobs[0].card.action == .kill)
    }

    @Test func serviceGoingAwayCancelsAConfirm() throws {
        let model = PanelModel(now: TestClock().source)
        let state = try Handoff.state(run: ["cargo"])
        model.apply(PollResult(status: .running, state: state), lastKnown: state)
        model.pressKill(Handoff.leaseId("cargo"))

        model.apply(PollResult(status: .stopped), lastKnown: state)

        #expect(!model.isConfirming)
        #expect(model.snapshot.groups[0].jobs[0].card.action == .none)
    }

    @Test func refreshTicksTimesBetweenPolls() throws {
        let clock = TestClock()
        let model = PanelModel(now: clock.source)
        var refreshes = 0
        model.onRefresh = { refreshes += 1 }
        let state = try Handoff.state(run: ["cargo"])
        model.apply(PollResult(status: .running, state: state), lastKnown: state)

        clock.advance(.seconds(2))
        model.refresh()

        #expect(model.snapshot.groups[0].jobs[0].card.time == "3:08 / ~5:00")
        #expect(refreshes == 2)
    }

    @Test func scenarioPlayerEndsARemovedJobAndDropsIt() async throws {
        let model = PanelModel(now: TestClock().source)
        let player = ScenarioPlayer(model: model)
        try player.show("c-queued")

        await model.pressRemove(Handoff.leaseId("npm"))?.value

        let waiting = try #require(model.snapshot.groups.first)
        #expect(waiting.jobs.map(\.card.strong) == ["Removed", ""])
        #expect(model.snapshot.header.summary == "1 waiting")
        withExtendedLifetime(player) {}
    }

    @Test func scenarioPlayerRefusesInTheRefusedScenario() async throws {
        let clock = TestClock()
        let model = PanelModel(now: clock.source)
        let player = ScenarioPlayer(model: model)
        try player.show(Handoff.refusedScenario)
        model.pressKill(Handoff.leaseId("cargo"))
        clock.advance(.seconds(1))

        await model.pressKill(Handoff.leaseId("cargo"))?.value

        #expect(model.snapshot.groups[0].jobs[0].card.detail
            == "Not stopped: \(Handoff.refusedReason)")
        withExtendedLifetime(player) {}
    }

    @Test func scenarioArgumentIsRead() {
        #expect(ScenarioPlayer.requested(in: ["Allot", "--scenario", "max"]) == "max")
        #expect(ScenarioPlayer.requested(in: ["Allot", "--scenario"]) == nil)
        #expect(ScenarioPlayer.requested(in: ["Allot"]) == nil)
    }
}
