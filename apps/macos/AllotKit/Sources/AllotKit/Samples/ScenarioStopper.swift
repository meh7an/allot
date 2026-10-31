/// The Debug scenarios' allotd: a Kill or Remove ends the job after 300 ms, except in
/// ``Handoff/refusedScenario``, where allotd refuses it.
public actor ScenarioStopper: JobStopper {
    private static let delay = Duration.milliseconds(300)

    private let refuses: Bool
    private let startedAt: ContinuousClock.Instant

    /// - Parameters:
    ///   - scenario: The scenario on show.
    ///   - startedAt: When it was put on show; its state's moment, ``Handoff/nowMs``.
    public init(scenario: String, startedAt: ContinuousClock.Instant = .now) {
        refuses = scenario == Handoff.refusedScenario
        self.startedAt = startedAt
    }

    public func kill(_ lease: Int64) async throws -> StopResult {
        try await answer()
    }

    public func remove(_ lease: Int64) async throws -> StopResult {
        try await answer()
    }

    private func answer() async throws -> StopResult {
        try await Task.sleep(for: Self.delay)
        guard !refuses else {
            return .refused(Handoff.refusedReason)
        }
        let elapsed = (ContinuousClock.now - startedAt) / .milliseconds(1)
        return StopResult(
            ended: true, text: "", endedAtMs: Handoff.nowMs + Int64(elapsed.rounded(.down)))
    }
}
