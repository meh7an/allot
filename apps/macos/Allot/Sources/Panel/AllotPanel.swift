import AppKit

/// Borderless, non-activating panel: it takes key focus without activating the app, so the app
/// in front keeps its menu bar and typing in the panel still works.
final class AllotPanel: NSPanel {
    var onCancel: (() -> Void)?

    init() {
        super.init(
            contentRect: .zero,
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: true)
        isFloatingPanel = true
        hidesOnDeactivate = false
        becomesKeyOnlyIfNeeded = false
        isOpaque = false
        backgroundColor = .clear
        hasShadow = true
        isReleasedWhenClosed = false
        animationBehavior = .none
    }

    override var canBecomeKey: Bool {
        true
    }

    override var canBecomeMain: Bool {
        false
    }

    override func cancelOperation(_ sender: Any?) {
        onCancel?()
    }
}

/// Holds the panel's content with its origin at the top: while the window's height animates,
/// AppKit moves unflipped subviews with the bottom edge until layout runs, so the content
/// would jump down and back on every growth.
final class TopAnchoredView: NSView {
    override var isFlipped: Bool {
        true
    }
}
