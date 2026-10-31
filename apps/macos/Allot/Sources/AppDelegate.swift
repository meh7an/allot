import AllotKit
import AppKit
import os

@main
@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    /// Read with `log stream --predicate 'subsystem == "tech.mehran.allot"'`.
    private static let log = Logger(subsystem: "tech.mehran.allot", category: "service")

    private let data = DataDirectory.fromEnvironment()
    private var statusController: StatusItemController?
    private var pollTask: Task<Void, Never>?
    private var lastStatus: ServiceStatus?

    static func main() {
        let app = NSApplication.shared
        // NSApplication holds its delegate weakly; this one lives as long as run() does.
        let delegate = AppDelegate()
        app.delegate = delegate
        app.run()
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        let model = PanelModel(settings: SettingsStore(file: data.settingsFile).load())
        let poller = DaemonPoller(client: .forInstall(data))
        let panel = PanelController(model: model, logDirectory: data.logDirectory)
        panel.onOpenChanged = { [weak poller] open in
            poller?.setPanelOpen(open)
        }
        poller.onUpdate = { [weak self, weak poller] result in
            model.apply(result, lastKnown: poller?.lastKnown)
            self?.logStatusChange(result)
        }
        statusController = StatusItemController(model: model, panel: panel)
        pollTask = Task {
            await poller.run()
        }
        Self.log.info("launched from \(Bundle.main.bundlePath, privacy: .public)")
    }

    func applicationWillTerminate(_ notification: Notification) {
        pollTask?.cancel()
    }

    func applicationSupportsSecureRestorableState(_ app: NSApplication) -> Bool {
        true
    }

    private func logStatusChange(_ result: PollResult) {
        guard result.status != lastStatus else {
            return
        }
        lastStatus = result.status
        let status = String(describing: result.status)
        let detail = result.detail ?? result.version ?? ""
        Self.log.info("service \(status, privacy: .public): \(detail, privacy: .public)")
    }
}
