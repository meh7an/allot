import AppKit
import SwiftUI

/// Header background that drags the panel while enabled, which is only while pinned. Clicks on
/// the header's controls never reach it.
struct DragHandle: NSViewRepresentable {
    let isEnabled: Bool

    func makeNSView(context: Context) -> DragHandleView {
        DragHandleView()
    }

    func updateNSView(_ view: DragHandleView, context: Context) {
        view.isEnabled = isEnabled
    }
}

final class DragHandleView: NSView {
    var isEnabled = false
    private var startMouse = NSPoint.zero
    private var startOrigin = NSPoint.zero
    private var dragging = false

    override var mouseDownCanMoveWindow: Bool {
        false
    }

    override func acceptsFirstMouse(for event: NSEvent?) -> Bool {
        true
    }

    override func mouseDown(with event: NSEvent) {
        guard isEnabled, let window else {
            return super.mouseDown(with: event)
        }
        dragging = true
        startMouse = NSEvent.mouseLocation
        startOrigin = window.frame.origin
    }

    override func mouseDragged(with event: NSEvent) {
        guard dragging, let window else {
            return super.mouseDragged(with: event)
        }
        let mouse = NSEvent.mouseLocation
        window.setFrameOrigin(NSPoint(
            x: startOrigin.x + mouse.x - startMouse.x,
            y: startOrigin.y + mouse.y - startMouse.y))
    }

    override func mouseUp(with event: NSEvent) {
        guard dragging else {
            return super.mouseUp(with: event)
        }
        dragging = false
    }
}
