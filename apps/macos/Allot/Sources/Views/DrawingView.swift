import AppKit
import SwiftUI

/// Draws with Core Graphics into a small layer. A SwiftUI `Canvas` renders through Metal, and one
/// that keeps redrawing keeps nearly 200 MB of graphics memory alive for a panel this size.
struct DrawingView: NSViewRepresentable {
    /// Draws in a top-left origin, in points.
    let draw: (CGContext, CGSize, EnvironmentValues) -> Void

    func makeNSView(context: Context) -> DrawingNSView {
        DrawingNSView()
    }

    func updateNSView(_ view: DrawingNSView, context: Context) {
        let environment = context.environment
        view.draw = { [draw] graphics, size in draw(graphics, size, environment) }
        view.needsDisplay = true
    }
}

final class DrawingNSView: NSView {
    var draw: ((CGContext, CGSize) -> Void)?

    override var isFlipped: Bool {
        true
    }

    /// Drawings take no clicks.
    override func hitTest(_ point: NSPoint) -> NSView? {
        nil
    }

    override func draw(_ dirtyRect: NSRect) {
        if let context = NSGraphicsContext.current?.cgContext {
            draw?(context, bounds.size)
        }
    }
}

extension Color {
    /// This colour as this environment shows it.
    func cgColor(in environment: EnvironmentValues) -> CGColor {
        resolve(in: environment).cgColor
    }
}
