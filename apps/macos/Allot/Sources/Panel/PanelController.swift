import AllotKit
import AppKit
import SwiftUI

/// Shows the panel under the status item and implements light dismiss, Stay open and Pin.
///
/// Light dismiss needs three signals at once: resign key catches Cmd-Tab, a global mouse-down
/// monitor catches clicks in other apps, the desktop and the menu bar, and an app activating
/// catches some switches first.
@MainActor
final class PanelController: NSObject, NSWindowDelegate {
    private static let gap: CGFloat = 6
    private static let edgeMargin: CGFloat = 8
    private static let reopenGuard = Duration.milliseconds(300)

    weak var statusButton: NSStatusBarButton?

    /// Called when the panel opens or closes.
    var onOpenChanged: ((Bool) -> Void)?

    private let model: PanelModel
    private let logDirectory: URL
    private let layout = PanelLayout()
    private let panel = AllotPanel()
    private let effectView = NSVisualEffectView()
    private var hosting: NSHostingView<PanelView>?
    private var isShown = false
    private var showGeneration = 0
    private var contentHeight: CGFloat = 0
    private var clickMonitor: Any?
    private var activationObserver: NSObjectProtocol?
    private var lastOutsideClose: ContinuousClock.Instant?

    init(model: PanelModel, logDirectory: URL) {
        self.model = model
        self.logDirectory = logDirectory
        super.init()
        installContent()
    }

    // MARK: - Open and close

    func toggle() {
        if isShown {
            close()
            return
        }
        // On macOS 27 a click on the status item reaches the global monitor before the button,
        // which has already closed the panel; that click was the closing one.
        if let lastOutsideClose, ContinuousClock.now - lastOutsideClose < Self.reopenGuard {
            self.lastOutsideClose = nil
            return
        }
        open()
    }

    func open() {
        // Reopening always starts in light-dismiss mode, under the status item.
        model.resetForOpen()
        applyMode()
        layout.maxHeight = maxHeight(on: statusScreen)
        measureContent()

        let frame = anchoredFrame()
        showGeneration += 1
        isShown = true
        let reduceMotion = NSWorkspace.shared.accessibilityDisplayShouldReduceMotion
        panel.setFrame(reduceMotion ? frame : frame.offsetBy(dx: 0, dy: 8), display: false)
        panel.alphaValue = 0
        panel.orderFrontRegardless()
        panel.makeKey()
        NSAnimationContext.runAnimationGroup { context in
            context.duration = 0.167
            context.timingFunction = CAMediaTimingFunction(name: .easeOut)
            panel.animator().alphaValue = 1
            panel.animator().setFrame(frame, display: true)
        }
        installDismissMonitors()
        onOpenChanged?(true)
    }

    func close(outsideClick: Bool = false) {
        guard isShown else {
            return
        }
        isShown = false
        removeDismissMonitors()
        lastOutsideClose = outsideClick ? ContinuousClock.now : nil
        onOpenChanged?(false)

        showGeneration += 1
        let generation = showGeneration
        NSAnimationContext.runAnimationGroup { context in
            context.duration = 0.1
            panel.animator().alphaValue = 0
        } completionHandler: {
            MainActor.assumeIsolated {
                // A reopen during the fade owns the panel now.
                if generation == self.showGeneration {
                    self.panel.orderOut(nil)
                }
            }
        }
    }

    func openLogs() {
        NSWorkspace.shared.open(logDirectory)
    }

    // MARK: - Modes

    private func toggleStayOpen() {
        let wasPinned = model.mode.isPinned
        model.toggleStayOpen()
        applyMode()
        if wasPinned {
            returnUnderStatusItem()
        }
    }

    private func togglePinned() {
        let wasPinned = model.mode.isPinned
        model.togglePinned()
        applyMode()
        // Pinning starts where the panel is; unpinning returns it under the status item.
        if wasPinned && !model.mode.isPinned {
            returnUnderStatusItem()
        }
    }

    /// Light dismiss sits above app windows, Stay open is a normal window that others can
    /// cover, and Pin floats above app windows on every Space.
    private func applyMode() {
        switch model.mode {
        case .pinned:
            panel.level = .floating
            panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .ignoresCycle]
        case .stayOpen:
            panel.level = .normal
            panel.collectionBehavior = [.moveToActiveSpace, .fullScreenAuxiliary, .ignoresCycle]
        case .lightDismiss:
            panel.level = .statusBar
            panel.collectionBehavior = [.moveToActiveSpace, .fullScreenAuxiliary, .ignoresCycle]
        }
    }

    private func returnUnderStatusItem() {
        panel.setFrame(anchoredFrame(), display: true, animate: true)
    }

    // MARK: - Size and position

    /// Lays the content out before showing, so the first open already has its full height.
    private func measureContent() {
        guard let hosting else {
            return
        }
        hosting.layoutSubtreeIfNeeded()
        let height = hosting.fittingSize.height
        if height > 0 {
            contentHeight = height
        }
    }

    private func contentHeightChanged(_ height: CGFloat) {
        guard abs(height - contentHeight) > 0.5 else {
            return
        }
        contentHeight = height
        guard isShown else {
            return
        }

        // The top edge stays put. Through the animator, so it replaces an open animation still
        // running toward the old size.
        let frame = panel.frame
        let newHeight = min(height, layout.maxHeight)
        NSAnimationContext.runAnimationGroup { context in
            context.duration = 0.1
            panel.animator().setFrame(
                NSRect(x: frame.minX, y: frame.maxY - newHeight, width: frame.width,
                       height: newHeight),
                display: true)
        }
        panel.invalidateShadow()
    }

    private var statusScreen: NSScreen? {
        statusButton?.window?.screen ?? NSScreen.main
    }

    private func anchoredFrame() -> NSRect {
        let buttonRect = statusButtonScreenRect()
        let visible = statusScreen?.visibleFrame ?? NSRect(x: 0, y: 0, width: 1440, height: 900)
        let width = Tokens.panelWidth
        let height = min(contentHeight, layout.maxHeight)

        let anchorX = buttonRect?.midX ?? visible.maxX
        let anchorY = min(buttonRect?.minY ?? visible.maxY, visible.maxY)
        let x = min(
            max(anchorX - width / 2, visible.minX + Self.edgeMargin),
            visible.maxX - Self.edgeMargin - width)
        let top = anchorY - Self.gap
        return NSRect(x: x, y: top - height, width: width, height: height)
    }

    /// The handoff's 640, or 70% of the screen's visible height when that is less.
    private func maxHeight(on screen: NSScreen?) -> CGFloat {
        let visibleHeight = screen?.visibleFrame.height ?? Tokens.panelMaxHeight
        return min(Tokens.panelMaxHeight, visibleHeight * 0.7)
    }

    private func statusButtonScreenRect() -> NSRect? {
        guard let button = statusButton, let window = button.window else {
            return nil
        }
        return window.convertToScreen(button.convert(button.bounds, to: nil))
    }

    private var isOverStatusItem: Bool {
        statusButtonScreenRect()?.contains(NSEvent.mouseLocation) ?? false
    }

    // MARK: - Light dismiss

    private func cancel() {
        if !model.mode.isStayOpen {
            close()
        }
    }

    func windowDidResignKey(_ notification: Notification) {
        // A click on the status item closes the panel through toggle().
        if !model.mode.isStayOpen && !isOverStatusItem {
            close()
        }
    }

    private func installDismissMonitors() {
        removeDismissMonitors()
        clickMonitor = NSEvent.addGlobalMonitorForEvents(
            matching: [.leftMouseDown, .rightMouseDown, .otherMouseDown]
        ) { [weak self] _ in
            MainActor.assumeIsolated { self?.outsideClick() }
        }
        activationObserver = NSWorkspace.shared.notificationCenter.addObserver(
            forName: NSWorkspace.didActivateApplicationNotification, object: nil, queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated { self?.cancel() }
        }
    }

    private func removeDismissMonitors() {
        if let clickMonitor {
            NSEvent.removeMonitor(clickMonitor)
        }
        if let activationObserver {
            NSWorkspace.shared.notificationCenter.removeObserver(activationObserver)
        }
        clickMonitor = nil
        activationObserver = nil
    }

    // Clicks inside the status item's rect are left to toggle(); a click at the menu bar's top
    // edge falls outside it, which the reopen guard covers.
    private func outsideClick() {
        if !model.mode.isStayOpen && !isOverStatusItem {
            close(outsideClick: true)
        }
    }

    // MARK: - Setup

    private func installContent() {
        panel.delegate = self
        panel.onCancel = { [weak self] in self?.cancel() }

        effectView.material = .popover
        effectView.blendingMode = .behindWindow
        effectView.state = .active
        effectView.maskImage = Self.roundedMask(radius: Tokens.panelCornerRadius)

        let actions = PanelActions(
            toggleStayOpen: { [weak self] in self?.toggleStayOpen() },
            togglePinned: { [weak self] in self?.togglePinned() },
            openLogs: { [weak self] in self?.openLogs() },
            quit: { NSApp.terminate(nil) },
            heightChanged: { [weak self] in self?.contentHeightChanged($0) })
        let hosting = NSHostingView(
            rootView: PanelView(model: model, layout: layout, actions: actions))
        // Natural height, pinned to the top: the panel's height is set from it, never the
        // reverse.
        hosting.sizingOptions = [.intrinsicContentSize]
        hosting.translatesAutoresizingMaskIntoConstraints = false
        effectView.addSubview(hosting)
        NSLayoutConstraint.activate([
            hosting.leadingAnchor.constraint(equalTo: effectView.leadingAnchor),
            hosting.trailingAnchor.constraint(equalTo: effectView.trailingAnchor),
            hosting.topAnchor.constraint(equalTo: effectView.topAnchor),
        ])
        panel.contentView = effectView
        self.hosting = hosting
    }

    private static func roundedMask(radius: CGFloat) -> NSImage {
        let edge = radius * 2 + 1
        let image = NSImage(size: NSSize(width: edge, height: edge), flipped: false) { rect in
            NSColor.black.setFill()
            NSBezierPath(roundedRect: rect, xRadius: radius, yRadius: radius).fill()
            return true
        }
        image.capInsets = NSEdgeInsets(top: radius, left: radius, bottom: radius, right: radius)
        image.resizingMode = .stretch
        return image
    }
}
