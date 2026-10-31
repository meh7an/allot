import Foundation

/// Starts allotd for Start Service.
@MainActor
public protocol ServiceLauncher: AnyObject {
    /// Starts allotd without waiting for it to answer.
    ///
    /// - Returns: `nil` when it was started, else why not.
    func start() async -> String?
}

/// Runs a program to its end; the tests pass a fake.
public protocol CommandRunner: Sendable {
    /// - Returns: The exit status, or `nil` when the program could not run.
    func run(_ program: URL, _ arguments: [String]) async -> Int32?

    /// Starts a program that keeps running after the app quits.
    ///
    /// - Throws: Why it could not start.
    func spawn(_ program: URL) throws
}

/// Starts the installed allotd the way `allot install` set it up: through the LaunchAgent
/// `tech.mehran.allotd` when this home has one for this install's allotd (`kickstart` when it
/// is loaded, `bootstrap` when not), else by running `~/.allot/bin/allotd` directly, as the
/// Windows tray app does.
@MainActor
public final class LaunchdServiceLauncher: ServiceLauncher {
    public static let label = "tech.mehran.allotd"

    private static let launchctl = URL(fileURLWithPath: "/bin/launchctl")

    private let data: DataDirectory
    private let uid: UInt32
    private let runner: any CommandRunner

    /// - Parameters:
    ///   - data: The install; its home holds the LaunchAgent.
    ///   - uid: The user whose GUI domain the agent belongs to.
    ///   - runner: Runs `launchctl` and allotd.
    public init(data: DataDirectory, uid: UInt32 = getuid(), runner: any CommandRunner) {
        self.data = data
        self.uid = uid
        self.runner = runner
    }

    /// `~/Library/LaunchAgents/tech.mehran.allotd.plist` of the install's home.
    public var agentFile: URL {
        data.root.deletingLastPathComponent()
            .appendingPathComponent("Library/LaunchAgents/\(Self.label).plist")
    }

    public var daemonExe: URL {
        data.root.appendingPathComponent("bin/allotd")
    }

    public func start() async -> String? {
        if agentRunsThisDaemon() {
            let service = "gui/\(uid)/\(Self.label)"
            let loaded = await runner.run(Self.launchctl, ["print", service]) == 0
            let arguments = loaded
                ? ["kickstart", service]
                : ["bootstrap", "gui/\(uid)", agentFile.path]
            if await runner.run(Self.launchctl, arguments) == 0 {
                return nil
            }
            // launchctl's refusals are not worth a banner line while allotd can still run
            // directly; launchd takes it over at the next login.
        }
        return spawn()
    }

    private func spawn() -> String? {
        let exe = daemonExe
        guard FileManager.default.isExecutableFile(atPath: exe.path) else {
            return "\(exe.path) is missing; run allot install"
        }
        do {
            try runner.spawn(exe)
            return nil
        } catch {
            return "\(exe.path): \(error.localizedDescription)"
        }
    }

    // The agent of another install (a dev build, another home) must not be started for this
    // one.
    private func agentRunsThisDaemon() -> Bool {
        guard let plist = try? Data(contentsOf: agentFile),
              let object = try? PropertyListSerialization.propertyList(from: plist, format: nil),
              let arguments = (object as? [String: Any])?["ProgramArguments"] as? [String],
              let program = arguments.first
        else {
            return false
        }
        return URL(fileURLWithPath: program).standardizedFileURL.path
            == daemonExe.standardizedFileURL.path
    }
}

/// Runs programs with `Process`, which reaps them when they exit.
public struct ProcessRunner: CommandRunner {
    public init() {}

    public func run(_ program: URL, _ arguments: [String]) async -> Int32? {
        await withCheckedContinuation { continuation in
            let process = Process()
            process.executableURL = program
            process.arguments = arguments
            process.standardInput = FileHandle.nullDevice
            process.standardOutput = FileHandle.nullDevice
            process.standardError = FileHandle.nullDevice
            process.terminationHandler = { process in
                continuation.resume(returning: process.terminationStatus)
            }
            do {
                try process.run()
            } catch {
                continuation.resume(returning: nil)
            }
        }
    }

    public func spawn(_ program: URL) throws {
        let process = Process()
        process.executableURL = program
        process.currentDirectoryURL = URL(fileURLWithPath: "/")
        process.standardInput = FileHandle.nullDevice
        process.standardOutput = FileHandle.nullDevice
        process.standardError = FileHandle.nullDevice
        try process.run()
    }
}
