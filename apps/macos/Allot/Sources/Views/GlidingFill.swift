import AppKit
import SwiftUI

/// A solid span of a bar, from `start` to `end` as fractions of its width. Core Animation draws
/// and moves it: a SwiftUI glide recomputes the view tree on the main thread at the display's
/// rate, for as long as the glide lasts, and these bars glide nearly all the time.
struct GlidingFill: NSViewRepresentable {
    let start: Double
    let end: Double
    let colour: Color

    /// `nil` moves at once.
    let motion: GlidingFillMotion?

    func makeNSView(context: Context) -> GlidingFillView {
        GlidingFillView()
    }

    func updateNSView(_ view: GlidingFillView, context: Context) {
        view.show(
            start: start, end: end, colour: colour.cgColor(in: context.environment),
            motion: motion)
    }
}

struct GlidingFillMotion {
    let duration: Double
    let curve: CAMediaTimingFunctionName

    static func linear(_ duration: Double) -> GlidingFillMotion {
        GlidingFillMotion(duration: duration, curve: .linear)
    }

    static func easeOut(_ duration: Double) -> GlidingFillMotion {
        GlidingFillMotion(duration: duration, curve: .easeOut)
    }
}

final class GlidingFillView: NSView {
    private let fill = CALayer()
    private var start = 0.0
    private var end = 0.0
    private var hasShown = false

    init() {
        super.init(frame: .zero)
        wantsLayer = true
        layer?.addSublayer(fill)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("not used from a nib")
    }

    /// Bars take no clicks.
    override func hitTest(_ point: NSPoint) -> NSView? {
        nil
    }

    /// The first value shows at once, and a new width only resizes.
    func show(start: Double, end: Double, colour: CGColor, motion: GlidingFillMotion?) {
        let moved = start != self.start || end != self.end
        self.start = start
        self.end = end
        apply(colour: colour, motion: hasShown && moved ? motion : nil)
        hasShown = true
    }

    override func layout() {
        super.layout()
        apply(colour: fill.backgroundColor, motion: nil)
    }

    private func apply(colour: CGColor?, motion: GlidingFillMotion?) {
        CATransaction.begin()
        defer {
            CATransaction.commit()
        }
        if let motion {
            CATransaction.setAnimationDuration(motion.duration)
            CATransaction.setAnimationTimingFunction(CAMediaTimingFunction(name: motion.curve))
        } else {
            CATransaction.setDisableActions(true)
        }
        let from = bounds.width * max(0, start)
        let to = bounds.width * max(start, end)
        fill.frame = CGRect(x: from, y: 0, width: to - from, height: bounds.height)
        // A colour never glides; it changes with the appearance and the card's state.
        if fill.backgroundColor != colour {
            CATransaction.setDisableActions(true)
            fill.backgroundColor = colour
        }
    }
}
