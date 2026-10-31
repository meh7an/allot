import AllotKit
import SwiftUI

/// The groups with one card per job, then the idle line. The cards are minimal until the job
/// card milestone adds the process tile, class chip, detail line, time and edge bar.
struct JobListView: View {
    let snapshot: PanelSnapshot

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            ForEach(snapshot.groups) { group in
                GroupHeading(group: group)
                ForEach(group.jobs) { job in
                    JobCardView(job: job)
                }
            }
            if let idle = snapshot.idle {
                IdleLineView(idle: idle)
            }
            if snapshot.showNothing {
                Text("No jobs running or waiting.")
                    .font(Tokens.Font.body)
                    .foregroundStyle(.secondary)
                    .padding(EdgeInsets(top: 4, leading: 6, bottom: 2, trailing: 6))
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .opacity(snapshot.isLastKnown ? Tokens.lastKnownOpacity : 1)
        .padding(EdgeInsets(top: 0, leading: 8, bottom: 8, trailing: 8))
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

private struct JobCardView: View {
    let job: JobRow

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            SessionStripView(strip: job.session)
            Text(job.command)
                .font(Tokens.Font.command)
                .lineLimit(1)
                .truncationMode(.tail)
                .help(job.command)
                .frame(maxWidth: .infinity, minHeight: 32, alignment: .leading)
                .padding(EdgeInsets(top: 2, leading: 12, bottom: 4, trailing: 8))
        }
        .background(Tokens.card)
        .clipShape(RoundedRectangle(cornerRadius: Tokens.cardRadius))
        .overlay(
            RoundedRectangle(cornerRadius: Tokens.cardRadius)
                .strokeBorder(Tokens.cardBorder, lineWidth: 1))
        .accessibilityElement(children: .combine)
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
