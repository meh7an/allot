import AllotContract

/// The daemon's time between polls: the latest state's `generated_at_ms` plus the time since it
/// arrived, so that elapsed and waited times tick every second without a new poll and without
/// trusting the local clock to match allotd's.
public final class StateClock {
    private let now: () -> ContinuousClock.Instant
    private var generatedAtMs: Int64 = 0
    private var receivedAt: ContinuousClock.Instant?

    /// - Parameter now: The local clock; the tests pass their own.
    public init(now: @escaping () -> ContinuousClock.Instant = { .now }) {
        self.now = now
    }

    /// The current moment in the daemon's clock, or `nil` while no live state is known.
    public var nowMs: Int64? {
        guard let receivedAt else {
            return nil
        }
        let since = (now() - receivedAt) / .milliseconds(1)
        return generatedAtMs + max(0, Int64(since.rounded(.down)))
    }

    /// Takes the state of a poll; `nil` stops the clock, as last known times stand still.
    public func received(_ state: UiState?) {
        guard let state else {
            receivedAt = nil
            return
        }
        generatedAtMs = state.generatedAtMs
        receivedAt = now()
    }
}
