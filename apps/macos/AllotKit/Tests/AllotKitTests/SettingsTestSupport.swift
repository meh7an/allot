import AllotContract
import Foundation
import Testing

@testable import AllotKit

/// Timers the tests fire by hand.
@MainActor
final class FakeTimers: SettingsTimers {
    @MainActor
    final class Entry: ScheduledTimer {
        let delay: Duration
        let action: @MainActor () -> Void
        private(set) var cancelled = false

        init(delay: Duration, action: @escaping @MainActor () -> Void) {
            self.delay = delay
            self.action = action
        }

        func cancel() {
            cancelled = true
        }
    }

    private var pending: [Entry] = []

    /// The delays of the timers that have neither fired nor been cancelled.
    var delays: [Duration] {
        pending.filter { !$0.cancelled }.map { $0.delay }
    }

    func schedule(
        after delay: Duration,
        _ action: @escaping @MainActor () -> Void
    ) -> any ScheduledTimer {
        let timer = Entry(delay: delay, action: action)
        pending.append(timer)
        return timer
    }

    /// Fires every pending timer once; timers they schedule wait for the next call.
    func fireAll() {
        let due = pending.filter { !$0.cancelled }
        pending.removeAll()
        for timer in due {
            timer.action()
        }
    }
}

/// A settings client whose answers each test queues; it records every patch.
final class FakeSettings: SettingsClient, @unchecked Sendable {
    struct Patch {
        let revision: String
        let changes: [Change]
    }

    private let lock = NSLock()
    private var reads: [SettingsReply<SettingsState>] = []
    private var writes: [SettingsReply<SettingsState>] = []
    private var sentPatches: [Patch] = []
    private var sentPreviews: [[Change]] = []
    private var current: SettingsState
    private var holding = false
    private var held: [() -> Void] = []

    init(state: SettingsState) {
        current = state
    }

    /// When true, a read stays in flight, with the state it was sent under, until
    /// ``completeReads()``.
    var holdReads: Bool {
        get { lock.withLock { holding } }
        set { lock.withLock { holding = newValue } }
    }

    /// Answers every read held back.
    func completeReads() {
        let answers = lock.withLock { () -> [() -> Void] in
            defer { held.removeAll() }
            return held
        }
        for answer in answers {
            answer()
        }
    }

    var state: SettingsState {
        lock.withLock { current }
    }

    var patches: [Patch] {
        lock.withLock { sentPatches }
    }

    var previews: [[Change]] {
        lock.withLock { sentPreviews }
    }

    func queueRead(_ reply: SettingsReply<SettingsState>) {
        lock.withLock { reads.append(reply) }
    }

    func queueWrite(_ reply: SettingsReply<SettingsState>) {
        lock.withLock { writes.append(reply) }
    }

    func settings() async throws -> SettingsReply<SettingsState> {
        let reply = lock.withLock { () -> SettingsReply<SettingsState> in
            reads.isEmpty ? .value(current) : reads.removeFirst()
        }
        guard holdReads else {
            return reply
        }
        return await withCheckedContinuation { continuation in
            lock.withLock {
                held.append { continuation.resume(returning: reply) }
            }
        }
    }

    func changeSettings(
        revision: String,
        changes: [Change]
    ) async throws -> SettingsReply<SettingsState> {
        lock.withLock { () -> SettingsReply<SettingsState> in
            sentPatches.append(Patch(revision: revision, changes: changes))
            return writes.isEmpty ? .value(current) : writes.removeFirst()
        }
    }

    func previewSettings(_ changes: [Change]) async throws -> SettingsReply<FitPreview> {
        lock.withLock { sentPreviews.append(changes) }
        return failure(.unavailable)
    }

    func classify(
        line: String,
        shell: Shell,
        changes: [Change]
    ) async throws -> SettingsReply<CommandTest> {
        failure(.unavailable)
    }
}

/// A refusal; its message is its kind unless given.
func failure<Value: Sendable>(
    _ kind: SettingsFailureKind,
    message: String? = nil,
    problems: [FieldError] = []
) -> SettingsReply<Value> {
    .failure(
        SettingsFailure(
            kind: kind, message: message ?? kind.rawValue, problems: problems))
}

/// The generated types have no initializers outside their module, so a test builds one from
/// its JSON.
func fieldError(_ key: String, _ message: String) throws -> FieldError {
    let object = ["key": key, "message": message]
    return try JSONDecoder().decode(
        FieldError.self, from: JSONSerialization.data(withJSONObject: object))
}

/// The settings-state fixture as JSON, for a test to change before it is decoded.
func stateObject() throws -> [String: Any] {
    let data = try Fixtures.data("settings-state.json")
    guard let object = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
        throw CocoaError(.coderReadCorrupt)
    }
    return object
}

func decodeState(_ object: [String: Any]) throws -> SettingsState {
    try JSONDecoder().decode(
        SettingsState.self, from: JSONSerialization.data(withJSONObject: object))
}

/// The settings-state fixture, changed by `edit`.
func fixtureState(_ edit: (inout [String: Any]) -> Void = { _ in }) throws -> SettingsState {
    var object = try stateObject()
    edit(&object)
    return try decodeState(object)
}

/// A decoded JSON document, comparable with `==` regardless of key order and of whether a
/// number was written `6` or `6.0`.
func jsonObject(_ data: Data) throws -> NSObject {
    try #require(try JSONSerialization.jsonObject(with: data) as? NSObject)
}

func jsonObject(_ text: String) throws -> NSObject {
    try jsonObject(Data(text.utf8))
}

func jsonObject(_ changes: [Change]) throws -> NSObject {
    try jsonObject(JSONEncoder().encode(changes))
}

/// Lets the tasks a test started run, including their hops off the main actor. A loaded CI
/// runner can take a while to schedule them, so this yields often before giving up.
func settle() async {
    for _ in 0..<25 {
        await Task.yield()
        try? await Task.sleep(for: .milliseconds(2))
    }
}

/// Plain digits for the number formatter the issue sentences take.
func plain(_ value: Double) -> String {
    value == value.rounded() ? String(Int64(value)) : String(value)
}
