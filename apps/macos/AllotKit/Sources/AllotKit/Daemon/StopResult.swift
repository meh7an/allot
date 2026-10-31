import AllotContract

/// What allotd answered a Kill or Remove from the panel.
public struct StopResult: Sendable, Equatable {
    /// allotd ended the lease, and its session will be told.
    public let ended: Bool

    /// Ended: a note for the Stopped line, or empty. Otherwise why not.
    public let text: String

    /// Ended: when the lease ended, in the daemon's clock.
    public let endedAtMs: Int64?

    public init(ended: Bool, text: String, endedAtMs: Int64? = nil) {
        self.ended = ended
        self.text = text
        self.endedAtMs = endedAtMs
    }

    /// A kill ends the lease even when part of the tree, or all of it, is left running; the
    /// note says so.
    public static func killed(_ response: KillResponse) -> StopResult {
        let note = if response.untouchedReason != nil {
            "nothing was running"
        } else {
            switch response.failed.count {
            case 0: ""
            case 1: "1 process still running"
            case let count: "\(count) processes still running"
            }
        }
        return StopResult(ended: true, text: note, endedAtMs: response.lease.endedAtMs)
    }

    public static func removed(_ response: RemoveResponse) -> StopResult {
        StopResult(ended: true, text: "", endedAtMs: response.lease.endedAtMs)
    }

    public static func refused(_ why: String) -> StopResult {
        StopResult(ended: false, text: why)
    }
}

/// Ends leases on the user's behalf: ``DaemonClient``, or a fake for the Debug scenarios.
public protocol JobStopper: Sendable {
    /// `POST /leases/{id}/kill`.
    ///
    /// - Throws: Only `CancellationError`.
    func kill(_ lease: Int64) async throws -> StopResult

    /// `POST /leases/{id}/remove`.
    ///
    /// - Throws: Only `CancellationError`.
    func remove(_ lease: Int64) async throws -> StopResult
}
