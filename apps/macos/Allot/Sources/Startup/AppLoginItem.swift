import AllotKit
import Foundation
import ServiceManagement

/// The app itself as a login item, through `SMAppService.mainApp`. macOS shows its own
/// "Background item added" notice the first time it is registered.
@MainActor
final class AppLoginItem: LoginItem {
    var status: LoginItemStatus {
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

    func set(_ on: Bool) throws {
        guard status.canToggle else {
            return
        }
        if on {
            try SMAppService.mainApp.register()
        } else {
            try SMAppService.mainApp.unregister()
        }
    }

    static func openSettings() {
        SMAppService.openSystemSettingsLoginItems()
    }
}
