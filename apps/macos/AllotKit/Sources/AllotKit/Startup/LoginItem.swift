/// Whether the app opens at login, as System Settings › Login Items has it.
public enum LoginItemStatus: Sendable, Equatable {
    case on
    case off

    /// Registered, but turned off in Login Items; only System Settings can turn it back on.
    case needsApproval

    /// The app runs translocated (opened from Downloads with its quarantine flag), so a login
    /// item would point at a read-only copy that is gone after a restart.
    case translocated

    public var isOn: Bool {
        self == .on
    }

    /// The switch can change it here; the other two need the user elsewhere.
    public var canToggle: Bool {
        self == .on || self == .off
    }

    /// The line under the switch, or empty.
    public var hint: String {
        switch self {
        case .on, .off: ""
        case .needsApproval: "Turned off in System Settings"
        case .translocated: "Move Allot to Applications to launch at login"
        }
    }

    /// Gatekeeper runs a quarantined app opened from Downloads from a random read-only path.
    public static func isTranslocated(bundlePath: String) -> Bool {
        bundlePath.contains("/AppTranslocation/")
    }
}

/// The app's login item; the app registers itself through `SMAppService`, and the tests and
/// Debug builds use ``MemoryLoginItem``.
@MainActor
public protocol LoginItem: AnyObject {
    var status: LoginItemStatus { get }

    /// - Throws: Why macOS refused the change.
    func set(_ on: Bool) throws
}

/// A login item that lives only as long as the app: Debug builds never register the build
/// folder's app at login.
@MainActor
public final class MemoryLoginItem: LoginItem {
    public private(set) var status: LoginItemStatus

    public init(_ status: LoginItemStatus = .off) {
        self.status = status
    }

    public func set(_ on: Bool) throws {
        guard status.canToggle else {
            return
        }
        status = on ? .on : .off
    }
}
