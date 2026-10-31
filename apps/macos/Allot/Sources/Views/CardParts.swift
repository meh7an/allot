import AllotKit
import AppKit
import SwiftUI

/// The 20 pt tile before the command: the app's icon, the initials, or a queued job's dashed
/// `#N`.
struct ProcessTile: View {
    let card: JobCard

    var body: some View {
        Group {
            if card.isQueued {
                Text(card.tile)
                    .font(Tokens.Font.initials)
                    .foregroundStyle(.secondary)
                    .frame(width: 20, height: 20)
                    .overlay(
                        RoundedRectangle(cornerRadius: 4)
                            .strokeBorder(
                                Tokens.queueTileBorder,
                                style: StrokeStyle(lineWidth: 1, dash: [3, 3])))
            } else if let icon = ProcessIcons.icon(forExe: card.exePath) {
                Image(nsImage: icon)
                    .resizable()
                    .interpolation(.high)
                    .frame(width: 16, height: 16)
                    .frame(width: 20, height: 20)
            } else {
                Text(card.tile)
                    .font(Tokens.Font.initials)
                    .frame(width: 20, height: 20)
                    .background(RoundedRectangle(cornerRadius: 4).fill(Tokens.tile))
            }
        }
        .accessibilityHidden(true)
    }
}

/// App icons by bundle path, for the app's life; failures are cached too.
@MainActor
enum ProcessIcons {
    private static var cache: [String: NSImage?] = [:]

    static func icon(forExe exe: String?) -> NSImage? {
        guard let bundle = ProcessIcon.bundlePath(forExe: exe) else {
            return nil
        }
        if let cached = cache[bundle] {
            return cached
        }
        let icon = FileManager.default.fileExists(atPath: bundle)
            ? NSWorkspace.shared.icon(forFile: bundle)
            : nil
        cache[bundle] = icon
        return icon
    }
}

/// The 32 pt control at a card's right: Kill, the red Kill? confirm, Remove, or a spacer.
struct CardActionView: View {
    let id: Int64
    let card: JobCard
    let actions: CardActions

    @State private var isHovered = false

    var body: some View {
        Group {
            switch card.action {
            case .kill:
                Button {
                    actions.kill(id)
                } label: {
                    Image(systemName: "xmark.circle")
                        .font(.system(size: 15))
                        .foregroundStyle(.secondary)
                        .frame(width: 32, height: 32)
                        .background(
                            RoundedRectangle(cornerRadius: 4)
                                .fill(isHovered ? Tokens.killHover : .clear))
                        .contentShape(Rectangle())
                }
            case .confirm:
                Button {
                    actions.kill(id)
                } label: {
                    HStack(spacing: 4) {
                        Image(systemName: "xmark.circle.fill")
                            .font(.system(size: 14))
                        Text("Kill?")
                            .font(Tokens.Font.action)
                    }
                    .foregroundStyle(.white)
                    .padding(EdgeInsets(top: 0, leading: 8, bottom: 0, trailing: 10))
                    .frame(height: 32)
                    .background(
                        RoundedRectangle(cornerRadius: 4)
                            .fill(isHovered ? Tokens.dangerFillHover : Tokens.dangerFill))
                    .contentShape(Rectangle())
                }
                .accessibilityHint("Stops the command and its child processes")
            case .remove:
                Button {
                    actions.remove(id)
                } label: {
                    Text("Remove")
                        .font(Tokens.Font.body)
                        .foregroundStyle(.secondary)
                        .padding(.horizontal, 8)
                        .frame(height: 32)
                        .background(
                            RoundedRectangle(cornerRadius: 4)
                                .fill(isHovered ? Tokens.buttonHover : .clear))
                        .contentShape(Rectangle())
                }
            case .none:
                Color.clear
                    .frame(width: 32, height: 32)
            }
        }
        .buttonStyle(.plain)
        .fixedSize()
        .help(card.actionName)
        .accessibilityLabel(card.actionName)
        .onHover { isHovered = $0 }
    }
}

/// The 2 pt bar along a card's bottom: elapsed or waited against the estimate.
struct EdgeBar: View {
    let fraction: Double
    let tone: EdgeBarTone

    var body: some View {
        GeometryReader { geometry in
            ZStack(alignment: .leading) {
                Tokens.track
                switch tone {
                case .waitingDashed:
                    Dashes(colour: Tokens.waiting)
                case .none:
                    EmptyView()
                default:
                    colour
                        .frame(width: geometry.size.width * fraction)
                        .animation(Tokens.barMotion, value: fraction)
                }
            }
        }
        .frame(height: 2)
        .opacity(tone == .none ? 0 : 1)
        .accessibilityHidden(true)
    }

    private var colour: Color {
        switch tone {
        case .running: Tokens.running
        case .over, .waiting, .waitingDashed: Tokens.waiting
        case .frozen, .none: Tokens.frozen
        }
    }
}

/// A wait with no estimate: 6 pt of amber, then 6 pt of amber at 18%, across the card.
private struct Dashes: View {
    let colour: Color

    var body: some View {
        Canvas { context, size in
            context.fill(
                Path(CGRect(origin: .zero, size: size)), with: .color(colour.opacity(0.18)))
            var x: CGFloat = 0
            while x < size.width {
                context.fill(
                    Path(CGRect(x: x, y: 0, width: 6, height: size.height)),
                    with: .color(colour))
                x += 12
            }
        }
    }
}

/// A benchmark's Drain / Quiet / Run stepper: three segments of 1 / 1.4 / 1 with 6 pt
/// between them, each a 3 pt bar above its label; Quiet has a tick per second of quiet.
struct BenchmarkStepper: View {
    let steps: BenchmarkSteps

    var body: some View {
        GeometryReader { geometry in
            let unit = (geometry.size.width - 12) / 3.4
            HStack(alignment: .top, spacing: 6) {
                segment(.drain, "Drain", width: unit) {
                    bar(fill(for: .drain), width: unit)
                }
                segment(.quiet, "Quiet", width: unit * 1.4) {
                    ticks(width: unit * 1.4)
                }
                segment(.run, "Run", width: unit) {
                    ZStack(alignment: .leading) {
                        bar(fill(for: .run), width: unit)
                        if steps.current == .run {
                            bar(Tokens.benchmark, width: unit * steps.runFraction)
                                .animation(Tokens.stepMotion, value: steps.runFraction)
                        }
                    }
                }
            }
        }
        .frame(height: 3 + 2 + 14)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(accessibilityText)
    }

    private func segment<Bar: View>(
        _ step: BenchmarkStep,
        _ label: String,
        width: CGFloat,
        @ViewBuilder bar: () -> Bar
    ) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            bar()
            Text(label)
                .font(Tokens.Font.small)
                .foregroundStyle(labelColour(step))
        }
        .frame(width: width, alignment: .leading)
    }

    private func bar(_ colour: Color, width: CGFloat) -> some View {
        RoundedRectangle(cornerRadius: 1)
            .fill(colour)
            .frame(width: max(0, width), height: 3)
    }

    private func ticks(width: CGFloat) -> some View {
        let count = BenchmarkSteps.tickCount
        let tick = (width - CGFloat(count - 1) * 2) / CGFloat(count)
        return HStack(spacing: 2) {
            ForEach(0..<count, id: \.self) { index in
                bar(tickColour(index), width: tick)
            }
        }
    }

    // Active: solid purple; completed: purple at 35%; upcoming: the track.
    private func fill(for step: BenchmarkStep) -> Color {
        if steps.current == .done || step < steps.current {
            return Tokens.benchmark.opacity(0.35)
        }
        return step == steps.current && step != .run ? Tokens.benchmark : Tokens.stepUpcoming
    }

    private func tickColour(_ index: Int) -> Color {
        switch steps.current {
        case .drain: Tokens.stepUpcoming
        case .quiet: index < steps.quietTicks ? Tokens.benchmark : Tokens.stepUpcoming
        case .run, .done: Tokens.benchmark.opacity(0.35)
        }
    }

    private func labelColour(_ step: BenchmarkStep) -> Color {
        if step == steps.current {
            return .primary
        }
        return step < steps.current ? .secondary : Tokens.subtle
    }

    private var accessibilityText: String {
        switch steps.current {
        case .drain: "Step 1 of 3, Drain"
        case .quiet: "Step 2 of 3, Quiet, \(steps.quietTicks) of \(BenchmarkSteps.tickCount) s"
        case .run: "Step 3 of 3, Run"
        case .done: "All steps done"
        }
    }
}
