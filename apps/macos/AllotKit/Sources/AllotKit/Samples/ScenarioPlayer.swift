import AllotContract

/// Shows handoff scenarios in a panel model instead of polling allotd, for a Debug build's
/// `--scenario`. Kill and Remove go to a ``ScenarioStopper``; a job it ends drops out of the
/// shown state, as it would from allotd's.
@MainActor
public final class ScenarioPlayer {
    private let model: PanelModel
    private var status = ServiceStatus.running
    private var state: UiState?

    public init(model: PanelModel) {
        self.model = model
        model.onStopAnswered = { [weak self] lease, _, result in
            if result.ended {
                self?.drop(lease)
            }
        }
    }

    /// The name after `--scenario` in `arguments`, or `nil` without one.
    public static func requested(in arguments: [String]) -> String? {
        guard let flag = arguments.firstIndex(of: "--scenario"), flag + 1 < arguments.count
        else {
            return nil
        }
        return arguments[flag + 1]
    }

    /// Shows a scenario from ``Handoff/scenarioNames``, with its stops frozen until the first
    /// press.
    ///
    /// - Throws: `CocoaError` for an unknown name.
    public func show(_ name: String) throws {
        (status, state) = try Handoff.scenario(name)
        model.stopper = ScenarioStopper(scenario: name)
        model.frozenStops = Handoff.stops(name)
        apply()
    }

    private func drop(_ lease: Int64) {
        guard let state, let without = try? Handoff.without(lease, in: state) else {
            return
        }
        self.state = without
        apply()
    }

    private func apply() {
        let live = status == .running ? state : nil
        model.apply(PollResult(status: status, state: live), lastKnown: state)
    }
}
