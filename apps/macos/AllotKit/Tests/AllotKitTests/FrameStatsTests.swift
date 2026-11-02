import Testing

@testable import AllotKit

@Test func steadyFramesDropNothing() {
    let stats = FrameStats(timestamps: [0, 0.01, 0.02, 0.03], frameSeconds: 0.01)
    #expect(stats.frames == 4)
    #expect(stats.dropped == 0)
    #expect(abs(stats.longestMs - 10) < 0.001)
}

@Test func aLongGapCountsTheRefreshesItMissed() {
    let stats = FrameStats(timestamps: [0, 0.01, 0.05, 0.06], frameSeconds: 0.01)
    #expect(stats.dropped == 3)
    #expect(abs(stats.longestMs - 40) < 0.001)
}

@Test func aSlightlyLateFrameIsNotADrop() {
    let stats = FrameStats(timestamps: [0, 0.0134, 0.02], frameSeconds: 0.01)
    #expect(stats.dropped == 0)
}

@Test func noFramesAndNoRateAreHarmless() {
    #expect(FrameStats(timestamps: [], frameSeconds: 0.01).frames == 0)
    #expect(FrameStats(timestamps: [0, 1], frameSeconds: 0).dropped == 0)
}

@Test func cpuIsAPercentageOfOneCore() {
    #expect(cpuPercentOfOneCore(cpuSeconds: 0.6, wallSeconds: 120) == 0.5)
    #expect(cpuPercentOfOneCore(cpuSeconds: 1, wallSeconds: 0) == 0)
}
