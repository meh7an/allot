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
    private var ticker: CardTicker?
    private var pollTask: Task<Void, Never>?
    private var lastStatus: ServiceStatus?
    #if DEBUG
    private var scenarios: ScenarioPlayer?
    #endif

    static func main() {
        let app = NSApplication.shared
        // NSApplication holds its delegate weakly; this one lives as long as run() does.
        let delegate = AppDelegate()
        app.delegate = delegate
        app.run()
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        let model = PanelModel(settings: SettingsStore(file: data.settingsFile).load())
        let ticker = CardTicker(model: model)
        self.ticker = ticker
        model.onRefresh = { [weak ticker] in
            ticker?.reschedule()
        }
        #if DEBUG
        if let name = ScenarioPlayer.requested(in: CommandLine.arguments) {
            launchScenario(name, model: model, ticker: ticker)
            return
        }
        #endif

        let client = DaemonClient.forInstall(data)
        let poller = DaemonPoller(client: client)
        let panel = PanelController(model: model, logDirectory: data.logDirectory)
        panel.onOpenChanged = { [weak poller] open in
            poller?.setPanelOpen(open)
            ticker.isOpen = open
        }
        poller.onUpdate = { [weak self, weak poller] result in
            model.apply(result, lastKnown: poller?.lastKnown)
            self?.logStatusChange(result)
        }
        model.stopper = client
        model.onStopAnswered = { [weak poller] lease, kind, result in
            Self.logStop(lease, kind, result)
            poller?.pollNow()
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

    private static func logStop(_ lease: Int64, _ kind: StopKind, _ result: StopResult) {
        let verb = kind == .kill ? "kill" : "remove"
        let outcome = if result.ended {
            result.text.isEmpty ? "ended" : "ended, \(result.text)"
        } else {
            "refused: \(result.text)"
        }
        log.info("\(verb, privacy: .public) of lease \(lease): \(outcome, privacy: .public)")
    }

    #if DEBUG
    // A scenario never polls: it shows the handoff's state, and its own stopper ends jobs.
    private func launchScenario(_ name: String, model: PanelModel, ticker: CardTicker) {
        let player = ScenarioPlayer(model: model)
        let panel = PanelController(
            model: model, logDirectory: data.logDirectory, scenarios: Handoff.scenarioNames)
        panel.onOpenChanged = { open in
            ticker.isOpen = open
        }
        panel.onShowScenario = { [weak self] in
            self?.showScenario($0)
        }
        scenarios = player
        statusController = StatusItemController(model: model, panel: panel)
        showScenario(name)
    }

    private func showScenario(_ name: String) {
        do {
            try scenarios?.show(name)
            Self.log.info("showing scenario \(name, privacy: .public)")
        } catch {
            Self.log.error("no scenario \(name, privacy: .public)")
        }
    }
    #endif
}
