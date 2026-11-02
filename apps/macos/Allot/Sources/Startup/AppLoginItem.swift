import AllotKit
import Foundation
import ServiceManagement

/// The app itself as a login item, through `SMAppService.mainApp`. macOS shows its own
/// "Background item added" notice the first time it is registered.
@MainActor
final class AppLoginItem: LoginItem {
    /// Called when a background read finds a new status.
    var onChange: (() -> Void)?

    private var known = AppLoginItem.read()

    /// The status as last read, and starts a new read: asking macOS takes about 20 ms, too long
    /// for the main thread while the panel opens.
    var status: LoginItemStatus {
        Task.detached(priority: .userInitiated) { [weak self] in
            let status = Self.read()
            await self?.show(status)
        }
        return known
    }

    func set(_ on: Bool) throws {
        guard known.canToggle else {
            return
        }
        if on {
            try SMAppService.mainApp.register()
        } else {
            try SMAppService.mainApp.unregister()
        }
        known = Self.read()
    }

    static func openSettings() {
        SMAppService.openSystemSettingsLoginItems()
    }

    private func show(_ status: LoginItemStatus) {
        if status != known {
            known = status
            onChange?()
        }
    }

    private nonisolated static func read() -> LoginItemStatus {
        if LoginItemStatus.isTranslocated(bundlePath: Bundle.main.bundlePath) {
            return .translocated
        }
        return switch SMAppService.mainApp.status {
        case .enabled: .on
        case .requiresApproval: .needsApproval
        case .notRegistered, .notFound: .off
        @unknown default: .off
        }
    }
}
