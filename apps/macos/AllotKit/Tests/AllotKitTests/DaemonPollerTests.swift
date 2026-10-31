import Foundation
import Testing

@testable import AllotKit

/// A poller of a daemon that serves the idle fixture while `isUp` says so and refuses the
/// connection otherwise.
@MainActor
private func poller(
    _ home: TemporaryHome,
    _ isUp: @escaping @Sendable () -> Bool
) throws -> DaemonPoller {
    try home.write("t", to: home.data.tokenFile)
    let body = try Fixtures.data("ui-state-idle.json")
    let transport = FakeTransport { _ in
        guard isUp() else {
            throw URLError(.cannotConnectToHost)
        }
        return (200, body)
    }
    return DaemonPoller(
        client: DaemonClient(port: 1, tokenFile: home.data.tokenFile, transport: transport))
}

private final class Flag: @unchecked Sendable {
    private let lock = NSLock()
    private var value: Bool

    init(_ value: Bool) {
        self.value = value
    }

    var current: Bool {
        get { lock.withLock { value } }
        set { lock.withLock { value = newValue } }
    }
}

@MainActor
@Suite struct DaemonPollerTests {
    @Test func runningPollsEveryFiveSecondsClosedAndEverySecondOpen() async throws {
        let home = try TemporaryHome()
        let poller = try poller(home) { true }

        #expect(try await poller.pollOnce() == .seconds(5))
        poller.setPanelOpen(true)
        #expect(try await poller.pollOnce() == .seconds(1))
    }

    @Test func stoppedDaemonBacksOffAndKeepsTheLastKnownState() async throws {
        let home = try TemporaryHome()
        let up = Flag(true)
        let poller = try poller(home) { up.current }
        var updates: [ServiceStatus] = []
        poller.onUpdate = { updates.append($0.status) }
        try await poller.pollOnce()

        up.current = false
        let delays = [
            try await poller.pollOnce(),
            try await poller.pollOnce(),
            try await poller.pollOnce(),
        ]

        #expect(delays == [.seconds(1), .seconds(2), .seconds(4)])
        #expect(poller.latest?.status == .stopped)
        #expect(poller.lastKnown?.sessions.count == 2)
        #expect(updates == [.running, .stopped, .stopped, .stopped])
    }

    @Test func openingThePanelRestartsTheBackoff() async throws {
        let home = try TemporaryHome()
        let poller = try poller(home) { false }
        try await poller.pollOnce()
        try await poller.pollOnce()

        poller.setPanelOpen(true)

        #expect(try await poller.pollOnce() == .seconds(1))
    }

    @Test func pollNowWakesTheLoopEarly() async throws {
        let home = try TemporaryHome()
        let poller = try poller(home) { true }
        var polls = 0
        poller.onUpdate = { _ in polls += 1 }
        let loop = Task { await poller.run() }
        defer { loop.cancel() }
        try await waitUntil { polls == 1 }

        // Closed and running, the next poll is 5 s away.
        poller.pollNow()

        try await waitUntil { polls == 2 }
    }

    @Test func cancellingTheLoopEndsIt() async throws {
        let home = try TemporaryHome()
        let poller = try poller(home) { true }
        let loop = Task { await poller.run() }

        loop.cancel()

        await loop.value
    }

    private func waitUntil(_ condition: () -> Bool) async throws {
        let deadline = ContinuousClock.now + .seconds(2)
        while !condition() {
            try #require(ContinuousClock.now < deadline, "timed out")
            try await Task.sleep(for: .milliseconds(10))
        }
    }
}
