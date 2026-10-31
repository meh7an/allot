import AllotKit
import AppKit
import Observation

/// What the right-click menu's service and login items act on; the app delegate supplies it.
@MainActor
struct ServiceControls {
    /// The service status of the latest poll.
    let status: () -> ServiceStatus

    /// The warning before Stop Service while jobs run or wait, or `nil` without any.
    let stopConfirmation: () -> String?

    let start: () -> Void
    let stop: () -> Void
    let setLaunchAtLogin: (Bool) -> Void
}

/// Owns the menu bar item: its icon follows the service and job state, a left click toggles the
/// panel and a right click shows a small menu.
@MainActor
final class StatusItemController: NSObject {
    private let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
    private let model: PanelModel
    private let panel: PanelController
    private let controls: ServiceControls?

    /// - Parameter controls: The service and login items; a Debug scenario has none.
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

    private func showMenu() {
        panel.close()
        model.refreshLogin()
        let menu = NSMenu()
        let summary = NSMenuItem(title: model.menuBar.summary, action: nil, keyEquivalent: "")
        summary.isEnabled = false
        menu.addItem(summary)
        menu.addItem(.separator())
        if let controls {
            let service = ServiceMenu.item(for: controls.status())
            let serviceItem = NSMenuItem(
                title: service.title, action: #selector(serviceChosen(_:)), keyEquivalent: "")
            serviceItem.target = self
            serviceItem.isEnabled = service.isEnabled
            serviceItem.representedObject = service.action == .stop
            menu.addItem(serviceItem)
            let login = NSMenuItem(
                title: "Launch at Login", action: #selector(toggleLogin), keyEquivalent: "")
            login.target = self
            login.state = model.login.isOn ? .on : .off
            login.isEnabled = model.login.canToggle
            menu.addItem(login)
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

    @objc private func serviceChosen(_ sender: NSMenuItem) {
        guard let controls else {
            return
        }
        guard sender.representedObject as? Bool == true else {
            controls.start()
            return
        }
        if let text = controls.stopConfirmation(), !confirmStop(text) {
            return
        }
        controls.stop()
    }

    private func confirmStop(_ text: String) -> Bool {
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

    @objc private func toggleLogin() {
        controls?.setLaunchAtLogin(!model.login.isOn)
    }

    @objc private func openLogs() {
        panel.openLogs()
    }
}
