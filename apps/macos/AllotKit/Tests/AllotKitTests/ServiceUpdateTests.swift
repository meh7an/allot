import Foundation
import Testing

@testable import AllotKit

private func version(_ text: String) throws -> ServiceVersion {
    try #require(ServiceVersion(text))
}

/// Answers `--version` per program path and records the runs with a log.
private final class FakeProgramRunner: ProgramRunner, @unchecked Sendable {
    private let lock = NSLock()
    private let versions: [String: String]
    private let status: Int32?
    private var runs: [(program: URL, arguments: [String], environment: [String: String])] = []

    init(versions: [String: String] = [:], status: Int32? = 0) {
        self.versions = versions
        self.status = status
    }

    var ran: [(program: URL, arguments: [String], environment: [String: String])] {
        lock.withLock { runs }
    }

    func output(_ program: URL, _ arguments: [String]) async -> String? {
        versions[program.path]
    }

    func run(
        _ program: URL,
        _ arguments: [String],
        log: URL,
        environment: [String: String]
    ) async -> Int32? {
        lock.withLock {
            runs.append((program: program, arguments: arguments, environment: environment))
        }
        return status
    }
}

/// An app bundle and a home under a fresh temporary folder.
private struct Layout {
    let bundle: URL
    let data: DataDirectory

    init() {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("allot-update-\(UUID().uuidString)", isDirectory: true)
        bundle = root.appendingPathComponent("Allot.app", isDirectory: true)
        data = DataDirectory.under(home: root.appendingPathComponent("home", isDirectory: true))
    }

    var helpers: URL {
        bundle.appendingPathComponent(ServiceUpdater.helpersPath, isDirectory: true)
    }

    var installed: URL {
        data.root.appendingPathComponent("bin/allotd")
    }

    func addHelpers(_ names: [String] = ["allot", "allotd", "allot-mcp"]) throws {
        try names.forEach { try addExecutable(helpers.appendingPathComponent($0)) }
    }

    func addExecutable(_ file: URL) throws {
        try FileManager.default.createDirectory(
            at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
        try Data("#!/bin/sh\n".utf8).write(to: file)
        try FileManager.default.setAttributes([.posixPermissions: 0o755], ofItemAtPath: file.path)
    }
}

@Suite struct ServiceVersionTests {
    @Test func readsReleasesAndPreReleases() throws {
        let release = try version("0.6.0")
        let beta = try version("0.6.0-beta.1")
        let build = try version("0.6.0+build.5")

        #expect(release.description == "0.6.0")
        #expect(beta.description == "0.6.0-beta.1")
        #expect(build == release)
    }

    @Test func refusesWhatIsNotAVersion() {
        for text in ["", "0.6", "0.6.0.1", "0..6", "v0.6.0", "0.6.+1", "0.6.0-", "0.6.0-a..b"] {
            #expect(ServiceVersion(text) == nil, "\(text)")
        }
    }

    @Test func ordersAsSemanticVersioning() throws {
        let ordered = try [
            "0.5.9", "0.6.0-alpha", "0.6.0-alpha.1", "0.6.0-alpha.beta", "0.6.0-beta.2",
            "0.6.0-beta.11", "0.6.0", "0.6.1", "0.10.0", "1.0.0",
        ].map(version)

        for (lower, higher) in zip(ordered, ordered.dropFirst()) {
            #expect(lower < higher, "\(lower) < \(higher)")
            #expect(!(higher < lower), "\(higher) < \(lower)")
        }
    }

    @Test func readsWhatAProgramPrints() throws {
        let release = try version("0.6.0")

        #expect(ServiceVersion.printed(by: "allotd", "allotd 0.6.0\n") == release)
        #expect(ServiceVersion.printed(by: "allotd", "allot 0.6.0\n") == nil)
        #expect(ServiceVersion.printed(by: "allotd", "error: unexpected argument") == nil)
    }
}

@Suite struct ServiceUpdatePlanTests {
    @Test func neverDowngradesOrRepeats() throws {
        let bundled = try version("0.6.0")
        for installed in try ["0.6.0", "0.6.1", "0.7.0-beta.1"].map(version) {
            let plan = ServiceUpdatePlan.decide(
                installed: .version(installed), bundled: bundled, firstRunOfVersion: true)
            #expect(plan == .upToDate, "\(installed)")
        }
    }

    @Test func theFirstRunInstallsAndLaterRunsOffer() throws {
        let bundled = try version("0.6.0")
        let beta = try version("0.6.0-beta.1")
        for installed in [InstalledService.missing, .version(beta)] {
            let update = ServiceUpdate(installed: installed, bundled: bundled)
            #expect(
                ServiceUpdatePlan.decide(
                    installed: installed, bundled: bundled, firstRunOfVersion: true)
                    == .installNow(update))
            #expect(
                ServiceUpdatePlan.decide(
                    installed: installed, bundled: bundled, firstRunOfVersion: false)
                    == .offer(update))
        }
    }

    @Test func anUnreadableServiceIsOnlyOffered() throws {
        let bundled = try version("0.6.0")

        let plan = ServiceUpdatePlan.decide(
            installed: .unreadable, bundled: bundled, firstRunOfVersion: true)

        #expect(plan == .offer(ServiceUpdate(installed: .unreadable, bundled: bundled)))
    }
}

@MainActor
@Suite struct ServiceUpdaterTests {
    @Test func aBuildWithoutAllThreeHelpersCarriesNone() async throws {
        let layout = Layout()
        try layout.addHelpers(["allot", "allotd"])
        let runner = FakeProgramRunner(
            versions: [layout.helpers.appendingPathComponent("allotd").path: "allotd 0.6.0\n"])
        let updater = ServiceUpdater(bundle: layout.bundle, data: layout.data, runner: runner)

        #expect(await updater.bundledVersion() == nil)
    }

    @Test func readsTheBundledAndTheInstalledVersion() async throws {
        let layout = Layout()
        try layout.addHelpers()
        try layout.addExecutable(layout.installed)
        let runner = FakeProgramRunner(versions: [
            layout.helpers.appendingPathComponent("allotd").path: "allotd 0.6.0\n",
            layout.installed.path: "allotd 0.5.0\n",
        ])
        let updater = ServiceUpdater(bundle: layout.bundle, data: layout.data, runner: runner)
        let bundled = try version("0.6.0")
        let installed = try version("0.5.0")

        #expect(await updater.bundledVersion() == bundled)
        #expect(await updater.installedService() == .version(installed))
    }

    @Test func tellsAMissingFromAnUnreadableService() async throws {
        let layout = Layout()
        let updater = ServiceUpdater(
            bundle: layout.bundle, data: layout.data, runner: FakeProgramRunner())

        #expect(await updater.installedService() == .missing)
        try layout.addExecutable(layout.installed)
        #expect(await updater.installedService() == .unreadable)
    }

    @Test func installRunsTheBundledAllotInstall() async throws {
        let layout = Layout()
        let runner = FakeProgramRunner()
        let updater = ServiceUpdater(bundle: layout.bundle, data: layout.data, runner: runner)

        #expect(await updater.install() == nil)
        let run = try #require(runner.ran.first)
        #expect(run.program == layout.helpers.appendingPathComponent("allot"))
        #expect(run.arguments == ["install"])
        #expect(updater.logFile.lastPathComponent == "app-update.log")
    }

    @Test func aFailedInstallSaysWhereItsOutputIs() async throws {
        let layout = Layout()
        let failing = ServiceUpdater(
            bundle: layout.bundle, data: layout.data, runner: FakeProgramRunner(status: 1))
        let missing = ServiceUpdater(
            bundle: layout.bundle, data: layout.data, runner: FakeProgramRunner(status: nil))

        #expect(
            await failing.install()
                == "allot install exited with 1; see app-update.log in the logs")
        #expect(await missing.install()?.hasSuffix("/Contents/Helpers/allot could not run") == true)
    }

    @Test func claudeIsFoundWhereItsInstallersPutIt() {
        let home = URL(fileURLWithPath: "/Users/someone", isDirectory: true)

        let environment = ServiceUpdater.environment(
            ["PATH": "/usr/bin:/opt/homebrew/bin:/bin", "HOME": home.path], home: home)

        #expect(
            environment["PATH"]
                == "/Users/someone/.local/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin")
        #expect(environment["HOME"] == home.path)
    }
}

@Suite struct ServiceUpdateBannerTests {
    private let older = ServiceUpdate(
        installed: .version(ServiceVersion("0.5.0")!), bundled: ServiceVersion("0.6.0")!)

    @Test func aRunningServiceShowsTheOfferAsANotice() {
        let shown = ServiceBanner.from(
            status: .running, lastKnown: nil, startProblem: nil, update: older)

        #expect(shown?.tone == .notice)
        #expect(shown?.text == "Service 0.5.0 is older than this app")
        #expect(shown?.button == .update)
        #expect(shown?.caption == "")
    }

    @Test func theOfferTakesThePlaceOfStartService() {
        let shown = ServiceBanner.from(
            status: .stopped, lastKnown: nil, startProblem: "old", update: older)

        #expect(shown?.tone == .danger)
        #expect(shown?.button == .update)
        #expect(shown?.problem == "")
    }

    @Test func anInstallInProgressAndItsFailureShow() {
        var update = ServiceUpdate(installed: .missing, bundled: ServiceVersion("0.6.0")!)
        #expect(
            ServiceBanner.from(status: .stopped, lastKnown: nil, startProblem: nil, update: update)?
                .button == .install)

        update.installing = true
        let installing = ServiceBanner.from(
            status: .starting, lastKnown: nil, startProblem: nil, update: update)
        #expect(installing?.text == "Service not installed")
        #expect(installing?.button == .updating)

        update.installing = false
        update.problem = "allot install exited with 1"
        let failed = ServiceBanner.from(
            status: .stopped, lastKnown: nil, startProblem: nil, update: update)
        #expect(failed?.problem == "allot install exited with 1")
    }

    @Test func aStartInProgressKeepsStarting() {
        let shown = ServiceBanner.from(
            status: .starting, lastKnown: nil, startProblem: nil, update: older)

        #expect(shown?.button == .starting)
    }
}
