/// One switch on the Settings window's Panel page, over a toggle in `tray.json`.
public struct PanelToggle {
    public let label: String

    /// The line under it, or empty.
    public let help: String

    public let key: WritableKeyPath<TraySettings, Bool>

    /// The toggle this one is indented under and needs on, or `nil`.
    public let parent: WritableKeyPath<TraySettings, Bool>?

    init(
        _ label: String,
        key: WritableKeyPath<TraySettings, Bool>,
        help: String = "",
        parent: WritableKeyPath<TraySettings, Bool>? = nil
    ) {
        self.label = label
        self.key = key
        self.help = help
        self.parent = parent
    }

    /// The toggle's value in a fresh `tray.json`.
    public var defaultValue: Bool {
        TraySettings()[keyPath: key]
    }

    /// Whether the switch can be changed: its parent, if any, is on.
    public func isEnabled(in settings: TraySettings) -> Bool {
        parent.map { settings[keyPath: $0] } ?? true
    }
}

/// The Panel page's Meters and Notifications groups and its text size (DESIGN.md section 4,
/// Panel), and its Reset Page. Launch at Login lives in the login item, not here, and Reset Page
/// leaves it alone.
public enum PanelToggles {
    public static let metersHelp = "Rows shown in the panel."

    public static let textSizeLabel = "Text size"

    /// The meters; VRAM reads "GPU memory" where the GPU shares system memory.
    public static func meters(sharesMemory: Bool) -> [PanelToggle] {
        [
            PanelToggle("CPU", key: \.meters.cpu),
            PanelToggle("RAM", key: \.meters.ram),
            PanelToggle("GPU", key: \.meters.gpu),
            PanelToggle(sharesMemory ? "GPU memory" : "VRAM", key: \.meters.vram),
            PanelToggle("Drives", key: \.meters.drives),
            PanelToggle(
                "Drive activity", key: \.meters.driveActivity, parent: \.meters.drives),
        ]
    }

    public static var notifications: [PanelToggle] {
        [
            PanelToggle(
                "Service stopped", key: \.notifications.serviceStopped,
                help: "When the service stops without you stopping it."),
            PanelToggle(
                "Running long", key: \.notifications.runningLong,
                help: "When a job runs 1.5 times longer than expected."),
            PanelToggle(
                "Benchmark finished", key: \.notifications.benchmarkFinished,
                help: "When a benchmark you started ends."),
            PanelToggle(
                "Job finished", key: \.notifications.jobFinished, help: "When any job ends."),
            PanelToggle(
                "Killed outside the panel", key: \.notifications.killedOutsidePanel,
                help: "When a job is stopped by something other than this panel."),
        ]
    }

    /// One line per setting that differs from its default, with the default, for Reset Page's
    /// confirmation: "Job finished: Off".
    public static func changedLines(_ settings: TraySettings, sharesMemory: Bool) -> [String] {
        let fresh = TraySettings()
        var lines = (meters(sharesMemory: sharesMemory) + notifications)
            .filter { settings[keyPath: $0.key] != $0.defaultValue }
            .map { "\($0.label): \(SettingsText.onOff($0.defaultValue))" }
        if settings.textSize != fresh.textSize {
            lines.append("\(textSizeLabel): \(fresh.textSize.title)")
        }
        return lines
    }

    /// Puts every meter and notification toggle and the text size back to their defaults.
    public static func reset(_ settings: inout TraySettings) {
        let fresh = TraySettings()
        for toggle in meters(sharesMemory: false) + notifications {
            settings[keyPath: toggle.key] = fresh[keyPath: toggle.key]
        }
        settings.textSize = fresh.textSize
    }
}
