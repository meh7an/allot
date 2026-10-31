import AppKit
import SwiftUI

/// The design handoff's tokens on macOS. Text uses the system's label colours, which keep their
/// vibrancy on the panel's material; the handoff's fills and state colours keep its values, in
/// light and dark. The accent is the user's System Settings accent (`Color.accentColor`).
enum Tokens {
    static let panelWidth: CGFloat = 360
    static let panelMaxHeight: CGFloat = 640
    static let panelCornerRadius: CGFloat = 12
    static let cardRadius: CGFloat = 6
    static let buttonRadius: CGFloat = 6
    static let buttonSize: CGFloat = 28

    /// Between header buttons: their accent fill covers the whole button, unlike the handoff's
    /// inset Fluent fill, so touching buttons would merge into one block.
    static let buttonGap: CGFloat = 4

    static let card = Color(light: .white.opacity(0.72), dark: .white.opacity(0.055))
    static let cardBorder = Color(light: .black.opacity(0.14), dark: .black.opacity(0.40))
    static let strip = Color(light: .black.opacity(0.075), dark: .white.opacity(0.09))

    static let running = Color(light: hex(0x0F7B0F), dark: hex(0x6CCB5F))
    static let waiting = Color(light: hex(0x9D5D00), dark: hex(0xFCE100))
    static let benchmark = Color(light: hex(0x6F3FB0), dark: hex(0xC9A8F0))
    static let danger = Color(light: hex(0xC42B1C), dark: hex(0xFF99A4))

    /// Last known jobs while the service is down.
    static let lastKnownOpacity = 0.55

    /// The handoff's reveal curve, cubic-bezier(.1, .9, .2, 1).
    static let reveal = Animation.timingCurve(0.1, 0.9, 0.2, 1, duration: 0.2)

    enum Font {
        static let summary = SwiftUI.Font.system(size: 14, weight: .semibold)
        static let heading = SwiftUI.Font.system(size: 12, weight: .semibold)
        static let body = SwiftUI.Font.system(size: 12)
        static let command = SwiftUI.Font.system(size: 13, design: .monospaced)
        static let id = SwiftUI.Font.system(size: 12, design: .monospaced)
        static let idInLine = SwiftUI.Font.system(size: 11, design: .monospaced)
    }

    private static func hex(_ value: UInt32) -> Color {
        Color(
            red: Double((value >> 16) & 0xFF) / 255,
            green: Double((value >> 8) & 0xFF) / 255,
            blue: Double(value & 0xFF) / 255)
    }
}

extension Color {
    /// A colour that follows the appearance it is drawn in, as the handoff's light and dark
    /// tables do.
    init(light: Color, dark: Color) {
        self.init(nsColor: NSColor(name: nil) { appearance in
            let isDark = appearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
            return NSColor(isDark ? dark : light)
        })
    }
}
