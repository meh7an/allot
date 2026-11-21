import AllotKit
import AppKit
import SwiftUI

/// The Service page's status row (DESIGN.md section 4, Service), following every poll.
struct ServiceStatusSection: View {
    let model: SettingsWindowModel

    var body: some View {
        let session = model.session
        let row = ServiceRow.from(
            model.serviceStatus, session: session.status, version: model.version,
            port: session.listeningPort ?? UInt32(model.port), startProblem: model.startProblem)
        SettingsSection(title: "Service") {
            HStack(spacing: 8) {
                Text(row.text)
                    .fixedSize(horizontal: false, vertical: true)
                    .accessibilityAddTraits(.updatesFrequently)
                Spacer(minLength: 16)
                if row.button == .start {
                    Button(row.buttonText) { press(row.button) }
                        .buttonStyle(.borderedProminent)
                } else {
                    Button(row.buttonText) { press(row.button) }
                        .disabled(row.button == .starting)
                }
                if row.opensLogs {
                    Button("Open Logs") { model.actions.openLogs() }
                }
            }
            .padding(.vertical, 6)
        }
    }

    private func press(_ button: ServiceButton) {
        switch button {
        case .stop:
            Task { await model.stopService() }
        case .start:
            model.actions.startService()
        case .starting:
            break
        }
    }
}

/// Files: the path of `config.toml` with Open File and Show in Finder.
struct FilesSection: View {
    let data: DataDirectory

    var body: some View {
        let file = data.configFile
        SettingsSection(title: "Files") {
            VStack(alignment: .leading, spacing: 4) {
                Text("Config file")
                Text((file.path as NSString).abbreviatingWithTildeInPath)
                    .font(SettingsFont.calloutMono)
                    .foregroundStyle(.secondary)
                    .textSelection(.enabled)
                HStack(spacing: 8) {
                    Button("Open File") { ConfigFile.open(file) }
                    Button("Show in Finder") { ConfigFile.showInFinder(file) }
                }
                .padding(.top, 4)
            }
            .padding(.vertical, 6)
        }
    }
}

/// Opens `config.toml` or shows it in Finder.
@MainActor
enum ConfigFile {
    /// Opens the file in the app macOS has for it, or TextEdit without one; shows its folder
    /// while it does not exist.
    static func open(_ file: URL) {
        guard FileManager.default.fileExists(atPath: file.path) else {
            showInFinder(file)
            return
        }
        if NSWorkspace.shared.urlForApplication(toOpen: file) != nil {
            NSWorkspace.shared.open(file)
            return
        }
        guard let textEdit = NSWorkspace.shared.urlForApplication(
            withBundleIdentifier: "com.apple.TextEdit")
        else {
            showInFinder(file)
            return
        }
        NSWorkspace.shared.open(
            [file], withApplicationAt: textEdit, configuration: NSWorkspace.OpenConfiguration(),
            completionHandler: nil)
    }

    /// Selects the file in Finder, or opens its folder while it does not exist.
    static func showInFinder(_ file: URL) {
        if FileManager.default.fileExists(atPath: file.path) {
            NSWorkspace.shared.activateFileViewerSelecting([file])
        } else {
            NSWorkspace.shared.open(file.deletingLastPathComponent())
        }
    }
}
