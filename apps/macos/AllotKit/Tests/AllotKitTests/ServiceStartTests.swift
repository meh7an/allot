import AllotContract
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

    @Test func stopServiceSendsThePanelsStopRecordRequest() async throws {
        let home = try TemporaryHome()
        try home.write("t", to: home.data.tokenFile)
        let transport = FakeTransport { _ in (202, Data()) }
        let poller = DaemonPoller(
            client: DaemonClient(port: 1, tokenFile: home.data.tokenFile, transport: transport))

        #expect(await poller.stopService() == nil)

        let request = try #require(transport.requests.first)
        let body = try #require(request.httpBody)
        let expected = try JSONDecoder().decode(
            ShutdownRequest.self, from: Fixtures.data("shutdown-request.json"))
        #expect(try JSONDecoder().decode(ShutdownRequest.self, from: body) == expected)
        #expect(request.value(forHTTPHeaderField: "Content-Type") == "application/json")
    }

    @Test func restartStopsWithoutARecordThenStartsOnTheNewPort() async throws {
        let home = try TemporaryHome()
        try home.write("t", to: home.data.tokenFile)
        let transport = FakeTransport { request in
            guard request.url?.path == "/shutdown" else {
                throw URLError(.cannotConnectToHost)
            }
            return (202, Data())
        }
        let launcher = FakeLauncher()
        let client = DaemonClient(
            port: 47823, tokenFile: home.data.tokenFile, transport: transport)
        let poller = DaemonPoller(client: client, port: 47823, launcher: launcher)

        #expect(poller.port == 47823)
        #expect(await poller.restartService(port: 47999) == nil)
        #expect(poller.port == 47999)

        let shutdown = try #require(transport.requests.first)
        #expect(shutdown.url?.absoluteString == "http://127.0.0.1:47823/shutdown")
        #expect(shutdown.httpBody == nil)
        #expect(launcher.starts == 1)
        _ = try await client.poll()
        #expect(transport.requests.last?.url?.port == 47999)
    }

    @Test func aRefusedRestartStartsNothing() async throws {
        let home = try TemporaryHome()
        try home.write("t", to: home.data.tokenFile)
        let transport = FakeTransport { _ in (500, Data()) }
        let launcher = FakeLauncher()
        let poller = DaemonPoller(
            client: DaemonClient(port: 1, tokenFile: home.data.tokenFile, transport: transport),
            launcher: launcher)

        #expect(await poller.restartService(port: 47999) == "POST /shutdown answered 500")
        #expect(launcher.starts == 0)
    }

    @Test func aCancelledStopIsNotReportedAsAccepted() async throws {
        let home = try TemporaryHome()
        try home.write("t", to: home.data.tokenFile)
        let transport = FakeTransport { _ in throw URLError(.cancelled) }
        let poller = DaemonPoller(
            client: DaemonClient(port: 1, tokenFile: home.data.tokenFile, transport: transport))

        #expect(await poller.stopService() == "the stop was cancelled")
    }
}
