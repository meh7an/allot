import AllotContract
import Foundation
import Testing

@testable import AllotKit

/// The macOS fixture: 14 cores at 38% with 2 reserved, RAM 24 of 36 GB with 4 GB reserved,
/// one Apple M3 Max sharing RAM (64%, 9 GB in use, 60% and 8 GB reserved), and two drives.
private func card(
    _ state: UiState,
    status: ServiceStatus = .running,
    toggles: MeterToggles = MeterToggles(),
    expanded: Bool = true,
    volumeName: (String) -> String? = { _ in nil }
) -> ResourcesView {
    ResourcesView.from(
        status: status, state: state, toggles: toggles, expanded: expanded,
        volumeName: volumeName)
}

private func row(_ view: ResourcesView, _ id: String) -> MeterRow? {
    view.meters.first { $0.id == id }
}

// A dedicated adapter, as on Linux or Windows: 5 of 12 GB with 2 GB reserved.
private func dedicated(totalMb: Any = 12288) throws -> UiState {
    try Edit.state(Fixtures.state("macos")) { object in
        var meters = object["meters"] as? Edit.Object ?? [:]
        var gpus = meters["gpus"] as? [Edit.Object] ?? []
        gpus[0]["unified_memory"] = false
        gpus[0]["vram_total_mb"] = totalMb
        gpus[0]["vram_used_mb"] = 5120
        gpus[0]["reserved_vram_mb"] = 2048
        meters["gpus"] = gpus
        object["meters"] = meters
    }
}

@Suite struct ResourcesViewTests {
    @Test(arguments: [ServiceStatus.starting, .stopped, .incompatible])
    func noCardWithoutLiveState(status: ServiceStatus) throws {
        let view = card(try Fixtures.state("macos"), status: status)

        #expect(view == .hidden)
    }

    @Test func sharedGpuMemoryIsTheRamBarsTintAndHasNoRowOfItsOwn() throws {
        let view = card(try Fixtures.state("macos"))

        #expect(view.meters.map(\.id) == ["cpu", "ram", "gpu:0"])
        #expect(view.sharesMemory)
        #expect(view.showsSharedLegend)
        let ram = try #require(row(view, "ram"))
        #expect(ram.value == "24.0 / 36 GB")
        #expect(ram.shared == 0.25)
        // Admission adds a shared adapter's reserved memory to the reserved RAM.
        #expect(ram.reserved == 12288.0 / 36864)
        #expect(ram.tooltip == "12 GB reserved · GPU uses 9.0 GB of it")
    }

    @Test func cpuAndGpuRows() throws {
        let view = card(try Fixtures.state("macos"))

        let cpu = try #require(row(view, "cpu"))
        #expect(cpu.label == "CPU")
        #expect(cpu.value == "38%")
        #expect(cpu.tooltip == "2 of 14 cores reserved")
        #expect(cpu.reserved == 2.0 / 14)
        #expect(abs(cpu.headroom - 0.85) < 1e-9)
        #expect(cpu.spark.count == 7)
        #expect(cpu.spark.last == 0.38)
        let gpu = try #require(row(view, "gpu:0"))
        #expect(gpu.label == "GPU")
        #expect(gpu.value == "64%")
        #expect(gpu.tooltip == "60% reserved")
    }

    @Test func vramOffDropsTheTintButKeepsTheReservation() throws {
        var toggles = MeterToggles()
        toggles.vram = false

        let view = card(try Fixtures.state("macos"), toggles: toggles)

        let ram = try #require(row(view, "ram"))
        #expect(ram.shared == nil)
        #expect(ram.tooltip == "12 GB reserved")
        #expect(!view.showsSharedLegend)
        #expect(view.sharesMemory)
    }

    @Test func dedicatedAdapterKeepsItsVramRow() throws {
        let view = card(try dedicated())

        #expect(view.meters.map(\.id) == ["cpu", "ram", "gpu:0", "vram:0"])
        #expect(!view.sharesMemory)
        #expect(row(view, "ram")?.shared == nil)
        #expect(row(view, "ram")?.tooltip == "4 GB reserved")
        let vram = try #require(row(view, "vram:0"))
        #expect(vram.value == "5.0 / 12 GB")
        #expect(vram.tooltip == "2 GB reserved")
    }

    @Test func vramWithoutATotalHasAnEmptyBar() throws {
        let view = card(try dedicated(totalMb: NSNull()))

        let vram = try #require(row(view, "vram:0"))
        #expect(vram.used == 0)
        #expect(vram.reserved == nil)
        #expect(vram.value == "5.0 GB")
        #expect(vram.tooltip == "2 GB reserved · total not reported")
    }

    @Test func drivesUseVolumeNamesAndTheirSpaceAndActivity() throws {
        let view = card(try Fixtures.state("macos")) { $0 == "/" ? "Macintosh HD" : nil }

        #expect(view.drives.map(\.label) == ["Macintosh HD", "Backup"])
        #expect(view.drives.map(\.id) == ["/", "/Volumes/Backup"])
        #expect(view.drives.map(\.value) == ["600 / 976 GB", "1 / 1.82 TB"])
        #expect(view.drives.map(\.activity) == ["R 3 · W 1 MB/s", "Idle"])
        #expect(view.showsSeparator)
    }

    @Test func driveLabelFallsBackToTheMountPoint() {
        #expect(ResourcesView.driveLabel(mountPoint: "/", volumeName: nil) == "/")
        #expect(ResourcesView.driveLabel(mountPoint: "/Volumes/Backup", volumeName: "") ==
            "Backup")
    }

    @Test func driveActivityOffLeavesItEmpty() throws {
        var toggles = MeterToggles()
        toggles.driveActivity = false

        let view = card(try Fixtures.state("macos"), toggles: toggles)

        #expect(view.drives.map(\.activity) == ["", ""])
    }

    @Test func collapsedShowsTheSummary() throws {
        let collapsed = card(try Fixtures.state("macos"), expanded: false)
        let expanded = card(try Fixtures.state("macos"))

        #expect(collapsed.summary == "CPU 38% · RAM 24.0 / 36 GB · GPU 64%")
        #expect(expanded.summary.isEmpty)
    }

    @Test func everyMeterOffSaysSo() throws {
        var toggles = MeterToggles()
        toggles.cpu = false
        toggles.ram = false
        toggles.gpu = false
        toggles.vram = false
        toggles.drives = false

        let view = card(try Fixtures.state("macos"), toggles: toggles)

        #expect(view.allOff)
        #expect(view.meters.isEmpty)
        #expect(view.drives.isEmpty)
        #expect(!view.showsLegend)
    }

    @Test func noReservationReadsNoReservations() throws {
        let view = card(try Fixtures.state("first-run"))

        #expect(view.meters.allSatisfy { $0.reserved == nil })
        #expect(row(view, "cpu")?.tooltip == "No reservations")
        #expect(!view.showsLegend)
    }

    @Test func benchmarkRunningAloneReservesTheWholeMachine() throws {
        let (_, state) = try Handoff.scenario("bench-run")

        let view = card(state)

        #expect(view.meters.allSatisfy { $0.reserved == 1 })
        #expect(row(view, "cpu")?.tooltip == "Whole machine reserved for the benchmark")
        #expect(row(view, "gpu:0")?.tooltip == "Whole machine reserved")
    }

    @Test func handoffMeterSetsDecode() throws {
        let (_, max) = try Handoff.scenario("max")
        let (_, idle) = try Handoff.scenario("empty")

        #expect(max.meters.drives.count == 3)
        #expect(idle.meters.drives.count == 2)
        #expect(card(max, expanded: false).summary == "CPU 88% · RAM 25.9 / 36 GB · GPU 78%")
    }
}

@Suite struct MenuCountsTests {
    @Test func countsLeaveDriveActivityOut() {
        var meters = MeterToggles()
        meters.vram = false
        meters.driveActivity = false

        #expect(MenuCounts.meters(meters) == "4 of 5")
        #expect(MenuCounts.notifications(NotificationToggles()) == "3 of 5")
    }
}
