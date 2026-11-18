import AppKit
import SwiftUI

/// The spike app: a status item that toggles the stand-in panel, like Allot.app, and one Settings
/// window. Opening the app again (`open -b tech.mehran.allot.settings-spike`) also opens
/// Settings, the path a future `allot settings` would take.
@main
@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    private let options = SpikeOptions()
    private var statusItem: NSStatusItem?
    private var panel: StandInPanel?
    private var settings: SettingsWindowController?

    static func main() {
        let app = NSApplication.shared
        let delegate = AppDelegate()
        app.delegate = delegate
        app.run()
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        SpikeLog.write("started, macOS \(ProcessInfo.processInfo.operatingSystemVersionString), policy \(NSApp.activationPolicy().name)")
        settings = SettingsWindowController(options: options)
        panel = StandInPanel(
            content: PanelControls(
                options: options,
                openSettings: { [weak self] in self?.openSettings(from: "panel button") },
                openLater: { [weak self] in self?.openLater() },
                quit: { NSApp.terminate(nil) }))

        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        item.button?.image = NSImage(systemSymbolName: "gearshape.2", accessibilityDescription: "Allot spike")
        item.button?.target = self
        item.button?.action = #selector(togglePanel)
        statusItem = item

        applyMainMenu()
        withObservationTracking {
            _ = options.installsMainMenu
        } onChange: {
            Task { @MainActor [weak self] in self?.observeMainMenuSwitch() }
        }
    }

    /// The Dock icon (while it shows) or a second `open` of the app.
    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        openSettings(from: "reopen (Dock or open -b)")
        return false
    }

    /// Closing Settings must never quit a menu bar app.
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        false
    }

    private func observeMainMenuSwitch() {
        applyMainMenu()
        withObservationTracking {
            _ = options.installsMainMenu
        } onChange: {
            Task { @MainActor [weak self] in self?.observeMainMenuSwitch() }
        }
    }

    @objc private func togglePanel() {
        guard let panel, let button = statusItem?.button, let buttonWindow = button.window else {
            return
        }

        if panel.isVisible {
            panel.orderOut(nil)
            return
        }

        let anchor = buttonWindow.convertToScreen(button.convert(button.bounds, to: nil))
        panel.setFrameTopLeftPoint(NSPoint(x: anchor.midX - panel.frame.width / 2, y: anchor.minY - 6))
        panel.orderFrontRegardless()
        panel.makeKey()
    }

    private func openSettings(from source: String) {
        // As the real flyout closes when an item is chosen.
        panel?.orderOut(nil)
        settings?.show(from: source)
    }

    private func openLater() {
        SpikeLog.write("opening Settings in 5 s; click into another app now")
        panel?.orderOut(nil)
        Task { @MainActor [weak self] in
            try? await Task.sleep(for: .seconds(5))
            self?.openSettings(from: "timer, 5 s later")
        }
    }

    /// An app menu with Settings and Quit, File > Close (Cmd-W) and an Edit menu. It never shows
    /// while the app is an accessory, but its key equivalents still work in the key window.
    private func applyMainMenu() {
        guard options.installsMainMenu else {
            NSApp.mainMenu = nil
            SpikeLog.write("main menu removed")
            return
        }

        let main = NSMenu()
        let appMenu = NSMenu()
        appMenu.addItem(withTitle: "Settings…", action: #selector(openSettingsFromMenu), keyEquivalent: ",").target = self
        appMenu.addItem(.separator())
        appMenu.addItem(withTitle: "Quit Allot Settings Spike", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
        main.addItem(submenu: appMenu, title: "Allot Settings Spike")

        let fileMenu = NSMenu(title: "File")
        fileMenu.addItem(withTitle: "Close", action: #selector(NSWindow.performClose(_:)), keyEquivalent: "w")
        main.addItem(submenu: fileMenu, title: "File")

        let editMenu = NSMenu(title: "Edit")
        editMenu.addItem(withTitle: "Undo", action: Selector(("undo:")), keyEquivalent: "z")
        editMenu.addItem(withTitle: "Redo", action: Selector(("redo:")), keyEquivalent: "Z")
        editMenu.addItem(.separator())
        editMenu.addItem(withTitle: "Cut", action: #selector(NSText.cut(_:)), keyEquivalent: "x")
        editMenu.addItem(withTitle: "Copy", action: #selector(NSText.copy(_:)), keyEquivalent: "c")
        editMenu.addItem(withTitle: "Paste", action: #selector(NSText.paste(_:)), keyEquivalent: "v")
        editMenu.addItem(withTitle: "Select All", action: #selector(NSText.selectAll(_:)), keyEquivalent: "a")
        main.addItem(submenu: editMenu, title: "Edit")

        NSApp.mainMenu = main
        SpikeLog.write("main menu installed")
    }

    @objc private func openSettingsFromMenu() {
        openSettings(from: "Cmd-, in the main menu")
    }
}

private extension NSMenu {
    func addItem(submenu: NSMenu, title: String) {
        let item = NSMenuItem(title: title, action: nil, keyEquivalent: "")
        item.submenu = submenu
        addItem(item)
    }
}
