import AllotKit
import AppKit
import Observation
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
    static let cardBorder = Color(
        light: .black.opacity(0.14), dark: .black.opacity(0.40),
        lightContrast: .black.opacity(0.7), darkContrast: .white.opacity(0.7))
    static let strip = Color(light: .black.opacity(0.075), dark: .white.opacity(0.09))

    static let separator = Color(
        light: .black.opacity(0.08), dark: .white.opacity(0.08),
        lightContrast: .black.opacity(0.5), darkContrast: .white.opacity(0.5))
    static let chip = Color(light: .black.opacity(0.07), dark: .white.opacity(0.10))
    static let chipText = Color(
        light: hex(0x3A3A3A), dark: hex(0xDEDEDE), lightContrast: .black, darkContrast: .white)
    static let tile = Color(light: .black.opacity(0.06), dark: .white.opacity(0.07))
    static let queueTileBorder = Color(
        light: .black.opacity(0.30), dark: .white.opacity(0.30),
        lightContrast: .black.opacity(0.7), darkContrast: .white.opacity(0.7))
    static let track = Color(
        light: .black.opacity(0.07), dark: .white.opacity(0.08),
        lightContrast: .black.opacity(0.3), darkContrast: .white.opacity(0.3))
    static let stepUpcoming = Color(
        light: .black.opacity(0.10), dark: .white.opacity(0.12),
        lightContrast: .black.opacity(0.3), darkContrast: .white.opacity(0.3))
    static let subtle = Color(
        light: hex(0x8A8A8A), dark: hex(0x9A9A9A),
        lightContrast: hex(0x4A4A4A), darkContrast: hex(0xC8C8C8))

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
    static let hatchLine = Color(
        light: .black.opacity(0.20), dark: .white.opacity(0.22),
        lightContrast: .black.opacity(0.6), darkContrast: .white.opacity(0.6))
    static let tickRing = Color(light: .white, dark: hex(0x2C2C2C))
    static let driveBar = Color(light: hex(0x6B6B6B), dark: hex(0xA8A8A8))

    /// The GPU's share of RAM inside the RAM bar's used fill.
    static let sharedOpacity = 0.45

    /// The handoff's reveal curve, cubic-bezier(.1, .9, .2, 1).
    static let reveal = Animation.timingCurve(0.1, 0.9, 0.2, 1, duration: 0.2)

    /// Cards entering and leaving, cubic-bezier(.4, 0, .2, 1) over 240 ms.
    static let cardDuration = 0.24
    static let cardMotion = Animation.timingCurve(0.4, 0, 0.2, 1, duration: cardDuration)

    /// The gap above each list row.
    static let rowGap: CGFloat = 6

    /// How far the list runs under the Resources card, for the notches beside its rounded top
    /// corners.
    static let notchDepth: CGFloat = 6

    /// Bars follow the ticking time.
    static let barMotion = GlidingFillMotion.linear(0.9)
    static let stepMotion = GlidingFillMotion.linear(0.7)

    /// The handoff's sizes at More options › Text Size. Reading one in a view's body subscribes
    /// the view to ``TextScale``, so a change redraws the panel. The tile's initials stay, since
    /// they must fit their 20 pt tile.
    @MainActor
    enum Font {
        static var summary: SwiftUI.Font { .system(size: sized(14), weight: .semibold) }
        static var heading: SwiftUI.Font { .system(size: sized(12), weight: .semibold) }
        static var body: SwiftUI.Font { .system(size: sized(12)) }
        static var command: SwiftUI.Font { .system(size: sized(13), design: .monospaced) }
        static var id: SwiftUI.Font { .system(size: sized(12), design: .monospaced) }
        static var idInLine: SwiftUI.Font { .system(size: sized(11), design: .monospaced) }
        static var small: SwiftUI.Font { .system(size: sized(11)) }
        static let initials = SwiftUI.Font.system(size: 9, weight: .semibold, design: .monospaced)
        static var action: SwiftUI.Font { .system(size: sized(12), weight: .semibold) }

        private static func sized(_ points: CGFloat) -> CGFloat {
            points * CGFloat(TextScale.shared.size.scale)
        }
    }

    private static func hex(_ value: UInt32) -> Color {
        Color(
            red: Double((value >> 16) & 0xFF) / 255,
            green: Double((value >> 8) & 0xFF) / 255,
            blue: Double(value & 0xFF) / 255)
    }
}

/// The current More options › Text Size, which every text token and the row sizes read.
@MainActor
@Observable
final class TextScale {
    static let shared = TextScale()

    var size = TextSize.standard
}

extension Color {
    /// A colour that follows the appearance it is drawn in, as the handoff's light and dark
    /// tables do. Increase Contrast picks the contrast pair; without one it changes nothing.
    init(light: Color, dark: Color, lightContrast: Color? = nil, darkContrast: Color? = nil) {
        self.init(nsColor: NSColor(name: nil) { appearance in
            switch appearance.bestMatch(from: [
                .aqua, .darkAqua, .accessibilityHighContrastAqua,
                .accessibilityHighContrastDarkAqua,
            ]) {
            case .accessibilityHighContrastAqua: NSColor(lightContrast ?? light)
            case .accessibilityHighContrastDarkAqua: NSColor(darkContrast ?? dark)
            case .darkAqua: NSColor(dark)
            default: NSColor(light)
            }
        })
    }
}
