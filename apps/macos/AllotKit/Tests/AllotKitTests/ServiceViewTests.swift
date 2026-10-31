import AllotContract
import Foundation
import Testing

@testable import AllotKit

private let utc = TimeZone(identifier: "UTC")!
private let english = Locale(identifier: "en_US")

private func banner(
    _ status: ServiceStatus,
    lastKnown: UiState? = nil,
    problem: String? = nil
) -> ServiceBanner? {
    ServiceBanner.from(
        status: status, lastKnown: lastKnown, startProblem: problem, timeZone: utc,
        locale: english)
}

@Suite struct ServiceBannerTests {
    @Test func noBannerWhileRunning() throws {
        #expect(banner(.running, lastKnown: try Fixtures.state("macos")) == nil)
    }

    @Test func stoppedOffersStartWithTheLastKnownTime() throws {
        let shown = try #require(banner(.stopped, lastKnown: try Fixtures.state("macos")))

        #expect(shown.text == "Service stopped · commands run unchecked")
        #expect(shown.button == .start)
        #expect(shown.problem.isEmpty)
        // 1792824720000 ms is 2026-10-24 06:52 UTC; ICU puts a narrow space before AM.
        #expect(shown.caption.hasPrefix("Last known · 6:52"))
        #expect(shown.caption.hasSuffix("AM"))
    }

    @Test func failedStartShowsItsReason() {
        let shown = banner(.stopped, problem: "/x/allotd is missing; run allot install")

        #expect(shown?.problem == "/x/allotd is missing; run allot install")
        #expect(shown?.caption == "")
    }

    @Test func startingMutesTheButtonAndHidesTheProblem() {
        let shown = banner(.starting, problem: "old")

        #expect(shown?.button == .starting)
        #expect(shown?.problem == "")
    }

    @Test func incompatibleAsksForAnInstallWithoutAButton() {
        let shown = banner(.incompatible)

        #expect(shown?.text == "Service needs an update · run allot install")
        #expect(shown?.button == BannerButton.none)
    }
}

@Suite struct ServiceMenuTests {
    @Test func itemFollowsTheStatus() {
        #expect(ServiceMenu.item(for: .running).title == "Stop Service…")
        #expect(ServiceMenu.item(for: .incompatible).action == .stop)
        #expect(ServiceMenu.item(for: .stopped).title == "Start Service")
        #expect(!ServiceMenu.item(for: .starting).isEnabled)
    }

    @Test func stopConfirmsOnlyWithJobs() throws {
        #expect(ServiceMenu.stopConfirmation(try Fixtures.state("first-run")) == nil)
        #expect(ServiceMenu.stopConfirmation(nil) == nil)

        let text = try #require(ServiceMenu.stopConfirmation(try Fixtures.state("macos")))

        #expect(text.hasPrefix("1 job is running and 1 request is waiting.\n\n"))
    }
}

@Suite struct LoginItemStatusTests {
    @Test func translocationIsReadFromThePath() {
        let moved = "/private/var/folders/x/T/AppTranslocation/ABC/d/Allot.app"

        #expect(LoginItemStatus.isTranslocated(bundlePath: moved))
        #expect(!LoginItemStatus.isTranslocated(bundlePath: "/Applications/Allot.app"))
    }

    @Test func onlyOnAndOffCanToggle() {
        #expect(LoginItemStatus.on.canToggle)
        #expect(LoginItemStatus.off.canToggle)
        #expect(!LoginItemStatus.needsApproval.canToggle)
        #expect(!LoginItemStatus.translocated.canToggle)
        #expect(LoginItemStatus.translocated.hint ==
            "Move Allot to Applications to launch at login")
    }
}
