import AllotKit
import SwiftUI

/// What a card's button does; the panel model and the app supply it.
@MainActor
struct CardActions {
    let kill: (Int64) -> Void
    let remove: (Int64) -> Void
    let startService: () -> Void
    let setLaunchAtLogin: (Bool) -> Void
    let openLoginItems: () -> Void
    let finishFirstRun: () -> Void
}

/// The service-down banner, the groups with one card per job, then the idle line, the
/// first-run card, and Recently finished while nothing runs or waits. Cards carry the row keys
/// a notification's Show scrolls to.
struct JobListView: View {
    let snapshot: PanelSnapshot
    let banner: ServiceBanner?
    let actions: CardActions

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            if let banner {
                ServiceBannerView(banner: banner, startService: actions.startService)
            }
            ForEach(snapshot.groups) { group in
                GroupHeading(group: group)
                    .opacity(dimming)
                ForEach(group.jobs) { job in
                    JobCardView(job: job, actions: actions)
                        .opacity(dimming)
                        .id(NotificationText.jobTarget(job.id))
                        .transition(.opacity)
                }
            }
            if let idle = snapshot.idle {
                IdleLineView(idle: idle)
                    .opacity(dimming)
            }
            if let welcome = snapshot.welcome {
                WelcomeCardView(
                    card: welcome, setLaunchAtLogin: actions.setLaunchAtLogin,
                    openLoginItems: actions.openLoginItems, done: actions.finishFirstRun)
            }
            if !snapshot.recent.isEmpty {
                RecentView(lines: snapshot.recent)
                    .id(NotificationText.recentTarget)
            }
            if snapshot.showNothing {
                Text("No jobs running or waiting.")
                    .font(Tokens.Font.body)
                    .foregroundStyle(.secondary)
                    .padding(EdgeInsets(top: 4, leading: 6, bottom: 2, trailing: 6))
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(EdgeInsets(top: 0, leading: 8, bottom: 8, trailing: 8))
        .animation(Tokens.cardMotion, value: snapshot.groups.map { $0.jobs.map(\.id) })
    }

    // Last known jobs show dimmed under the banner, which keeps its full colour.
    private var dimming: Double {
        snapshot.isLastKnown ? Tokens.lastKnownOpacity : 1
    }
}

private struct GroupHeading: View {
    let group: JobGroup

    var body: some View {
        HStack(spacing: 6) {
            Image(systemName: symbol)
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(colour)
            Text(group.heading)
                .font(Tokens.Font.heading)
                .monospacedDigit()
                .foregroundStyle(.secondary)
        }
        .padding(EdgeInsets(top: 6, leading: 6, bottom: 0, trailing: 6))
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isHeader)
    }

    private var symbol: String {
        switch group.kind {
        case .benchmark: "timer"
        case .running: "play.circle.fill"
        case .waiting: "clock"
        }
    }

    private var colour: Color {
        switch group.kind {
        case .benchmark: Tokens.benchmark
        case .running: Tokens.running
        case .waiting: Tokens.waiting
        }
    }
}

/// The shared card frame: background, 1 pt border and radius 6.
private struct CardFrame: ViewModifier {
    func body(content: Content) -> some View {
        content
            .background(Tokens.card)
            .clipShape(RoundedRectangle(cornerRadius: Tokens.cardRadius))
            .overlay(
                RoundedRectangle(cornerRadius: Tokens.cardRadius)
                    .strokeBorder(Tokens.cardBorder, lineWidth: 1))
    }
}

extension View {
    func cardFrame() -> some View {
        modifier(CardFrame())
    }
}

private struct SessionStripView: View {
    let strip: SessionStrip

    var body: some View {
        HStack(spacing: 8) {
            Text(strip.shortId)
                .font(Tokens.Font.id)
            Text(strip.project)
                .font(Tokens.Font.body)
                .foregroundStyle(.secondary)
                .lineLimit(1)
                .truncationMode(.tail)
                .help(strip.projectPath)
                .frame(maxWidth: .infinity, alignment: .leading)
            Text(strip.usage)
                .font(Tokens.Font.body)
                .monospacedDigit()
                .foregroundStyle(.secondary)
                .lineLimit(1)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 5)
        .background(Tokens.strip)
        .overlay(alignment: .bottom) {
            Rectangle()
                .fill(Tokens.cardBorder)
                .frame(height: 1)
        }
    }
}

private struct IdleLineView: View {
    let idle: IdleLine

    var body: some View {
        HStack(spacing: 8) {
            Text(idle.label)
                .font(Tokens.Font.body)
                .monospacedDigit()
            Text(idle.ids)
                .font(Tokens.Font.idInLine)
                .lineLimit(1)
                .truncationMode(.tail)
            if !idle.usage.isEmpty {
                Text(idle.usage)
                    .font(Tokens.Font.body)
                    .monospacedDigit()
            }
        }
        .foregroundStyle(.secondary)
        .padding(EdgeInsets(top: 4, leading: 6, bottom: 2, trailing: 6))
        .accessibilityElement(children: .combine)
    }
}

/// The Recently finished heading and one card of 36 pt rows.
private struct RecentView: View {
    let lines: [RecentLine]

    var body: some View {
        HStack(spacing: 6) {
            Image(systemName: "checkmark.circle")
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(Tokens.running)
            Text("Recently finished")
                .font(Tokens.Font.heading)
                .foregroundStyle(.secondary)
        }
        .padding(EdgeInsets(top: 6, leading: 6, bottom: 0, trailing: 6))
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isHeader)
        VStack(spacing: 0) {
            ForEach(lines) { line in
                HStack(spacing: 8) {
                    Text(line.command)
                        .font(Tokens.Font.command)
                        .lineLimit(1)
                        .truncationMode(.tail)
                        .help(line.command)
                        .frame(maxWidth: .infinity, alignment: .leading)
                    Text(line.shortId)
                        .font(Tokens.Font.idInLine)
                        .foregroundStyle(.secondary)
                    Text(line.meta)
                        .font(Tokens.Font.body)
                        .monospacedDigit()
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }
                .padding(.horizontal, 12)
                .frame(height: 36)
                .overlay(alignment: .top) {
                    if line.id > 0 {
                        Rectangle()
                            .fill(Tokens.separator)
                            .frame(height: 1)
                    }
                }
                .accessibilityElement(children: .combine)
            }
        }
        .cardFrame()
    }
}

/// One job: the session strip, the row with its tile, command, chip and action, the detail
/// line, a benchmark's stepper, and the edge bar along the bottom.
private struct JobCardView: View {
    let job: JobRow
    let actions: CardActions

    var body: some View {
        let card = job.card
        VStack(alignment: .leading, spacing: 0) {
            SessionStripView(strip: job.session)
            VStack(alignment: .leading, spacing: 0) {
                HStack(spacing: 8) {
                    ProcessTile(card: card)
                        .opacity(card.isEnded ? Tokens.endedOpacity : 1)
                    Text(job.command)
                        .font(Tokens.Font.command)
                        .strikethrough(card.isStruck)
                        .lineLimit(1)
                        .truncationMode(.tail)
                        .help(job.command)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .opacity(card.isEnded ? Tokens.endedOpacity : 1)
                    ClassChip(card: card)
                        .opacity(card.isEnded ? Tokens.endedOpacity : 1)
                    CardActionView(id: job.id, card: card, actions: actions)
                }
                .frame(minHeight: 32)
                DetailLine(card: card)
                    .padding(EdgeInsets(top: 0, leading: 28, bottom: 0, trailing: 8))
                if let steps = card.steps {
                    BenchmarkStepper(steps: steps)
                        .padding(EdgeInsets(top: 8, leading: 28, bottom: 0, trailing: 8))
                }
            }
            .padding(EdgeInsets(top: 4, leading: 12, bottom: 10, trailing: 4))
        }
        .overlay(alignment: .bottom) {
            EdgeBar(fraction: card.barFraction, tone: card.barTone)
        }
        .cardFrame()
        .accessibilityElement(children: .contain)
        .onChange(of: card.announcement) { _, announcement in
            if !announcement.isEmpty {
                AccessibilityNotification.Announcement(announcement).post()
            }
        }
    }
}

private struct ClassChip: View {
    let card: JobCard

    var body: some View {
        HStack(spacing: 3) {
            if card.isBenchmark {
                Image(systemName: "timer")
                    .font(.system(size: 10, weight: .semibold))
            }
            Text(card.chip)
                .font(Tokens.Font.small)
        }
        .foregroundStyle(card.isBenchmark ? Tokens.onBenchmark : Tokens.chipText)
        .padding(EdgeInsets(top: 0, leading: card.isBenchmark ? 5 : 8, bottom: 0, trailing: 8))
        .frame(height: 18)
        .background(
            Capsule().fill(card.isBenchmark ? Tokens.benchmark : Tokens.chip))
        .fixedSize()
        .help(card.chipTooltip)
    }
}

/// The detail line: the semibold start, the detail and the time; a benchmark's line has a
/// leading icon and wraps.
private struct DetailLine: View {
    let card: JobCard

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 6) {
            switch card.icon {
            case .timer:
                Image(systemName: "timer")
                    .font(.system(size: 12))
                    .foregroundStyle(Tokens.benchmark)
                    .accessibilityHidden(true)
            case .check:
                Image(systemName: "checkmark.circle.fill")
                    .font(.system(size: 12))
                    .foregroundStyle(Tokens.running)
                    .accessibilityHidden(true)
            case .none:
                EmptyView()
            }
            Text("\(strong)\(card.detail)")
                .foregroundStyle(card.detailIsDanger ? Tokens.danger : Color.secondary)
                .lineLimit(card.icon == .none ? 1 : nil)
                .truncationMode(.tail)
                .fixedSize(horizontal: false, vertical: card.icon != .none)
                .help(card.detailTooltip)
                .frame(maxWidth: .infinity, alignment: .leading)
            if !card.time.isEmpty {
                Text(card.time)
                    .monospacedDigit()
                    .foregroundStyle(card.timeIsWarning ? Tokens.waiting : Color.secondary)
                    .lineLimit(1)
                    .fixedSize()
            }
        }
        .font(Tokens.Font.body)
    }

    private var strong: Text {
        Text(card.strong)
            .fontWeight(.semibold)
            .foregroundStyle(.primary)
    }
}
