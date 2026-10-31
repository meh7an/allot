import Testing

@testable import AllotKit

private let cargo = Handoff.leaseId("cargo")
private let npm = Handoff.leaseId("npm")

/// Stops that have seen the `several` state: cargo and py run, npm waits.
private func seeded(_ clock: TestClock) throws -> JobStops {
    let stops = JobStops(now: clock.source)
    stops.observe(try Handoff.state(run: ["cargo", "py"], wait: ["npm"]))
    return stops
}

private let ended = StopResult(ended: true, text: "", endedAtMs: Handoff.nowMs)

@Suite struct JobStopsTests {
    @Test func firstPressConfirmsAndCountsDown() throws {
        let clock = TestClock()
        let stops = try seeded(clock)

        #expect(stops.pressKill(cargo) == .confirming)
        #expect(stops.views[cargo] == StopView(kind: .kill, phase: .confirming, secondsLeft: 4))
        #expect(stops.isConfirming)
        clock.advance(.milliseconds(1500))
        #expect(stops.views[cargo]?.secondsLeft == 3)
        clock.advance(.milliseconds(2400))
        #expect(stops.views[cargo]?.secondsLeft == 1)
    }

    @Test func confirmDropsAfterFourSeconds() throws {
        let clock = TestClock()
        let stops = try seeded(clock)
        stops.pressKill(cargo)

        clock.advance(.seconds(4))

        #expect(stops.expire())
        #expect(stops.views.isEmpty)
    }

    @Test func secondPressWithinTheDoublePressGuardIsIgnored() throws {
        let clock = TestClock()
        let stops = try seeded(clock)
        stops.pressKill(cargo)

        clock.advance(.milliseconds(300))

        #expect(stops.pressKill(cargo) == .ignored)
        #expect(stops.views[cargo]?.phase == .confirming)
    }

    @Test func secondPressAfterTheGuardSends() throws {
        let clock = TestClock()
        let stops = try seeded(clock)
        stops.pressKill(cargo)

        clock.advance(.milliseconds(301))

        #expect(stops.pressKill(cargo) == .send)
        #expect(stops.views[cargo]?.phase == .sending)
        #expect(stops.pressKill(cargo) == .ignored)
    }

    @Test func endedStopHoldsFourSecondsWithItsNote() throws {
        let clock = TestClock()
        let stops = try seeded(clock)
        stops.pressKill(cargo)
        clock.advance(.seconds(1))
        stops.pressKill(cargo)

        stops.finished(cargo, StopResult(ended: true, text: "1 process still running",
                                         endedAtMs: Handoff.nowMs))

        #expect(stops.views[cargo] == StopView(
            kind: .kill, phase: .ended, message: "1 process still running",
            endedAtMs: Handoff.nowMs))
        #expect(stops.pressKill(cargo) == .ignored)
        clock.advance(.milliseconds(3999))
        #expect(!stops.expire())
        clock.advance(.milliseconds(1))
        #expect(stops.expire())
    }

    @Test func refusalHoldsSixSecondsAndAPressConfirmsAgain() throws {
        let clock = TestClock()
        let stops = try seeded(clock)
        stops.pressKill(cargo)
        clock.advance(.seconds(1))
        stops.pressKill(cargo)

        stops.finished(cargo, .refused("it is protected"))

        #expect(stops.views[cargo] == StopView(
            kind: .kill, phase: .failed, message: "it is protected"))
        clock.advance(.seconds(5))
        #expect(!stops.expire())
        #expect(stops.pressKill(cargo) == .confirming)
    }

    @Test func refusalWithoutAPressGoesAfterSixSeconds() throws {
        let clock = TestClock()
        let stops = try seeded(clock)
        #expect(stops.pressRemove(npm))
        stops.finished(npm, .refused("the service is not running"))

        clock.advance(.seconds(6))

        #expect(stops.expire())
    }

    @Test func removeSendsAtOnce() throws {
        let clock = TestClock()
        let stops = try seeded(clock)

        #expect(stops.pressRemove(npm))
        #expect(stops.views[npm] == StopView(kind: .remove, phase: .sending))
        #expect(!stops.pressRemove(npm))
    }

    @Test func pressesOfTheWrongKindOrAnUnknownJobAreIgnored() throws {
        let stops = try seeded(TestClock())

        #expect(stops.pressKill(npm) == .ignored)
        #expect(!stops.pressRemove(cargo))
        #expect(stops.pressKill(999) == .ignored)
        #expect(stops.views.isEmpty)
    }

    @Test func cancelOnlyTakesBackAConfirm() throws {
        let stops = try seeded(TestClock())
        stops.pressKill(cargo)
        stops.pressRemove(npm)

        #expect(!stops.cancel(npm))
        #expect(stops.cancel(cargo))
        #expect(!stops.cancelConfirms())
        #expect(stops.views.keys.sorted() == [npm])
    }

    @Test func cancelConfirmsTakesBackEveryConfirm() throws {
        let stops = try seeded(TestClock())
        stops.pressKill(cargo)
        stops.pressKill(Handoff.leaseId("py"))

        #expect(stops.cancelConfirms())
        #expect(!stops.isConfirming)
    }

    @Test func confirmIsDroppedOnceItsJobIsGone() throws {
        let stops = try seeded(TestClock())
        stops.pressKill(cargo)

        stops.observe(try Handoff.state(run: ["py"], wait: ["npm"]))

        #expect(stops.views.isEmpty)
    }

    @Test func sendingStopKeepsItsCardAtItsPlace() throws {
        let clock = TestClock()
        let stops = try seeded(clock)
        stops.pressKill(cargo)
        clock.advance(.seconds(1))
        stops.pressKill(cargo)
        stops.pressRemove(npm)

        let state = try Handoff.state(run: ["py"])
        stops.observe(state)
        let jobs = stops.merge(state)

        #expect(jobs.active.map(\.lease.id) == [cargo, Handoff.leaseId("py")])
        #expect(jobs.queue.map(\.lease.id) == [npm])
    }

    @Test func mergeLeavesAListedJobAlone() throws {
        let stops = try seeded(TestClock())
        stops.pressRemove(npm)
        let state = try Handoff.state(run: ["cargo", "py"], wait: ["npm"])

        #expect(stops.merge(state) == ShownJobs(state))
    }

    @Test func nextChangeIsTheNextCountdownSecondOrHoldEnd() throws {
        let clock = TestClock()
        let stops = try seeded(clock)
        #expect(stops.nextChange == nil)

        stops.pressKill(cargo)
        clock.advance(.milliseconds(400))
        #expect(stops.nextChange == .milliseconds(600))

        clock.advance(.milliseconds(600))
        stops.pressKill(cargo)
        #expect(stops.nextChange == nil)
        stops.finished(cargo, ended)
        clock.advance(.seconds(1))
        #expect(stops.nextChange == .seconds(3))
    }

    @Test(arguments: [
        ("released", StopKind.finished, ""),
        ("killed", .killedOutside, ""),
        ("expired", .endedByAllotd, "no command running"),
    ] as [(String, StopKind, String)])
    func jobThatEndsElsewhereHoldsItsCard(
        cause: String,
        kind: StopKind,
        message: String
    ) throws {
        let stops = try seeded(TestClock())
        let state = try Edit.state(try Handoff.state(run: ["py"], wait: ["npm"])) {
            Edit.ended(&$0, lease: cargo, cause: cause, reason: "no command running")
        }

        stops.observe(state)

        #expect(stops.views[cargo] == StopView(
            kind: kind, phase: .ended, message: message, endedAtMs: Handoff.nowMs))
        #expect(stops.merge(state).active.first?.lease.id == cargo)
    }

    @Test func jobGoneWithoutAnEndedEntryIsNotHeld() throws {
        let stops = try seeded(TestClock())

        stops.observe(try Handoff.state(run: ["py"], wait: ["npm"]))

        #expect(stops.views.isEmpty)
    }

    @Test func endedBenchmarkIsNotHeldSinceRecentRunsCarryIt() throws {
        let stops = JobStops(now: TestClock().source)
        stops.observe(try Handoff.state(bench: "run"))
        let state = try Edit.state(try Handoff.state()) {
            Edit.ended(&$0, lease: Handoff.leaseId("bench"), cause: "released")
        }

        stops.observe(state)

        #expect(stops.views.isEmpty)
    }
}
