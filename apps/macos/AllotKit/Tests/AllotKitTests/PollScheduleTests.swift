import Testing

@testable import AllotKit

@Suite struct PollScheduleTests {
    @Test func runningFollowsThePanel() {
        var schedule = PollSchedule()

        #expect(schedule.next(panelOpen: true, status: .running) == .seconds(1))
        #expect(schedule.next(panelOpen: false, status: .running) == .seconds(5))
    }

    @Test func startingPollsTwiceASecond() {
        var schedule = PollSchedule()

        #expect(schedule.next(panelOpen: false, status: .starting) == .milliseconds(500))
    }

    @Test(arguments: [ServiceStatus.stopped, .incompatible])
    func downDoublesUpToTheClosedCap(status: ServiceStatus) {
        var schedule = PollSchedule()

        let delays = (0..<7).map { _ in schedule.next(panelOpen: false, status: status) }

        #expect(delays.map { $0.components.seconds } == [1, 2, 4, 8, 16, 30, 30])
    }

    @Test func openPanelCapsTheRetryAtFiveSeconds() {
        var schedule = PollSchedule()

        let delays = (0..<4).map { _ in schedule.next(panelOpen: true, status: .stopped) }

        #expect(delays.map { $0.components.seconds } == [1, 2, 4, 5])
    }

    @Test func runningOrResetStartsTheBackoffOver() {
        var schedule = PollSchedule()
        _ = schedule.next(panelOpen: false, status: .stopped)
        _ = schedule.next(panelOpen: false, status: .stopped)

        _ = schedule.next(panelOpen: false, status: .running)
        #expect(schedule.next(panelOpen: false, status: .stopped) == .seconds(1))

        schedule.reset()
        #expect(schedule.next(panelOpen: false, status: .stopped) == .seconds(1))
    }
}
