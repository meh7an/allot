import AllotKit
import SwiftUI

/// The header row: the mark, the summary, then Pin (only while Stay open is on), Stay open and
/// More options. While pinned, its empty space drags the panel.
struct HeaderView: View {
    let model: PanelModel
    let actions: PanelActions

    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        let mode = model.mode
        HStack(spacing: 10) {
            HeaderGlyphView(glyph: model.snapshot.header.glyph)
            Text(model.snapshot.header.summary)
                .font(Tokens.Font.summary)
                .monospacedDigit()
                .lineLimit(1)
                .truncationMode(.tail)
                .frame(maxWidth: .infinity, alignment: .leading)
                .accessibilityAddTraits(.isHeader)
            HStack(spacing: 0) {
                HeaderButton(
                    symbol: mode.isPinned ? "pin.fill" : "pin",
                    label: mode.isPinned ? "Unpin" : "Pin",
                    isOn: mode.isPinned,
                    action: actions.togglePinned)
                    // The gap after Pin is part of its reveal, so it closes with the button.
                    .padding(.trailing, Tokens.buttonGap)
                    .frame(
                        width: mode.isStayOpen ? Tokens.buttonSize + Tokens.buttonGap : 0,
                        alignment: .leading)
                    .clipped()
                    .opacity(mode.isStayOpen ? 1 : 0)
                    .disabled(!mode.isStayOpen)
                    .accessibilityHidden(!mode.isStayOpen)
                HeaderButton(
                    symbol: mode.isStayOpen ? "lock.fill" : "lock.open",
                    label: "Stay open",
                    isOn: mode.isStayOpen,
                    action: actions.toggleStayOpen)
                MoreOptionsMenu(
                    actions: actions,
                    service: actions.chooseService == nil
                        ? nil : ServiceMenu.item(for: model.serviceStatus),
                    scenarios: actions.scenarios)
                    .equatable()
                    .padding(.leading, Tokens.buttonGap)
            }
            .animation(reduceMotion ? nil : Tokens.reveal, value: mode.isStayOpen)
        }
        .padding(EdgeInsets(top: 8, leading: 14, bottom: 6, trailing: 8))
        .background(DragHandle(isEnabled: mode.isPinned))
    }
}

/// The 16 pt glyph: the solid mark while a job runs, the dimmed mark when idle, and a red error
/// icon while the service is down.
private struct HeaderGlyphView: View {
    let glyph: HeaderGlyph

    var body: some View {
        Group {
            switch glyph {
            case .busy:
                AllotMark().fill(.primary)
            case .idle:
                AllotMark().fill(.primary).opacity(0.5)
            case .error:
                Image(systemName: "exclamationmark.circle.fill")
                    .font(.system(size: 14))
                    .foregroundStyle(Tokens.danger)
            }
        }
        .frame(width: 16, height: 16)
        .accessibilityHidden(true)
    }
}

private struct HeaderButton: View {
    let symbol: String
    let label: String
    let isOn: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: 13, weight: .medium))
                .foregroundStyle(isOn ? Color.white : Color.primary)
                .frame(width: Tokens.buttonSize, height: Tokens.buttonSize)
                .background(
                    RoundedRectangle(cornerRadius: Tokens.buttonRadius)
                        .fill(isOn ? Color.accentColor : Color.clear))
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(label)
        .accessibilityLabel(label)
        .accessibilityValue(isOn ? "On" : "Off")
        .accessibilityAddTraits(.isToggle)
    }
}

/// More options, shrunk to DESIGN.md section 9: Settings…, Stop or Start Service, Open Logs and
/// Quit; Debug builds add the handoff scenarios and Test Notifications. Meters, Notifications,
/// Text Size and Launch at Login live on the Settings window's Panel page.
///
/// It is rebuilt only when what it shows changes: every rebuild replaces the native menu, and
/// the header around it updates with every poll and every second.
private struct MoreOptionsMenu: View, Equatable {
    let actions: PanelActions

    /// The service item, or `nil` where the menu has none.
    let service: ServiceMenuItem?

    let scenarios: [String]

    nonisolated static func == (lhs: Self, rhs: Self) -> Bool {
        lhs.service == rhs.service && lhs.scenarios == rhs.scenarios
    }

    var body: some View {
        Menu {
            if let openSettings = actions.openSettings {
                Button(ServiceMenu.settingsTitle, systemImage: "gearshape", action: openSettings)
                    .keyboardShortcut(",")
            }
            if let service, let chooseService = actions.chooseService {
                Button(service.title, action: chooseService)
                    .disabled(!service.isEnabled)
            }
            Divider()
            Button("Open Logs", systemImage: "doc.text", action: actions.openLogs)
            if !scenarios.isEmpty {
                Menu("Scenario") {
                    ForEach(scenarios, id: \.self) { name in
                        Button(name) { actions.showScenario(name) }
                    }
                }
            }
            if let testNotifications = actions.testNotifications {
                Button("Test Notifications", action: testNotifications)
            }
            Divider()
            Button("Quit Allot", action: actions.quit)
        } label: {
            Image(systemName: "ellipsis")
                .font(.system(size: 13, weight: .medium))
        }
        .menuStyle(.borderlessButton)
        .menuIndicator(.hidden)
        .fixedSize()
        .frame(width: Tokens.buttonSize, height: Tokens.buttonSize)
        .help("More options")
        .accessibilityLabel("More options")
    }
}
