import AllotContract
import Testing

@testable import AllotKit

private let alice = "a1b2c3d4-0000-4000-8000-000000000001"
private let bob = "b0b0b0b0-0000-4000-8000-000000000002"
private let carol = "c0c0c0c0-0000-4000-8000-000000000003"

@Suite struct PanelSnapshotTests {
    @Test(arguments: [
        ("first-run", "Idle", HeaderGlyph.idle),
        ("idle", "Idle", .idle),
        ("busy", "2 running · 2 waiting", .busy),
        ("benchmark", "Benchmark waiting for quiet", .idle),
        ("macos", "1 running · 1 waiting", .busy),
        ("linux", "2 running · 1 waiting", .busy),
    ])
    func fixtureHeaderMatchesTheWindowsSummary(
        name: String,
        summary: String,
        glyph: HeaderGlyph
    ) throws {
        let snapshot = PanelSnapshot.from(status: .running, state: try Fixtures.state(name))

        #expect(snapshot.header == PanelHeader(glyph: glyph, summary: summary))
        #expect(!snapshot.isLastKnown)
    }

    @Test(arguments: [
        (ServiceStatus.stopped, "Service stopped"),
        (.starting, "Starting service…"),
        (.incompatible, "Service needs an update"),
    ])
    func serviceDownShowsTheErrorGlyphAndLastKnownJobs(
        status: ServiceStatus,
        summary: String
    ) throws {
        let snapshot = PanelSnapshot.from(status: status, state: try Fixtures.state("busy"))

        #expect(snapshot.header == PanelHeader(glyph: .error, summary: summary))
        #expect(snapshot.isLastKnown)
        #expect(snapshot.groups.map(\.heading) == ["Running · 2", "Waiting · 2"])
        #expect(!snapshot.showNothing)
    }

    @Test func downWithoutAnyStateShowsNoJobs() {
        let snapshot = PanelSnapshot.from(status: .stopped, state: nil)

        #expect(snapshot.groups.isEmpty)
        #expect(snapshot.idle == nil)
        #expect(!snapshot.showNothing)
    }

    @Test func busyFixtureRowsCarryTheirSessionStrip() throws {
        let snapshot = PanelSnapshot.from(status: .running, state: try Fixtures.state("busy"))
        let rows = snapshot.groups.flatMap(\.jobs)

        #expect(rows.map(\.id) == [42, 41, 43, 44])
        #expect(rows[1].command == "cargo build --release")
        #expect(rows[1].session == SessionStrip(
            shortId: "3f2a9c1e",
            project: "app",
            projectPath: "E:\\work\\app",
            usage: "7.8 cores · 2.9 GB"))
        // 43 waits for a session that also runs 42.
        #expect(rows[2].session.usage == "also running")
        #expect(rows.map(\.isQueued) == [false, false, true, true])
    }

    @Test func idleLineListsSessionsWithoutJobs() throws {
        let busy = PanelSnapshot.from(status: .running, state: try Fixtures.state("busy"))
        let idle = PanelSnapshot.from(status: .running, state: try Fixtures.state("idle"))

        #expect(busy.idle == IdleLine(
            label: "Idle · 1", ids: "7c19e2aa", usage: "0.3 cores · 0.5 GB"))
        #expect(idle.idle == IdleLine(label: "Idle · 2", ids: "3f2a9c1e  b81d44f0", usage: ""))
        #expect(idle.showNothing)
    }

    @Test func groupsComeInBenchmarkRunningWaitingOrder() throws {
        let state = try States.state(
            sessions: [States.session(alice), States.session(bob), States.session(carol)],
            active: [States.active(1, session: alice)],
            queue: [
                States.queued(2, session: bob, benchmark: true, phase: "draining"),
                States.queued(3, session: carol),
            ])

        let snapshot = PanelSnapshot.from(status: .running, state: state)

        #expect(snapshot.groups.map(\.heading) == ["Benchmark", "Running · 1", "Waiting · 1"])
        #expect(snapshot.groups.map(\.kind) == [.benchmark, .running, .waiting])
        #expect(snapshot.header.summary == "1 running · benchmark waiting")
        #expect(snapshot.benchmark == .draining)
    }

    @Test(arguments: [
        ("draining", BenchmarkPhase.draining, "Benchmark waiting"),
        ("measuring", .waitingForQuiet, "Benchmark waiting for quiet"),
        ("noisy", .waitingForQuiet, "Benchmark waiting for quiet"),
    ])
    func queuedBenchmarkPhaseFollowsItsVerdict(
        verdict: String,
        phase: BenchmarkPhase,
        summary: String
    ) throws {
        let state = try States.state(
            queue: [States.queued(1, session: alice, benchmark: true, phase: verdict)])

        let snapshot = PanelSnapshot.from(status: .running, state: state)

        #expect(snapshot.benchmark == phase)
        #expect(snapshot.header == PanelHeader(glyph: .idle, summary: summary))
    }

    @Test func activeBenchmarkRunsAloneInTheBenchmarkGroup() throws {
        let state = try States.state(
            active: [States.active(1, session: alice, label: "cargo bench", benchmark: true)],
            queue: [States.queued(2, session: bob)])

        let snapshot = PanelSnapshot.from(status: .running, state: state)

        #expect(snapshot.header == PanelHeader(glyph: .busy, summary: "Benchmark running alone"))
        #expect(snapshot.groups.map(\.heading) == ["Benchmark", "Waiting · 1"])
        #expect(snapshot.groups[0].jobs.map(\.command) == ["cargo bench"])
    }

    @Test func jobOfAnUnregisteredSessionUsesTheLeaseCwd() throws {
        let state = try States.state(
            active: [States.active(1, session: alice, cwd: "/Users/user/work/allot/")])

        let strip = PanelSnapshot.from(status: .running, state: state).groups[0].jobs[0].session

        #expect(strip == SessionStrip(
            shortId: "a1b2c3d4",
            project: "allot",
            projectPath: "/Users/user/work/allot/",
            usage: ""))
    }

    @Test func sessionWithoutCwdFallsBackToTheLeaseCwd() throws {
        let state = try States.state(
            sessions: [States.session(alice, cwd: nil)],
            active: [States.active(1, session: alice, cwd: "/Users/user/work/tools")])

        let strip = PanelSnapshot.from(status: .running, state: state).groups[0].jobs[0].session

        #expect(strip.project == "tools")
        #expect(strip.usage == "1.5 cores · 2.0 GB")
    }
}
