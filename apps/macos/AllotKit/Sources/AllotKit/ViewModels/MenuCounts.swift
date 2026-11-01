/// The counts of the More options menu's Meters and Notifications items.
public enum MenuCounts {
    private static let meterItems = 5
    private static let notificationItems = 5

    /// For example `4 of 5`; Drive activity is part of Drives, so it does not count.
    public static func meters(_ toggles: MeterToggles) -> String {
        let on = [toggles.cpu, toggles.ram, toggles.gpu, toggles.vram, toggles.drives]
        return "\(on.filter { $0 }.count) of \(meterItems)"
    }

    /// For example `3 of 5`, or `Off` while macOS has notifications off for the app, since then
    /// none of them shows.
    public static func notifications(_ toggles: NotificationToggles, off: Bool = false) -> String {
        if off {
            return "Off"
        }
        let on = [
            toggles.serviceStopped,
            toggles.runningLong,
            toggles.benchmarkFinished,
            toggles.jobFinished,
            toggles.killedOutsidePanel,
        ]
        return "\(on.filter { $0 }.count) of \(notificationItems)"
    }
}
