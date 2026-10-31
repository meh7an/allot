import Foundation
import Testing

@testable import AllotKit

/// Records launchctl calls, answers them with fixed exit codes, and records spawns.
private final class FakeRunner: CommandRunner, @unchecked Sendable {
    private let lock = NSLock()
    private let answer: @Sendable ([String]) -> Int32?
    private var calls: [[String]] = []
    private var spawns: [URL] = []

    init(_ answer: @escaping @Sendable ([String]) -> Int32? = { _ in 0 }) {
        self.answer = answer
    }

    var launchctl: [[String]] {
        lock.withLock { calls }
    }

    var spawned: [URL] {
        lock.withLock { spawns }
    }

    func run(_ program: URL, _ arguments: [String]) async -> Int32? {
        lock.withLock { calls.append(arguments) }
        return answer(arguments)
    }

    func spawn(_ program: URL) throws {
        lock.withLock { spawns.append(program) }
    }
}

private func agent(program: String) -> String {
    """
    <?xml version="1.0" encoding="UTF-8"?>
    <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" \
    "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
    <plist version="1.0"><dict>
    <key>Label</key><string>tech.mehran.allotd</string>
    <key>ProgramArguments</key><array><string>\(program)</string></array>
    <key>RunAtLoad</key><true/>
    </dict></plist>
    """
}

/// A temporary home with an executable `~/.allot/bin/allotd`.
private func installedHome() throws -> TemporaryHome {
    let home = try TemporaryHome()
    let exe = home.data.root.appendingPathComponent("bin/allotd")
    try home.write("#!/bin/sh\n", to: exe)
    try FileManager.default.setAttributes([.posixPermissions: 0o755], ofItemAtPath: exe.path)
    return home
}

@MainActor
@Suite struct ServiceLauncherTests {
    @Test func loadedAgentIsKickstarted() async throws {
        let home = try installedHome()
        let runner = FakeRunner()
        let launcher = LaunchdServiceLauncher(data: home.data, uid: 501, runner: runner)
        try home.write(agent(program: launcher.daemonExe.path), to: launcher.agentFile)

        #expect(await launcher.start() == nil)
        #expect(runner.launchctl == [
            ["print", "gui/501/tech.mehran.allotd"],
            ["kickstart", "gui/501/tech.mehran.allotd"],
        ])
        #expect(runner.spawned.isEmpty)
    }

    @Test func unloadedAgentIsBootstrapped() async throws {
        let home = try installedHome()
        let runner = FakeRunner { $0.first == "print" ? 113 : 0 }
        let launcher = LaunchdServiceLauncher(data: home.data, uid: 501, runner: runner)
        try home.write(agent(program: launcher.daemonExe.path), to: launcher.agentFile)

        #expect(await launcher.start() == nil)
        #expect(runner.launchctl.last == ["bootstrap", "gui/501", launcher.agentFile.path])
    }

    @Test func refusedLaunchctlFallsBackToRunningAllotd() async throws {
        let home = try installedHome()
        let runner = FakeRunner { _ in 5 }
        let launcher = LaunchdServiceLauncher(data: home.data, uid: 501, runner: runner)
        try home.write(agent(program: launcher.daemonExe.path), to: launcher.agentFile)

        #expect(await launcher.start() == nil)
        #expect(runner.spawned == [launcher.daemonExe])
    }

    @Test func anotherInstallsAgentIsLeftAlone() async throws {
        let home = try installedHome()
        let runner = FakeRunner()
        let launcher = LaunchdServiceLauncher(data: home.data, uid: 501, runner: runner)
        try home.write(agent(program: "/opt/other/allotd"), to: launcher.agentFile)

        #expect(await launcher.start() == nil)
        #expect(runner.launchctl.isEmpty)
        #expect(runner.spawned == [launcher.daemonExe])
    }

    @Test func missingAllotdAsksForAnInstall() async throws {
        let home = try TemporaryHome()
        let runner = FakeRunner()
        let launcher = LaunchdServiceLauncher(data: home.data, uid: 501, runner: runner)

        let problem = await launcher.start()

        #expect(problem == "\(launcher.daemonExe.path) is missing; run allot install")
        #expect(runner.spawned.isEmpty)
    }
}

/// A launcher that answers with a fixed problem and counts its starts.
@MainActor
private final class FakeLauncher: ServiceLauncher {
    private let problem: String?
    private(set) var starts = 0

    init(problem: String? = nil) {
        self.problem = problem
    }

    func start() async -> String? {
        starts += 1
        return problem
    }
}

@MainActor
private func stoppedPoller(
    _ home: TemporaryHome,
    _ launcher: FakeLauncher,
    _ clock: TestClock
) throws -> DaemonPoller {
    try home.write("t", to: home.data.tokenFile)
    let transport = FakeTransport { _ in throw URLError(.cannotConnectToHost) }
    return DaemonPoller(
        client: DaemonClient(port: 1, tokenFile: home.data.tokenFile, transport: transport),
        launcher: launcher,
        now: clock.source)
}

@MainActor
@Suite struct DaemonPollerStartTests {
    @Test func startReportsStartingThenGivesUpAfterTenSeconds() async throws {
        let home = try TemporaryHome()
        let clock = TestClock()
        let launcher = FakeLauncher()
        let poller = try stoppedPoller(home, launcher, clock)

        #expect(await poller.startService() == nil)
        try await poller.pollOnce()
        let starting = poller.latest?.status
        clock.advance(.seconds(11))
        try await poller.pollOnce()

        #expect(launcher.starts == 1)
        #expect(starting == .starting)
        #expect(poller.latest?.status == .stopped)
        #expect(poller.startProblem == "allotd did not answer within 10 s; see logs/allotd.log")
    }

    @Test func failedLaunchStaysStoppedWithItsReason() async throws {
        let home = try TemporaryHome()
        let poller = try stoppedPoller(home, FakeLauncher(problem: "no allotd"), TestClock())

        #expect(await poller.startService() == "no allotd")
        try await poller.pollOnce()

        #expect(poller.latest?.status == .stopped)
        #expect(poller.startProblem == "no allotd")
    }

    @Test func stopServicePostsShutdown() async throws {
        let home = try TemporaryHome()
        try home.write("t", to: home.data.tokenFile)
        let transport = FakeTransport { _ in (202, Data()) }
        let poller = DaemonPoller(
            client: DaemonClient(port: 1, tokenFile: home.data.tokenFile, transport: transport))

        #expect(await poller.stopService() == nil)
        #expect(transport.requests.first?.url?.path == "/shutdown")
        #expect(transport.requests.first?.httpMethod == "POST")
    }
}
