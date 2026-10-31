import Testing

@testable import AllotKit

@Suite struct StateClockTests {
    @Test func noLiveStateNoTime() {
        #expect(StateClock(now: TestClock().source).nowMs == nil)
    }

    @Test func timeRunsOnFromTheStateMoment() throws {
        let clock = TestClock()
        let state = StateClock(now: clock.source)
        state.received(try Handoff.state())

        clock.advance(.milliseconds(1500))

        #expect(state.nowMs == Handoff.nowMs + 1500)
    }

    @Test func lastKnownStateStopsTheClock() throws {
        let state = StateClock(now: TestClock().source)
        state.received(try Handoff.state())

        state.received(nil)

        #expect(state.nowMs == nil)
    }
}

@Suite struct ProcessIconTests {
    @Test(arguments: [
        ("/Applications/Xcode.app/Contents/Developer/usr/bin/xcodebuild",
         "/Applications/Xcode.app"),
        ("/Applications/LM Studio.app/Contents/Resources/app/.webpack/lms",
         "/Applications/LM Studio.app"),
        ("/Applications/Outer.app/Contents/Helpers/Inner.app/Contents/MacOS/Inner",
         "/Applications/Outer.app"),
        ("/opt/homebrew/bin/cargo", nil),
        ("/usr/local/bin/Tool.app", nil),
        ("cargo", nil),
        ("", nil),
    ] as [(String, String?)])
    func onlyAppBundlesGiveAnIcon(exe: String, bundle: String?) {
        #expect(ProcessIcon.bundlePath(forExe: exe) == bundle)
    }

    @Test func unknownPathHasNoIcon() {
        #expect(ProcessIcon.bundlePath(forExe: nil) == nil)
    }
}
