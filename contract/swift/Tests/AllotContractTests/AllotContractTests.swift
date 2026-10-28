import AllotContract
import Foundation
import Testing

/// `contract/fixtures`, four levels above this file.
private let fixtureDirectory = URL(filePath: #filePath)
    .deletingLastPathComponent()
    .deletingLastPathComponent()
    .deletingLastPathComponent()
    .deletingLastPathComponent()
    .appending(path: "fixtures")

private let fixtureNames: [String] =
    ((try? FileManager.default.contentsOfDirectory(
        atPath: fixtureDirectory.path(percentEncoded: false))) ?? [])
    .filter { $0.hasSuffix(".json") }
    .map { String($0.dropLast(".json".count)) }
    .sorted()

private func fixtureData(_ name: String) throws -> Data {
    try Data(contentsOf: fixtureDirectory.appending(path: "\(name).json"))
}

private func decode<T: Decodable>(_ type: T.Type, _ name: String) throws -> T {
    try JSONDecoder().decode(type, from: fixtureData(name))
}

/// Decodes a fixture as the root type its name starts with and encodes it again.
private func reencode(_ name: String) throws -> Data {
    let data = try fixtureData(name)
    func reencode<T: Codable>(_ type: T.Type) throws -> Data {
        try JSONEncoder().encode(JSONDecoder().decode(type, from: data))
    }
    if name.hasPrefix("ui-state-") {
        return try reencode(UiState.self)
    }
    if name.hasPrefix("kill-response-") {
        return try reencode(KillResponse.self)
    }
    if name.hasPrefix("remove-response") {
        return try reencode(RemoveResponse.self)
    }
    throw CocoaError(.fileReadUnknown, userInfo: [NSFilePathErrorKey: name])
}

/// Records an issue unless every member of `expected`, at any depth, is in `actual` with the
/// same value. `JSONDecoder` ignores members the types lack, so they go missing on the way
/// back. A `null` member is only checked for not holding a value, because the encoder leaves
/// `nil` out.
private func expectContains(_ expected: Any, _ actual: Any?, at path: String) {
    switch expected {
    case let object as [String: Any]:
        guard let actualObject = actual as? [String: Any] else {
            Issue.record("\(path): not an object")
            return
        }
        for (key, value) in object {
            if value is NSNull {
                #expect(
                    actualObject[key] == nil || actualObject[key] is NSNull,
                    "\(path).\(key): null became a value")
            } else if let actualValue = actualObject[key] {
                expectContains(value, actualValue, at: "\(path).\(key)")
            } else {
                Issue.record("\(path).\(key): dropped by the decoder")
            }
        }
    case let array as [Any]:
        guard let actualArray = actual as? [Any], actualArray.count == array.count else {
            Issue.record("\(path): not an array of \(array.count)")
            return
        }
        for (index, item) in array.enumerated() {
            expectContains(item, actualArray[index], at: "\(path)[\(index)]")
        }
    case let number as NSNumber:
        #expect(actual as? NSNumber == number, "\(path): \(number) became \(actual ?? "nothing")")
    case let text as String:
        #expect(actual as? String == text, "\(path): \(text) became \(actual ?? "nothing")")
    default:
        Issue.record("\(path): unexpected JSON value \(expected)")
    }
}

@Test func fixturesAreFound() {
    #expect(fixtureNames.count >= 9)
}

@Test(arguments: fixtureNames)
func fixtureDecodesWithoutDroppingMembers(name: String) throws {
    let expected = try JSONSerialization.jsonObject(with: fixtureData(name))
    let actual = try JSONSerialization.jsonObject(with: reencode(name))

    expectContains(expected, actual, at: name)
}

@Test func windowsFixturesDecodeWithTheDefaultsAllotdLeavesOut() throws {
    let state = try decode(UiState.self, "ui-state-busy")

    let gpu = try #require(state.meters.gpus.first)
    #expect(gpu.unifiedMemory == false)
    #expect(gpu.processLoad == .exact)
    #expect(state.active[0].lease.isolation == nil)
}

@Test func linuxAndMacosFixturesKeepIsolationAndUnifiedMemory() throws {
    let linux = try decode(UiState.self, "ui-state-linux")
    let macos = try decode(UiState.self, "ui-state-macos")

    let isolation = try #require(linux.active[1].lease.isolation)
    #expect(isolation.kind == .cgroup)
    #expect(isolation.limits.cpuCores == 8)
    #expect(isolation.limits.memoryMaxMb == nil)
    let gpu = try #require(macos.meters.gpus.first)
    #expect(gpu.unifiedMemory)
    #expect(gpu.processLoad == .estimated)
    #expect(gpu.vramTotalMb == nil)
}

@Test func benchmarkFixtureDecodesItsProgressByPhase() throws {
    let state = try decode(UiState.self, "ui-state-benchmark")

    guard case .measuring(let progress) = state.queue[0].verdict.benchmark else {
        Issue.record("the benchmark is not measuring")
        return
    }
    #expect(progress.measuredSecs == 6)
    #expect(progress.neededSecs == 10)
    #expect(progress.gpuAvgPct == 0.5)
}

@Test func unknownEnumValuesAndPhasesAreRejected() throws {
    let state = try #require(String(data: fixtureData("remove-response"), encoding: .utf8))
    let benchmark = try #require(String(data: fixtureData("ui-state-benchmark"), encoding: .utf8))
    let pausedState = state.replacingOccurrences(of: "\"expired\"", with: "\"paused\"")
    let warmingPhase = benchmark.replacingOccurrences(of: "\"measuring\"", with: "\"warming\"")

    #expect(pausedState != state)
    #expect(warmingPhase != benchmark)
    #expect(throws: DecodingError.self) {
        try JSONDecoder().decode(RemoveResponse.self, from: Data(pausedState.utf8))
    }
    #expect(throws: DecodingError.self) {
        try JSONDecoder().decode(UiState.self, from: Data(warmingPhase.utf8))
    }
}
