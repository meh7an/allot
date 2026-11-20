import AllotKit
import AppKit
import Observation

/// What the menus' Settings and service items act on; the app delegate supplies it.
@MainActor
struct ServiceControls {
    /// The service status of the latest poll.
    let status: () -> ServiceStatus

    /// The warning before Stop Service while jobs run or wait, or `nil` without any.
    let stopConfirmation: () -> String?

    let start: () -> Void
    let stop: () -> Void
    let openSettings: () -> Void

    /// Stop or Start Service, by the latest poll; Stop asks first while jobs run or wait.
    func chooseService() {
        switch ServiceMenu.item(for: status()).action {
        case .stop:
            if let text = stopConfirmation(), !Self.confirmStop(text) {
                return
            }
            stop()
        case .start:
            start()
        case .none:
            break
        }
    }

    private static func confirmStop(_ text: String) -> Bool {
        let alert = NSAlert()
        alert.alertStyle = .warning
        alert.messageText = ServiceMenu.confirmTitle
        alert.informativeText = text
        alert.addButton(withTitle: "Stop Service")
        alert.addButton(withTitle: "Cancel")
        // A menu bar app is never frontmost by itself, and the alert must not open behind.
        NSApp.activate()
        return alert.runModal() == .alertFirstButtonReturn
    }
}

/// Owns the menu bar item: its icon follows the service and job state, a left click toggles the
/// panel and a right click shows a small menu.
@MainActor
final class StatusItemController: NSObject {
    private let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
    private let model: PanelModel
    private let panel: PanelController
    private let controls: ServiceControls?

    /// - Parameter controls: The Settings and service items; a Debug scenario has none.
    init(model: PanelModel, panel: PanelController, controls: ServiceControls? = nil) {
        self.model = model
        self.panel = panel
        self.controls = controls
        super.init()

        guard let button = item.button else {
            return
        }
        button.target = self
        button.action = #selector(clicked(_:))
        button.sendAction(on: [.leftMouseDown, .rightMouseDown])
        panel.statusButton = button
        observe()
    }

    // Re-arms itself after every change, since observation tracking fires once.
    private func observe() {
        withObservationTracking {
            render(model.menuBar)
        } onChange: { [weak self] in
            Task { @MainActor in self?.observe() }
        }
    }

    private func render(_ status: MenuBarStatus) {
        guard let button = item.button else {
            return
        }
        let image = NSImage(named: status.icon.assetName)
        image?.isTemplate = true
        button.image = image
        button.toolTip = status.tooltip
        button.setAccessibilityLabel(status.tooltip)
    }

    @objc private func clicked(_ sender: NSStatusBarButton) {
        if NSApp.currentEvent?.type == .rightMouseDown {
            showMenu()
        } else {
            panel.toggle()
        }
    }

    // The shrunk menu of DESIGN.md section 9: Settings, Stop or Start Service, Open Logs, Quit.
    private func showMenu() {
        panel.close()
        let menu = NSMenu()
        let summary = NSMenuItem(title: model.menuBar.summary, action: nil, keyEquivalent: "")
        summary.isEnabled = false
        menu.addItem(summary)
        menu.addItem(.separator())
        if let controls {
            let settings = NSMenuItem(
                title: ServiceMenu.settingsTitle, action: #selector(openSettings),
                keyEquivalent: ",")
            settings.target = self
            menu.addItem(settings)
            let service = ServiceMenu.item(for: controls.status())
            let serviceItem = NSMenuItem(
                title: service.title, action: #selector(serviceChosen), keyEquivalent: "")
            serviceItem.target = self
            serviceItem.isEnabled = service.isEnabled
            menu.addItem(serviceItem)
            menu.addItem(.separator())
        }
        let logs = NSMenuItem(title: "Open Logs", action: #selector(openLogs), keyEquivalent: "")
        logs.target = self
        menu.addItem(logs)
        menu.addItem(.separator())
        menu.addItem(
            withTitle: "Quit Allot", action: #selector(NSApplication.terminate(_:)),
            keyEquivalent: "q")
        // A menu set on the item opens on the next click; this one shows now and is removed so
        // that a left click keeps toggling the panel.
        item.menu = menu
        item.button?.performClick(nil)
        item.menu = nil
    }

    @objc private func serviceChosen() {
        controls?.chooseService()
    }

    @objc private func openSettings() {
        controls?.openSettings()
    }

    @objc private func openLogs() {
        panel.openLogs()
    }
}
