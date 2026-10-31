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

    @Test(arguments: [
        (0, "0:00"), (-5, "0:00"), (186, "3:06"), (3599, "59:59"), (3725, "1:02:05"),
    ] as [(Int64, String)])
    func durationReadsMinutesThenHours(seconds: Int64, expected: String) {
        #expect(Format.duration(seconds) == expected)
    }

    @Test(arguments: [
        (0, "1 min"), (29, "1 min"), (90, "2 min"), (130, "2 min"), (3600, "1 h"),
        (4800, "1 h 20 min"),
    ] as [(UInt64, String)])
    func minutesRoundHalvesAwayFromZero(seconds: UInt64, expected: String) {
        #expect(Format.minutes(seconds) == expected)
    }

    @Test(arguments: [
        (59, "just now"), (60, "1 min ago"), (3599, "59 min ago"), (7200, "2 h ago"),
        (259_200, "3 d ago"),
    ] as [(Int64, String)])
    func agoNamesTheLargestUnit(seconds: Int64, expected: String) {
        #expect(Format.ago(seconds) == expected)
    }

    @Test(arguments: [
        ("cargo", "CA"), ("/opt/homebrew/bin/python3.13", "PY"), ("cargo.exe", "CA"),
        ("\"node\"", "NO"), (#"C:\tools\go.exe"#, "GO"), ("x", "X"), ("--", ""), ("", ""),
    ])
    func initialsAreTheFirstTwoLettersOrDigits(program: String, expected: String) {
        #expect(Format.initials(program) == expected)
    }

    @Test(arguments: [
        (1, "1 core"), (8, "8 cores"), (2.5, "2.5 cores"), (0.25, "0.3 cores"),
    ] as [(Float, String)])
    func coresDropAZeroDecimal(cores: Float, expected: String) {
        #expect(Format.cores(cores) == expected)
    }

    @Test(arguments: [
        (#"{"cores": 8, "ram_mb": 4096, "gpu_pct": 0, "vram_mb": 0, "est_secs": 1}"#,
         "8 cores · 4 GB"),
        (#"{"cores": 2, "ram_mb": 4096, "gpu_pct": 80, "vram_mb": 4096, "est_secs": 1}"#,
         "2 cores · 4 GB · GPU 80% · 4 GB VRAM"),
        (#"{"cores": 1, "ram_mb": 1536, "gpu_pct": 0, "vram_mb": 0, "est_secs": 1}"#,
         "1 core · 1.5 GB"),
        (#"{"cores": 4, "ram_mb": 0, "gpu_pct": 0, "vram_mb": 0, "est_secs": 1}"#, "4 cores"),
    ])
    func reservationShowsOnlyWhatIsReserved(json: String, expected: String) throws {
        let profile = try JSONDecoder().decode(JobProfile.self, from: Data(json.utf8))

        #expect(Format.reservation(profile) == expected)
    }
}
