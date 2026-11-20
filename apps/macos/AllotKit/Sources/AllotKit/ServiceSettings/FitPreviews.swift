import AllotContract
import Observation

/// allotd's fit preview for the values on screen, unsaved edits included: asked 600 ms after the
/// session last changed, while a page that shows it is open in a visible window.
@MainActor
@Observable
public final class FitPreviews {
    /// Why a live part shows nothing yet, before the first preview arrives.
    public static let notYet = "Can't work this out yet. Trying again in a moment."

    /// The latest preview, or `nil` before the first one.
    public private(set) var latest: FitPreview?

    private let session: SettingsSession
    private let timers: any SettingsTimers
    @ObservationIgnored private var debounce: (any ScheduledTimer)?
    @ObservationIgnored private var request = 0
    @ObservationIgnored private var active = false

    public init(session: SettingsSession, timers: any SettingsTimers) {
        self.session = session
        self.timers = timers
    }

    /// Why no preview can be shown now, or `nil` when ``latest`` applies.
    public var unavailable: String? {
        switch session.status {
        case .ready, .loading: nil
        default: LiveText.serviceStopped
        }
    }

    /// The fit of a class or preset by name, or `nil` while none is known.
    ///
    /// - Parameter table: `classes` or `presets`.
    public func fit(_ table: String, _ name: String) -> SizeFit? {
        guard unavailable == nil, let latest else {
            return nil
        }
        let fits = table == "classes" ? latest.classes : latest.presets
        return fits.first { $0.name == name }
    }

    /// Starts or stops previewing; starting asks at once.
    public func setActive(_ active: Bool) {
        self.active = active
        cancelDebounce()
        if active {
            Task { await ask() }
        }
    }

    /// The session changed: asks again once it has been quiet for the save delay.
    public func sessionChanged() {
        guard active else {
            return
        }
        cancelDebounce()
        debounce = timers.schedule(after: SettingsSession.saveDelay) { [weak self] in
            guard let self else {
                return
            }
            debounce = nil
            Task { await self.ask() }
        }
    }

    /// Asks allotd now; an answer to an older request is dropped.
    public func ask() async {
        guard active, session.status == .ready else {
            return
        }
        request += 1
        let asked = request
        guard let reply = try? await session.preview(), asked == request else {
            return
        }
        if case .value(let preview) = reply {
            latest = preview
        }
    }

    private func cancelDebounce() {
        debounce?.cancel()
        debounce = nil
    }
}
