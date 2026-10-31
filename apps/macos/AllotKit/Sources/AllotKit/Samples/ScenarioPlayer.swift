import AllotContract

/// Shows handoff scenarios in a panel model instead of polling allotd, for a Debug build's
/// `--scenario`. Kill and Remove go to a ``ScenarioStopper``; a job it ends drops out of the
/// shown state, as it would from allotd's.
@MainActor
public final class ScenarioPlayer {
    /// How long the down scenario's Start Service shows Starting… before the service is back.
    public static let startDelay = Duration.milliseconds(1400)

    private let model: PanelModel
    private var status = ServiceStatus.running
    private var state: UiState?
    private var starting: Task<Void, Never>?

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
    /// press and its Resources card set as the prototype sets it. The first-run scenario
    /// clears Done and the first showing, so the card and the panel's own opening come back.
    ///
    /// - Throws: `CocoaError` for an unknown name.
    public func show(_ name: String) throws {
        starting?.cancel()
        (status, state) = try Handoff.scenario(name)
        model.stopper = ScenarioStopper(scenario: name)
        model.frozenStops = Handoff.stops(name)
        model.updateSettings { settings in
            if let preset = Handoff.preset(name) {
                settings.resourcesExpanded = preset.expanded
                if let meters = preset.meters {
                    settings.meters = meters
                }
            }
            if name == "first-run" {
                settings.firstRunDone = false
                settings.firstRunShown = false
            }
        }
        apply()
    }

    /// Start Service: Starting… for ``startDelay``, then the service is back with
    /// ``Handoff/startedScenario``.
    public func startService() {
        status = .starting
        apply()
        starting = Task { [weak self] in
            try? await Task.sleep(for: Self.startDelay)
            guard !Task.isCancelled else {
                return
            }
            try? self?.show(Handoff.startedScenario)
        }
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
