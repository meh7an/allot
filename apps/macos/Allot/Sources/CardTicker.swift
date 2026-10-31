import AllotKit

/// Rebuilds the snapshot between polls: every second while the panel is open, so times tick,
/// and whenever a Kill? countdown second or a Stopped hold is due, open or closed. Each rebuild
/// times the next one, so a poll that arrives first pushes the tick back and a second never
/// costs two updates.
@MainActor
final class CardTicker {
    private static let tick = Duration.seconds(1)

    // A few milliseconds late, so the second or the hold has really passed.
    private static let slack = Duration.milliseconds(15)

    private let model: PanelModel
    private var task: Task<Void, Never>?

    var isOpen = false {
        didSet {
            reschedule()
        }
    }

    init(model: PanelModel) {
        self.model = model
    }

    func reschedule() {
        task?.cancel()
        let stop = model.nextStopChange.map { $0 + Self.slack }
        let delays = [stop, isOpen ? Self.tick : nil].compactMap { $0 }
        guard let delay = delays.min() else {
            task = nil
            return
        }
        task = Task { [weak self] in
            try? await Task.sleep(for: delay)
            guard !Task.isCancelled else {
                return
            }
            self?.model.refresh()
        }
    }
}
