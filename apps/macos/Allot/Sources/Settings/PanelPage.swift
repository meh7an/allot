import AllotKit
import SwiftUI

/// The Panel page: the app's own settings in `tray.json` and the login item (DESIGN.md
/// section 4, Panel). It saves at once and works whatever allotd's state.
struct PanelPage: View {
    let model: SettingsWindowModel

    var body: some View {
        let panel = model.panel
        let settings = panel.settings
        VStack(alignment: .leading, spacing: 20) {
            SettingsSection(title: "Meters", help: PanelToggles.metersHelp) {
                toggles(PanelToggles.meters(sharesMemory: panel.resources.sharesMemory))
            }
            SettingsSection(title: "Notifications") {
                if panel.notificationPermission.isOff {
                    HStack(spacing: 8) {
                        Text("Notifications for Allot are off in System Settings.")
                            .fixedSize(horizontal: false, vertical: true)
                        Spacer(minLength: 16)
                        Button("Open Notification Settings…") { Notifier.openSettings() }
                    }
                    .padding(.vertical, 6)
                    Divider()
                }
                toggles(PanelToggles.notifications)
            }
            SettingsSection(title: "Startup") {
                SettingRow(label: "Launch at Login", help: loginHelp) {
                    Toggle("Launch at Login", isOn: Binding(
                        get: { panel.login.isOn },
                        set: { model.actions.setLaunchAtLogin($0) }))
                        .toggleStyle(.switch)
                        .labelsHidden()
                }
                .disabled(!panel.login.canToggle)
                if panel.login == .needsApproval {
                    Button("Open Login Items…") { model.actions.openLoginItems() }
                        .padding(.bottom, 6)
                }
            }
            SettingsSection(title: PanelToggles.textSizeLabel) {
                SettingRow(label: PanelToggles.textSizeLabel, help: "Text in the panel.") {
                    Picker(PanelToggles.textSizeLabel, selection: Binding(
                        get: { settings.textSize },
                        set: { size in panel.updateSettings { $0.textSize = size } })
                    ) {
                        ForEach(TextSize.allCases) { size in
                            Text(size.title).tag(size)
                        }
                    }
                    .pickerStyle(.segmented)
                    .labelsHidden()
                    .fixedSize()
                }
            }
            let changed = PanelToggles.changedLines(
                settings, sharesMemory: panel.resources.sharesMemory)
            ResetSection(items: [
                ResetSection.Item(title: "Reset Page…", isEnabled: !changed.isEmpty) {
                    Task { await resetPage() }
                },
            ])
        }
    }

    private var loginHelp: String {
        let help = "Start Allot when you sign in."
        let hint = model.panel.login.hint
        return hint.isEmpty ? help : "\(help) \(hint)."
    }

    private func toggles(_ toggles: [PanelToggle]) -> some View {
        let panel = model.panel
        return ForEach(toggles.indices, id: \.self) { index in
            let toggle = toggles[index]
            if index > 0 {
                Divider()
            }
            SettingRow(label: toggle.label, help: toggle.help) {
                Toggle(toggle.label, isOn: Binding(
                    get: { panel.settings[keyPath: toggle.key] },
                    set: { on in panel.updateSettings { $0[keyPath: toggle.key] = on } }))
                    .toggleStyle(.switch)
                    .labelsHidden()
            }
            .padding(.leading, toggle.parent == nil ? 0 : 20)
            .disabled(!toggle.isEnabled(in: panel.settings))
        }
    }

    private func resetPage() async {
        let panel = model.panel
        let lines = PanelToggles.changedLines(
            panel.settings, sharesMemory: panel.resources.sharesMemory)
        guard !lines.isEmpty else {
            return
        }
        if await model.confirm(SettingsText.resetPage("Panel", lines: lines)) {
            panel.updateSettings { PanelToggles.reset(&$0) }
        }
    }
}
