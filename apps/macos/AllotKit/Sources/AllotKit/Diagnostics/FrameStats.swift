/// Frames counted against the display's own rate, from the times a display link fired.
public struct FrameStats: Sendable, Equatable {
    public let frames: Int
    public let dropped: Int
    public let longestMs: Double

    /// - Parameters:
    ///   - timestamps: When each frame fired, in seconds, oldest first.
    ///   - frameSeconds: One refresh of the display.
    public init(timestamps: [Double], frameSeconds: Double) {
        frames = timestamps.count
        var dropped = 0
        var longest = 0.0
        for (earlier, later) in zip(timestamps, timestamps.dropFirst()) {
            let gap = later - earlier
            longest = max(longest, gap)
            if frameSeconds > 0 {
                dropped += max(0, Int((gap / frameSeconds).rounded()) - 1)
            }
        }
        self.dropped = dropped
        longestMs = longest * 1000
    }
}

/// CPU over a stretch of wall time, as a percentage of one core.
public func cpuPercentOfOneCore(cpuSeconds: Double, wallSeconds: Double) -> Double {
    wallSeconds > 0 ? cpuSeconds / wallSeconds * 100 : 0
}
