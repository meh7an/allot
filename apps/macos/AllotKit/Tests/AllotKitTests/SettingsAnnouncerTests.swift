import AllotContract
import Foundation
import Testing

@testable import AllotKit

// What the Settings window speaks to screen readers by itself: errors and outcomes (M7.1).

private let english = SettingsText.numbers(Locale(identifier: "en_US"))

@MainActor
private final class Spoken {
    var lines: [String] = []
}

@MainActor
private func loadedSession() async throws -> (
    session: SettingsSession, client: FakeSettings, timers: FakeTimers
) {
    let client = FakeSettings(state: try fixtureState())
    let timers = FakeTimers()
    let session = SettingsSession(
        client: client, timers: timers, platform: .macos, readStopRecord: { nil })
    try await session.refresh()
    return (session, client, timers)
}

@MainActor
private func view(_ session: SettingsSession, page: String) -> AnnouncerView {
    AnnouncerView.of(
        session,
        banners: SettingsBanners.banners(session, pageId: page, port: 47823, number: english),
        number: english)
}

@Suite struct SettingsAnnouncerTests {
    @Test func theFirstViewOnlySetsWhatIsOnScreen() {
        let announcer = SettingsAnnouncer()
        let stopped = AnnouncerView(banners: ["The service is stopped."])

        #expect(announcer.update(stopped).isEmpty)
        #expect(announcer.update(stopped).isEmpty)
    }

    @Test func aBannerIsSpokenOnceWhenItAppears() {
        let announcer = SettingsAnnouncer()
        _ = announcer.update(AnnouncerView())

        #expect(announcer.update(AnnouncerView(banners: ["A", "B"])) == "A B")
        #expect(announcer.update(AnnouncerView(banners: ["B"])).isEmpty)
        #expect(announcer.update(AnnouncerView(banners: ["A", "B"])) == "A")
    }

    @Test func anotherPagesBannersAreReadWithThePageNotSpoken() {
        let announcer = SettingsAnnouncer()
        _ = announcer.update(AnnouncerView(banners: ["A"]))

        announcer.pageShown()

        #expect(
            announcer.update(AnnouncerView(banners: ["A", "Restart the service."])).isEmpty)
    }

    @Test func anErrorIsSpokenWhenItAppearsOrItsTextChangesNotWhileItStays() {
        let announcer = SettingsAnnouncer()
        let range = AnnouncedIssue(
            key: "a", text: "Headroom: Enter a number at least 0 and below 100.")
        let blank = AnnouncedIssue(key: "a", text: "Headroom: Enter a value.")

        #expect(announcer.update(AnnouncerView(issues: [range])) == range.text)
        #expect(announcer.update(AnnouncerView(issues: [range])).isEmpty)
        #expect(announcer.update(AnnouncerView(issues: [blank])) == blank.text)
        #expect(announcer.update(AnnouncerView(issues: [])).isEmpty)
        #expect(announcer.update(AnnouncerView(issues: [blank])) == blank.text)
    }

    @Test func aSaveThatEndsWithNothingUnsavedSaysSoOnce() {
        let announcer = SettingsAnnouncer()
        _ = announcer.update(AnnouncerView())

        #expect(announcer.update(AnnouncerView(save: .saving)).isEmpty)
        #expect(
            announcer.update(AnnouncerView(save: .saved)) == SettingsAnnouncer.savedAnnouncement)
        #expect(announcer.update(AnnouncerView(save: .saved)).isEmpty)
    }

    @Test func aSaveThatEndsInAProblemSpeaksTheProblemNotSaved() {
        let announcer = SettingsAnnouncer()
        _ = announcer.update(AnnouncerView(save: .saving))

        #expect(announcer.update(AnnouncerView(save: .unsaved(count: 1))).isEmpty)

        _ = announcer.update(AnnouncerView(save: .saving))
        #expect(
            announcer.update(
                AnnouncerView(
                    banners: ["config.toml changed outside Settings."], save: .saved))
                == "config.toml changed outside Settings.")
    }
}

@MainActor
@Suite struct SettingsAnnouncerSessionTests {
    @Test func anOutOfRangeValueIsSpokenWithItsLabelThenTheSave() async throws {
        let (session, _, timers) = try await loadedSession()
        let announcer = SettingsAnnouncer()
        _ = announcer.update(view(session, page: "admission"))
        let spoken = Spoken()
        session.onChange = {
            let text = announcer.update(view(session, page: "admission"))
            if !text.isEmpty {
                spoken.lines.append(text)
            }
        }
        let headroom = try #require(session.setting("admission.headroom_pct") as? NumberModel)

        headroom.commit(150)
        session.onChange?()
        headroom.commit(20)
        await settle()
        timers.fireAll()
        await settle()

        #expect(
            spoken.lines == [
                "\(headroom.label): Enter a number at least 0 and below 100.",
                SettingsAnnouncer.savedAnnouncement,
            ])
    }

    @Test func theServiceGoingAwaySpeaksItsBanner() async throws {
        let (session, client, _) = try await loadedSession()
        let announcer = SettingsAnnouncer()
        _ = announcer.update(view(session, page: "admission"))

        client.queueRead(failure(.unreachable))
        try await session.refresh()

        #expect(
            announcer.update(view(session, page: "admission"))
                == "The service isn't answering on port 47823. Trying again every 5 s.")
    }
}
