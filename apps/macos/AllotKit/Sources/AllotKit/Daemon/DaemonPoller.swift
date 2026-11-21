import AllotContract

/// Polls the daemon on the ``PollSchedule`` and keeps the last state it served, which the
/// service-down view shows dimmed. Start Service launches allotd and reports it as starting
/// until it answers or ``startTimeout`` passes.
@MainActor
public final class DaemonPoller {
    /// How long Start Service reports ``ServiceStatus/starting`` before giving up.
    public static let startTimeout = Duration.seconds(10)

    /// How long a restart waits for the old allotd to stop answering.
    public static let stopTimeout = Duration.seconds(10)

    private static let stopCheck = Duration.milliseconds(200)

    /// Called after every poll.
    public var onUpdate: ((PollResult) -> Void)?

    public private(set) var latest: PollResult?

    /// The newest state allotd served, kept while it is down.
    public private(set) var lastKnown: UiState?

    /// Why the last Start Service failed: the launch itself, or allotd not answering within
    /// ``startTimeout``. Cleared by the next start and once allotd answers.
    public private(set) var startProblem: String?

    /// The port this panel dials, which differs from `daemon.port` until allotd restarts. The
    /// client holds the same value; it is kept here too so the main actor reads it without a
    /// hop.
    public private(set) var port: UInt16

    private let client: DaemonClient
    private let launcher: (any ServiceLauncher)?
    private let now: () -> ContinuousClock.Instant
    private var schedule = PollSchedule()
    private var panelOpen = false
    private var wakePending = false
    private var sleeper: Task<Void, Never>?
    private var startingUntil: ContinuousClock.Instant?

    /// - Parameters:
    ///   - client: Polls allotd and asks it to stop.
    ///   - port: The port `client` dials.
    ///   - launcher: Starts allotd for Start Service; without one, Start Service only polls.
    ///   - now: The local clock of the starting window; the tests pass their own.
    public init(
        client: DaemonClient,
        port: UInt16 = DaemonConfig.defaultPort,
        launcher: (any ServiceLauncher)? = nil,
        now: @escaping () -> ContinuousClock.Instant = { .now }
    ) {
        self.client = client
        self.port = port
        self.launcher = launcher
        self.now = now
    }

    /// Switches between the open and closed cadence; opening also polls at once.
    public func setPanelOpen(_ open: Bool) {
        panelOpen = open
        if open {
            pollNow()
        }
    }

    /// Launches allotd and polls quickly until it answers or ``startTimeout`` passes.
    ///
    /// - Returns: `nil` when allotd was launched, else why not.
    @discardableResult
    public func startService() async -> String? {
        let problem = await launcher?.start()
        // A failed launch polls too, so whoever shows the poll shows the problem at once.
        startProblem = problem
        if problem == nil {
            startingUntil = now() + Self.startTimeout
        }
        pollNow()
        return problem
    }

    /// Asks allotd to stop, then polls at once so the icon and panel show it stopped.
    ///
    /// - Returns: `nil` when allotd accepted, else why not.
    @discardableResult
    public func stopService() async -> String? {
        let problem: String?
        do {
            problem = try await client.shutdown()
        } catch {
            return "the stop was cancelled"
        }
        pollNow()
        return problem
    }

    /// Restarts allotd so a new port or token takes effect: stops it without a stop record,
    /// waits until it no longer answers, then starts it and polls it on `port`.
    ///
    /// - Parameter port: The port `config.toml` names now.
    /// - Returns: `nil` when allotd stopped and was launched again, else why not.
    @discardableResult
    public func restartService(port: UInt16) async -> String? {
        let problem: String?
        do {
            problem = try await client.shutdown(record: false)
        } catch {
            return "the restart was cancelled"
        }
        if let problem {
            pollNow()
            return problem
        }
        // A new allotd started while the old one still holds its instance lock exits at once.
        let until = now() + Self.stopTimeout
        while (try? await client.health()) != nil {
            if now() >= until {
                pollNow()
                return "the service did not stop within \(Self.stopTimeout.components.seconds) s"
            }
            guard (try? await Task.sleep(for: Self.stopCheck)) != nil else {
                return "the restart was cancelled"
            }
        }
        await client.usePort(port)
        self.port = port
        return await startService()
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
        let result = whileStarting(try await client.poll())
        let delay = schedule.next(panelOpen: panelOpen, status: result.status)
        if let state = result.state {
            lastKnown = state
        }
        latest = result
        onUpdate?(result)
        return delay
    }

    // A launched allotd takes a moment to bind its port; until then it looks stopped.
    private func whileStarting(_ result: PollResult) -> PollResult {
        guard result.status == .stopped else {
            startingUntil = nil
            startProblem = nil
            return result
        }
        guard let until = startingUntil else {
            return result
        }
        if now() < until {
            return PollResult(
                status: .starting, version: result.version, detail: result.detail)
        }
        startingUntil = nil
        let detail = "allotd did not answer within \(Self.startTimeout.components.seconds) s; "
            + "see logs/allotd.log"
        startProblem = detail
        return PollResult(status: .stopped, version: result.version, detail: detail)
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
