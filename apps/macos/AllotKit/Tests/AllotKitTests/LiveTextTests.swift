import AllotContract
import Foundation
import Testing

@testable import AllotKit

// The cases are contract/text-fixtures/live-text.json, which the GNOME and Windows tests run
// too, so every port reads the same value as the same text.
@Suite struct LiveTextTests {
    private typealias Object = [String: Any]

    private static func cases() throws -> Object {
        let data = try Fixtures.textData("live-text.json")
        return try #require(try JSONSerialization.jsonObject(with: data) as? Object)
    }

    private static func entries(_ cases: Object, _ name: String) throws -> [Object] {
        try #require(cases[name] as? [Object])
    }

    private static func numbers(_ cases: Object) throws -> @Sendable (Double) -> String {
        let locale = try #require(cases["locale"] as? String)
        return SettingsText.numbers(Locale(identifier: locale))
    }

    @Test func numbersRoundHalfAwayFromZeroLikeTheOtherPanels() throws {
        let cases = try Self.cases()
        let number = try Self.numbers(cases)

        for entry in try Self.entries(cases, "numbers") {
            let value = try #require(entry["value"] as? Double)
            #expect(number(value) == entry["text"] as? String)
        }
    }

    @Test func headroomReadsMemoryInGigabytesRoundedHalfAwayFromZero() throws {
        let cases = try Self.cases()
        let number = try Self.numbers(cases)

        for entry in try Self.entries(cases, "headroom") {
            let kept = try #require(entry["kept"] as? Object)
            let decoded = try JSONDecoder().decode(
                KeptFree.self, from: JSONSerialization.data(withJSONObject: kept))
            #expect(LiveText.headroom(decoded, number: number) == entry["text"] as? String)
        }
    }

    @Test func estimatesRoundTheReservedCoresHalfAwayFromZero() throws {
        let cases = try Self.cases()
        let number = try Self.numbers(cases)

        for entry in try Self.entries(cases, "estimates") {
            let margin = try #require(entry["margin_pct"] as? Double)
            let text = LiveText.estimates(
                lower: true, minRuns: 3, marginPct: margin, number: number)
            #expect(text == entry["text"] as? String)
        }
    }

    @Test func theFitsCaptionRoundsTheBusyShareHalfAwayFromZero() throws {
        let cases = try Self.cases()
        let number = try Self.numbers(cases)

        for entry in try Self.entries(cases, "fits_caption") {
            let busy = try #require(entry["cpu_busy_pct"] as? Double)
            #expect(LiveText.fitsCaption(busy, number: number) == entry["text"] as? String)
        }
    }
}
