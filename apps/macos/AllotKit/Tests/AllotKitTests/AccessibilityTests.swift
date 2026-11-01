import Foundation
import Testing

@testable import AllotKit

@Suite struct TextSizeTests {
    @Test func columnsKeepTheHandoffWidthsAtTheDefaultSize() {
        let columns = MeterColumns(scale: TextSize.standard.scale)

        #expect(columns.label == 48 && columns.value == 108)
    }

    @Test func columnsGrowWithTheText() {
        let large = MeterColumns(scale: TextSize.large.scale)
        let larger = MeterColumns(scale: TextSize.larger.scale)

        #expect(large.label == 57 && large.value == 127)
        #expect(larger.label == 65 && larger.value == 146)
    }

    @Test func theValueGivesWayBeforeTheBar() {
        let wide = MeterColumns(scale: 2)

        #expect(wide.label == 96 && wide.value == 158)
        #expect(MeterColumns(scale: 6).value == 0)
    }

    @Test func aScaleBelowOneKeepsTheHandoffWidths() {
        #expect(MeterColumns(scale: 0.5) == MeterColumns(scale: 1))
    }

    @Test func textSizeSurvivesTheSettingsFile() throws {
        var settings = TraySettings()
        settings.textSize = .larger

        let data = try JSONEncoder().encode(settings)
        let text = try #require(String(data: data, encoding: .utf8))

        #expect(text.contains("\"text_size\":\"larger\""))
        #expect(try JSONDecoder().decode(TraySettings.self, from: data).textSize == .larger)
    }

    @Test func aMissingOrUnknownTextSizeIsTheDefault() throws {
        let missing = Data("{}".utf8)
        let unknown = Data(#"{"text_size": "enormous"}"#.utf8)

        #expect(try JSONDecoder().decode(TraySettings.self, from: missing).textSize == .standard)
        #expect(try JSONDecoder().decode(TraySettings.self, from: unknown).textSize == .standard)
    }
}

@Suite struct SpokenRowTests {
    @Test func aMeterReadsItsValueThenItsTooltip() {
        #expect(meter(tooltip: "10 of 16 cores reserved").spokenValue
            == "64%, 10 of 16 cores reserved")
        #expect(meter(tooltip: "").spokenValue == "64%")
    }

    @Test func aDriveReadsItsSpaceRatesAndMountPoint() {
        let drive = DriveRow(
            id: "/Volumes/Backup", label: "Backup", used: 0.5, activity: "R 42 · W 8 MB/s",
            value: "612 / 953 GB")
        let quiet = DriveRow(
            id: "/", label: "Macintosh HD", used: 0.5, activity: "", value: "1.21 / 1.82 TB")

        #expect(drive.spokenValue == "612 / 953 GB, R 42 · W 8 MB/s, /Volumes/Backup")
        #expect(quiet.spokenValue == "1.21 / 1.82 TB, /")
    }

    private func meter(tooltip: String) -> MeterRow {
        MeterRow(
            id: "cpu", label: "CPU", used: 0.64, shared: nil, reserved: nil, headroom: 0.85,
            value: "64%", tooltip: tooltip, spark: [])
    }
}

@MainActor
@Suite struct PanelAnnouncementTests {
    private func openModel() throws -> (model: PanelModel, spoken: Spoken) {
        let model = PanelModel()
        let state = try Fixtures.state("macos")
        model.apply(PollResult(status: .running, state: state), lastKnown: state)
        let spoken = Spoken()
        model.onAnnounce = { spoken.texts.append($0) }
        model.setPanelOpen(true)
        return (model, spoken)
    }

    @Test func whatThePanelOpensOnIsNotAnnounced() throws {
        let (model, spoken) = try openModel()

        model.refresh()

        #expect(spoken.texts.isEmpty)
    }

    @Test func aChangedSummaryIsAnnouncedOnce() throws {
        let (model, spoken) = try openModel()

        model.apply(PollResult(status: .stopped, detail: "refused"), lastKnown: nil)
        model.refresh()

        #expect(spoken.texts == ["Service stopped"])
    }

    @Test func aClosedPanelSaysNothing() throws {
        let (model, spoken) = try openModel()
        model.setPanelOpen(false)

        model.apply(PollResult(status: .stopped, detail: "refused"), lastKnown: nil)

        #expect(spoken.texts.isEmpty)
    }

    @Test func openingAgainStartsFromWhatIsShownThen() throws {
        let (model, spoken) = try openModel()
        model.setPanelOpen(false)
        model.apply(PollResult(status: .stopped, detail: "refused"), lastKnown: nil)

        model.setPanelOpen(true)
        model.refresh()

        #expect(spoken.texts.isEmpty)
    }

    @Test func cancellingOneConfirmLeavesTheOthers() throws {
        let (model, _) = try openModel()
        let state = try Fixtures.state("macos")
        let lease = try #require(state.active.first?.lease.id)

        model.pressKill(lease)
        #expect(model.isConfirming)
        model.cancelConfirm(lease + 1)
        #expect(model.isConfirming)
        model.cancelConfirm(lease)
        #expect(!model.isConfirming)
    }

    @MainActor
    private final class Spoken {
        var texts: [String] = []
    }
}
