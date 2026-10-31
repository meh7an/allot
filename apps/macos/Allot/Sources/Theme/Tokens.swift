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

    static let separator = Color(light: .black.opacity(0.08), dark: .white.opacity(0.08))
    static let chip = Color(light: .black.opacity(0.07), dark: .white.opacity(0.10))
    static let chipText = Color(light: hex(0x3A3A3A), dark: hex(0xDEDEDE))
    static let tile = Color(light: .black.opacity(0.06), dark: .white.opacity(0.07))
    static let queueTileBorder = Color(light: .black.opacity(0.30), dark: .white.opacity(0.30))
    static let track = Color(light: .black.opacity(0.07), dark: .white.opacity(0.08))
    static let stepUpcoming = Color(light: .black.opacity(0.10), dark: .white.opacity(0.12))
    static let subtle = Color(light: hex(0x8A8A8A), dark: hex(0x9A9A9A))

    /// Stopped and removed bars, and frozen last known ones.
    static let frozen = hex(0x8A8A8A)

    static let running = Color(light: hex(0x0F7B0F), dark: hex(0x6CCB5F))
    static let waiting = Color(light: hex(0x9D5D00), dark: hex(0xFCE100))
    static let benchmark = Color(light: hex(0x6F3FB0), dark: hex(0xC9A8F0))
    static let danger = Color(light: hex(0xC42B1C), dark: hex(0xFF99A4))

    /// Text on the solid Benchmark chip.
    static let onBenchmark = Color(light: .white, dark: hex(0x1A1A1A))

    /// The Kill? button, the same in light and dark.
    static let dangerFill = hex(0xC42B1C)
    static let dangerFillHover = hex(0xA32317)

    /// Behind the Kill icon while hovered.
    static let killHover = hex(0xC42B1C).opacity(0.14)

    /// Behind the Remove button while hovered.
    static let buttonHover = Color(light: .black.opacity(0.06), dark: .white.opacity(0.08))

    /// Ended cards: tile, command and chip.
    static let endedOpacity = 0.5

    /// Last known jobs while the service is down.
    static let lastKnownOpacity = 0.55

    /// The service-down banner.
    static let bannerFill = Color(
        light: hex(0xC42B1C).opacity(0.08), dark: hex(0xFF99A4).opacity(0.10))
    static let bannerBorder = Color(
        light: hex(0xC42B1C).opacity(0.30), dark: hex(0xFF99A4).opacity(0.30))
    static let bannerText = Color(light: hex(0x8A1E14), dark: hex(0xFFB3BB))

    /// The Resources card's bars: the hatched headroom leases can't take, the reserved tick's
    /// ring in the surface colour, and the drive bars.
    static let hatchBack = Color(light: .black.opacity(0.05), dark: .white.opacity(0.05))
    static let hatchLine = Color(light: .black.opacity(0.20), dark: .white.opacity(0.22))
    static let tickRing = Color(light: .white, dark: hex(0x2C2C2C))
    static let driveBar = Color(light: hex(0x6B6B6B), dark: hex(0xA8A8A8))

    /// The GPU's share of RAM inside the RAM bar's used fill.
    static let sharedOpacity = 0.45

    /// The handoff's reveal curve, cubic-bezier(.1, .9, .2, 1).
    static let reveal = Animation.timingCurve(0.1, 0.9, 0.2, 1, duration: 0.2)

    /// Cards entering and leaving, cubic-bezier(.4, 0, .2, 1) over 240 ms.
    static let cardMotion = Animation.timingCurve(0.4, 0, 0.2, 1, duration: 0.24)

    /// Bars follow the ticking time.
    static let barMotion = Animation.linear(duration: 0.9)
    static let stepMotion = Animation.linear(duration: 0.7)

    enum Font {
        static let summary = SwiftUI.Font.system(size: 14, weight: .semibold)
        static let heading = SwiftUI.Font.system(size: 12, weight: .semibold)
        static let body = SwiftUI.Font.system(size: 12)
        static let command = SwiftUI.Font.system(size: 13, design: .monospaced)
        static let id = SwiftUI.Font.system(size: 12, design: .monospaced)
        static let idInLine = SwiftUI.Font.system(size: 11, design: .monospaced)
        static let small = SwiftUI.Font.system(size: 11)
        static let initials = SwiftUI.Font.system(size: 9, weight: .semibold, design: .monospaced)
        static let action = SwiftUI.Font.system(size: 12, weight: .semibold)
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
