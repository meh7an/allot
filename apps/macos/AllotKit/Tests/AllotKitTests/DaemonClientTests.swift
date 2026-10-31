import Foundation
import Testing

@testable import AllotKit

private func client(_ home: TemporaryHome, _ transport: FakeTransport) -> DaemonClient {
    DaemonClient(port: 47999, tokenFile: home.data.tokenFile, transport: transport)
}

private func health(version: String) -> Data {
    Data(#"{"version": "\#(version)", "pid": 1, "uptime_secs": 5}"#.utf8)
}

@Suite struct DaemonClientTests {
    @Test func runningStateIsDecodedWithTheToken() async throws {
        let home = try TemporaryHome()
        try home.write("secret-token\n", to: home.data.tokenFile)
        let body = try Fixtures.data("ui-state-busy.json")
        let transport = FakeTransport { _ in (200, body) }

        let result = try await client(home, transport).poll()

        #expect(result.status == .running)
        #expect(result.state?.active.count == 2)
        #expect(result.version == "0.2.0")
        let request = try #require(transport.requests.first)
        #expect(request.url?.absoluteString == "http://127.0.0.1:47999/ui/state")
        #expect(request.value(forHTTPHeaderField: "Authorization") == "Bearer secret-token")
        #expect(request.timeoutInterval == 3)
    }

    @Test func missingTokenIsStoppedWithoutARequest() async throws {
        let home = try TemporaryHome()
        let transport = FakeTransport { _ in (200, Data()) }

        let result = try await client(home, transport).poll()

        #expect(result.status == .stopped)
        #expect(result.detail?.contains("no token") == true)
        #expect(transport.requests.isEmpty)
    }

    @Test(arguments: [URLError.Code.cannotConnectToHost, .timedOut, .networkConnectionLost])
    func unreachableDaemonIsStopped(code: URLError.Code) async throws {
        let home = try TemporaryHome()
        try home.write("t", to: home.data.tokenFile)
        let transport = FakeTransport { _ in throw URLError(code) }

        let result = try await client(home, transport).poll()

        #expect(result.status == .stopped)
    }

    @Test func noFirstSampleIsStarting() async throws {
        let home = try TemporaryHome()
        try home.write("t", to: home.data.tokenFile)
        let transport = FakeTransport { _ in (503, Data()) }

        #expect(try await client(home, transport).poll().status == .starting)
    }

    @Test func undecodableStateIsIncompatible() async throws {
        let home = try TemporaryHome()
        try home.write("t", to: home.data.tokenFile)
        let transport = FakeTransport { _ in (200, Data(#"{"generated_at_ms": "soon"}"#.utf8)) }

        #expect(try await client(home, transport).poll().status == .incompatible)
    }

    @Test func oldDaemonWithoutUiStateIsIncompatibleWithItsVersion() async throws {
        let home = try TemporaryHome()
        try home.write("t", to: home.data.tokenFile)
        let transport = FakeTransport { request in
            request.url?.path == "/health" ? (200, health(version: "0.1.0")) : (404, Data())
        }

        let result = try await client(home, transport).poll()

        #expect(result.status == .incompatible)
        #expect(result.version == "0.1.0")
        let healthRequest = try #require(transport.requests.last)
        #expect(healthRequest.value(forHTTPHeaderField: "Authorization") == nil)
    }

    @Test func notFoundWithoutHealthIsStopped() async throws {
        let home = try TemporaryHome()
        try home.write("t", to: home.data.tokenFile)
        let transport = FakeTransport { _ in (404, Data()) }

        #expect(try await client(home, transport).poll().status == .stopped)
    }

    @Test func refusedTokenIsReadAgainOnce() async throws {
        let home = try TemporaryHome()
        try home.write("old", to: home.data.tokenFile)
        let body = try Fixtures.data("ui-state-idle.json")
        let transport = FakeTransport { request in
            let token = request.value(forHTTPHeaderField: "Authorization")
            return token == "Bearer new" ? (200, body) : (401, Data())
        }
        let daemon = client(home, transport)
        #expect(try await daemon.poll().status == .incompatible)

        try home.write("new", to: home.data.tokenFile)
        let result = try await daemon.poll()

        #expect(result.status == .running)
        #expect(transport.requests.count == 4)
    }

    @Test func otherStatusIsIncompatible() async throws {
        let home = try TemporaryHome()
        try home.write("t", to: home.data.tokenFile)
        let transport = FakeTransport { _ in (500, Data()) }

        let result = try await client(home, transport).poll()

        #expect(result.status == .incompatible)
        #expect(result.detail == "GET /ui/state answered 500")
    }

    @Test func cancelledRequestThrowsInsteadOfReportingStopped() async throws {
        let home = try TemporaryHome()
        try home.write("t", to: home.data.tokenFile)
        let transport = FakeTransport { _ in throw URLError(.cancelled) }

        await #expect(throws: CancellationError.self) {
            _ = try await client(home, transport).poll()
        }
    }
}
