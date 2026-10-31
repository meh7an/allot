import AllotContract

/// Polls the daemon on the ``PollSchedule`` and keeps the last state it served, which the
/// service-down view shows dimmed.
@MainActor
public final class DaemonPoller {
    /// Called after every poll.
    public var onUpdate: ((PollResult) -> Void)?

    public private(set) var latest: PollResult?

    /// The newest state allotd served, kept while it is down.
    public private(set) var lastKnown: UiState?

    private let client: DaemonClient
    private var schedule = PollSchedule()
    private var panelOpen = false
    private var wakePending = false
    private var sleeper: Task<Void, Never>?

    public init(client: DaemonClient) {
        self.client = client
    }

    /// Switches between the open and closed cadence; opening also polls at once.
    public func setPanelOpen(_ open: Bool) {
        panelOpen = open
        if open {
            pollNow()
        }
    }

    /// Polls at once and restarts the backoff. A wake that arrives mid-poll ends the next wait
    /// instead of being lost.
    public func pollNow() {
        schedule.reset()
        wakePending = true
        sleeper?.cancel()
    }

    /// Polls until the calling task is cancelled.
    public func run() async {
        do {
            while true {
                let delay = try await pollOnce()
                try await wait(delay)
            }
        } catch {
            // Only cancellation ends the loop.
        }
    }

    /// - Returns: How long to wait before the next poll.
    /// - Throws: Only `CancellationError`.
    @discardableResult
    public func pollOnce() async throws -> Duration {
        let result = try await client.poll()
        let delay = schedule.next(panelOpen: panelOpen, status: result.status)
        if let state = result.state {
            lastKnown = state
        }
        latest = result
        onUpdate?(result)
        return delay
    }

    private func wait(_ delay: Duration) async throws {
        if !wakePending {
            let sleeper = Task {
                _ = try? await Task.sleep(for: delay)
            }
            self.sleeper = sleeper
            await withTaskCancellationHandler {
                await sleeper.value
            } onCancel: {
                sleeper.cancel()
            }
            self.sleeper = nil
        }
        wakePending = false
        try Task.checkCancellation()
    }
}
