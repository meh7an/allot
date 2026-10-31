import AllotKit
import SwiftUI

/// The docked Resources card: a header that toggles between the summary and the meter rows,
/// then the drives and the legend.
struct ResourcesCard: View {
    let view: ResourcesView
    let toggle: () -> Void

    var body: some View {
        VStack(spacing: 0) {
            Button(action: toggle) {
                HStack(spacing: 8) {
                    Text("Resources")
                        .font(Tokens.Font.heading)
                    Text(view.summary)
                        .font(Tokens.Font.body)
                        .monospacedDigit()
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                        .truncationMode(.tail)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .opacity(view.isExpanded ? 0 : 1)
                    Image(systemName: "chevron.down")
                        .font(.system(size: 11, weight: .semibold))
                        .rotationEffect(.degrees(view.isExpanded ? 180 : 0))
                        .frame(width: 32, height: 32)
                }
                .padding(.leading, 12)
                .padding(.trailing, 4)
                .frame(height: 40)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .help(view.isExpanded ? "Hide resources" : "Show resources")
            .accessibilityLabel("Resources")
            .accessibilityValue(view.isExpanded ? "Expanded" : "Collapsed")
            .accessibilityHint(view.summary)

            if view.isExpanded {
                ResourceRows(view: view)
                    .transition(.opacity.combined(with: .move(edge: .bottom)))
            }
        }
        .clipped()
        .cardFrame()
        .animation(Tokens.cardMotion, value: view.isExpanded)
    }
}

private struct ResourceRows: View {
    let view: ResourcesView

    var body: some View {
        VStack(spacing: 0) {
            if view.allOff {
                Text("All meters are off. Turn them on from More options › Meters.")
                    .font(Tokens.Font.body)
                    .foregroundStyle(.secondary)
                    .padding(EdgeInsets(top: 2, leading: 12, bottom: 10, trailing: 12))
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            ForEach(view.meters) { row in
                MeterRowView(row: row)
            }
            if view.showsSeparator {
                Rectangle()
                    .fill(Tokens.separator)
                    .frame(height: 1)
                    .padding(EdgeInsets(top: 4, leading: 12, bottom: 4, trailing: 12))
            }
            ForEach(view.drives) { drive in
                DriveRowView(drive: drive)
            }
            if view.showsLegend || view.showsSharedLegend {
                Legend(reserved: view.showsLegend, shared: view.showsSharedLegend)
                    .padding(EdgeInsets(top: 4, leading: 12, bottom: 8, trailing: 12))
            } else {
                Spacer().frame(height: 6)
            }
        }
    }
}

/// The row grid of the handoff: a 48 pt label, the bar, and a 108 pt value.
private struct RowGrid<Middle: View, Value: View>: View {
    let label: String
    let middle: Middle
    let value: Value

    init(label: String, @ViewBuilder middle: () -> Middle, @ViewBuilder value: () -> Value) {
        self.label = label
        self.middle = middle()
        self.value = value()
    }

    var body: some View {
        HStack(spacing: 8) {
            Text(label)
                .font(Tokens.Font.heading)
                .lineLimit(1)
                .truncationMode(.tail)
                .frame(width: 48, alignment: .leading)
            middle
                .frame(maxWidth: .infinity)
            value
                .frame(width: 108, alignment: .trailing)
        }
        .padding(.horizontal, 12)
        .frame(height: 28)
    }
}

private struct MeterRowView: View {
    let row: MeterRow

    var body: some View {
        RowGrid(label: row.label) {
            BulletBar(row: row)
                .help(row.tooltip)
        } value: {
            HStack(spacing: 6) {
                if !row.spark.isEmpty {
                    Sparkline(samples: row.spark)
                        .frame(width: 36, height: 14)
                        .accessibilityHidden(true)
                }
                Text(row.value)
                    .font(Tokens.Font.body)
                    .monospacedDigit()
                    .lineLimit(1)
            }
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(row.label)
        .accessibilityValue("\(row.value), \(row.tooltip)")
    }
}

private struct DriveRowView: View {
    let drive: DriveRow

    var body: some View {
        RowGrid(label: drive.label) {
            HStack(spacing: 8) {
                GeometryReader { proxy in
                    ZStack(alignment: .leading) {
                        RoundedRectangle(cornerRadius: 2).fill(Tokens.track)
                        RoundedRectangle(cornerRadius: 2)
                            .fill(Tokens.driveBar)
                            .frame(width: proxy.size.width * drive.used)
                    }
                }
                .frame(height: 4)
                if !drive.activity.isEmpty {
                    Text(drive.activity)
                        .font(Tokens.Font.small)
                        .monospacedDigit()
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                        .fixedSize()
                }
            }
        } value: {
            Text(drive.value)
                .font(Tokens.Font.body)
                .monospacedDigit()
                .lineLimit(1)
        }
        .help(drive.id)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(drive.label)
        .accessibilityValue([drive.value, drive.activity].filter { !$0.isEmpty }
            .joined(separator: ", "))
    }
}

/// The 6 pt bullet bar: the track, hatched headroom from `headroom` on, the used fill (its GPU
/// share lighter, at its end), and the 2 by 14 pt reserved tick reaching above and below.
private struct BulletBar: View {
    private static let barHeight: CGFloat = 6
    private static let tickSize = CGSize(width: 2, height: 14)

    let row: MeterRow

    var body: some View {
        Canvas { context, size in
            let bar = CGRect(
                x: 0, y: (size.height - Self.barHeight) / 2, width: size.width,
                height: Self.barHeight)
            let barShape = Path(roundedRect: bar, cornerRadius: 2)
            context.drawLayer { layer in
                layer.clip(to: barShape)
                layer.fill(Path(bar), with: .color(Tokens.track))
                drawHatch(in: layer, bar: bar)
                let used = bar.width * row.used
                let shared = bar.width * (row.shared ?? 0)
                layer.fill(
                    Path(CGRect(
                        x: bar.minX, y: bar.minY, width: used - shared, height: bar.height)),
                    with: .color(.accentColor))
                layer.fill(
                    Path(CGRect(
                        x: bar.minX + used - shared, y: bar.minY, width: shared,
                        height: bar.height)),
                    with: .color(.accentColor.opacity(Tokens.sharedOpacity)))
            }
            if let reserved = row.reserved {
                let x = min(max(bar.width * reserved - Self.tickSize.width / 2, 0),
                            size.width - Self.tickSize.width)
                let tick = CGRect(
                    x: x, y: (size.height - Self.tickSize.height) / 2,
                    width: Self.tickSize.width, height: Self.tickSize.height)
                context.fill(Path(tick.insetBy(dx: -1, dy: -1)), with: .color(Tokens.tickRing))
                context.fill(Path(tick), with: .color(.primary))
            }
        }
        .frame(height: Self.tickSize.height + 2)
    }

    // 1.5 pt diagonal lines every 4·√2 pt, as the handoff's 135° repeating gradient.
    private func drawHatch(in context: GraphicsContext, bar: CGRect) {
        let start = bar.width * row.headroom
        guard start < bar.width else {
            return
        }
        let hatch = CGRect(x: start, y: bar.minY, width: bar.width - start, height: bar.height)
        var layer = context
        layer.clip(to: Path(hatch))
        layer.fill(Path(hatch), with: .color(Tokens.hatchBack))
        let step = 4 * 2.0.squareRoot()
        var lines = Path()
        var x = hatch.minX - bar.height
        while x < hatch.maxX {
            lines.move(to: CGPoint(x: x, y: bar.maxY))
            lines.addLine(to: CGPoint(x: x + bar.height, y: bar.minY))
            x += step
        }
        layer.stroke(lines, with: .color(Tokens.hatchLine), lineWidth: 1.5)
    }
}

/// The CPU's last 24 samples, aligned right in a 44 by 16 view box as on the handoff.
private struct Sparkline: View {
    private static let slots = 24

    let samples: [Double]

    var body: some View {
        Canvas { context, size in
            guard samples.count > 1 else {
                return
            }
            let step = size.width / CGFloat(Self.slots - 1)
            let offset = Self.slots - samples.count
            var line = Path()
            for (index, sample) in samples.enumerated() {
                let point = CGPoint(
                    x: CGFloat(offset + index) * step,
                    y: size.height - 1 - CGFloat(sample) * (size.height - 2))
                if index == 0 {
                    line.move(to: point)
                } else {
                    line.addLine(to: point)
                }
            }
            context.stroke(
                line, with: .color(.accentColor),
                style: StrokeStyle(lineWidth: 1.4, lineCap: .round, lineJoin: .round))
        }
    }
}

private struct Legend: View {
    let reserved: Bool
    let shared: Bool

    var body: some View {
        HStack(spacing: 12) {
            item("Used") {
                RoundedRectangle(cornerRadius: 2).fill(Color.accentColor)
                    .frame(width: 10, height: 6)
            }
            if shared {
                item("GPU") {
                    RoundedRectangle(cornerRadius: 2)
                        .fill(Color.accentColor.opacity(Tokens.sharedOpacity))
                        .frame(width: 10, height: 6)
                }
            }
            if reserved {
                item("Reserved") {
                    Rectangle().fill(.primary).frame(width: 2, height: 10)
                }
            }
            item("Headroom") {
                RoundedRectangle(cornerRadius: 2).fill(Tokens.hatchLine)
                    .frame(width: 10, height: 6)
            }
            Spacer(minLength: 0)
        }
        .font(Tokens.Font.small)
        .foregroundStyle(.secondary)
        .accessibilityElement(children: .combine)
    }

    private func item(_ title: String, @ViewBuilder swatch: () -> some View) -> some View {
        HStack(spacing: 4) {
            swatch()
            Text(title)
        }
    }
}
