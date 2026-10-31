import AllotContract

public enum StopPress: Sendable, Equatable {
    /// Nothing happens: the double-press guard, a stop already under way, or an unknown job.
    case ignored

    /// The first press of Kill: the card asks Kill?.
    case confirming

    /// The caller sends the request to allotd and reports back with ``JobStops/finished(_:_:)``.
    case send
}

/// The jobs a snapshot shows: allotd's lists, with the cards of ended stops put back.
public struct ShownJobs: Sendable, Equatable {
    public var active: [ActiveJob]
    public var queue: [QueuedJob]

    public init(active: [ActiveJob], queue: [QueuedJob]) {
        self.active = active
        self.queue = queue
    }

    public init(_ state: UiState) {
        self.init(active: state.active, queue: state.queue)
    }
}

/// The Kill and Remove state of every card (handoff section 4): the two-step Kill? confirm, the
/// request in flight, and the Stopped, Removed or refused line that holds before the card goes.
/// It keeps the last seen copy of a job whose stop is under way or done, because allotd stops
/// listing the lease the moment it ends, while the card still holds for ``endedFor``.
public final class JobStops {
    /// How long Kill? waits for the second press.
    public static let confirmFor = Duration.seconds(4)

    /// A second press this soon after the first is ignored, so a double-click can't kill.
    public static let doublePress = Duration.milliseconds(300)

    /// How long Stopped or Removed shows before the card collapses.
    public static let endedFor = Duration.seconds(4)

    /// How long a refusal shows before the card reads normally again.
    public static let failedFor = Duration.seconds(6)

    private let now: () -> ContinuousClock.Instant
    private var stops: [Int64: Stop] = [:]
    private var seen: UiState?

    /// - Parameter now: The local clock; the tests pass their own.
    public init(now: @escaping () -> ContinuousClock.Instant = { .now }) {
        self.now = now
    }

    /// The cards with a stop, as they read now; call ``expire()`` first.
    public var views: [Int64: StopView] {
        stops.mapValues(view(of:))
    }

    /// Whether a Kill? confirm is up, which Esc cancels before it closes the panel.
    public var isConfirming: Bool {
        stops.values.contains { $0.phase == .confirming }
    }

    /// The time until a card's text next changes by itself (a countdown second, or a hold
    /// running out), or `nil` when none will.
    public var nextChange: Duration? {
        var next: Duration?
        for stop in stops.values {
            let held = age(of: stop)
            let due: Duration? = switch stop.phase {
            case .confirming: Duration.seconds(1) - remainder(held)
            case .ended: Self.endedFor - held
            case .failed: Self.failedFor - held
            case .sending: nil
            }
            if let due, next.map({ due < $0 }) ?? true {
                next = max(due, .zero)
            }
        }
        return next
    }

    /// Takes a poll's live state: copies of the jobs with a stop are refreshed, and a confirm or
    /// refusal is dropped once its job is gone or has moved between running and waiting.
    public func observe(_ state: UiState) {
        let before = seen
        seen = state
        for (lease, stop) in stops {
            if let held = Self.find(state, lease), held.kind == stop.kind {
                stops[lease]?.held = held
            } else if stop.phase == .confirming || stop.phase == .failed {
                stops[lease] = nil
            }
        }
        if let before {
            holdOutsideEnds(before, state)
        }
    }

    /// A press of a running job's Kill, or of its Kill? confirm.
    @discardableResult
    public func pressKill(_ lease: Int64) -> StopPress {
        expire()
        if let stop = stops[lease] {
            switch stop.phase {
            case .confirming where age(of: stop) <= Self.doublePress:
                return .ignored
            case .confirming:
                stops[lease]?.phase = .sending
                stops[lease]?.since = now()
                return .send
            case .sending, .ended:
                return .ignored
            case .failed:
                break
            }
        }
        guard let held = Self.find(seen, lease), held.active != nil else {
            return .ignored
        }
        stops[lease] = Stop(kind: .kill, phase: .confirming, since: now(), held: held)
        return .confirming
    }

    /// A press of a queued request's Remove.
    ///
    /// - Returns: True when the caller sends the request to allotd.
    @discardableResult
    public func pressRemove(_ lease: Int64) -> Bool {
        expire()
        if let stop = stops[lease], stop.phase != .failed {
            return false
        }
        guard let held = Self.find(seen, lease), held.queued != nil else {
            return false
        }
        stops[lease] = Stop(kind: .remove, phase: .sending, since: now(), held: held)
        return true
    }

    /// Esc on Kill?, or the panel losing focus.
    ///
    /// - Returns: True when a confirm was cancelled.
    @discardableResult
    public func cancel(_ lease: Int64) -> Bool {
        guard stops[lease]?.phase == .confirming else {
            return false
        }
        stops[lease] = nil
        return true
    }

    /// Cancels every confirm: the panel closed, or the service went away.
    ///
    /// - Returns: True when any was cancelled.
    @discardableResult
    public func cancelConfirms() -> Bool {
        let confirming = stops.filter { $0.value.phase == .confirming }.keys
        for lease in confirming {
            stops[lease] = nil
        }
        return !confirming.isEmpty
    }

    /// allotd's answer to a request ``pressKill(_:)`` or ``pressRemove(_:)`` sent.
    public func finished(_ lease: Int64, _ result: StopResult) {
        guard var stop = stops[lease], stop.phase == .sending else {
            return
        }
        stop.phase = result.ended ? .ended : .failed
        stop.since = now()
        stop.message = result.text
        if result.ended {
            stop.endedAtMs = result.endedAtMs
        }
        stops[lease] = stop
    }

    /// Drops confirms past their countdown and holds past their time.
    ///
    /// - Returns: True when any was dropped.
    @discardableResult
    public func expire() -> Bool {
        let done = stops.filter { _, stop in
            switch stop.phase {
            case .confirming: age(of: stop) >= Self.confirmFor
            case .ended: age(of: stop) >= Self.endedFor
            case .failed: age(of: stop) >= Self.failedFor
            case .sending: false
            }
        }.keys
        for lease in done {
            stops[lease] = nil
        }
        return !done.isEmpty
    }

    /// `state`'s jobs with the last seen copy of every job whose stop is under way or done put
    /// back where it was, once allotd no longer lists it.
    public func merge(_ state: UiState) -> ShownJobs {
        var jobs = ShownJobs(state)
        let missing = stops.values
            .filter { $0.phase == .sending || $0.phase == .ended }
            .filter { Self.find(state, $0.held.leaseId) == nil }
            .sorted { $0.held.index < $1.held.index }
        for stop in missing {
            if let job = stop.held.active {
                jobs.active.insert(job, at: min(stop.held.index, jobs.active.count))
            } else if let request = stop.held.queued {
                jobs.queue.insert(request, at: min(stop.held.index, jobs.queue.count))
            }
        }
        return jobs
    }

    // A running job allotd stopped listing without a stop of this panel holds its card too,
    // read from allotd's ended list: released (Finished), killed elsewhere, or ended by allotd.
    // Removals, queued requests and benchmarks (their Finished card comes from recent runs) are
    // not held, and neither is anything the ended list does not explain.
    private func holdOutsideEnds(_ before: UiState, _ state: UiState) {
        for (index, job) in before.active.enumerated() {
            let lease = job.lease
            guard stops[lease.id] == nil,
                  lease.kind != .benchmarkExclusive,
                  Self.find(state, lease.id) == nil,
                  let ended = state.ended.first(where: { $0.leaseId == lease.id })
            else {
                continue
            }
            let kind: StopKind
            switch ended.cause {
            case .released: kind = .finished
            case .killed: kind = .killedOutside
            case .expired: kind = .endedByAllotd
            case .removed: continue
            }
            var stop = Stop(
                kind: kind,
                phase: .ended,
                since: now(),
                held: Held(leaseId: lease.id, active: job, queued: nil, index: index))
            stop.message = kind == .endedByAllotd ? ended.reason : ""
            stop.endedAtMs = ended.endedAtMs
            stops[lease.id] = stop
        }
    }

    private static func find(_ state: UiState?, _ lease: Int64) -> Held? {
        guard let state else {
            return nil
        }
        if let index = state.active.firstIndex(where: { $0.lease.id == lease }) {
            return Held(leaseId: lease, active: state.active[index], queued: nil, index: index)
        }
        if let index = state.queue.firstIndex(where: { $0.lease.id == lease }) {
            return Held(leaseId: lease, active: nil, queued: state.queue[index], index: index)
        }
        return nil
    }

    private func age(of stop: Stop) -> Duration {
        now() - stop.since
    }

    // The part of a whole second gone by, exact, so the countdown lands on the second.
    private func remainder(_ age: Duration) -> Duration {
        Duration(secondsComponent: 0, attosecondsComponent: max(0, age.components.attoseconds))
    }

    private func view(of stop: Stop) -> StopView {
        let elapsed = (age(of: stop) / .seconds(1)).rounded(.down)
        let left = stop.phase == .confirming
            ? max(1, Int(Self.confirmFor / .seconds(1) - elapsed))
            : 0
        return StopView(
            kind: stop.kind,
            phase: stop.phase,
            secondsLeft: left,
            message: stop.message,
            endedAtMs: stop.endedAtMs)
    }

    /// The job as last seen: the running job or the waiting request, and its place in
    /// allotd's active or queue list.
    private struct Held {
        let leaseId: Int64
        let active: ActiveJob?
        let queued: QueuedJob?
        let index: Int

        var kind: StopKind {
            active == nil ? .remove : .kill
        }
    }

    private struct Stop {
        let kind: StopKind
        var phase: StopPhase

        /// When it entered the phase, in local time.
        var since: ContinuousClock.Instant

        var held: Held
        var message = ""
        var endedAtMs: Int64?
    }
}
