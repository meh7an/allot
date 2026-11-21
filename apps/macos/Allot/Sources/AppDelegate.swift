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
    private var settingsModel: SettingsWindowModel?
    private var settingsWindow: SettingsWindowController?
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
        notifier.onPermission = { [weak model] in model?.setNotificationPermission($0) }
        self.notifier = notifier
        Task {
            await notifier.refreshPermission()
        }
        let client = DaemonClient.forInstall(data)
        let poller = DaemonPoller(
            client: client,
            port: DaemonConfig.readPort(data),
            launcher: LaunchdServiceLauncher(data: data, runner: ProcessRunner()))
        self.poller = poller
        #if DEBUG
        let panel = PanelController(
            model: model, logDirectory: data.logDirectory, offersSettings: true,
            offersTestNotifications: true)
        panel.onTestNotifications = { [weak notifier] in
            notifier?.show((try? Handoff.notifications()) ?? [])
        }
        #else
        let panel = PanelController(
            model: model, logDirectory: data.logDirectory, offersSettings: true)
        #endif
        self.panel = panel
        panel.onOpenChanged = { [weak self, weak poller, weak notifier] open in
            poller?.setPanelOpen(open)
            self?.ticker?.isOpen = open
            // System Settings may have turned notifications on or off meanwhile.
            if open, let notifier {
                Task {
                    await notifier.refreshPermission()
                }
            }
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
        let controls = ServiceControls(
            status: { [weak poller] in poller?.latest?.status ?? .starting },
            stopConfirmation: { [weak poller] in
                ServiceMenu.stopConfirmation(poller?.latest?.state)
            },
            start: { [weak self] in self?.startService() },
            stop: { [weak self] in self?.stopService() },
            openSettings: { [weak self] in self?.openSettings() })
        panel.onOpenSettings = controls.openSettings
        panel.onChooseService = { controls.chooseService() }
        statusController = StatusItemController(model: model, panel: panel, controls: controls)
        settingsModel = SettingsWindowModel(
            session: SettingsSession.forInstall(client: client, data: data),
            panel: model,
            data: data,
            actions: SettingsActions(
                startService: { [weak self] in self?.startService() },
                stopConfirmation: { controls.stopConfirmation() },
                stopService: { [weak self] in await self?.stopServiceReportingProblem() },
                restartService: { [weak self] in await self?.restartService() },
                port: { [weak poller] in poller?.port ?? DaemonConfig.defaultPort },
                startProblem: { [weak poller] in poller?.startProblem },
                openLogs: { [weak panel] in panel?.openLogs() },
                setLaunchAtLogin: { [weak self] in self?.setLaunchAtLogin($0) },
                openLoginItems: { AppLoginItem.openSettings() }))
        installMainMenu()
        pollTask = Task {
            await poller.run()
        }
        if let cycle = MeasureCycle.requested(
            panel: panel, model: model, logDirectory: data.logDirectory)
        {
            Task {
                await cycle.run()
            }
        }
        Self.log.info("launched from \(Bundle.main.bundlePath, privacy: .public)")
    }

    /// Quit sends what Settings has not saved first: an edit inside the 600 ms burst or a save in
    /// flight would be lost to the exit. It waits at most two seconds.
    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        guard let session = settingsModel?.session else {
            return .terminateNow
        }
        Task {
            await session.settle(timeout: .seconds(2))
            NSApp.reply(toApplicationShouldTerminate: true)
        }
        return .terminateLater
    }

    func applicationWillTerminate(_ notification: Notification) {
        pollTask?.cancel()
    }

    func applicationSupportsSecureRestorableState(_ app: NSApplication) -> Bool {
        true
    }

    /// The Dock icon while Settings shows, or opening the app again, opens Settings.
    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows: Bool) -> Bool {
        openSettings()
        return false
    }

    /// Closing Settings must never quit a menu bar app.
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        false
    }

    // Settings, login item, drive names and the ticker, shared by the real app and scenarios.
    private func makeModel(settingsFile: URL) -> PanelModel {
        let store = SettingsStore(file: settingsFile)
        let model = PanelModel(settings: store.load())
        #if DEBUG
        model.loginItem = MemoryLoginItem()
        #else
        let loginItem = AppLoginItem()
        loginItem.onChange = { [weak model] in model?.refreshLogin() }
        model.loginItem = loginItem
        #endif
        model.volumeName = { VolumeNames.name(mountedAt: $0) }
        TextScale.shared.size = model.settings.textSize
        model.onSettingsChanged = { settings in
            TextScale.shared.size = settings.textSize
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
        settingsModel?.apply(result)
        panel.prepareOnce()
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

    // Built on the first open; a Debug scenario has no Settings.
    private func openSettings() {
        guard let settingsModel else {
            return
        }
        panel?.close()
        let window = settingsWindow ?? SettingsWindowController(model: settingsModel)
        settingsWindow = window
        window.show()
    }

    private func restartService() async -> String? {
        guard let poller else {
            return "the app does not poll the service"
        }
        rules.expectStop()
        let port = DaemonConfig.readPort(data)
        let problem = await poller.restartService(port: port)
        if let problem {
            Self.log.error("restart service: \(problem, privacy: .public)")
        } else {
            Self.log.info("restart service: launched on port \(port)")
        }
        return problem
    }

    /// An app menu with Settings… and Quit, File › Close and an Edit menu. It shows only while
    /// Settings puts the app in the Dock, but its keys work in the Settings window always:
    /// Cmd-W, Cmd-Q, and copy and paste in its fields (DESIGN.md R5).
    private func installMainMenu() {
        let main = NSMenu()
        let appMenu = NSMenu(title: "Allot")
        let settings = appMenu.addItem(
            withTitle: ServiceMenu.settingsTitle, action: #selector(openSettingsFromMenu),
            keyEquivalent: ",")
        settings.target = self
        appMenu.addItem(.separator())
        appMenu.addItem(
            withTitle: "Quit Allot", action: #selector(NSApplication.terminate(_:)),
            keyEquivalent: "q")
        main.addSubmenu(appMenu)

        let fileMenu = NSMenu(title: "File")
        fileMenu.addItem(
            withTitle: "Close", action: #selector(NSWindow.performClose(_:)), keyEquivalent: "w")
        main.addSubmenu(fileMenu)

        let editMenu = NSMenu(title: "Edit")
        editMenu.addItem(withTitle: "Undo", action: Selector(("undo:")), keyEquivalent: "z")
        editMenu.addItem(withTitle: "Redo", action: Selector(("redo:")), keyEquivalent: "Z")
        editMenu.addItem(.separator())
        editMenu.addItem(withTitle: "Cut", action: #selector(NSText.cut(_:)), keyEquivalent: "x")
        editMenu.addItem(withTitle: "Copy", action: #selector(NSText.copy(_:)), keyEquivalent: "c")
        editMenu.addItem(
            withTitle: "Paste", action: #selector(NSText.paste(_:)), keyEquivalent: "v")
        editMenu.addItem(
            withTitle: "Select All", action: #selector(NSText.selectAll(_:)), keyEquivalent: "a")
        main.addSubmenu(editMenu)
        NSApp.mainMenu = main
    }

    @objc private func openSettingsFromMenu() {
        openSettings()
    }

    private func stopService() {
        Task {
            _ = await stopServiceReportingProblem()
        }
    }

    // The panel and menu only log a refused stop; Settings shows it as a notice.
    private func stopServiceReportingProblem() async -> String? {
        guard let poller else {
            return "the app does not poll the service"
        }
        rules.expectStop()
        let problem = await poller.stopService()
        if let problem {
            Self.log.error("stop service: \(problem, privacy: .public)")
        } else {
            Self.log.info("stop service: accepted")
        }
        return problem
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
        // The detail can hold the account name, as in the path of the token file.
        Self.log.info("service \(status, privacy: .public): \(detail, privacy: .private)")
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

private extension NSMenu {
    func addSubmenu(_ submenu: NSMenu) {
        let item = NSMenuItem(title: submenu.title, action: nil, keyEquivalent: "")
        item.submenu = submenu
        addItem(item)
    }
}
