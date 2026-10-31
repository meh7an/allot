import Foundation
import Testing

@testable import AllotKit

@Suite struct DaemonConfigTests {
    @Test(arguments: [
        ("[daemon]\nport = 50000\n", 50000),
        ("[daemon]\nport = 47_900 # moved\n", 47900),
        ("daemon.port = 51000\n", 51000),
        ("[daemon]\r\nport = 52000\r\n", 52000),
        ("[admission]\nport = 50000\n", 47823),
        ("[daemon]\nport = 0\n", 47823),
        ("[daemon]\nport = 70000\n", 47823),
        ("[daemon]\nport = \"50000\"\n", 47823),
        ("", 47823),
    ])
    func portFromToml(toml: String, expected: UInt16) {
        #expect(DaemonConfig.port(from: toml) == expected)
    }

    @Test func missingConfigGivesTheDefaultPort() throws {
        let home = try TemporaryHome()

        #expect(DaemonConfig.readPort(home.data) == DaemonConfig.defaultPort)
    }

    @Test func configFileIsReadFromTheDataDirectory() throws {
        let home = try TemporaryHome()
        try home.write("[daemon]\nport = 48000\n", to: home.data.configFile)

        #expect(DaemonConfig.readPort(home.data) == 48000)
    }

    @Test func dataDirectoryFollowsHome() {
        let data = DataDirectory.fromEnvironment(["HOME": "/Users/user"])

        #expect(data.root.path == "/Users/user/.allot")
        #expect(data.tokenFile.path == "/Users/user/.allot/token")
        #expect(data.configFile.path == "/Users/user/.allot/config.toml")
        #expect(data.logDirectory.path == "/Users/user/.allot/logs")
        #expect(data.settingsFile.path == "/Users/user/.allot/tray.json")
    }

    @Test func emptyHomeFallsBackToTheAccountHome() {
        let data = DataDirectory.fromEnvironment(["HOME": ""])

        #expect(data.root.path == NSHomeDirectory() + "/.allot")
    }
}
