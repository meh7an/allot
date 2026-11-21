import SwiftUI

/// The Settings window's text sizes, which follow the panel's Text Size (`text_size` in
/// `tray.json`, read through ``TextScale``). macOS does not scale semantic text styles with a
/// system setting, so every text in the window takes one of these. Reading one in a view's body
/// subscribes the view to ``TextScale``, so a change redraws the window. Native controls draw
/// their own text (the segmented picker, the unit menus, the slider): those keep the system size.
@MainActor
enum SettingsFont {
    static var body: Font { .system(size: sized(13)) }
    static var callout: Font { .system(size: sized(12)) }
    static var headline: Font { .system(size: sized(13), weight: .semibold) }
    static var title: Font { .system(size: sized(15), weight: .semibold) }
    static var caption: Font { .system(size: sized(10)) }
    static var captionStrong: Font { .system(size: sized(10), weight: .semibold) }
    static var mono: Font { .system(size: sized(13), design: .monospaced) }
    static var calloutMono: Font { .system(size: sized(12), design: .monospaced) }
    static var captionMono: Font { .system(size: sized(10), design: .monospaced) }

    /// The multiple that widths of text boxes follow, so a box keeps its digits visible.
    static var scale: CGFloat {
        CGFloat(TextScale.shared.size.scale)
    }

    private static func sized(_ points: CGFloat) -> CGFloat {
        points * scale
    }
}

/// What a setting's control says as its accessibility hint: its help, the key, and any error or
/// note, set by the row around the control and read by the control itself.
private struct SettingHintKey: EnvironmentKey {
    static let defaultValue = ""
}

extension EnvironmentValues {
    var settingHint: String {
        get { self[SettingHintKey.self] }
        set { self[SettingHintKey.self] = newValue }
    }
}

/// Gives a control the hint its row set.
private struct RowHint: ViewModifier {
    @Environment(\.settingHint) private var hint

    func body(content: Content) -> some View {
        content.accessibilityHint(hint)
    }
}

extension View {
    /// The row's hint: its error first, then its notes, help and key.
    func rowHint() -> some View {
        modifier(RowHint())
    }
}

/// The sentences a row gives a screen reader.
enum RowText {
    /// "Error: Enter a whole number. Disabled while ... Help text. Setting key: x.y".
    static func hint(help: String, key: String?, notes: [RowNote]) -> String {
        var parts: [String] = []
        for note in notes {
            switch note {
            case .issue(let text): parts.append("Error: \(text)")
            case .info(let text): parts.append(text)
            }
        }
        if !help.isEmpty {
            parts.append(help)
        }
        if let key {
            parts.append("Setting key: \(key)")
        }
        return parts.joined(separator: " ")
    }
}
