import AllotContract
import Foundation
import Testing

@testable import AllotKit

@Suite struct FormatTests {
    @Test(arguments: [
        ("3f2a9c1e-77b0-4f6e-a2d8-5c9e1b4f7a02", "3f2a9c1e"),
        ("short", "short"),
        ("", ""),
    ])
    func shortIdTakesEightCharacters(session: String, expected: String) {
        #expect(Format.shortId(session) == expected)
    }

    @Test(arguments: [
        ("/Users/user/work/allot", "allot"),
        ("/Users/user/work/allot/", "allot"),
        ("E:\\work\\app", "app"),
        ("E:\\work\\app\\", "app"),
        ("/", ""),
        ("", ""),
    ])
    func projectIsTheLastFolder(cwd: String, expected: String) {
        #expect(Format.project(cwd) == expected)
    }

    @Test func projectOfNilIsEmpty() {
        #expect(Format.project(nil) == "")
    }

    @Test(arguments: [
        (7.75, 2970, "7.8 cores · 2.9 GB"),
        (2.25, 1024, "2.3 cores · 1.0 GB"),
        (0.0, 0, "0.0 cores · 0.0 GB"),
        (16.0, 32768, "16.0 cores · 32.0 GB"),
    ])
    func usageRoundsHalvesAwayFromZero(cores: Double, ramMb: Int, expected: String) throws {
        let json = #"{"cores": \#(cores), "ram_mb": \#(ramMb), "gpu_pct": 0, "vram_mb": 0}"#
        let usage = try JSONDecoder().decode(ResourceUsage.self, from: Data(json.utf8))

        #expect(Format.usage(usage) == expected)
    }
}
