import Testing

@testable import AllotKit

private let alice = "a1b2c3d4-0000-4000-8000-000000000001"
private let bob = "b0b0b0b0-0000-4000-8000-000000000002"

@Suite struct MenuBarStatusTests {
    @Test(arguments: [
        ("first-run", MenuBarIcon.idle),
        ("idle", .idle),
        ("busy", .queued),
        ("benchmark", .benchmark),
        ("macos", .queued),
    ])
    func fixtureIcon(name: String, icon: MenuBarIcon) throws {
        let snapshot = PanelSnapshot.from(status: .running, state: try Fixtures.state(name))

        #expect(MenuBarStatus.from(snapshot).icon == icon)
    }

    @Test func runningWithoutWaitingIsBusy() throws {
        let state = try States.state(active: [States.active(1, session: alice)])

        let status = MenuBarStatus.from(PanelSnapshot.from(status: .running, state: state))

        #expect(status == MenuBarStatus(icon: .busy, summary: "1 running"))
        #expect(status.tooltip == "Allot · 1 running")
    }

    @Test func benchmarkWinsOverWaitingAndRunning() throws {
        let state = try States.state(
            active: [States.active(1, session: alice)],
            queue: [
                States.queued(2, session: bob, benchmark: true, phase: "draining"),
                States.queued(3, session: alice),
            ])

        let status = MenuBarStatus.from(PanelSnapshot.from(status: .running, state: state))

        #expect(status.icon == .benchmark)
    }

    @Test(arguments: [ServiceStatus.stopped, .starting, .incompatible])
    func downWinsOverLastKnownJobs(status: ServiceStatus) throws {
        let snapshot = PanelSnapshot.from(status: status, state: try Fixtures.state("benchmark"))

        #expect(MenuBarStatus.from(snapshot).icon == .down)
    }

    @Test func everyIconNamesItsImageSet() {
        #expect(MenuBarIcon.allCases.map(\.assetName) == [
            "MenuBarIdle", "MenuBarBusy", "MenuBarBenchmark", "MenuBarQueued", "MenuBarDown",
        ])
    }
}
