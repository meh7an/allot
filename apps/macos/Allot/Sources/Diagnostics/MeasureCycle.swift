import AllotKit
import AppKit
import Darwin
import QuartzCore

/// A scripted measuring run, off unless `ALLOT_MEASURE=cycle` is set: closed, repeated opens,
/// open and pinned, frames while open, closed again, then quit. Writes `logs/measure.log`.
/// The usage phases run without a display link, because a link forces frames of its own.
@MainActor
final class MeasureCycle: NSObject {
    private static let opens = 10
    private static let animationSeconds = 0.25
    private static let settleSeconds = 1.5

    private let panel: PanelController
    private let model: PanelModel
    private let logFile: URL
    private let phaseSeconds: Double
    private var link: CADisplayLink?
    private var frames: [Double] = []
    private var frameSeconds = 0.0
    private var openStarted = 0.0
    private var firstFrame: Double?

    /// The run the environment asks for, if any.
    static func requested(
        panel: PanelController, model: PanelModel, logDirectory: URL
    ) -> MeasureCycle? {
        let environment = ProcessInfo.processInfo.environment
        guard environment["ALLOT_MEASURE"] == "cycle" else {
            return nil
        }
        let seconds = environment["ALLOT_MEASURE_SECS"].flatMap(Double.init) ?? 120
        return MeasureCycle(
            panel: panel, model: model, logDirectory: logDirectory, phaseSeconds: seconds)
    }

    private init(
        panel: PanelController, model: PanelModel, logDirectory: URL, phaseSeconds: Double
    ) {
        self.panel = panel
        self.model = model
        self.phaseSeconds = phaseSeconds
        logFile = logDirectory.appendingPathComponent("measure.log")
        try? FileManager.default.createDirectory(
            at: logDirectory, withIntermediateDirectories: true)
    }

    func run() async {
        write("start screen_fps=\(NSScreen.main?.maximumFramesPerSecond ?? 0)")
        await sleep(20)
        await usage("closed")

        for number in 1...Self.opens {
            await measureOpen(number)
        }
        await sleep(Self.settleSeconds)

        panel.open()
        panel.pinForMeasuring()
        await sleep(10)
        await usage("open")
        await measureFrames("open-frames", seconds: min(phaseSeconds, 30))
        snapshot()
        panel.close()

        await sleep(10)
        await usage("closed-after")
        write("done")
        NSApp.terminate(nil)
    }

    // MARK: - Phases

    private func usage(_ phase: String) async {
        let before = Self.sample()
        let started = CACurrentMediaTime()
        await sleep(phaseSeconds)
        let after = Self.sample()
        let wall = CACurrentMediaTime() - started
        let cpu = cpuPercentOfOneCore(
            cpuSeconds: after.cpuSeconds - before.cpuSeconds, wallSeconds: wall)
        write(
            "usage phase=\(phase) secs=\(Self.text(wall)) cpu_pct=\(Self.text(cpu)) "
                + "footprint_mb=\(Self.text(after.footprintMB)) "
                + "peak_footprint_mb=\(Self.text(after.peakFootprintMB)) "
                + "visible=\(panel.measuredView.window?.occlusionState.contains(.visible) ?? false)")
    }

    private func measureOpen(_ number: Int) async {
        startLink()
        firstFrame = nil
        openStarted = CACurrentMediaTime()
        panel.open()
        let opened = CACurrentMediaTime()
        await sleep(Self.animationSeconds)
        let openFrames = FrameStats(timestamps: frames, frameSeconds: frameSeconds)
        let latency = firstFrame.map { ($0 - openStarted) * 1000 } ?? -1
        await sleep(Self.settleSeconds)

        frames = []
        panel.close()
        await sleep(Self.animationSeconds)
        let closeFrames = FrameStats(timestamps: frames, frameSeconds: frameSeconds)
        stopLink()
        write(
            "open n=\(number) latency_ms=\(Self.text(latency)) "
                + "open_call_ms=\(Self.text((opened - openStarted) * 1000)) "
                + "frames=\(openFrames.frames) dropped=\(openFrames.dropped) "
                + "longest_ms=\(Self.text(openFrames.longestMs)) "
                + "close_frames=\(closeFrames.frames) close_dropped=\(closeFrames.dropped) "
                + "refresh_ms=\(Self.text(frameSeconds * 1000)) "
                + "peak_footprint_mb=\(Self.text(Self.sample().peakFootprintMB))")
        await sleep(Self.settleSeconds)
    }

    private func measureFrames(_ phase: String, seconds: Double) async {
        startLink()
        await sleep(seconds)
        let stats = FrameStats(timestamps: frames, frameSeconds: frameSeconds)
        stopLink()
        write(
            "frames phase=\(phase) secs=\(Self.text(seconds)) frames=\(stats.frames) "
                + "dropped=\(stats.dropped) longest_ms=\(Self.text(stats.longestMs)) "
                + "refresh_ms=\(Self.text(frameSeconds * 1000))")
    }

    // MARK: - Display link

    private func startLink() {
        frames = []
        let view = panel.measuredView
        let link = view.displayLink(target: self, selector: #selector(frameFired(_:)))
        link.add(to: .main, forMode: .common)
        self.link = link
    }

    private func stopLink() {
        link?.invalidate()
        link = nil
    }

    @objc private func frameFired(_ link: CADisplayLink) {
        let now = CACurrentMediaTime()
        if firstFrame == nil {
            firstFrame = now
        }
        frames.append(now)
        frameSeconds = link.duration
    }

    // MARK: - Reading

    private struct Sample {
        let cpuSeconds: Double
        let footprintMB: Double
        let peakFootprintMB: Double
    }

    private static func sample() -> Sample {
        var clock = timespec()
        clock_gettime(CLOCK_PROCESS_CPUTIME_ID, &clock)
        let cpu = Double(clock.tv_sec) + Double(clock.tv_nsec) / 1_000_000_000
        var info = rusage_info_v4()
        let answer = withUnsafeMutablePointer(to: &info) { pointer in
            pointer.withMemoryRebound(to: rusage_info_t?.self, capacity: 1) {
                proc_pid_rusage(getpid(), RUSAGE_INFO_V4, $0)
            }
        }
        guard answer == 0 else {
            return Sample(cpuSeconds: cpu, footprintMB: -1, peakFootprintMB: -1)
        }
        let megabyte = 1024.0 * 1024.0
        return Sample(
            cpuSeconds: cpu,
            footprintMB: Double(info.ri_phys_footprint) / megabyte,
            peakFootprintMB: Double(info.ri_lifetime_max_phys_footprint) / megabyte)
    }

    // MARK: - Helpers

    // What the panel drew, and where, for a run nobody watches. Drawing it this way takes about
    // 200 MB for a moment, so it comes after the last reading of an open panel.
    private func snapshot() {
        let view = panel.measuredView
        write("panel frame=\(view.window?.frame ?? .zero)")
        guard let image = view.bitmapImageRepForCachingDisplay(in: view.bounds) else {
            return
        }
        view.cacheDisplay(in: view.bounds, to: image)
        let file = logFile.deletingLastPathComponent().appendingPathComponent("measure-open.png")
        try? image.representation(using: .png, properties: [:])?.write(to: file)
    }

    private func sleep(_ seconds: Double) async {
        try? await Task.sleep(for: .seconds(seconds))
    }

    private func write(_ line: String) {
        let text = "\(Date().timeIntervalSince1970) measure: \(line)\n"
        guard let data = text.data(using: .utf8) else {
            return
        }
        guard let handle = try? FileHandle(forWritingTo: logFile) else {
            try? data.write(to: logFile)
            return
        }
        defer {
            try? handle.close()
        }
        _ = try? handle.seekToEnd()
        try? handle.write(contentsOf: data)
    }

    private static func text(_ value: Double) -> String {
        String(format: "%.2f", value)
    }
}
