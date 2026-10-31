import Foundation
import Testing

@testable import AllotKit

private func cargo(_ change: (inout Edit.Object) -> Void = { _ in }) throws -> JobCard {
    let state = try Edit.state(try Handoff.state(run: ["cargo"]), change)
    return .forActive(state.active[0], nowMs: Handoff.nowMs, lastKnown: false)
}

private func queued(
    _ wait: [String],
    _ index: Int = 0,
    _ change: (inout Edit.Object) -> Void = { _ in }
) throws -> JobCard {
    let state = try Edit.state(try Handoff.state(wait: wait), change)
    return .forQueued(
        state.queue[index], nowMs: Handoff.nowMs, afterBenchmark: false, lastKnown: false)
}

@Suite struct JobCardTests {
    @Test func runningBarIsElapsedOverTheEstimate() throws {
        let card = try cargo()

        #expect(abs(card.barFraction - 0.62) < 0.001)
        #expect(card.barTone == .running)
        #expect(card.detailTooltip == card.detail)
    }

    @Test func pastTheEstimateTheBarIsFullAndAmber() throws {
        let card = try cargo { Edit.estimate(&$0, "active", 0, secs: 100) }

        #expect(card.time == "1:26 over")
        #expect(card.timeIsWarning)
        #expect(card.barFraction == 1)
        #expect(card.barTone == .over)
    }

    @Test func withoutAnEstimateOnlyTheElapsedTimeShows() throws {
        let card = try cargo { Edit.estimate(&$0, "active", 0, secs: 0) }

        #expect(card.time == "3:06")
        #expect(card.barFraction == 0)
        #expect(!card.timeIsWarning)
    }

    @Test func withoutAProcessTheCommandNamesTheJob() throws {
        let card = try cargo { Edit.job(&$0, "active", 0) { $0["process"] = NSNull() } }

        #expect(card.tile == "CA")
        #expect(card.detail == "8 cores · 4 GB")
        #expect(card.actionName == "Kill cargo build --release")
        #expect(card.exePath == nil)
    }

    @Test func timesTickWithTheClock() throws {
        let state = try Handoff.state(run: ["cargo"], wait: ["npm"])

        let active = JobCard.forActive(
            state.active[0], nowMs: Handoff.nowMs + 10_000, lastKnown: false)
        let waiting = JobCard.forQueued(
            state.queue[0], nowMs: Handoff.nowMs + 10_000, afterBenchmark: false,
            lastKnown: false)

        #expect(active.time == "3:16 / ~5:00")
        #expect(waiting.time == "0:52 / ~2 min")
    }

    @Test func headOfTheQueueWithAMinuteLeftStartsNext() throws {
        let card = try queued(["npm"]) { object in
            Edit.verdict(&object, 0) { $0["wait_estimate_secs"] = 60 }
        }

        #expect(card.time == "starts next · ~1 min")
        #expect(abs(card.barFraction - 42.0 / 102) < 0.001)
    }

    @Test func headOfTheQueueThatFitsStartsNextWithoutAnEstimate() throws {
        let card = try queued(["npmB"]) { object in
            Edit.verdict(&object, 0) { $0["fits"] = true }
        }

        #expect(card.time == "starts next")
        #expect(card.barFraction == 1)
    }

    @Test func secondInLineWithAMinuteLeftDoesNotStartNext() throws {
        let card = try queued(["npm", "npmN"], 1) { object in
            Edit.verdict(&object, 1) { $0["wait_estimate_secs"] = 60 }
        }

        #expect(card.time == "3:16 / ~4 min")
    }

    @Test func lastKnownQueuedJobHasNoRemove() throws {
        let state = try Handoff.state(wait: ["npm"])

        let card = JobCard.forQueued(
            state.queue[0], nowMs: Handoff.nowMs, afterBenchmark: false, lastKnown: true)

        #expect(card.action == .none)
        #expect(card.actionName == "")
        #expect(card.barTone == .frozen)
    }

    @Test func drainedBenchmarkThatDoesNotFitWaitsForRoom() throws {
        let state = try Edit.state(try Handoff.state(bench: "drain")) { object in
            Edit.verdict(&object, 0) { verdict in
                verdict["benchmark"] = NSNull()
                verdict["reasons"] = ["cores: 16.0 requested, 12.4 free"]
            }
        }

        let card = JobCard.forQueued(
            state.queue[0], nowMs: Handoff.nowMs, afterBenchmark: false, lastKnown: false,
            othersActive: false)

        #expect(card.detail == "Waiting for room · cores: 16.0 requested, 12.4 free")
        #expect(card.steps?.current == .drain)
    }

    @Test func benchmarkWithoutProgressDrainsWhileOthersRun() throws {
        let state = try Edit.state(try Handoff.state(bench: "drain")) { object in
            Edit.verdict(&object, 0) { $0["benchmark"] = NSNull() }
        }

        let card = JobCard.forQueued(
            state.queue[0], nowMs: Handoff.nowMs, afterBenchmark: false, lastKnown: false,
            othersActive: true)

        #expect(card.detail == "Draining · waiting for running jobs to finish")
    }

    @Test func runningBenchmarkPastItsEstimateSaysHowFarOver() throws {
        let state = try Handoff.state(bench: "run")

        let card = JobCard.forActive(
            state.active[0], nowMs: Handoff.nowMs + 102_000, lastKnown: false)

        #expect(card.detail == "Running alone · 0:30 over")
        #expect(card.steps?.runFraction == 1)
    }

    @Test func runningBenchmarkWithoutAnEstimateShowsElapsedOnly() throws {
        let state = try Edit.state(try Handoff.state(bench: "run")) {
            Edit.estimate(&$0, "active", 0, secs: 0)
        }

        let card = JobCard.forActive(state.active[0], nowMs: Handoff.nowMs, lastKnown: false)

        #expect(card.detail == "Running alone · 0:48")
        #expect(card.steps?.runFraction == 0)
    }

    @Test func finishedBenchmarkThatWasNotQuietSaysSo() throws {
        let run = try Handoff.finishedBenchmark(quietAtStart: false)
        let noise = try #require(run.noise)

        let card = JobCard.forFinished(run, noise: noise)

        #expect(card.detail == "Finished in 2:04 · not quiet at start · background noise avg "
            + "1.8%, peak 6.2% CPU")
        #expect(card.tile == "CA")
    }

    @Test func lastKnownBenchmarkKeepsOnlyItsPlainLine() throws {
        let state = try Handoff.state(bench: "run")

        let card = JobCard.forActive(state.active[0], nowMs: Handoff.nowMs, lastKnown: true)

        #expect(card.detail == "cargo 30412 · was at 0:48 / ~2:00")
        #expect(card.steps == nil)
        #expect(card.barTone == .none)
    }
}

@Suite struct RecentLineTests {
    @Test func recentRunsShowOnlyWhileNothingRunsOrWaits() throws {
        let state = try Edit.state(try Handoff.state(run: ["cargo"], recent: true)) { _ in }

        let snapshot = PanelSnapshot.from(status: .running, state: state)

        #expect(snapshot.recent.isEmpty)
    }

    @Test func noJobsAndNoRecentRunsSaysSo() throws {
        let snapshot = PanelSnapshot.from(status: .running, state: try Handoff.state())

        #expect(snapshot.showNothing)
        #expect(snapshot.recent.isEmpty)
    }

    @Test func recentRunWithoutASessionHasNoId() throws {
        let state = try Edit.state(try Handoff.state(recent: true)) { object in
            var runs = object["recent"] as? [Edit.Object] ?? []
            runs[0]["session"] = nil
            object["recent"] = runs
        }

        let snapshot = PanelSnapshot.from(status: .running, state: state)

        #expect(snapshot.recent[0].shortId == "")
    }
}
