import Foundation

/// A version as allot's binaries print it, `major.minor.patch` with an optional pre-release
/// such as `0.6.0-beta.1`, ordered as semantic versioning orders them: a pre-release comes
/// before its release.
public struct ServiceVersion: Sendable, Comparable, CustomStringConvertible {
    public let description: String
    private let release: [Int]
    private let preRelease: [String]

    /// - Parameter text: For example `0.6.0` or `0.6.0-beta.1`; build metadata after `+` is
    ///   ignored.
    public init?(_ text: String) {
        let version = text.prefix { $0 != "+" }
        let parts = version.split(separator: "-", maxSplits: 1, omittingEmptySubsequences: false)
        let numbers = parts[0].split(separator: ".", omittingEmptySubsequences: false)
        let release = numbers.compactMap(Self.number)
        guard numbers.count == 3, release.count == 3 else {
            return nil
        }
        let preRelease = parts.count == 2
            ? parts[1].split(separator: ".", omittingEmptySubsequences: false).map(String.init)
            : []
        guard !preRelease.contains(where: \.isEmpty) else {
            return nil
        }
        description = String(version)
        self.release = release
        self.preRelease = preRelease
    }

    /// The version in what `<program> --version` prints, such as `allotd 0.6.0`.
    public static func printed(by program: String, _ output: String) -> ServiceVersion? {
        let line = output.split(whereSeparator: \.isNewline).first ?? ""
        let words = line.split(separator: " ")
        guard words.count == 2, words[0] == program else {
            return nil
        }
        return ServiceVersion(String(words[1]))
    }

    public static func == (lhs: Self, rhs: Self) -> Bool {
        lhs.release == rhs.release && lhs.preRelease == rhs.preRelease
    }

    public static func < (lhs: Self, rhs: Self) -> Bool {
        if lhs.release != rhs.release {
            return lhs.release.lexicographicallyPrecedes(rhs.release)
        }
        switch (lhs.preRelease.isEmpty, rhs.preRelease.isEmpty) {
        case (true, _):
            return false
        case (false, true):
            return true
        case (false, false):
            return precedes(lhs.preRelease, rhs.preRelease)
        }
    }

    // Numeric identifiers compare as numbers and sort before alphanumeric ones; a shorter list
    // that the longer one starts with comes first.
    private static func precedes(_ lhs: [String], _ rhs: [String]) -> Bool {
        for (left, right) in zip(lhs, rhs) where left != right {
            switch (number(Substring(left)), number(Substring(right))) {
            case let (leftNumber?, rightNumber?):
                return leftNumber < rightNumber
            case (.some, nil):
                return true
            case (nil, .some):
                return false
            case (nil, nil):
                return left < right
            }
        }
        return lhs.count < rhs.count
    }

    // Digits only: Int alone would also take a sign.
    private static func number(_ text: Substring) -> Int? {
        guard !text.isEmpty, text.allSatisfy({ $0.isASCII && $0.isNumber }) else {
            return nil
        }
        return Int(text)
    }
}

/// What `~/.allot/bin` holds.
public enum InstalledService: Sendable, Equatable {
    case missing

    case version(ServiceVersion)

    /// An allotd whose `--version` printed no version, such as one Gatekeeper stops.
    case unreadable
}

/// The service binaries this app carries while `~/.allot/bin` holds older ones, none, or ones
/// whose version can't be read.
public struct ServiceUpdate: Sendable, Equatable {
    public let installed: InstalledService

    public let bundled: ServiceVersion

    /// The bundled `allot install` runs.
    public var installing = false

    /// Why the last update failed, or `nil`.
    public var problem: String?

    public init(installed: InstalledService, bundled: ServiceVersion) {
        self.installed = installed
        self.bundled = bundled
    }
}

/// What the app does with the binaries it carries when it starts.
public enum ServiceUpdatePlan: Sendable, Equatable {
    /// `~/.allot/bin` holds this version or a newer one: the app never downgrades.
    case upToDate

    /// The first run of this app version installs at once.
    case installNow(ServiceUpdate)

    /// Any later run waits for Update Service.
    case offer(ServiceUpdate)

    /// - Parameters:
    ///   - installed: What `~/.allot/bin` holds.
    ///   - bundled: The version of the allotd this app carries.
    ///   - firstRunOfVersion: This app version has not run before.
    public static func decide(
        installed: InstalledService,
        bundled: ServiceVersion,
        firstRunOfVersion: Bool
    ) -> ServiceUpdatePlan {
        let update = ServiceUpdate(installed: installed, bundled: bundled)
        switch installed {
        case .version(let version) where version >= bundled:
            return .upToDate
        // It may be newer than the app, so only the user replaces it.
        case .unreadable:
            return .offer(update)
        case .missing, .version:
            return firstRunOfVersion ? .installNow(update) : .offer(update)
        }
    }
}

/// Runs the binaries the app carries and the installed ones; the tests pass a fake.
public protocol ProgramRunner: Sendable {
    /// - Returns: What the program printed, or `nil` when it could not run or failed.
    func output(_ program: URL, _ arguments: [String]) async -> String?

    /// Runs a program to its end with its output and errors in `log`, which it replaces.
    ///
    /// - Returns: The exit status, or `nil` when the program or the log could not be opened.
    func run(
        _ program: URL,
        _ arguments: [String],
        log: URL,
        environment: [String: String]
    ) async -> Int32?
}

/// Installs the `allot`, `allotd` and `allot-mcp` this app carries in `Contents/Helpers` into
/// `~/.allot/bin` through the bundled `allot install`, which also restarts allotd through the
/// LaunchAgent `tech.mehran.allotd` and sets up Claude Code.
@MainActor
public final class ServiceUpdater {
    public static let helpersPath = "Contents/Helpers"

    /// The output of the last update, in the logs folder.
    public static let logName = "app-update.log"

    private static let daemonName = "allotd"

    private let helpers: URL
    private let data: DataDirectory
    private let runner: any ProgramRunner

    /// - Parameters:
    ///   - bundle: The app bundle.
    ///   - data: The install the binaries go into.
    ///   - runner: Runs the binaries.
    public init(bundle: URL, data: DataDirectory, runner: any ProgramRunner) {
        helpers = bundle.appendingPathComponent(Self.helpersPath, isDirectory: true)
        self.data = data
        self.runner = runner
    }

    public var logFile: URL {
        data.logDirectory.appendingPathComponent(Self.logName)
    }

    /// The version of the allotd this app carries, or `nil` when it carries none, as a build
    /// from Xcode does.
    public func bundledVersion() async -> ServiceVersion? {
        let required = ["allot", Self.daemonName, "allot-mcp"]
        guard required.allSatisfy({ isExecutable(helpers.appendingPathComponent($0)) }) else {
            return nil
        }
        return await version(of: helpers.appendingPathComponent(Self.daemonName))
    }

    public func installedService() async -> InstalledService {
        let daemon = data.root.appendingPathComponent("bin/\(Self.daemonName)")
        guard isExecutable(daemon) else {
            return .missing
        }
        return await version(of: daemon).map(InstalledService.version) ?? .unreadable
    }

    /// Runs the bundled `allot install`, with its output in ``logFile``.
    ///
    /// - Returns: `nil` when it finished, else why not.
    public func install() async -> String? {
        let allot = helpers.appendingPathComponent("allot")
        let home = data.root.deletingLastPathComponent()
        let environment = Self.environment(ProcessInfo.processInfo.environment, home: home)
        let status = await runner.run(allot, ["install"], log: logFile, environment: environment)
        switch status {
        case 0?:
            return nil
        case let code?:
            return "allot install exited with \(code); see \(Self.logName) in the logs"
        case nil:
            return "\(allot.path) could not run"
        }
    }

    /// An app opened from Finder gets launchd's PATH, which lacks where Claude Code's installer
    /// and Homebrew put `claude`; `allot install` runs it to register the MCP server.
    nonisolated static func environment(
        _ base: [String: String],
        home: URL
    ) -> [String: String] {
        var environment = base
        let first = [home.appendingPathComponent(".local/bin").path, "/opt/homebrew/bin",
                     "/usr/local/bin"]
        let rest = (base["PATH"] ?? "/usr/bin:/bin:/usr/sbin:/sbin").split(separator: ":")
            .map(String.init)
            .filter { !first.contains($0) }
        environment["PATH"] = (first + rest).joined(separator: ":")
        return environment
    }

    private func version(of program: URL) async -> ServiceVersion? {
        guard let output = await runner.output(program, ["--version"]) else {
            return nil
        }
        return ServiceVersion.printed(by: Self.daemonName, output)
    }

    private func isExecutable(_ file: URL) -> Bool {
        FileManager.default.isExecutableFile(atPath: file.path)
    }
}

extension ProcessRunner: ProgramRunner {
    // The pipe is read once the program ends, which holds for a few lines such as --version.
    public func output(_ program: URL, _ arguments: [String]) async -> String? {
        await withCheckedContinuation { continuation in
            let process = Process()
            process.executableURL = program
            process.arguments = arguments
            process.standardInput = FileHandle.nullDevice
            process.standardOutput = Pipe()
            process.standardError = FileHandle.nullDevice
            process.terminationHandler = { process in
                let pipe = process.standardOutput as? Pipe
                let data = pipe?.fileHandleForReading.readDataToEndOfFile() ?? Data()
                let text = process.terminationStatus == 0
                    ? String(data: data, encoding: .utf8) : nil
                continuation.resume(returning: text)
            }
            do {
                try process.run()
            } catch {
                continuation.resume(returning: nil)
            }
        }
    }

    public func run(
        _ program: URL,
        _ arguments: [String],
        log: URL,
        environment: [String: String]
    ) async -> Int32? {
        let files = FileManager.default
        guard (try? files.createDirectory(
            at: log.deletingLastPathComponent(), withIntermediateDirectories: true)) != nil,
            files.createFile(atPath: log.path, contents: nil),
            let handle = try? FileHandle(forWritingTo: log)
        else {
            return nil
        }
        return await withCheckedContinuation { continuation in
            let process = Process()
            process.executableURL = program
            process.arguments = arguments
            process.environment = environment
            process.currentDirectoryURL = URL(fileURLWithPath: "/")
            process.standardInput = FileHandle.nullDevice
            process.standardOutput = handle
            process.standardError = handle
            process.terminationHandler = { process in
                try? (process.standardOutput as? FileHandle)?.close()
                continuation.resume(returning: process.terminationStatus)
            }
            do {
                try process.run()
            } catch {
                try? handle.close()
                continuation.resume(returning: nil)
            }
        }
    }
}
