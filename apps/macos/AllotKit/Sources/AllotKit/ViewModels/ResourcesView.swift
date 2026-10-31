import AllotContract

/// One bullet-bar row of the Resources card: CPU, RAM, or a GPU's engine load or video memory.
public struct MeterRow: Sendable, Equatable, Identifiable {
    /// Stable across polls: `cpu`, `ram`, `gpu:0`, `vram:0`.
    public let id: String

    /// For example `CPU`, or `GPU 0` with several adapters.
    public let label: String

    /// The used fill, as a fraction of the bar.
    public let used: Double

    /// The part of the used fill that is GPU memory, as a fraction of the bar; only on the RAM
    /// row of a Mac whose GPU shares RAM, else `nil`.
    public let shared: Double?

    /// Where the reserved tick sits, as a fraction of the bar; `nil` without a reservation.
    public let reserved: Double?

    /// Where the hatched headroom starts, as a fraction of the bar.
    public let headroom: Double

    /// For example `64%` or `22.4 / 32 GB`.
    public let value: String

    /// The bar's detail, for example `10 of 16 cores reserved`.
    public let tooltip: String

    /// The CPU sparkline's samples as fractions, oldest first; empty on other rows.
    public let spark: [Double]
}

/// One drive of the Resources card.
public struct DriveRow: Sendable, Equatable, Identifiable {
    /// The mount point allotd names the drive by, for example `/` or `/Volumes/Backup`.
    public let id: String

    /// The volume's name, for example `Macintosh HD`.
    public let label: String

    /// The usage bar's fill, as a fraction.
    public let used: Double

    /// For example `R 42 · W 8 MB/s` or `Idle`; empty while Drive activity is off.
    public let activity: String

    /// For example `612 / 953 GB` or `1.21 / 1.82 TB`.
    public let value: String
}

/// The docked Resources card for one poll (handoff section 5), built by a pure function so that
/// every string is testable. The Windows tray app's `ResourcesView`, except that a GPU sharing
/// RAM has no VRAM row: its memory shows as a tint inside the RAM bar.
public struct ResourcesView: Sendable, Equatable {
    private static let sparkSamples = 24
    private static let mebibytesPerTebibyte = 1024.0 * 1024
    private static let bytesPerMebibyte = 1024.0 * 1024
    private static let idleRateMb = 0.1
    private static let wholeRateMb = 10.0

    /// The service serves live state; there is no card without data.
    public let isVisible: Bool

    /// The card shows its rows instead of the summary.
    public let isExpanded: Bool

    /// The one-line summary shown while collapsed, for example `CPU 64% · RAM 22.4 / 32 GB ·
    /// GPU 78%`; empty while expanded.
    public let summary: String

    /// CPU, RAM, then GPU and VRAM per adapter, as the toggles allow.
    public let meters: [MeterRow]

    /// One row per drive while Drives is on.
    public let drives: [DriveRow]

    /// The hairline between the meters and the drives.
    public let showsSeparator: Bool

    /// Every meter is off; the card says how to turn them on.
    public let allOff: Bool

    /// Some bar has a reserved tick, so the legend explains it.
    public let showsLegend: Bool

    /// The RAM bar has a GPU tint, so the legend names it.
    public let showsSharedLegend: Bool

    /// Some GPU uses RAM as its memory, so the Meters menu calls VRAM `GPU Memory`.
    public let sharesMemory: Bool

    public static let hidden = ResourcesView(
        isVisible: false, isExpanded: false, summary: "", meters: [], drives: [],
        showsSeparator: false, allOff: false, showsLegend: false, showsSharedLegend: false,
        sharesMemory: false)

    /// - Parameters:
    ///   - status: The service status of the latest poll.
    ///   - state: The latest state; ignored unless the service is running.
    ///   - toggles: The More options › Meters check items.
    ///   - expanded: The card shows its rows instead of the summary.
    ///   - volumeName: The volume name mounted at a mount point, or `nil` when it is unknown.
    public static func from(
        status: ServiceStatus,
        state: UiState?,
        toggles: MeterToggles,
        expanded: Bool,
        volumeName: (String) -> String? = { _ in nil }
    ) -> ResourcesView {
        guard status == .running, let state else {
            return .hidden
        }

        let meters = state.meters
        let reservations = Reservations.of(state)
        let headroom = clamp(1 - Double(meters.headroomPct) / 100)
        // allotd counts a shared adapter's memory in RAM already, and admission adds its
        // reserved video memory to the reserved RAM.
        let sharing = meters.gpus.filter(\.unifiedMemory)
        let sharedMb = toggles.vram ? sharing.map(\.vramUsedMb).reduce(0, +) : 0
        let sharedReservedMb = sharing.map(\.reservedVramMb).reduce(0, +)

        var rows: [MeterRow] = []
        if toggles.cpu {
            rows.append(cpu(meters.cpu, reservations, headroom))
        }
        if toggles.ram {
            rows.append(ram(
                meters.ram, sharedMb: sharedMb, sharedReservedMb: sharedReservedMb,
                reservations, headroom))
        }
        let several = meters.gpus.count > 1
        for (index, gpu) in meters.gpus.enumerated() {
            let suffix = several ? " \(index)" : ""
            let prefix = several ? "\(gpu.name) · " : ""
            if toggles.gpu {
                rows.append(self.gpu(
                    gpu, id: "gpu:\(index)", label: "GPU\(suffix)", prefix: prefix,
                    reservations, headroom))
            }
            if toggles.vram && !gpu.unifiedMemory {
                rows.append(vram(
                    gpu, id: "vram:\(index)", label: "VRAM\(suffix)", prefix: prefix,
                    reservations, headroom))
            }
        }

        let drives = toggles.drives
            ? meters.drives.map { drive(
                $0, activity: toggles.driveActivity, volumeName: volumeName($0.name)) }
            : []
        let allOff = !(toggles.cpu || toggles.ram || toggles.gpu || toggles.vram
            || toggles.drives)
        return ResourcesView(
            isVisible: true,
            isExpanded: expanded,
            summary: expanded ? "" : summary(meters, toggles),
            meters: rows,
            drives: drives,
            showsSeparator: !rows.isEmpty && !drives.isEmpty,
            allOff: allOff,
            showsLegend: rows.contains { $0.reserved != nil },
            showsSharedLegend: rows.contains { $0.shared != nil },
            sharesMemory: !sharing.isEmpty)
    }

    /// A drive's label: its volume name, or the mount point's last component, or the mount
    /// point itself for `/`.
    public static func driveLabel(mountPoint: String, volumeName: String?) -> String {
        if let volumeName, !volumeName.isEmpty {
            return volumeName
        }
        return mountPoint.split(separator: "/").last.map(String.init) ?? mountPoint
    }

    private static func cpu(
        _ cpu: CpuMeter,
        _ reservations: Reservations,
        _ headroom: Double
    ) -> MeterRow {
        let cores = upToOneDecimal(Double(cpu.reservedCores))
        let tooltip = reservations.benchmark ? "Whole machine reserved for the benchmark"
            : cpu.reservedCores > 0 ? "\(cores) of \(cpu.logicalCores) cores reserved"
            : reservations.none
        let reserved = reservations.benchmark
            ? 1 : fraction(Double(cpu.reservedCores), Double(cpu.logicalCores))
        let spark = cpu.historyPct.suffix(sparkSamples).map { clamp(Double($0) / 100) }
        return MeterRow(
            id: "cpu",
            label: "CPU",
            used: clamp(Double(cpu.usagePct) / 100),
            shared: nil,
            reserved: tick(reserved),
            headroom: headroom,
            value: percent(cpu.usagePct),
            tooltip: tooltip,
            spark: spark)
    }

    private static func ram(
        _ ram: RamMeter,
        sharedMb: UInt64,
        sharedReservedMb: UInt64,
        _ reservations: Reservations,
        _ headroom: Double
    ) -> MeterRow {
        let reservedMb = ram.reservedMb + sharedReservedMb
        var tooltip = reservations.benchmark ? Reservations.wholeMachine
            : reservedMb > 0 ? "\(Format.size(reservedMb)) GB reserved"
            : reservations.none
        if sharedMb > 0 {
            tooltip += " · GPU uses \(Format.gigabytes(sharedMb)) GB of it"
        }
        let used = fraction(Double(ram.usedMb), Double(ram.totalMb))
        let reserved = reservations.benchmark
            ? 1 : fraction(Double(reservedMb), Double(ram.totalMb))
        return MeterRow(
            id: "ram",
            label: "RAM",
            used: used,
            shared: sharedMb > 0
                ? min(used, fraction(Double(sharedMb), Double(ram.totalMb))) : nil,
            reserved: tick(reserved),
            headroom: headroom,
            value: "\(Format.gigabytes(ram.usedMb)) / \(Format.size(ram.totalMb)) GB",
            tooltip: tooltip,
            spark: [])
    }

    private static func gpu(
        _ gpu: GpuMeter,
        id: String,
        label: String,
        prefix: String,
        _ reservations: Reservations,
        _ headroom: Double
    ) -> MeterRow {
        let tooltip = reservations.benchmark ? Reservations.wholeMachine
            : gpu.reservedPct > 0 ? "\(percent(gpu.reservedPct)) reserved"
            : reservations.none
        let reserved = reservations.benchmark ? 1 : clamp(Double(gpu.reservedPct) / 100)
        return MeterRow(
            id: id,
            label: label,
            used: clamp(Double(gpu.busiestPct) / 100),
            shared: nil,
            reserved: tick(reserved),
            headroom: headroom,
            value: percent(gpu.busiestPct),
            tooltip: prefix + tooltip,
            spark: [])
    }

    // Without a reported total there is nothing to measure the bar against: it stays empty, and
    // the value reads the used memory alone.
    private static func vram(
        _ gpu: GpuMeter,
        id: String,
        label: String,
        prefix: String,
        _ reservations: Reservations,
        _ headroom: Double
    ) -> MeterRow {
        let tooltip = reservations.benchmark ? Reservations.wholeMachine
            : gpu.reservedVramMb > 0 ? "\(Format.size(gpu.reservedVramMb)) GB reserved"
            : reservations.none
        guard let total = gpu.vramTotalMb else {
            return MeterRow(
                id: id,
                label: label,
                used: 0,
                shared: nil,
                reserved: nil,
                headroom: headroom,
                value: "\(Format.gigabytes(gpu.vramUsedMb)) GB",
                tooltip: "\(prefix)\(tooltip) · total not reported",
                spark: [])
        }
        let reserved = reservations.benchmark
            ? 1 : fraction(Double(gpu.reservedVramMb), Double(total))
        return MeterRow(
            id: id,
            label: label,
            used: fraction(Double(gpu.vramUsedMb), Double(total)),
            shared: nil,
            reserved: tick(reserved),
            headroom: headroom,
            value: "\(Format.gigabytes(gpu.vramUsedMb)) / \(Format.size(total)) GB",
            tooltip: prefix + tooltip,
            spark: [])
    }

    private static func drive(
        _ drive: DriveUsage,
        activity: Bool,
        volumeName: String?
    ) -> DriveRow {
        DriveRow(
            id: drive.name,
            label: driveLabel(mountPoint: drive.name, volumeName: volumeName),
            used: fraction(Double(drive.usedMb), Double(drive.totalMb)),
            activity: activity ? self.activity(drive) : "",
            value: space(usedMb: drive.usedMb, totalMb: drive.totalMb))
    }

    /// `612 / 953 GB` in whole gigabytes, or from 1000 GB `1.21 / 1.82 TB`.
    private static func space(usedMb: UInt64, totalMb: UInt64) -> String {
        let totalGb = Double(totalMb) / 1024
        if totalGb.rounded(.toNearestOrAwayFromZero) < 1000 {
            return "\(Format.whole(Double(usedMb) / 1024)) / \(Format.whole(totalGb)) GB"
        }
        let used = upToTwoDecimals(Double(usedMb) / mebibytesPerTebibyte)
        let total = upToTwoDecimals(Double(totalMb) / mebibytesPerTebibyte)
        return "\(used) / \(total) TB"
    }

    /// `R 42 · W 8 MB/s`, one side alone when the other is idle, or `Idle`.
    private static func activity(_ drive: DriveUsage) -> String {
        let parts = [
            rate(drive.readBytesPerSec).map { "R \($0)" },
            rate(drive.writeBytesPerSec).map { "W \($0)" },
        ].compactMap { $0 }
        return parts.isEmpty ? "Idle" : "\(parts.joined(separator: " · ")) MB/s"
    }

    // Megabytes per second with one decimal below 10, whole above; nil under 0.1 MB/s.
    private static func rate(_ bytesPerSec: UInt64) -> String? {
        let megabytes = Double(bytesPerSec) / bytesPerMebibyte
        guard megabytes >= idleRateMb else {
            return nil
        }
        return megabytes < wholeRateMb ? upToOneDecimal(megabytes) : Format.whole(megabytes)
    }

    // The first adapter stands for the GPU in the summary, as on the handoff's board.
    private static func summary(_ meters: Meters, _ toggles: MeterToggles) -> String {
        var parts: [String] = []
        if toggles.cpu {
            parts.append("CPU \(percent(meters.cpu.usagePct))")
        }
        if toggles.ram {
            let ram = meters.ram
            parts.append("RAM \(Format.gigabytes(ram.usedMb)) / \(Format.size(ram.totalMb)) GB")
        }
        if toggles.gpu, let first = meters.gpus.first {
            parts.append("GPU \(percent(first.busiestPct))")
        }
        return parts.joined(separator: " · ")
    }

    private static func tick(_ reserved: Double) -> Double? {
        reserved > 0 ? reserved : nil
    }

    private static func fraction(_ part: Double, _ whole: Double) -> Double {
        whole > 0 ? clamp(part / whole) : 0
    }

    private static func clamp(_ value: Double) -> Double {
        min(max(value, 0), 1)
    }

    private static func percent(_ pct: Float) -> String {
        "\(Format.whole(Double(pct)))%"
    }

    // .NET's `0.#` and `0.##`, as the Windows card prints them.
    private static func upToOneDecimal(_ value: Double) -> String {
        trimmed(value, decimals: 1)
    }

    private static func upToTwoDecimals(_ value: Double) -> String {
        trimmed(value, decimals: 2)
    }

    private static func trimmed(_ value: Double, decimals: Int) -> String {
        let scale = decimals == 1 ? 10.0 : 100.0
        let rounded = (value * scale).rounded(.toNearestOrAwayFromZero) / scale
        var text = String(format: "%.\(decimals)f", rounded)
        while text.hasSuffix("0") {
            text.removeLast()
        }
        if text.hasSuffix(".") {
            text.removeLast()
        }
        return text
    }

    /// What the leases reserve as a whole: nothing at all reads "No reservations" on every bar,
    /// a bar of its own reads "Not reserved", and a benchmark running alone has the whole
    /// machine.
    private struct Reservations {
        static let wholeMachine = "Whole machine reserved"

        let benchmark: Bool
        let any: Bool

        var none: String {
            any ? "Not reserved" : "No reservations"
        }

        static func of(_ state: UiState) -> Reservations {
            let meters = state.meters
            let benchmark = state.active.contains { $0.lease.kind == .benchmarkExclusive }
            let any = meters.cpu.reservedCores > 0
                || meters.ram.reservedMb > 0
                || meters.gpus.contains { $0.reservedPct > 0 || $0.reservedVramMb > 0 }
            return Reservations(benchmark: benchmark, any: any)
        }
    }
}
