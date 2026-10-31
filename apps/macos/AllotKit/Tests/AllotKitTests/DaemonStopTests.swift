import Foundation
import Testing

@testable import AllotKit

private let lease = """
    {"id": 7, "holder": {"session": "s", "pid": 1, "pid_start_time": 1}, "kind": "normal",
     "state": "expired", "class": "cpu_heavy", "end_cause": "killed",
     "profile": {"cores": 1, "ram_mb": 0, "gpu_pct": 0, "vram_mb": 0, "est_secs": 0},
     "label": "cargo build", "cwd": "/w", "requested_at_ms": 1, "granted_at_ms": 2,
     "ended_at_ms": 1792824720000, "expires_at_ms": 3}
    """

private func killBody(failed: Int = 0, untouched: String? = nil) -> Data {
    let failures = (0..<failed).map { #"{"pid": \#($0), "name": "x", "reason": "denied"}"# }
    let reason = untouched.map { #""\#($0)""# } ?? "null"
    return Data("""
        {"lease": \(lease), "terminated": [{"pid": 9, "name": "cargo"}],
         "failed": [\(failures.joined(separator: ","))], "spared": [],
         "untouched_reason": \(reason)}
        """.utf8)
}

private func stopper(
    _ handler: @escaping FakeTransport.Handler
) throws -> (DaemonClient, FakeTransport, TemporaryHome) {
    let home = try TemporaryHome()
    try home.write("secret-token\n", to: home.data.tokenFile)
    let transport = FakeTransport(handler)
    let client = DaemonClient(port: 47999, tokenFile: home.data.tokenFile, transport: transport)
    return (client, transport, home)
}

@Suite struct DaemonStopTests {
    @Test func killPostsWithTheTokenAndATenSecondLimit() async throws {
        let (client, transport, home) = try stopper { _ in (200, killBody()) }

        let result = try await client.kill(7)

        #expect(result == StopResult(ended: true, text: "", endedAtMs: 1_792_824_720_000))
        let request = try #require(transport.requests.first)
        #expect(request.httpMethod == "POST")
        #expect(request.url?.absoluteString == "http://127.0.0.1:47999/leases/7/kill")
        #expect(request.value(forHTTPHeaderField: "Authorization") == "Bearer secret-token")
        #expect(request.timeoutInterval == 10)
        withExtendedLifetime(home) {}
    }

    @Test(arguments: [
        (0, nil, ""),
        (1, nil, "1 process still running"),
        (3, nil, "3 processes still running"),
        (0, "no bound process", "nothing was running"),
    ] as [(Int, String?, String)])
    func killNotesWhatWasLeft(failed: Int, untouched: String?, note: String) async throws {
        let body = killBody(failed: failed, untouched: untouched)
        let (client, _, home) = try stopper { _ in (200, body) }

        #expect(try await client.kill(7).text == note)
        withExtendedLifetime(home) {}
    }

    @Test func removePostsToTheRemoveRoute() async throws {
        let body = Data(#"{"lease": \#(lease)}"#.utf8)
        let (client, transport, home) = try stopper { _ in (200, body) }

        let result = try await client.remove(7)

        #expect(result.ended)
        #expect(transport.requests.first?.url?.path == "/leases/7/remove")
        withExtendedLifetime(home) {}
    }

    @Test(arguments: [
        (409, "lease 7 was not killed: the bound process 1 (launchd) is protected",
         "the bound process 1 (launchd) is protected"),
        (409, "lease 7 has already ended", "the job has already ended"),
        (409, "", "the service refused"),
        (404, "no such lease", "the job no longer exists"),
        (503, "", "the service has just started; try again"),
        (401, "", "the service refused the token"),
        (500, "boom", "the service answered 500"),
    ])
    func refusalsSayWhy(status: Int, body: String, reason: String) async throws {
        let (client, _, home) = try stopper { _ in (status, Data(body.utf8)) }

        #expect(try await client.kill(7) == .refused(reason))
        withExtendedLifetime(home) {}
    }

    @Test(arguments: [
        (URLError.Code.timedOut, "the service did not answer within 10 s"),
        (.cannotConnectToHost, "the service is not running"),
    ] as [(URLError.Code, String)])
    func unreachableServiceSaysSo(code: URLError.Code, reason: String) async throws {
        let (client, _, home) = try stopper { _ in throw URLError(code) }

        #expect(try await client.remove(7) == .refused(reason))
        withExtendedLifetime(home) {}
    }

    @Test func unreadableAnswerIsRefused() async throws {
        let (client, _, home) = try stopper { _ in (200, Data("{}".utf8)) }

        #expect(try await client.kill(7) == .refused("the service's answer could not be read"))
        withExtendedLifetime(home) {}
    }

    @Test func missingTokenSendsNothing() async throws {
        let home = try TemporaryHome()
        let transport = FakeTransport { _ in (200, Data()) }
        let client = DaemonClient(port: 47999, tokenFile: home.data.tokenFile, transport: transport)

        #expect(try await client.kill(7) == .refused("the service's token file is missing"))
        #expect(transport.requests.isEmpty)
    }
}
