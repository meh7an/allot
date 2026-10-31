import Foundation

/// What the GUI remembers in `tray.json`, in the Windows tray app's format, so either app reads
/// the other's file. Members the file lacks keep their defaults and unknown ones are ignored.
/// Launch at login is not here: it lives in the login item itself.
public struct TraySettings: Codable, Sendable, Equatable {
    public var meters = MeterToggles()
    public var notifications = NotificationToggles()

    /// Whether the Resources card shows its meter rows; collapsed shows the summary line.
    public var resourcesExpanded = false

    /// The first-run card was dismissed with Done.
    public var firstRunDone = false

    /// The first-run card has shown once: the panel opened by itself and Launch at login was
    /// turned on, which neither does again.
    public var firstRunShown = false

    public init() {}

    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let defaults = Self()
        meters = try container.decodeIfPresent(MeterToggles.self, forKey: .meters)
            ?? defaults.meters
        notifications = try container.decodeIfPresent(
            NotificationToggles.self, forKey: .notifications) ?? defaults.notifications
        resourcesExpanded = try container.decodeIfPresent(Bool.self, forKey: .resourcesExpanded)
            ?? defaults.resourcesExpanded
        firstRunDone = try container.decodeIfPresent(Bool.self, forKey: .firstRunDone)
            ?? defaults.firstRunDone
        firstRunShown = try container.decodeIfPresent(Bool.self, forKey: .firstRunShown)
            ?? defaults.firstRunShown
    }

    private enum CodingKeys: String, CodingKey {
        case meters
        case notifications
        case resourcesExpanded = "resources_expanded"
        case firstRunDone = "first_run_done"
        case firstRunShown = "first_run_shown"
    }
}

/// The More options › Meters check items; all on by default.
public struct MeterToggles: Codable, Sendable, Equatable {
    public var cpu = true
    public var ram = true
    public var gpu = true
    public var vram = true
    public var drives = true

    /// Read and write rates on the drive rows; disabled in the menu while Drives is off.
    public var driveActivity = true

    public init() {}

    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        func toggle(_ key: CodingKeys) throws -> Bool {
            try container.decodeIfPresent(Bool.self, forKey: key) ?? true
        }
        cpu = try toggle(.cpu)
        ram = try toggle(.ram)
        gpu = try toggle(.gpu)
        vram = try toggle(.vram)
        drives = try toggle(.drives)
        driveActivity = try toggle(.driveActivity)
    }

    private enum CodingKeys: String, CodingKey {
        case cpu
        case ram
        case gpu
        case vram
        case drives
        case driveActivity = "drive_activity"
    }
}

/// The More options › Notifications check items.
public struct NotificationToggles: Codable, Sendable, Equatable {
    public var serviceStopped = true
    public var runningLong = true
    public var benchmarkFinished = true
    public var jobFinished = false
    public var killedOutsidePanel = false

    public init() {}

    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let defaults = Self()
        func toggle(_ key: CodingKeys, _ fallback: Bool) throws -> Bool {
            try container.decodeIfPresent(Bool.self, forKey: key) ?? fallback
        }
        serviceStopped = try toggle(.serviceStopped, defaults.serviceStopped)
        runningLong = try toggle(.runningLong, defaults.runningLong)
        benchmarkFinished = try toggle(.benchmarkFinished, defaults.benchmarkFinished)
        jobFinished = try toggle(.jobFinished, defaults.jobFinished)
        killedOutsidePanel = try toggle(.killedOutsidePanel, defaults.killedOutsidePanel)
    }

    // The Windows name of the last key is kept: both apps share the file.
    private enum CodingKeys: String, CodingKey {
        case serviceStopped = "service_stopped"
        case runningLong = "running_long"
        case benchmarkFinished = "benchmark_finished"
        case jobFinished = "job_finished"
        case killedOutsidePanel = "killed_outside_flyout"
    }
}
