import AppKit
import SwiftUI

/// Stand-in for the real panel: borderless and non-activating like `AllotPanel`, so clicking
/// Settings in it starts from the same state (the app not active, another app in front).
final class StandInPanel: NSPanel {
    init(content: some View) {
        super.init(
            contentRect: NSRect(x: 0, y: 0, width: 340, height: 300),
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: true)
        isFloatingPanel = true
        hidesOnDeactivate = false
        becomesKeyOnlyIfNeeded = false
        isReleasedWhenClosed = false
        hasShadow = true
        level = .statusBar
        collectionBehavior = [.moveToActiveSpace, .fullScreenAuxiliary, .ignoresCycle]
        let host = NSHostingView(rootView: content)
        host.wantsLayer = true
        host.layer?.cornerRadius = 10
        host.layer?.masksToBounds = true
        contentView = host
    }

    override var canBecomeKey: Bool {
        true
    }

    override var canBecomeMain: Bool {
        false
    }

    override func cancelOperation(_ sender: Any?) {
        orderOut(nil)
    }
}

/// The panel's controls: the Settings button the real More options menu will have, and the
/// switches the checklist compares.
struct PanelControls: View {
    @Bindable var options: SpikeOptions
    let openSettings: () -> Void
    let openLater: () -> Void
    let quit: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Stand-in panel").font(.headline)
            Button("Settings…", action: openSettings)
                .keyboardShortcut(",", modifiers: .command)
            Button("Open Settings in 5 s", action: openLater)
            Picker("Dock", selection: $options.dockPolicy) {
                ForEach(DockPolicy.allCases) { Text($0.rawValue).tag($0) }
            }
            Picker("Order", selection: $options.activationOrder) {
                ForEach(ActivationOrder.allCases) { Text($0.rawValue).tag($0) }
            }
            Toggle("Install a main menu (Cmd-W, Cmd-Q, Edit)", isOn: $options.installsMainMenu)
            Spacer()
            HStack {
                Text(SpikeLog.file.path).font(.caption2).textSelection(.enabled)
                Spacer()
                Button("Quit spike", action: quit)
            }
        }
        .padding(14)
        .frame(width: 340, height: 300)
        .background(.regularMaterial)
    }
}
