import Testing

@testable import AllotKit

/// The snapshot of a scenario as the app's `--scenario` shows it, before any press.
private func scenarioSnapshot(_ name: String) throws -> PanelSnapshot {
    let (status, state) = try Handoff.scenario(name)
    return PanelSnapshot.from(status: status, state: state, stops: Handoff.stops(name))
}

private func scenarioCard(_ name: String, _ command: String) throws -> JobCard {
    let rows = try scenarioSnapshot(name).groups.flatMap(\.jobs)
    return try #require(rows.first { $0.command == command }).card
}

@Suite struct HandoffScenarioTests {
    @Test(arguments: [
        ("empty", "Idle", HeaderGlyph.idle),
        ("one", "1 running", .busy),
        ("several", "2 running · 1 waiting", .busy),
        ("kill-confirm", "2 running · 1 waiting", .busy),
        ("killed", "1 running · 1 waiting", .busy),
        ("bench-drain", "1 running · benchmark waiting", .busy),
        ("bench-quiet", "Benchmark waiting for quiet", .idle),
        ("bench-run", "Benchmark running alone", .busy),
        ("bench-done", "1 waiting", .idle),
        ("bench-noisy", "Benchmark waiting for quiet", .idle),
        ("bench-forced", "Benchmark running alone", .busy),
        ("down", "Service stopped", .error),
        ("max", "4 running · 2 waiting", .busy),
        ("icons", "4 running · 2 waiting", .busy),
        ("c-active", "1 running", .busy),
        ("c-over", "1 running", .busy),
        ("c-queued", "2 waiting", .idle),
        ("c-bench", "Benchmark waiting for quiet", .idle),
        ("c-confirm", "1 running", .busy),
        ("c-killed", "Idle", .idle),
        ("c-removed", "Idle", .idle),
        ("c-refused", "1 running", .busy),
    ] as [(String, String, HeaderGlyph)])
    func summaryMatchesTheHandoff(name: String, summary: String, glyph: HeaderGlyph) throws {
        let header = try scenarioSnapshot(name).header

        #expect(header.summary == summary)
        #expect(header.glyph == glyph)
    }

    @Test func everyListedScenarioIsCovered() {
        #expect(Handoff.scenarioNames.count == 22)
    }

    @Test(arguments: [
        ("cargo build --release", "cargo 18244 · 8 cores · 4 GB", "3:06 / ~5:00", false),
        ("python train.py --epochs 20 --data ./shards",
         "python3.13 22108 · 2 cores · 4 GB · GPU 80% · 4 GB VRAM", "1:40 over", true),
        ("pnpm --filter web vite build --mode production",
         "node 9120 · 4 cores · 2 GB · GPU 10% · 1 GB VRAM", "0:52 / ~1:30", false),
        ("go vet ./...", "go 14772 · 1 core · 1 GB", "0:20 / ~1:00", false),
    ])
    func activeCardsShowProcessReservationAndTime(
        command: String,
        detail: String,
        time: String,
        over: Bool
    ) throws {
        let card = try scenarioCard("max", command)

        #expect(card.detail == detail)
        #expect(card.time == time)
        #expect(card.timeIsWarning == over)
        #expect(card.barTone == (over ? .over : .running))
        #expect(card.action == .kill)
    }

    @Test(arguments: [
        ("cargo build --release", "CA", "CPU heavy", "Kill cargo"),
        ("python train.py --epochs 20 --data ./shards", "PY", "GPU heavy", "Kill python3.13"),
        ("pnpm --filter web vite build --mode production", "NO", "Mixed", "Kill node"),
        ("go vet ./...", "GO", "Light", "Kill go"),
    ])
    func activeCardsHaveInitialsChipAndKillName(
        command: String,
        tile: String,
        chip: String,
        name: String
    ) throws {
        let card = try scenarioCard("max", command)

        #expect(card.tile == tile)
        #expect(card.chip == chip)
        #expect(card.actionName == name)
        #expect(!card.isQueued && !card.isBenchmark)
    }

    @Test func queuedCardWithAnEstimateShowsWaitedAgainstTheTotal() throws {
        let card = try scenarioCard("several", "npm run test:e2e")

        #expect(card.tile == "#1")
        #expect(card.isQueued)
        #expect(card.detail == "2 cores · 4 GB")
        #expect(card.time == "0:42 / ~2 min")
        #expect(card.timeIsWarning)
        #expect(card.barTone == .waiting)
        #expect(abs(card.barFraction - 0.35) < 0.001)
        #expect(card.action == .remove)
        #expect(card.actionName == "Remove npm run test:e2e from queue")
    }

    @Test func queuedCardWithoutAnEstimateDependsOnTheQueue() throws {
        let card = try scenarioCard("c-queued", "cargo test --workspace")

        #expect(card.tile == "#2")
        #expect(card.detail == "8 cores · 4 GB")
        #expect(card.time == "0:18 · depends on queue")
        #expect(card.barTone == .waitingDashed)
        #expect(card.chip == "Custom")
        #expect(card.chipTooltip == "Custom reservation")
    }

    @Test(arguments: ["bench-drain", "bench-quiet", "bench-run"])
    func queuedJobBehindABenchmarkWaitsAfterIt(name: String) throws {
        let card = try scenarioCard(name, "npm run test:e2e")

        #expect(card.time == "1:12 · after benchmark")
        #expect(card.barTone == .waitingDashed)
    }

    @Test func queuedCardOfASessionThatAlsoRunsSaysSo() throws {
        let rows = try scenarioSnapshot("max").groups.flatMap(\.jobs)
        let usage = { (command: String) in rows.first { $0.command == command }?.session.usage }

        #expect(usage("cargo build --release") == "7.8 cores · 2.9 GB")
        #expect(usage("cargo test --workspace") == "also running")
        #expect(usage("npm run test:e2e") == "0.4 cores · 1.1 GB")
    }

    @Test func drainingBenchmarkHasTheStatusLineAndStepper() throws {
        let card = try scenarioCard("bench-drain", "cargo bench --bench parser")

        #expect(card.detail == "Draining · waiting for running jobs to finish")
        #expect(card.time == "")
        #expect(card.icon == .timer)
        #expect(card.isBenchmark && card.chip == "Benchmark")
        #expect(card.steps == BenchmarkSteps(current: .drain, quietTicks: 0, runFraction: 0))
        #expect(card.barTone == .none)
        #expect(card.action == .remove)
        #expect(card.actionName == "Remove cargo bench --bench parser from queue")
    }

    @Test func quietBenchmarkCountsTicks() throws {
        let card = try scenarioCard("bench-quiet", "cargo bench --bench parser")

        #expect(card.detail == "Waiting for quiet · 6 / 10 s · background 3% CPU")
        #expect(card.steps == BenchmarkSteps(current: .quiet, quietTicks: 6, runFraction: 0))
    }

    @Test func noisyBenchmarkSaysWhenItStartsAnyway() throws {
        let card = try scenarioCard("bench-noisy", "cargo bench --bench parser")

        #expect(card.detail
            == "Not quiet · background 14% CPU (limit 5%) · starting anyway in 0:42")
        #expect(card.steps?.quietTicks == BenchmarkSteps.tickCount)
    }

    @Test func runningBenchmarkRunsAlone() throws {
        let row = try #require(
            try scenarioSnapshot("bench-run").groups.first { $0.kind == .benchmark }?.jobs.first)

        #expect(row.card.detail == "Running alone · 0:48 / ~2:00")
        #expect(row.card.tile == "CA")
        #expect(row.card.action == .kill)
        #expect(row.card.steps?.current == .run)
        #expect(abs((row.card.steps?.runFraction ?? 0) - 0.4) < 0.001)
        #expect(row.session.usage == "15.6 cores · 3.1 GB")
    }

    @Test func forcedBenchmarkWasNotQuietAtStart() throws {
        let card = try scenarioCard("bench-forced", "cargo bench --bench parser")

        #expect(card.detail == "Running alone · not quiet at start · 0:48 / ~2:00")
    }

    @Test func finishedBenchmarkHoldsWithItsNoise() throws {
        let snapshot = try scenarioSnapshot("bench-done")
        let benchmark = try #require(snapshot.groups.first { $0.kind == .benchmark })
        let row = try #require(benchmark.jobs.first)

        #expect(row.id == Handoff.leaseId("bench"))
        #expect(row.isEnding)
        #expect(row.session.project == "parser-perf")
        #expect(row.card.detail
            == "Finished in 2:04 · background noise avg 1.8%, peak 6.2% CPU")
        #expect(row.card.icon == .check)
        #expect(row.card.steps == BenchmarkSteps(current: .done, quietTicks: 10, runFraction: 1))
        #expect(row.card.action == .none)
        let waiting = try #require(snapshot.groups.first { $0.kind == .waiting })
        #expect(waiting.jobs.first?.card.time == "starts next · ~1 min")
    }

    @Test func emptyScenarioListsRecentRuns() throws {
        let snapshot = try scenarioSnapshot("empty")

        #expect(snapshot.groups.isEmpty)
        #expect(!snapshot.showNothing)
        #expect(snapshot.recent.map(\.command) == ["cargo build --release", "npm run test:e2e"])
        #expect(snapshot.recent.map(\.shortId) == ["3f2a9c1e", "77a0e5d2"])
        #expect(snapshot.recent.map(\.meta) == ["4:52 · 4 min ago", "2:10 · 11 min ago"])
    }

    @Test func downShowsTheLastKnownJobFrozen() throws {
        let card = try scenarioCard("down", "cargo build --release")

        #expect(card.detail == "cargo 18244 · was at 3:06 / ~5:00")
        #expect(card.time == "")
        #expect(card.barTone == .frozen)
        #expect(card.action == .none)
    }

    @Test(arguments: ["kill-confirm", "c-confirm"])
    func confirmScenarioAsksKill(name: String) throws {
        let card = try scenarioCard(name, "cargo build --release")

        #expect(card.strong == "")
        #expect(card.detail == "Stops cargo and its child processes")
        #expect(card.detailIsDanger)
        #expect(card.time == "3s")
        #expect(card.action == .confirm)
        #expect(card.actionName == "Confirm kill cargo")
    }

    @Test(arguments: ["killed", "c-killed"])
    func killedScenarioHoldsStopped(name: String) throws {
        let card = try scenarioCard(name, "cargo build --release")

        #expect(card.strong == "Stopped")
        #expect(card.detail == " · Claude was notified")
        #expect(card.time == "at 3:06")
        #expect(card.barTone == .frozen)
        #expect(card.isEnded && card.isStruck)
        #expect(card.action == .none)
        #expect(card.announcement == "Stopped cargo · Claude was notified")
    }

    @Test func removedScenarioHoldsRemoved() throws {
        let card = try scenarioCard("c-removed", "npm run test:e2e")

        #expect(card.tile == "#1")
        #expect(card.strong == "Removed")
        #expect(card.detail == " · Claude was notified")
        #expect(card.time == "")
        #expect(card.barTone == .none)
        #expect(card.announcement == "Removed npm run test:e2e · Claude was notified")
    }

    @Test func iconsScenarioPointsAtAppBundles() throws {
        let rows = try scenarioSnapshot("icons").groups.flatMap(\.jobs)
        let bundles = rows.filter { !$0.isQueued }.map {
            ProcessIcon.bundlePath(forExe: $0.card.exePath)
        }

        #expect(bundles == [
            "/System/Applications/Utilities/Terminal.app",
            "/System/Applications/Utilities/Activity Monitor.app",
            "/System/Applications/Calculator.app",
            nil,
        ])
    }

    @Test func groupsComeInBenchmarkRunningWaitingOrder() throws {
        let snapshot = try scenarioSnapshot("bench-drain")

        #expect(snapshot.groups.map(\.kind) == [.benchmark, .running, .waiting])
        #expect(snapshot.groups.map(\.heading) == ["Benchmark", "Running · 1", "Waiting · 1"])
        #expect(snapshot.benchmark == .draining)
    }
}
