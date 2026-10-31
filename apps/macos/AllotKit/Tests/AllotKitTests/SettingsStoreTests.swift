import Foundation
import Testing

@testable import AllotKit

@Suite struct SettingsStoreTests {
    @Test func missingFileGivesTheDefaults() throws {
        let home = try TemporaryHome()

        let settings = SettingsStore(file: home.data.settingsFile).load()

        #expect(settings == TraySettings())
        #expect(settings.meters.cpu && settings.meters.driveActivity)
        #expect(settings.notifications.serviceStopped && !settings.notifications.jobFinished)
        #expect(!settings.resourcesExpanded && !settings.firstRunDone && !settings.firstRunShown)
    }

    @Test func windowsFileIsReadWithMissingMembersDefaulted() throws {
        let home = try TemporaryHome()
        // As the Windows app writes it, minus two members, plus comments, a null, an unknown
        // member and a trailing comma, all of which it accepts.
        try home.write(
            """
            {
              // written by allot-tray
              "meters": {"cpu": false, "drive_activity": false},
              "notifications": {"job_finished": true, "killed_outside_flyout": true},
              "resources_expanded": true,
              "first_run_done": null,
              "future_member": [1, 2],
            }
            """,
            to: home.data.settingsFile)

        let settings = SettingsStore(file: home.data.settingsFile).load()

        #expect(!settings.meters.cpu && settings.meters.ram && !settings.meters.driveActivity)
        #expect(settings.notifications.jobFinished && settings.notifications.killedOutsidePanel)
        #expect(settings.notifications.serviceStopped)
        #expect(settings.resourcesExpanded)
        #expect(!settings.firstRunDone)
    }

    @Test func corruptFileGivesTheDefaults() throws {
        let home = try TemporaryHome()
        try home.write(#"{"meters": {"cpu": "off"}}"#, to: home.data.settingsFile)

        #expect(SettingsStore(file: home.data.settingsFile).load() == TraySettings())
    }

    @Test func savedSettingsRoundTripInSnakeCase() throws {
        let home = try TemporaryHome()
        let store = SettingsStore(file: home.data.settingsFile)
        var settings = TraySettings()
        settings.meters.gpu = false
        settings.notifications.killedOutsidePanel = true
        settings.firstRunShown = true

        try store.save(settings)

        #expect(store.load() == settings)
        let text = try String(contentsOf: home.data.settingsFile, encoding: .utf8)
        #expect(text.contains(#""killed_outside_flyout" : true"#))
        #expect(text.contains(#""first_run_shown" : true"#))
        #expect(text.hasSuffix("}\n"))
    }
}
