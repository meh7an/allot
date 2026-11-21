import AllotContract
import Foundation
import Observation

/// A timer that has not fired yet.
@MainActor
public protocol ScheduledTimer: AnyObject {
    func cancel()
}

/// Runs an action once after a delay; the tests pass a fake they fire by hand.
@MainActor
public protocol SettingsTimers: AnyObject {
    /// Runs `action` once after `delay`, unless the timer is cancelled first.
    func schedule(
        after delay: Duration,
        _ action: @escaping @MainActor () -> Void
    ) -> any ScheduledTimer
}

/// The real timers, on `Task.sleep`.
@MainActor
public final class TaskTimers: SettingsTimers {
    public init() {}

    public func schedule(
        after delay: Duration,
        _ action: @escaping @MainActor () -> Void
    ) -> any ScheduledTimer {
        let task = Task { @MainActor in
            try? await Task.sleep(for: delay)
            if !Task.isCancelled {
                action()
            }
        }
        return TaskTimer(task: task)
    }
}

@MainActor
private final class TaskTimer: ScheduledTimer {
    private let task: Task<Void, Never>

    init(task: Task<Void, Never>) {
        self.task = task
    }

    func cancel() {
        task.cancel()
    }
}

/// - ``loading``: no answer yet.
/// - ``ready``: allotd answered; the values can be changed unless the file is rejected.
/// - ``stopped``: allotd does not answer and the stop record says the user stopped it.
/// - ``down``: allotd does not answer.
/// - ``token``: allotd refused this app's key.
/// - ``failed``: allotd answered something unusable; the session's detail says what.
///
/// Every state but ``ready`` keeps the last values read-only.
public enum SessionStatus: Sendable, Equatable {
    case loading
    case ready
    case stopped
    case down
    case token
    case failed
}

/// The save status line: "All changes saved", "Saving…" or "{n} changes not saved".
public enum SaveStatus: Sendable, Equatable {
    case saved
    case saving
    case unsaved(count: Int)
}

/// The settings of a group this platform shows.
public struct GroupModel {
    public let spec: SettingsGroupSpec
    public let settings: [ValueModel]
}

public struct PageModel {
    public let spec: SettingsPageSpec
    public let groups: [GroupModel]
}

/// A change allotd did not write because `config.toml` changed by hand since it was read: the
/// setting, which shows the file's value now, and the changes that would write the user's.
public struct ConflictEdit {
    public let setting: ValueModel
    public let changes: [Change]
}

/// The edits a hand edit got in the way of; Apply my change sends them again.
public struct Conflict {
    public let edits: [ConflictEdit]
}

/// The Settings window's allotd settings: one view model per setting, fetched from
/// `GET /settings` and saved through `PATCH /settings` with the file's revision. Single values
/// save as one burst 600 ms after the last change; tables, rules and chips save at once. The
/// Panel page's own settings are not here: they stay in ``SettingsStore``.
@MainActor
@Observable
public final class SettingsSession: ValueHost {
    /// Single values save this long after the last change, as one `PATCH`.
    public static let saveDelay = Duration.milliseconds(600)

    /// While the window is visible, values are fetched again this often.
    public static let refreshInterval = Duration.seconds(5)

    public private(set) var status = SessionStatus.loading
    public private(set) var detail: String?
    public private(set) var stopRecord: StopRecord?
    public private(set) var fileError: String?
    public private(set) var pendingRestart: [String] = []
    public private(set) var listeningPort: UInt32?
    public private(set) var pages: [PageModel] = []
    public private(set) var conflict: Conflict?

    /// Why the last save was not written though allotd answered, such as a held file; the
    /// changes stay unsaved until the next save.
    public private(set) var writeProblem: String?

    /// Counts every notification, so a view that reads ``saveStatus`` or the models'
    /// ``ValueModel/issue`` is redrawn.
    public private(set) var updateCount = 0

    /// Called after every change of the session's state.
    @ObservationIgnored public var onChange: (() -> Void)?

    private let client: any SettingsClient
    private let timers: any SettingsTimers
    private let platform: Platform
    private let readStopRecord: @Sendable () async -> String?
    @ObservationIgnored private var revision: String?
    @ObservationIgnored private var models: [String: ValueModel] = [:]
    @ObservationIgnored private var pending = PendingList()
    @ObservationIgnored private var saveTimer: (any ScheduledTimer)?
    @ObservationIgnored private var refreshTimer: (any ScheduledTimer)?
    @ObservationIgnored private var saving = false

    // Whether the last save failed in a way another try will not fix; only a flush the user asks
    // for, or an edit, sends the changes again.
    @ObservationIgnored private var retryStopped = false

    // Counts the saves that started or finished, so a refresh sent before one is dropped.
    @ObservationIgnored private var saveEpoch = 0
    @ObservationIgnored private var visible = false
    @ObservationIgnored private var disposed = false

    /// - Parameters:
    ///   - client: The settings routes.
    ///   - timers: Times the save burst and the refresh.
    ///   - platform: Settings for other platforms are left out.
    ///   - readStopRecord: The text of `~/.allot/stopped.json`, or `nil` when there is none.
    public init(
        client: any SettingsClient,
        timers: any SettingsTimers,
        platform: Platform = .macos,
        readStopRecord: @escaping @Sendable () async -> String?
    ) {
        self.client = client
        self.timers = timers
        self.platform = platform
        self.readStopRecord = readStopRecord
    }

    /// A session of the local install: the real timers and the stop record under `data`.
    public static func forInstall(
        client: any SettingsClient,
        data: DataDirectory
    ) -> SettingsSession {
        SettingsSession(
            client: client,
            timers: TaskTimers(),
            readStopRecord: { try? String(contentsOf: data.stopFile, encoding: .utf8) })
    }

    /// Whether allotd's settings cannot be changed now.
    public var readOnly: Bool {
        status != .ready || fileError != nil
    }

    public var saveStatus: SaveStatus {
        _ = updateCount
        if saving {
            return .saving
        }
        var count = pending.count
        for model in models.values
        where model.issue != nil && model.dirty && !pending.contains(model.key) {
            count += 1
        }
        return count == 0 ? .saved : .unsaved(count: count)
    }

    public func setting(_ key: String) -> ValueModel? {
        models[key]
    }

    /// The settings on a page whose saved value differs from the default, for Reset page.
    public func changedOn(_ pageId: String) -> [ValueModel] {
        guard let page = pages.first(where: { $0.spec.id == pageId }) else {
            return []
        }
        return Self.changed([page])
    }

    /// Every changed setting on every page, for Reset all.
    public func changedSettings() -> [ValueModel] {
        Self.changed(pages)
    }

    /// Fetches the values again: on show, on focus and every 5 s while visible. Changed values
    /// update in place; edits not yet saved stay.
    ///
    /// - Throws: Only `CancellationError`.
    public func refresh() async throws {
        let epoch = saveEpoch
        let reply = try await client.settings()
        // A save began or ended while this request was out, so its revision and values may
        // predate the save's own answer.
        if disposed || epoch != saveEpoch {
            return
        }
        switch reply {
        case .value(let state):
            take(state)
            if !pending.isEmpty && !readOnly && !retryStopped {
                schedule(.zero)
            }
        case .failure(let failure):
            await unanswered(failure)
        }
        notify()
    }

    /// Starts or stops fetching every 5 s. Hiding the window sends a pending burst first.
    public func setVisible(_ visible: Bool) {
        self.visible = visible
        clearRefresh()
        if visible {
            tick()
        } else if !pending.isEmpty {
            Task { await flush() }
        }
    }

    /// Sends the pending changes now instead of after the burst.
    public func flush() async {
        clearSave()
        retryStopped = false
        if pending.isEmpty && !saving {
            // Nothing is left to try again, so a problem that undid its changes has run its
            // course.
            writeProblem = nil
        }
        guard !saving, !pending.isEmpty, !readOnly, let revision else {
            notify()
            return
        }
        let batch = pending.values
        pending.removeAll()
        saving = true
        saveEpoch += 1
        notify()
        let sent = batch.flatMap(\.changes)
        let reply: SettingsReply<SettingsState>
        do {
            reply = try await client.changeSettings(revision: revision, changes: sent)
        } catch {
            // Only a cancellation reaches here: the task awaiting the save was cancelled, as when
            // the quit timeout in ``settle(timeout:)`` runs out.
            saving = false
            saveEpoch += 1
            return
        }
        saving = false
        saveEpoch += 1
        if disposed {
            return
        }
        var sendSoon = true
        switch reply {
        case .value(let state):
            writeProblem = nil
            for entry in batch {
                entry.model.markSaved(entry.generation)
            }
            take(state)
        case .failure(let failure):
            await refused(batch, failure)
            // Only a held file frees up on its own within moments. Changes refused because
            // allotd did not answer go out again from the refresh that finds it back; any other
            // refusal waits for Try again or the next edit.
            sendSoon = failure.kind == .held
            retryStopped = ![.held, .unreachable, .token].contains(failure.kind)
        }
        if !pending.isEmpty && saveTimer == nil && sendSoon {
            schedule(Self.saveDelay)
        }
        notify()
    }

    /// For Quit: waits for a save in flight, sends what is pending, and gives up after
    /// `timeout`.
    public func settle(timeout: Duration) async {
        let clock = ContinuousClock()
        let deadline = clock.now + timeout
        while saving && clock.now < deadline {
            try? await Task.sleep(for: .milliseconds(25))
        }
        guard clock.now < deadline else {
            return
        }
        // The flush runs on the main actor like this session; the timer only cancels it, so
        // nothing that is not Sendable crosses a task.
        let flushing = Task { await self.flush() }
        let timer = Task {
            try? await clock.sleep(until: deadline)
            flushing.cancel()
        }
        await flushing.value
        timer.cancel()
    }

    /// Sends the changes a hand edit got in the way of again, over the file's values.
    public func applyMine() {
        guard let conflict else {
            return
        }
        self.conflict = nil
        for edit in conflict.edits {
            pending.set(
                edit.setting.key,
                Pending(
                    model: edit.setting, changes: edit.changes,
                    generation: edit.setting.generation))
        }
        Task { await flush() }
    }

    /// Drops the changes a hand edit got in the way of; the file's values stay.
    public func keepFileValues() {
        conflict = nil
        notify()
    }

    /// Resets every changed setting on a page, or on every page, in one write. The window asks
    /// first.
    ///
    /// - Parameter pageId: `nil` for Reset all.
    public func resetChanged(_ pageId: String?) {
        let settings = pageId.map(changedOn) ?? changedSettings()
        for model in settings {
            model.reset()
        }
        Task { await flush() }
    }

    /// What fits with the shown values, unsaved edits included, and `changes`, such as a row
    /// edited in a dialog.
    ///
    /// - Throws: Only `CancellationError`.
    public func preview(_ changes: [Change] = []) async throws -> SettingsReply<FitPreview> {
        try await client.previewSettings(unsaved() + changes)
    }

    /// How the hook treats `line`, under the shown values and `changes`, such as a rule list
    /// edited in a dialog.
    ///
    /// - Throws: Only `CancellationError`.
    public func classify(
        _ line: String,
        shell: Shell,
        changes: [Change] = []
    ) async throws -> SettingsReply<CommandTest> {
        try await client.classify(line: line, shell: shell, changes: unsaved() + changes)
    }

    /// Stops the timers; answers that arrive later are dropped.
    public func dispose() {
        disposed = true
        clearSave()
        clearRefresh()
    }

    // MARK: ValueHost

    public func valueOf(_ key: String) -> SettingData? {
        models[key]?.value
    }

    public func labelOf(_ key: String) -> String {
        models[key]?.label ?? key
    }

    public func edited(_ model: ValueModel, changes: [Change], now: Bool) {
        retryStopped = false
        if changes.isEmpty {
            pending.remove(model.key)
        } else {
            pending.set(
                model.key, Pending(model: model, changes: changes, generation: model.generation))
        }
        if now {
            Task { await flush() }
            return
        }
        if !changes.isEmpty {
            schedule(Self.saveDelay)
        }
        notify()
    }

    // MARK: Private

    private func take(_ state: SettingsState) {
        status = .ready
        detail = nil
        stopRecord = nil
        revision = state.revision
        fileError = state.fileError
        pendingRestart = state.pendingRestart
        listeningPort = state.listeningPort
        var values: [String: SettingData] = [:]
        for entry in state.values {
            values[entry.key] = entry.data
        }
        if models.isEmpty {
            pages = build(state, values)
            return
        }
        for model in models.values {
            if let value = values[model.key] {
                model.load(value)
            }
        }
    }

    private func build(_ state: SettingsState, _ values: [String: SettingData]) -> [PageModel] {
        state.spec.pages.map { (page: SettingsPageSpec) -> PageModel in
            var groups: [GroupModel] = []
            for group in page.groups {
                var settings: [ValueModel] = []
                for spec in group.settings
                where spec.platforms.isEmpty || spec.platforms.contains(platform) {
                    if let value = values[spec.key],
                        let model = makeValueModel(spec, host: self, value: value)
                    {
                        models[model.key] = model
                        settings.append(model)
                    }
                }
                if !settings.isEmpty {
                    groups.append(GroupModel(spec: group, settings: settings))
                }
            }
            return PageModel(spec: page, groups: groups)
        }
    }

    private func unanswered(_ failure: SettingsFailure) async {
        detail = failure.message
        switch failure.kind {
        case .unreachable:
            stopRecord = Self.parseStopRecord(await readStopRecord())
            status = stopRecord == nil ? .down : .stopped
        case .token:
            status = .token
        default:
            status = .failed
        }
    }

    private func refused(_ batch: [Pending], _ failure: SettingsFailure) async {
        switch failure.kind {
        case .conflict:
            conflict = Conflict(
                edits: batch.map { ConflictEdit(setting: $0.model, changes: $0.changes) })
            for entry in batch {
                entry.model.undo()
            }
            try? await refresh()
        case .invalid:
            if route(batch, failure) {
                schedule(.zero)
                return
            }
            // No batched setting owns the problem, such as a file that is broken: say why the
            // edits are undone instead of dropping them silently.
            writeProblem = failure.message
            for entry in batch {
                entry.model.undo()
            }
            try? await refresh()
        case .token, .unreachable:
            restore(batch)
            await unanswered(failure)
        default:
            writeProblem = failure.message
            restore(batch)
        }
    }

    /// Shows each refused value's reason under its field, and sends the rest of the batch
    /// again.
    ///
    /// - Returns: Whether any problem belonged to a change in the batch.
    private func route(_ batch: [Pending], _ failure: SettingsFailure) -> Bool {
        var refused: Set<ObjectIdentifier> = []
        for problem in failure.problems {
            if let entry = batch.first(where: { Self.owns($0.model.key, problem.key) }) {
                let sentence = entry.model.describeProblem(problem.key, problem.message)
                entry.model.refuse(sentence)
                refused.insert(ObjectIdentifier(entry.model))
            }
        }
        if refused.isEmpty {
            return false
        }
        restore(batch.filter { !refused.contains(ObjectIdentifier($0.model)) })
        return true
    }

    /// Puts changes back in the burst, unless the user changed the same setting since.
    private func restore(_ batch: [Pending]) {
        for entry in batch where !pending.contains(entry.model.key) {
            pending.set(entry.model.key, entry)
        }
    }

    private func unsaved() -> [Change] {
        pending.values.flatMap(\.changes)
    }

    private func schedule(_ delay: Duration) {
        clearSave()
        saveTimer = timers.schedule(after: delay) { [weak self] in
            guard let self else {
                return
            }
            saveTimer = nil
            Task { await self.flush() }
        }
    }

    private func tick() {
        refreshTimer = timers.schedule(after: Self.refreshInterval) { [weak self] in
            guard let self else {
                return
            }
            refreshTimer = nil
            if visible && !disposed {
                tick()
            }
        }
        Task { try? await refresh() }
    }

    private func clearSave() {
        saveTimer?.cancel()
        saveTimer = nil
    }

    private func clearRefresh() {
        refreshTimer?.cancel()
        refreshTimer = nil
    }

    private func notify() {
        updateCount += 1
        onChange?()
    }

    /// Whether the setting `key` owns a problem reported for `problemKey`: the key itself, a
    /// row of a table (`presets.big.min_cores`) or a rule (`classifier.rules[2].program`).
    private static func owns(_ key: String, _ problemKey: String) -> Bool {
        problemKey == key || problemKey.hasPrefix("\(key).") || problemKey.hasPrefix("\(key)[")
    }

    private static func changed(_ pages: [PageModel]) -> [ValueModel] {
        pages.flatMap(\.groups).flatMap(\.settings).filter { $0.canReset }
    }

    private nonisolated static let cliStopAtZero = try? JSONDecoder().decode(
        StopRecord.self, from: Data(#"{"by": "cli", "stopped_at_ms": 0}"#.utf8))

    /// The stop record, as allotd reads it: a file that is not a record still means the user
    /// stopped allotd, from the CLI, since only its presence counts.
    ///
    /// - Parameter text: The file's text, or `nil` when there is no file.
    public nonisolated static func parseStopRecord(_ text: String?) -> StopRecord? {
        guard let text else {
            return nil
        }
        return (try? JSONDecoder().decode(StopRecord.self, from: Data(text.utf8)))
            ?? cliStopAtZero
    }
}

/// The edit of one setting waiting for the burst to end.
private struct Pending {
    let model: ValueModel
    let changes: [Change]
    let generation: Int
}

/// The pending edits by setting key, in the order they were first made, as a JavaScript `Map`
/// keeps them: setting a key again keeps its place, removing it and setting it again moves it
/// to the end.
private struct PendingList {
    private var order: [String] = []
    private var entries: [String: Pending] = [:]

    var isEmpty: Bool {
        entries.isEmpty
    }

    var count: Int {
        entries.count
    }

    var values: [Pending] {
        order.compactMap { entries[$0] }
    }

    func contains(_ key: String) -> Bool {
        entries[key] != nil
    }

    mutating func set(_ key: String, _ entry: Pending) {
        if entries[key] == nil {
            order.append(key)
        }
        entries[key] = entry
    }

    mutating func remove(_ key: String) {
        if entries.removeValue(forKey: key) != nil {
            order.removeAll { $0 == key }
        }
    }

    mutating func removeAll() {
        order.removeAll()
        entries.removeAll()
    }
}
