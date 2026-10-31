import AllotKit
import AppKit
import os

@main
@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    /// Read with `log stream --predicate 'subsystem == "tech.mehran.allot"'`.
    private static let log = Logger(subsystem: "tech.mehran.allot", category: "service")

    private let data = DataDirectory.fromEnvironment()
    private let rules = NotificationRules()
    private var model: PanelModel?
    private var poller: DaemonPoller?
    private var panel: PanelController?
    private var notifier: Notifier?
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
        #if DEBUG
        if let name = ScenarioPlayer.requested(in: CommandLine.arguments) {
            launchScenario(name)
            return
        }
        #endif

        let model = makeModel(settingsFile: data.settingsFile)
        let notifier = Notifier()
        notifier.install()
        self.notifier = notifier
        let client = DaemonClient.forInstall(data)
        let poller = DaemonPoller(
            client: client,
            launcher: LaunchdServiceLauncher(data: data, runner: ProcessRunner()))
        self.poller = poller
        #if DEBUG
        let panel = PanelController(
            model: model, logDirectory: data.logDirectory, offersTestNotifications: true)
        panel.onTestNotifications = { [weak notifier] in
            notifier?.show((try? Handoff.notifications()) ?? [])
        }
        #else
        let panel = PanelController(model: model, logDirectory: data.logDirectory)
        #endif
        self.panel = panel
        panel.onOpenChanged = { [weak self, weak poller] open in
            poller?.setPanelOpen(open)
            self?.ticker?.isOpen = open
        }
        panel.onStartService = { [weak self] in self?.startService() }
        panel.onSetLaunchAtLogin = { [weak self] in self?.setLaunchAtLogin($0) }
        panel.onOpenLoginItems = { AppLoginItem.openSettings() }
        notifier.onAction = { [weak self] button, target in
            self?.notificationAction(button, target: target)
        }
        poller.onUpdate = { [weak self] result in
            self?.show(result)
        }
        model.stopper = client
        model.onKillSent = { [weak self] lease in
            self?.rules.noteLocalKill(lease)
        }
        model.onStopAnswered = { [weak poller] lease, kind, result in
            Self.logStop(lease, kind, result)
            poller?.pollNow()
        }
        statusController = StatusItemController(
            model: model, panel: panel,
            controls: ServiceControls(
                status: { [weak poller] in poller?.latest?.status ?? .starting },
                stopConfirmation: { [weak poller] in
                    ServiceMenu.stopConfirmation(poller?.latest?.state)
                },
                start: { [weak self] in self?.startService() },
                stop: { [weak self] in self?.stopService() },
                setLaunchAtLogin: { [weak self] in self?.setLaunchAtLogin($0) }))
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

    // Settings, login item, drive names and the ticker, shared by the real app and scenarios.
    private func makeModel(settingsFile: URL) -> PanelModel {
        let store = SettingsStore(file: settingsFile)
        let model = PanelModel(settings: store.load())
        #if DEBUG
        model.loginItem = MemoryLoginItem()
        #else
        model.loginItem = AppLoginItem()
        #endif
        model.volumeName = { VolumeNames.name(mountedAt: $0) }
        model.onSettingsChanged = { settings in
            do {
                try store.save(settings)
            } catch {
                let reason = String(describing: error)
                Self.log.error(
                    "saving \(settingsFile.path, privacy: .public): \(reason, privacy: .public)")
            }
        }
        let ticker = CardTicker(model: model)
        self.ticker = ticker
        model.onRefresh = { [weak ticker] in
            ticker?.reschedule()
        }
        self.model = model
        return model
    }

    private func show(_ result: PollResult) {
        guard let model, let poller, let panel else {
            return
        }
        model.apply(result, lastKnown: poller.lastKnown, startProblem: poller.startProblem)
        logStatusChange(result)
        let update = rules.observe(
            status: result.status, live: result.state, lastKnown: poller.lastKnown,
            toggles: model.settings.notifications, panelOpen: panel.isOpen)
        notifier?.apply(update)
        if model.snapshot.showsFirstRunForTheFirstTime(model.settings) {
            showFirstRun(model, panel)
        }
    }

    // Once ever: Launch at login takes its default, on, the panel opens by itself, and macOS
    // asks whether the app may notify.
    private func showFirstRun(_ model: PanelModel, _ panel: PanelController) {
        model.updateSettings { $0.firstRunShown = true }
        if model.login.canToggle {
            setLaunchAtLogin(true)
        }
        notifier?.requestPermission()
        panel.open()
        Self.log.info("first run shown")
    }

    private func startService() {
        #if DEBUG
        if let scenarios {
            scenarios.startService()
            return
        }
        #endif
        guard let poller else {
            return
        }
        Task {
            if let problem = await poller.startService() {
                Self.log.error("start service: \(problem, privacy: .public)")
            } else {
                Self.log.info("start service: launched")
            }
        }
    }

    private func stopService() {
        guard let poller else {
            return
        }
        rules.expectStop()
        Task {
            if let problem = await poller.stopService() {
                Self.log.error("stop service: \(problem, privacy: .public)")
            } else {
                Self.log.info("stop service: accepted")
            }
        }
    }

    private func setLaunchAtLogin(_ on: Bool) {
        do {
            try model?.setLaunchAtLogin(on)
            Self.log.info("launch at login \(on ? "on" : "off", privacy: .public)")
        } catch {
            let state = on ? "on" : "off"
            let reason = String(describing: error)
            Self.log.error(
                "launch at login \(state, privacy: .public): \(reason, privacy: .public)")
        }
    }

    private func notificationAction(_ button: NotificationButton?, target: String?) {
        switch button {
        case .startService:
            startService()
        case .openLogs:
            panel?.openLogs()
        case .show, nil:
            panel?.open(scrollingTo: target)
        }
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
    // A scenario never polls and never notifies: it shows the handoff's state, its own stopper
    // ends jobs, and its settings live in tray-scenario.json.
    private func launchScenario(_ name: String) {
        let model = makeModel(
            settingsFile: data.root.appendingPathComponent("tray-scenario.json"))
        let player = ScenarioPlayer(model: model)
        let panel = PanelController(
            model: model, logDirectory: data.logDirectory, scenarios: Handoff.scenarioNames)
        self.panel = panel
        panel.onOpenChanged = { [weak self] open in
            self?.ticker?.isOpen = open
        }
        panel.onShowScenario = { [weak self] in
            self?.showScenario($0)
        }
        panel.onStartService = { [weak self] in self?.startService() }
        panel.onSetLaunchAtLogin = { [weak self] in self?.setLaunchAtLogin($0) }
        panel.onOpenLoginItems = { AppLoginItem.openSettings() }
        scenarios = player
        statusController = StatusItemController(model: model, panel: panel)
        showScenario(name)
        if model.snapshot.showsFirstRunForTheFirstTime(model.settings) {
            model.updateSettings { $0.firstRunShown = true }
            panel.open()
        }
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
