import Observation

/// What the Dock and Cmd-Tab do while the Settings window is open.
enum DockPolicy: String, CaseIterable, Identifiable {
    /// `.regular` while open, back to `.accessory` on close: the common practice.
    case regularWhileOpen = "Dock and Cmd-Tab while open"
    /// Stays `.accessory`: no Dock icon, not in Cmd-Tab.
    case stayAccessory = "Stay accessory"

    var id: Self { self }
}

/// The order of the calls that bring the window to the front.
enum ActivationOrder: String, CaseIterable, Identifiable {
    /// `makeKeyAndOrderFront`, then `NSApp.activate()` (cooperative, macOS 14+).
    case orderThenActivate = "makeKeyAndOrderFront, then activate()"
    /// `NSApp.activate()`, then `makeKeyAndOrderFront`.
    case activateThenOrder = "activate(), then makeKeyAndOrderFront"
    /// The deprecated `activate(ignoringOtherApps: true)`, then `makeKeyAndOrderFront`.
    case ignoringOtherApps = "activate(ignoringOtherApps:) (deprecated)"

    var id: Self { self }
}

@MainActor
@Observable
final class SpikeOptions {
    var dockPolicy = DockPolicy.regularWhileOpen
    var activationOrder = ActivationOrder.orderThenActivate
    /// Without a main menu, key equivalents such as Cmd-W, Cmd-Q, Cmd-C and Cmd-V have nothing
    /// to dispatch to; the checklist tries both.
    var installsMainMenu = true
}
