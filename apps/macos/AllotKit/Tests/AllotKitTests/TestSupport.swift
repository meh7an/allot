import AllotContract
import Foundation

@testable import AllotKit

/// `contract/fixtures`, six levels above this file.
private let fixtureDirectory = URL(filePath: #filePath)
    .deletingLastPathComponent()
    .deletingLastPathComponent()
    .deletingLastPathComponent()
    .deletingLastPathComponent()
    .deletingLastPathComponent()
    .deletingLastPathComponent()
    .appending(path: "contract/fixtures")

enum Fixtures {
    static func data(_ name: String) throws -> Data {
        try Data(contentsOf: fixtureDirectory.appending(path: name))
    }

    static func state(_ name: String) throws -> UiState {
        try JSONDecoder().decode(UiState.self, from: data("ui-state-\(name).json"))
    }
}

/// Builds UI states from a few jobs, since the generated types have no public memberwise
/// initializers: the JSON is the first-run fixture with sessions, active and queue replaced.
enum States {
    typealias Object = [String: Any]

    static func state(
        sessions: [Object] = [],
        active: [Object] = [],
        queue: [Object] = []
    ) throws -> UiState {
        let base = try Fixtures.data("ui-state-first-run.json")
        guard var object = try JSONSerialization.jsonObject(with: base) as? Object else {
            throw CocoaError(.coderReadCorrupt)
        }
        object["sessions"] = sessions
        object["active"] = active
        object["queue"] = queue
        let data = try JSONSerialization.data(withJSONObject: object)
        return try JSONDecoder().decode(UiState.self, from: data)
    }

    static func session(
        _ id: String,
        cwd: String? = "/Users/user/work/app",
        cores: Double = 1.5,
        ramMb: Int = 2048
    ) -> Object {
        [
            "session": id,
            "cwd": cwd.map { $0 as Any } ?? NSNull(),
            "usage": ["cores": cores, "ram_mb": ramMb, "gpu_pct": 0.0, "vram_mb": 0],
        ]
    }

    static func active(
        _ id: Int64,
        session: String,
        label: String = "cargo build",
        benchmark: Bool = false,
        cwd: String = "/Users/user/work/app"
    ) -> Object {
        [
            "lease": lease(id, session, label, benchmark, cwd, state: "active"),
            "process": NSNull(),
            "quiet_at_start": NSNull(),
        ]
    }

    /// - Parameter phase: The benchmark verdict's phase: `draining`, `measuring` or `noisy`.
    static func queued(
        _ id: Int64,
        session: String,
        label: String = "cargo test",
        benchmark: Bool = false,
        phase: String? = nil,
        cwd: String = "/Users/user/work/app"
    ) -> Object {
        var verdict: Object = ["fits": false, "reasons": [], "wait_estimate_secs": NSNull()]
        switch phase {
        case "draining":
            verdict["benchmark"] = ["phase": "draining", "active_leases": 1]
        case "measuring":
            verdict["benchmark"] = [
                "phase": "measuring", "measured_secs": 4, "needed_secs": 10,
                "cpu_avg_pct": 3.0, "gpu_avg_pct": 0.0,
            ]
        case "noisy":
            verdict["benchmark"] = [
                "phase": "noisy", "cpu_avg_pct": 30.0, "cpu_limit_pct": 10.0,
                "gpu_avg_pct": 0.0, "gpu_limit_pct": 10.0, "grant_anyway_in_secs": 50,
            ]
        default:
            break
        }
        return [
            "lease": lease(id, session, label, benchmark, cwd, state: "queued"),
            "position": 1,
            "verdict": verdict,
        ]
    }

    private static func lease(
        _ id: Int64,
        _ session: String,
        _ label: String,
        _ benchmark: Bool,
        _ cwd: String,
        state: String
    ) -> Object {
        [
            "id": id,
            "holder": ["session": session, "pid": 100, "pid_start_time": 1],
            "kind": benchmark ? "benchmark_exclusive" : "normal",
            "state": state,
            "class": "cpu_heavy",
            "profile": [
                "cores": 4.0, "ram_mb": 2048, "gpu_pct": 0.0, "vram_mb": 0, "est_secs": 60,
            ],
            "label": label,
            "cwd": cwd,
            "requested_at_ms": 1_792_824_700_000,
            "granted_at_ms": state == "active" ? 1_792_824_710_000 as Any : NSNull(),
            "ended_at_ms": NSNull(),
            "expires_at_ms": 1_792_824_765_000,
        ]
    }
}

/// Answers each request from a handler and records what was sent.
final class FakeTransport: HTTPTransport, @unchecked Sendable {
    typealias Handler = @Sendable (URLRequest) throws -> (Int, Data)

    private let lock = NSLock()
    private let handler: Handler
    private var sent: [URLRequest] = []

    init(_ handler: @escaping Handler) {
        self.handler = handler
    }

    var requests: [URLRequest] {
        lock.withLock { sent }
    }

    func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        lock.withLock { sent.append(request) }
        let (status, body) = try handler(request)
        // The request URL is always an absolute http URL here.
        let response = HTTPURLResponse(
            url: request.url!, statusCode: status, httpVersion: "HTTP/1.1", headerFields: nil)!
        return (body, response)
    }
}

/// A temporary home with `.allot` inside, removed when the test drops it.
final class TemporaryHome {
    let url: URL

    init() throws {
        url = FileManager.default.temporaryDirectory
            .appending(path: "allotkit-\(UUID().uuidString)", directoryHint: .isDirectory)
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
    }

    var data: DataDirectory {
        DataDirectory.under(home: url)
    }

    func write(_ text: String, to file: URL) throws {
        try FileManager.default.createDirectory(
            at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
        try Data(text.utf8).write(to: file)
    }

    deinit {
        try? FileManager.default.removeItem(at: url)
    }
}
