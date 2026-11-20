import AllotKit
import AppKit
import SwiftUI

/// The one Settings window for the app's lifetime: an `NSWindow` with toolbar tabs (DESIGN.md
/// D8), each page a SwiftUI view. Closing hides it (D14), so the page, scroll and unsaved edits
/// survive; values are fetched again on every show. The window fits each page's height, so only
/// its position is remembered.
///
/// While it is open the app shows in the Dock and Cmd-Tab, then goes back to a menu bar app
/// (M0.2's spike default; its checklist decides, DESIGN.md R4).
@MainActor
final class SettingsWindowController: NSObject, NSWindowDelegate {
    private static let topLeftKey = "SettingsWindowTopLeft"

    private let model: SettingsWindowModel
    private let window: NSWindow
    private let tabs = SettingsTabController()

    init(model: SettingsWindowModel) {
        self.model = model
        window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: SettingsPage.width, height: 400),
            styleMask: [.titled, .closable, .miniaturizable],
            backing: .buffered,
            defer: false)
        super.init()
        window.title = "Allot Settings"
        window.toolbarStyle = .preference
        // The controller keeps the window; closing must only hide it.
        window.isReleasedWhenClosed = false
        window.delegate = self
        model.window = window

        tabs.tabStyle = .toolbar
        tabs.canPropagateSelectedChildViewControllerTitle = false
        for page in SettingsWindowModel.pages {
            let host = NSHostingController(rootView: SettingsPage(model: model, pageId: page.id))
            host.sizingOptions = [.preferredContentSize]
            let item = NSTabViewItem(viewController: host)
            item.identifier = page.id
            item.label = page.label
            item.image = NSImage(systemSymbolName: page.symbol, accessibilityDescription: nil)
            tabs.addTabViewItem(item)
        }
        tabs.onSelect = { [weak self] id in
            self?.model.selectPage(id)
        }
        tabs.onResize = { [weak self] size in
            self?.fit(size)
        }
        window.contentViewController = tabs
        place()
    }

    var isOpen: Bool {
        window.isVisible
    }

    /// Brings the one window to the front with keyboard focus, on `pageId` or the page shown
    /// last.
    func show(page pageId: String? = nil) {
        if let pageId, let index = tabs.tabViewItems.firstIndex(where: {
            $0.identifier as? String == pageId
        }) {
            tabs.selectedTabViewItemIndex = index
        }
        if NSApp.activationPolicy() != .regular {
            NSApp.setActivationPolicy(.regular)
        }
        if window.isMiniaturized {
            window.deminiaturize(nil)
        }
        let wasVisible = window.isVisible
        window.makeKeyAndOrderFront(nil)
        NSApp.activate()
        if !wasVisible {
            model.setVisible(true)
        }
    }

    // MARK: NSWindowDelegate

    func windowWillClose(_ notification: Notification) {
        // Closing with a dialog open cancels the dialog.
        if let sheet = window.attachedSheet {
            window.endSheet(sheet, returnCode: .cancel)
        }
        model.setVisible(false)
        // Switching back while the window is still on screen can hide it with the app; do it
        // after the close finishes.
        // A reopen before then keeps the Dock icon.
        Task { @MainActor [window] in
            if !window.isVisible {
                NSApp.setActivationPolicy(.accessory)
            }
        }
    }

    func windowDidBecomeKey(_ notification: Notification) {
        model.focused()
    }

    func windowDidMove(_ notification: Notification) {
        let frame = window.frame
        UserDefaults.standard.set(
            NSStringFromPoint(NSPoint(x: frame.minX, y: frame.maxY)), forKey: Self.topLeftKey)
    }

    // MARK: Private

    // The saved top-left corner, kept on a screen; centred without one.
    private func place() {
        guard let saved = UserDefaults.standard.string(forKey: Self.topLeftKey) else {
            window.center()
            return
        }
        let topLeft = NSPointFromString(saved)
        let onScreen = NSScreen.screens.contains { $0.visibleFrame.contains(topLeft) }
        if onScreen {
            window.setFrameTopLeftPoint(topLeft)
        } else {
            window.center()
        }
    }

    // Fits the window to the page, keeping its top edge, and on its screen.
    private func fit(_ contentSize: NSSize) {
        guard contentSize.width > 0, contentSize.height > 0 else {
            return
        }
        let current = window.frame
        var frame = window.frameRect(forContentRect: NSRect(origin: .zero, size: contentSize))
        frame.origin = NSPoint(x: current.minX, y: current.maxY - frame.height)
        if let visible = (window.screen ?? NSScreen.main)?.visibleFrame {
            if frame.minY < visible.minY {
                frame.origin.y = visible.minY
            }
            // The wider Sizes page grows to the right; near the screen's edge it moves left.
            if frame.maxX > visible.maxX {
                frame.origin.x = max(visible.minX, visible.maxX - frame.width)
            }
        }
        if frame != current {
            window.setFrame(frame, display: true, animate: window.isVisible)
        }
    }
}

/// The toolbar tabs, which report the shown page and its size so the window can follow.
@MainActor
private final class SettingsTabController: NSTabViewController {
    var onSelect: ((String) -> Void)?
    var onResize: ((NSSize) -> Void)?

    override func tabView(_ tabView: NSTabView, didSelect tabViewItem: NSTabViewItem?) {
        super.tabView(tabView, didSelect: tabViewItem)
        guard let tabViewItem else {
            return
        }
        if let id = tabViewItem.identifier as? String {
            onSelect?(id)
        }
        if let child = tabViewItem.viewController {
            onResize?(child.preferredContentSize)
        }
    }

    override func preferredContentSizeDidChange(for viewController: NSViewController) {
        super.preferredContentSizeDidChange(for: viewController)
        let selected = tabViewItems.indices.contains(selectedTabViewItemIndex)
            ? tabViewItems[selectedTabViewItemIndex].viewController : nil
        if viewController === selected {
            onResize?(viewController.preferredContentSize)
        }
    }
}
