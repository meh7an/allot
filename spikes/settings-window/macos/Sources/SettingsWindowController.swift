import AppKit
import SwiftUI

/// One Settings window for the app's lifetime: an `NSWindow` hosting SwiftUI, shown again on
/// every open (never a second copy), with its frame remembered by `setFrameAutosaveName`. The
/// SwiftUI `Settings` scene needs a SwiftUI `App`, which the real menu bar app does not have.
@MainActor
final class SettingsWindowController: NSWindowController, NSWindowDelegate {
    private static let autosaveName = "AllotSettingsSpike"

    private let options: SpikeOptions
    private let diagnostics: WindowDiagnostics
    private var opens = 0

    init(options: SpikeOptions) {
        self.options = options
        // Built before super.init, which the hosting view needs, so not read from self.
        let diagnostics = WindowDiagnostics()
        self.diagnostics = diagnostics
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 720, height: 480),
            styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView],
            backing: .buffered,
            defer: false)
        window.title = "Allot Settings (spike)"
        window.titlebarAppearsTransparent = false
        window.toolbarStyle = .unified
        window.minSize = NSSize(width: 560, height: 360)
        // The controller keeps the window; closing must not free it.
        window.isReleasedWhenClosed = false
        let host = NSHostingController(rootView: SettingsView(diagnostics: diagnostics))
        // The hosting controller would otherwise resize the window to the view's ideal size
        // and fight the restored frame.
        host.sizingOptions = []
        window.contentViewController = host
        super.init(window: window)
        window.delegate = self
        // Set after the content, so the content's first layout does not overwrite the saved
        // frame; centre only when there is none.
        if !window.setFrameUsingName(Self.autosaveName) {
            window.center()
        }
        window.setFrameAutosaveName(Self.autosaveName)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("not used")
    }

    var isOpen: Bool {
        window?.isVisible == true
    }

    /// Brings the one window to the front with keyboard focus, from wherever it was asked for.
    func show(from source: String) {
        guard let window else {
            return
        }

        opens += 1
        let state = window.isVisible ? (window.isMiniaturized ? "minimized" : "already open") : "hidden"
        SpikeLog.write(
            "open #\(opens) from \(source): window \(state), app active \(NSApp.isActive), "
                + "policy \(NSApp.activationPolicy().name), order \(options.activationOrder.rawValue)")
        if options.dockPolicy == .regularWhileOpen, NSApp.activationPolicy() != .regular {
            let switched = NSApp.setActivationPolicy(.regular)
            SpikeLog.write("setActivationPolicy(.regular) returned \(switched)")
        }

        if window.isMiniaturized {
            window.deminiaturize(nil)
        }

        switch options.activationOrder {
        case .orderThenActivate:
            window.makeKeyAndOrderFront(nil)
            NSApp.activate()
        case .activateThenOrder:
            NSApp.activate()
            window.makeKeyAndOrderFront(nil)
        case .ignoringOtherApps:
            NSApp.activate(ignoringOtherApps: true)
            window.makeKeyAndOrderFront(nil)
        }

        report("right after")
        // Activation is asynchronous; look again once it had time to land.
        Task { @MainActor [weak self] in
            try? await Task.sleep(for: .milliseconds(300))
            self?.report("300 ms later")
        }
    }

    func windowWillClose(_ notification: Notification) {
        SpikeLog.write("Settings closing; frame \(NSStringFromRect(window?.frame ?? .zero))")
        guard options.dockPolicy == .regularWhileOpen else {
            return
        }

        // Switching back while the window is still on screen can hide it with the app; do it
        // after the close finishes.
        Task { @MainActor in
            let switched = NSApp.setActivationPolicy(.accessory)
            SpikeLog.write("setActivationPolicy(.accessory) returned \(switched); app active \(NSApp.isActive)")
        }
    }

    func windowDidBecomeKey(_ notification: Notification) {
        refresh()
    }

    func windowDidResignKey(_ notification: Notification) {
        refresh()
    }

    func windowDidChangeScreen(_ notification: Notification) {
        SpikeLog.write("Settings moved to screen \(window?.screen?.localizedName ?? "?") scale \(window?.backingScaleFactor ?? 0)")
        refresh()
    }

    func windowDidChangeBackingProperties(_ notification: Notification) {
        SpikeLog.write("Settings backing scale now \(window?.backingScaleFactor ?? 0)")
        refresh()
    }

    func windowDidEndLiveResize(_ notification: Notification) {
        refresh()
    }

    func windowDidMove(_ notification: Notification) {
        refresh()
    }

    private func report(_ when: String) {
        guard let window else {
            return
        }

        SpikeLog.write(
            "\(when): app active \(NSApp.isActive), key \(window.isKeyWindow), "
                + "main \(window.isMainWindow), visible \(window.isVisible), "
                + "frontmost app \(NSWorkspace.shared.frontmostApplication?.localizedName ?? "?")")
        refresh()
    }

    private func refresh() {
        guard let window else {
            return
        }

        diagnostics.text =
            "policy \(NSApp.activationPolicy().name), app active \(NSApp.isActive), key \(window.isKeyWindow)\n"
            + "frame \(NSStringFromRect(window.frame)) on \(window.screen?.localizedName ?? "?") "
            + "at scale \(window.backingScaleFactor)"
    }
}

/// The footer line the window shows about itself.
@MainActor
@Observable
final class WindowDiagnostics {
    var text = ""
}

extension NSApplication.ActivationPolicy {
    var name: String {
        switch self {
        case .regular: "regular"
        case .accessory: "accessory"
        case .prohibited: "prohibited"
        @unknown default: "unknown"
        }
    }
}
