import AllotKit
import AppKit
import Observation

/// Owns the menu bar item: its icon follows the service and job state, a left click toggles the
/// panel and a right click shows a small menu.
@MainActor
final class StatusItemController: NSObject {
    private let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
    private let model: PanelModel
    private let panel: PanelController

    init(model: PanelModel, panel: PanelController) {
        self.model = model
        self.panel = panel
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
        let menu = NSMenu()
        let summary = NSMenuItem(title: model.menuBar.summary, action: nil, keyEquivalent: "")
        summary.isEnabled = false
        menu.addItem(summary)
        menu.addItem(.separator())
        let logs = NSMenuItem(title: "Open Logs", action: #selector(openLogs), keyEquivalent: "")
        logs.target = self
        menu.addItem(logs)
        menu.addItem(
            withTitle: "Quit Allot", action: #selector(NSApplication.terminate(_:)),
            keyEquivalent: "q")
        // A menu set on the item opens on the next click; this one shows now and is removed so
        // that a left click keeps toggling the panel.
        item.menu = menu
        item.button?.performClick(nil)
        item.menu = nil
    }

    @objc private func openLogs() {
        panel.openLogs()
    }
}
