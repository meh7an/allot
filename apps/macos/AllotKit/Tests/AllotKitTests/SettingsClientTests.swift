import AllotContract
import Foundation
import Testing

@testable import AllotKit

private struct Unexpected: Error {
    let reply: String
}

private func daemon(
    _ home: TemporaryHome,
    token: String? = "t",
    _ handler: @escaping FakeTransport.Handler
) throws -> (DaemonClient, FakeTransport) {
    if let token {
        try home.write(token, to: home.data.tokenFile)
    }
    let transport = FakeTransport(handler)
    let client = DaemonClient(port: 1, tokenFile: home.data.tokenFile, transport: transport)
    return (client, transport)
}

private func value<Value>(_ reply: SettingsReply<Value>) throws -> Value {
    guard case .value(let value) = reply else {
        throw Unexpected(reply: String(describing: reply))
    }
    return value
}

private func failure<Value>(_ reply: SettingsReply<Value>) throws -> SettingsFailure {
    guard case .failure(let failure) = reply else {
        throw Unexpected(reply: String(describing: reply))
    }
    return failure
}

@Suite struct SettingsClientTests {
    @Test func settingsStateFixtureDecodesWithEverySettingsValue() async throws {
        let home = try TemporaryHome()
        let body = try Fixtures.data("settings-state.json")
        let (client, transport) = try daemon(home) { _ in (200, body) }

        let state = try value(try await client.settings())

        #expect(state.revision == "69fd99abe4f654c7")
        #expect(state.pendingRestart == ["daemon.port"])
        #expect(state.listeningPort == 47823)
        #expect(state.fileError == nil)
        #expect(state.values.count == 23)
        let request = try #require(transport.requests.first)
        #expect(request.httpMethod == "GET")
        #expect(request.url?.absoluteString == "http://127.0.0.1:1/settings")
        #expect(request.value(forHTTPHeaderField: "Authorization") == "Bearer t")
        #expect(request.timeoutInterval == 3)
        #expect(request.httpBody == nil)
        withExtendedLifetime(home) {}
    }

    @Test func aPatchIsSentAsTheSettingsPatchFixtureEncodesIt() async throws {
        let home = try TemporaryHome()
        let body = try Fixtures.data("settings-state.json")
        let (client, transport) = try daemon(home) { _ in (200, body) }
        let fixture = try Fixtures.data("settings-patch.json")
        let patch = try JSONDecoder().decode(SettingsPatch.self, from: fixture)

        _ = try value(
            try await client.changeSettings(revision: patch.revision, changes: patch.changes))

        let requests = transport.requests
        #expect(requests.count == 1)
        for request in requests {
            #expect(request.httpMethod == "PATCH")
            #expect(request.url?.absoluteString == "http://127.0.0.1:1/settings")
            #expect(request.value(forHTTPHeaderField: "Content-Type") == "application/json")
            let sent = try #require(request.httpBody)
            #expect(try jsonObject(sent) == jsonObject(fixture))
        }
        withExtendedLifetime(home) {}
    }

    @Test func anAnswerThatDoesNotDecodeGetsAShortSentenceNotTheDecoderDump() async throws {
        let home = try TemporaryHome()
        let (client, _) = try daemon(home) { _ in (200, Data("{}".utf8)) }

        let reply = try await client.settings()

        guard case .failure(let failure) = reply else {
            Issue.record("a body that is not a settings state decoded")
            return
        }
        #expect(failure.kind == .failed)
        #expect(failure.message == "the service's answer could not be read")
        withExtendedLifetime(home) {}
    }

    @Test func aCommandTestSendsTheClassifyRequestFixtureAndDecodesTheAnswer() async throws {
        let home = try TemporaryHome()
        let body = try Fixtures.data("command-test.json")
        let (client, transport) = try daemon(home) { _ in (200, body) }
        let fixture = try Fixtures.data("classify-request.json")
        let request = try JSONDecoder().decode(ClassifyRequest.self, from: fixture)

        let test = try value(
            try await client.classify(
                line: request.line, shell: request.shell, changes: request.changes))

        #expect(test.commands.map(\.kind) == [.heavy, .printOnly, .notHeavy])
        #expect(test.lease?.`class` == .cpuHeavy)
        let requests = transport.requests
        #expect(requests.count == 1)
        for sent in requests {
            #expect(sent.url?.absoluteString == "http://127.0.0.1:1/classify")
            #expect(sent.httpMethod == "POST")
            let sentBody = try #require(sent.httpBody)
            #expect(try jsonObject(sentBody) == jsonObject(fixture))
        }
        withExtendedLifetime(home) {}
    }

    @Test func aPreviewSendsTheUnsavedChangesAndDecodesWhatFits() async throws {
        let home = try TemporaryHome()
        let body = try Fixtures.data("fit-preview.json")
        let (client, transport) = try daemon(home) { _ in (200, body) }
        let changes = [Contract.setNumber("admission.headroom_pct", 30)]

        let preview = try value(try await client.previewSettings(changes))

        #expect(preview.keptFree.totalCores == 16)
        let request = try #require(transport.requests.first)
        #expect(request.url?.absoluteString == "http://127.0.0.1:1/settings/preview")
        let sent = try #require(request.httpBody)
        let expected = """
            {"changes": [{"kind": "set_number", "key": "admission.headroom_pct", "value": 30}]}
            """
        #expect(try jsonObject(sent) == jsonObject(expected))
        withExtendedLifetime(home) {}
    }

    @Test(arguments: [
        (409, "settings-refusal-conflict", SettingsFailureKind.conflict),
        (422, "settings-refusal-invalid", .invalid),
    ] as [(Int, String, SettingsFailureKind)])
    func refusalsKeepAllotdsMessageProblemsAndRevision(
        status: Int,
        file: String,
        kind: SettingsFailureKind
    ) async throws {
        let home = try TemporaryHome()
        let body = try Fixtures.data("\(file).json")
        let refusal = try JSONDecoder().decode(SettingsRefusal.self, from: body)
        let (client, _) = try daemon(home) { _ in (status, body) }

        let failed = try failure(try await client.settings())

        #expect(failed.kind == kind)
        #expect(failed.message == refusal.message)
        #expect(failed.problems == refusal.problems)
        withExtendedLifetime(home) {}
    }

    @Test(arguments: [
        (423, #"{"message": "config.toml is open", "problems": []}"#,
         SettingsFailureKind.held, "config.toml is open"),
        (503, "", .unavailable, "the service answered 503"),
        (404, "", .outdated, "the service answered 404"),
        (500, "internal error\n", .failed, "internal error"),
        (401, "", .token, "the service refused the token"),
    ] as [(Int, String, SettingsFailureKind, String)])
    func eachOtherAnswerHasItsOwnKind(
        status: Int,
        body: String,
        kind: SettingsFailureKind,
        message: String
    ) async throws {
        let home = try TemporaryHome()
        let (client, _) = try daemon(home) { _ in (status, Data(body.utf8)) }

        let failed = try failure(try await client.settings())

        #expect(failed.kind == kind)
        #expect(failed.message == message)
        withExtendedLifetime(home) {}
    }

    @Test func anAnswerThatIsNotASettingsStateFails() async throws {
        let home = try TemporaryHome()
        let (client, _) = try daemon(home) { _ in (200, Data(#"{"spec": 1}"#.utf8)) }

        #expect(try failure(try await client.settings()).kind == .failed)
        withExtendedLifetime(home) {}
    }

    @Test func aRefusedTokenIsReadAgainOnceBeforeItIsATokenFailure() async throws {
        let home = try TemporaryHome()
        let (client, transport) = try daemon(home) { _ in (401, Data()) }

        #expect(try failure(try await client.settings()).kind == .token)
        #expect(transport.requests.count == 2)
        withExtendedLifetime(home) {}
    }

    @Test func noTokenAndNoAnswerAreToldApartAndACancelThrows() async throws {
        let noTokenHome = try TemporaryHome()
        let (noToken, noTokenTransport) = try daemon(noTokenHome, token: nil) { _ in (200, Data()) }
        let downHome = try TemporaryHome()
        let (down, _) = try daemon(downHome) { _ in throw URLError(.cannotConnectToHost) }
        let cancelledHome = try TemporaryHome()
        let (cancelled, _) = try daemon(cancelledHome) { _ in throw URLError(.cancelled) }

        let missing = try failure(try await noToken.settings())
        let unreachable = try failure(try await down.settings())

        #expect(missing.kind == .token)
        #expect(noTokenTransport.requests.isEmpty)
        #expect(unreachable.kind == .unreachable)
        #expect(unreachable.message == URLError(.cannotConnectToHost).localizedDescription)
        await #expect(throws: CancellationError.self) {
            _ = try await cancelled.settings()
        }
        withExtendedLifetime((noTokenHome, downHome, cancelledHome)) {}
    }
}
