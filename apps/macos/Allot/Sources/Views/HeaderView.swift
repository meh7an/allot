import AllotKit
import SwiftUI

/// The header row: the mark, the summary, then Pin (only while Stay open is on), Stay open and
/// More options. While pinned, its empty space drags the panel.
struct HeaderView: View {
    let model: PanelModel
    let actions: PanelActions

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
                MoreOptionsMenu(model: model, actions: actions)
                    .padding(.leading, Tokens.buttonGap)
            }
            .animation(Tokens.reveal, value: mode.isStayOpen)
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
        .accessibilityAddTraits(isOn ? .isSelected : [])
    }
}

/// More options: the Meters and Notifications check items with their counts, Launch at Login,
/// Open Logs and Quit; Debug builds add the handoff scenarios and Test Notifications. A check
/// item closes the menu, as every macOS menu does.
private struct MoreOptionsMenu: View {
    let model: PanelModel
    let actions: PanelActions

    var body: some View {
        let settings = model.settings
        Menu {
            Menu {
                check("CPU", \.meters.cpu)
                check("RAM", \.meters.ram)
                check("GPU", \.meters.gpu)
                check(model.resources.sharesMemory ? "GPU Memory" : "VRAM", \.meters.vram)
                check("Drives", \.meters.drives)
                check("Drive Activity", \.meters.driveActivity)
                    .disabled(!settings.meters.drives)
            } label: {
                Label("Meters", systemImage: "chart.bar.xaxis")
            }
            .badge(Text(MenuCounts.meters(settings.meters)))
            Menu {
                check("Service Stopped", \.notifications.serviceStopped)
                check("Running Long", \.notifications.runningLong)
                check("Benchmark Finished", \.notifications.benchmarkFinished)
                check("Job Finished", \.notifications.jobFinished)
                check("Killed Outside the Panel", \.notifications.killedOutsidePanel)
            } label: {
                Label("Notifications", systemImage: "bell")
            }
            .badge(Text(MenuCounts.notifications(settings.notifications)))
            Divider()
            Toggle("Launch at Login", isOn: Binding(
                get: { model.login.isOn },
                set: { actions.setLaunchAtLogin($0) }))
                .disabled(!model.login.canToggle)
            Button("Open Logs", systemImage: "doc.text", action: actions.openLogs)
            if !actions.scenarios.isEmpty {
                Menu("Scenario") {
                    ForEach(actions.scenarios, id: \.self) { name in
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

    private func check(_ title: String, _ key: WritableKeyPath<TraySettings, Bool>) -> some View {
        Toggle(title, isOn: Binding(
            get: { model.settings[keyPath: key] },
            set: { on in model.updateSettings { $0[keyPath: key] = on } }))
    }
}
