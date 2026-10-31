import Foundation

/// Locations of the per-user files under `~/.allot`, as in `allot_shared::Paths`.
public struct DataDirectory: Sendable, Equatable {
    private static let appDirectory = ".allot"

    public let root: URL

    public init(root: URL) {
        self.root = root
    }

    /// The install under `$HOME`, which is where allotd looks too. A GUI app launched by
    /// launchd always has HOME; the account's home directory covers the rare case without it.
    public static func fromEnvironment(
        _ environment: [String: String] = ProcessInfo.processInfo.environment
    ) -> DataDirectory {
        let home = environment["HOME"].flatMap { $0.isEmpty ? nil : $0 } ?? NSHomeDirectory()
        return under(home: URL(fileURLWithPath: home, isDirectory: true))
    }

    /// The install rooted at `<home>/.allot`.
    public static func under(home: URL) -> DataDirectory {
        DataDirectory(root: home.appendingPathComponent(appDirectory, isDirectory: true))
    }

    /// The bearer token clients send; allotd writes it on first start.
    public var tokenFile: URL {
        root.appendingPathComponent("token")
    }

    /// The daemon config, which holds the port. Optional; allotd runs on defaults without it.
    public var configFile: URL {
        root.appendingPathComponent("config.toml")
    }

    public var logDirectory: URL {
        root.appendingPathComponent("logs", isDirectory: true)
    }

    /// The GUI's own settings, shared in format with the Windows tray app.
    public var settingsFile: URL {
        root.appendingPathComponent("tray.json")
    }
}
