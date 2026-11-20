import AllotKit
import AppKit
import Observation
import SwiftUI

/// What the Settings window asks the app to do with the service and the login item.
@MainActor
struct SettingsActions {
    let startService: @MainActor () -> Void

    /// What Stop Service asks first, or `nil` while no job runs or waits.
    let stopConfirmation: @MainActor () -> String?

    /// Stops allotd with the stop record; the window has asked first.
    let stopService: @MainActor () -> Void

    /// Stops allotd and starts it on the port `config.toml` names now; `nil` when it worked,
    /// else why not.
    let restartService: @MainActor () async -> String?

    let openLogs: @MainActor () -> Void
    let setLaunchAtLogin: @MainActor (Bool) -> Void
    let openLoginItems: @MainActor () -> Void
}

/// A brief notice at the bottom of the window, with Undo where the change can be undone.
struct SettingsNotice: Identifiable {
    let id = UUID()
    let message: String
    let undo: (@MainActor () -> Void)?
}

/// The Settings window's state shared by its pages: allotd's settings, the fit preview, the
/// panel's own settings, the latest poll for the Service row, and the notice and confirmations.
@MainActor
@Observable
final class SettingsWindowModel {
    /// The pages, in toolbar order: id, label and SF Symbol (DESIGN.md section 10, macOS).
    static let pages: [(id: String, label: String, symbol: String)] = [
        (SettingsBanners.panelPage, "Panel", "gauge.with.dots.needle.33percent"),
        ("admission", "Admission", "scalemass"),
        ("benchmarks", "Benchmarks", "timer"),
        ("leases", "Leases", "key"),
        (SettingsBanners.servicePage, "Service", "server.rack"),
    ]

    private static let noticeTime = Duration.seconds(10)

    // The fit preview runs while a page that shows it is open.
    private static let fitPages: Set<String> = ["admission"]

    let session: SettingsSession
    let fits: FitPreviews
    let panel: PanelModel
    let data: DataDirectory
    let number = SettingsText.numbers(.current)

    /// The latest poll, for the Service page's status row.
    private(set) var serviceStatus = ServiceStatus.starting
    private(set) var version: String?

    private(set) var notice: SettingsNotice?

    @ObservationIgnored let actions: SettingsActions

    /// Shows the confirmations as sheets on it.
    @ObservationIgnored weak var window: NSWindow?

    @ObservationIgnored private var pageId = SettingsBanners.panelPage
    @ObservationIgnored private var visible = false
    @ObservationIgnored private var noticeTask: Task<Void, Never>?
    @ObservationIgnored private var announcedStatus = ""

    init(
        session: SettingsSession,
        panel: PanelModel,
        data: DataDirectory,
        actions: SettingsActions
    ) {
        self.session = session
        self.panel = panel
        self.data = data
        self.actions = actions
        fits = FitPreviews(session: session, timers: TaskTimers())
        session.onChange = { [weak self] in
            self?.sessionChanged()
        }
    }

    /// The port this panel dials, from `config.toml`.
    var port: UInt16 {
        DaemonConfig.readPort(data)
    }

    /// The save status line, or Loading while allotd has not answered yet.
    var saveStatus: String {
        session.status == .loading
            ? SettingsText.loading : SettingsText.saveStatus(session.saveStatus)
    }

    /// Takes a poll; called after every poll of the app.
    func apply(_ result: PollResult) {
        if serviceStatus != result.status {
            serviceStatus = result.status
        }
        let newVersion = result.state?.service.version ?? result.version ?? version
        if newVersion != version {
            version = newVersion
        }
    }

    /// The window was shown or hidden: fetching runs only while it shows, and hiding sends what
    /// is pending.
    func setVisible(_ visible: Bool) {
        self.visible = visible
        session.setVisible(visible)
        fits.setActive(visible && Self.fitPages.contains(pageId))
        if visible {
            panel.refreshLogin()
        } else {
            dismissNotice()
        }
    }

    func selectPage(_ id: String) {
        pageId = id
        fits.setActive(visible && Self.fitPages.contains(id))
    }

    /// Values are fetched on focus too, so a hand edit shows without waiting for the next tick.
    func focused() {
        if visible {
            Task { try? await session.refresh() }
        }
    }

    func showNotice(_ message: String, undo: (@MainActor () -> Void)?) {
        noticeTask?.cancel()
        let shown = SettingsNotice(message: message, undo: undo)
        notice = shown
        AccessibilityNotification.Announcement(message).post()
        noticeTask = Task { [weak self] in
            try? await Task.sleep(for: Self.noticeTime)
            if !Task.isCancelled, self?.notice?.id == shown.id {
                self?.notice = nil
            }
        }
    }

    func undoNotice() {
        let undo = notice?.undo
        dismissNotice()
        undo?()
    }

    func dismissNotice() {
        noticeTask?.cancel()
        noticeTask = nil
        notice = nil
    }

    /// Asks with a sheet on the window; `false` when it is cancelled or there is no window.
    func confirm(_ confirmation: Confirmation) async -> Bool {
        guard let window, window.attachedSheet == nil else {
            return false
        }
        let alert = NSAlert()
        alert.alertStyle = .warning
        alert.messageText = confirmation.title
        alert.informativeText = confirmation.body
        alert.addButton(withTitle: confirmation.button)
        alert.addButton(withTitle: "Cancel")
        return await alert.beginSheetModal(for: window) == .alertFirstButtonReturn
    }

    /// Stop Service, asking first while jobs run or wait.
    func stopService() async {
        if let confirmation = actions.stopConfirmation() {
            let asked = Confirmation(
                title: ServiceMenu.confirmTitle, body: confirmation, button: "Stop Service")
            guard await confirm(asked) else {
                return
            }
        }
        actions.stopService()
    }

    func run(_ action: BannerAction) {
        switch action {
        case .startService:
            actions.startService()
        case .restartService:
            Task { await restart() }
        case .tryNow, .checkAgain:
            Task { try? await session.refresh() }
        case .openLogs:
            actions.openLogs()
        case .openFile:
            ConfigFile.open(data.configFile)
        case .applyMine:
            session.applyMine()
        case .keepFileValues:
            session.keepFileValues()
        case .saveAgain:
            Task { await session.flush() }
        }
    }

    private func restart() async {
        if let problem = await actions.restartService() {
            showNotice("The service did not restart: \(problem)", undo: nil)
        }
        try? await session.refresh()
    }

    private func sessionChanged() {
        fits.sessionChanged()
        // The save status is read politely once per change, never per keystroke.
        let status = saveStatus
        if visible, status != announcedStatus {
            announcedStatus = status
            AccessibilityNotification.Announcement(status).post()
        }
    }
}
