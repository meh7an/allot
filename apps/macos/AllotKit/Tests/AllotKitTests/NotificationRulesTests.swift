import AllotContract
import Foundation
import Testing

@testable import AllotKit

private typealias Object = Edit.Object

private let cargoSession = "3f2a9c1e-1111-4000-8000-000000000000"

/// The macOS fixture with the given recent runs and notices, its running job estimated at
/// `estSecs` (granted 300 s before the state's time).
private func state(
    estSecs: Int = 1800,
    recent: [Object] = [],
    notices: [Object] = []
) throws -> UiState {
    try Edit.state(Fixtures.state("macos")) { object in
        Edit.estimate(&object, "active", 0, secs: estSecs)
        object["recent"] = recent
        object["notices"] = notices
    }
}

private func run(
    _ lease: Int64,
    _ command: String,
    cause: String = "released",
    noise: Bool = false
) -> Object {
    var run: Object = [
        "command": command, "cwd": "/Users/user/code/allot",
        "declared": [
            "cores": 4.0, "ram_mb": 2048, "gpu_pct": 0.0, "vram_mb": 0, "est_secs": 60,
        ],
        "end_cause": cause, "ended_at_ms": Handoff.nowMs - 1000, "lease_id": lease,
        "session": cargoSession, "started_at_ms": Handoff.nowMs - 293_000,
    ]
    if noise {
        run["noise"] = [
            "avg_cpu_pct": 1.8, "avg_gpu_pct": 0.0, "peak_cpu_pct": 6.2, "peak_gpu_pct": 0.0,
            "quiet_at_start": true,
        ]
    }
    return run
}

private func notice(_ lease: Int64, cause: String = "killed") -> Object {
    [
        "cause": cause, "command": "cargo build --release", "ended_at_ms": Handoff.nowMs,
        "lease_id": lease, "session": cargoSession,
    ]
}

private var everything: NotificationToggles {
    var toggles = NotificationToggles()
    toggles.jobFinished = true
    toggles.killedOutsidePanel = true
    return toggles
}

@MainActor
@Suite struct NotificationRulesTests {
    private let clock = TestClock()

    private func rules() -> NotificationRules {
        NotificationRules(now: clock.source)
    }

    private func observe(
        _ rules: NotificationRules,
        _ live: UiState?,
        status: ServiceStatus = .running,
        toggles: NotificationToggles = everything,
        panelOpen: Bool = false
    ) -> NotificationUpdate {
        rules.observe(
            status: status, live: live, lastKnown: live, toggles: toggles, panelOpen: panelOpen)
    }

    @Test func firstStateIsABaseline() throws {
        let rules = rules()
        let busy = try state(
            estSecs: 100, recent: [run(40, "cargo test")], notices: [notice(41)])

        #expect(observe(rules, busy) == .none)
        #expect(observe(rules, busy) == .none)
    }

    @Test func runningLongNotifiesOnce() throws {
        let rules = rules()
        _ = observe(rules, try state())

        let update = observe(rules, try state(estSecs: 100))
        let again = observe(rules, try state(estSecs: 100))

        let message = try #require(update.show.first)
        #expect(update.show.count == 1)
        #expect(message.kind == .runningLong)
        #expect(message.title == "python train.py --device mps is running long")
        #expect(message.body == "5:00 so far against a ~1:40 estimate · b81d44f0 · ml-trainer")
        #expect(message.buttons == [.show])
        #expect(message.target == "job:71")
        #expect(again == .none)
    }

    @Test func openPanelSilencesButCountsTheEventAsSeen() throws {
        let rules = rules()
        _ = observe(rules, try state())

        #expect(observe(rules, try state(estSecs: 100), panelOpen: true) == .none)
        #expect(observe(rules, try state(estSecs: 100)) == .none)
    }

    @Test func serviceStopNeedsTwoPollsAndIsWithdrawnWhenBack() throws {
        let rules = rules()
        let live = try state()
        _ = observe(rules, live)

        let first = rules.observe(
            status: .stopped, live: nil, lastKnown: live, toggles: everything, panelOpen: false)
        let second = rules.observe(
            status: .stopped, live: nil, lastKnown: live, toggles: everything, panelOpen: false)
        let back = observe(rules, live)

        #expect(first == .none)
        let message = try #require(second.show.first)
        #expect(message.tag == "service")
        #expect(message.buttons == [.startService, .openLogs])
        #expect(message.body ==
            "Heavy commands now run unchecked. \(live.sessions.count) Claude sessions are "
            + "connected.")
        #expect(back.withdraw == ["service"])
    }

    @Test func chosenStopStaysSilent() throws {
        let rules = rules()
        let live = try state()
        _ = observe(rules, live)
        rules.expectStop()
        clock.advance(.seconds(2))

        for _ in 0..<3 {
            #expect(observe(rules, nil, status: .stopped) == .none)
        }
    }

    @Test func finishesWithinAMinuteMerge() throws {
        let rules = rules()
        _ = observe(rules, try state())

        let one = observe(rules, try state(recent: [run(40, "cargo test")]))
        clock.advance(.seconds(20))
        let two = observe(
            rules, try state(recent: [run(41, "go vet ./..."), run(40, "cargo test")]))
        clock.advance(.seconds(61))
        let later = observe(rules, try state(recent: [
            run(42, "npm test"), run(41, "go vet ./..."), run(40, "cargo test"),
        ]))

        #expect(one.show.map(\.title) == ["cargo test finished"])
        #expect(one.show.first?.tag == "done-1")
        #expect(one.show.first?.body.hasPrefix("4:52 · 3f2a9c1e · allot") == true)
        #expect(two.show.map(\.title) == ["2 jobs finished"])
        #expect(two.show.first?.body == "cargo test and go vet ./...")
        #expect(two.show.first?.tag == "done-1")
        #expect(later.show.map(\.tag) == ["done-2"])
        #expect(later.show.first?.title == "npm test finished")
    }

    @Test func benchmarkFinishedAndKilledRunsDoNotCountAsFinishes() throws {
        let rules = rules()
        _ = observe(rules, try state())

        let update = observe(rules, try state(recent: [
            run(50, "cargo bench", noise: true), run(51, "cargo build", cause: "killed"),
        ]))

        #expect(update.show.map(\.kind) == [.benchmarkFinished])
        #expect(update.show.first?.title == "Benchmark finished in 4:52")
        #expect(update.show.first?.body ==
            "Background noise avg 1.8%, peak 6.2% CPU · cargo bench")
    }

    @Test func killsFromOutsideNotifyButThePanelsOwnDoNot() throws {
        let rules = rules()
        _ = observe(rules, try state())
        rules.noteLocalKill(61)

        let update = observe(rules, try state(notices: [
            notice(61), notice(60), notice(62, cause: "removed"),
        ]))

        #expect(update.show.map(\.tag) == ["killed-60"])
        #expect(update.show.first?.body == "Killed from the command line · Claude was notified")
    }

    @Test func toggleOffCountsAsSeen() throws {
        let rules = rules()
        _ = observe(rules, try state())
        let quiet = NotificationToggles()

        #expect(observe(rules, try state(recent: [run(40, "cargo test")]), toggles: quiet) ==
            .none)
        #expect(observe(rules, try state(recent: [run(40, "cargo test")])) == .none)
    }

    @Test func handoffSamplesCoverEveryKind() throws {
        let samples = try Handoff.notifications()

        #expect(samples.map(\.kind) == [
            .serviceStopped, .runningLong, .benchmarkFinished, .jobFinished, .jobFinished,
            .killedOutsidePanel,
        ])
        #expect(samples[4].title == "3 jobs finished")
        #expect(samples[4].body == "cargo build --release, pnpm --filter web vite build "
            + "--mode production and 1 more")
    }
}
