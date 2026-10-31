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
}
