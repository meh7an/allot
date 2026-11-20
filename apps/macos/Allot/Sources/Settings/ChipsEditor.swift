import AllotKit
import SwiftUI

/// A string list as chips with remove buttons, then a box and an Add button (DESIGN.md section 3,
/// String list): the print-only flags, and a rule's arguments in its sheet. Each add or remove
/// goes to the model at once; the row around it shows the model's issue.
struct ChipsEditor: View {
    let model: StringListModel

    /// The box's accessible name, such as "New print-only flag".
    let entryName: String

    /// An example entry.
    let placeholder: String

    /// The Add button, such as "Add Flag".
    let addLabel: String

    @State private var entry = ""
    @FocusState private var entryFocused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            let entries = model.entries
            if !entries.isEmpty {
                FlowLayout(spacing: 6) {
                    ForEach(Array(entries.enumerated()), id: \.offset) { index, text in
                        Chip(text: text) {
                            model.remove(at: index)
                        }
                    }
                }
                .accessibilityElement(children: .contain)
                .accessibilityLabel(model.label)
            }
            HStack(spacing: 8) {
                TextField(entryName, text: $entry, prompt: Text(placeholder))
                    .labelsHidden()
                    .font(.body.monospaced())
                    .autocorrectionDisabled()
                    .frame(width: 180)
                    .focused($entryFocused)
                    .onSubmit(add)
                Button(addLabel, action: add)
            }
        }
    }

    private func add() {
        if model.add(entry) {
            entry = ""
        }
        entryFocused = true
    }
}

/// One entry of a list with its remove button.
private struct Chip: View {
    let text: String
    let remove: @MainActor () -> Void

    var body: some View {
        HStack(spacing: 2) {
            Text(text)
                .font(.body.monospaced())
            Button {
                remove()
            } label: {
                Image(systemName: "xmark.circle.fill")
                    .foregroundStyle(.secondary)
            }
            .buttonStyle(.borderless)
            .help("Remove \(text)")
            .accessibilityLabel("Remove \(text)")
        }
        .padding(EdgeInsets(top: 3, leading: 8, bottom: 3, trailing: 4))
        .background(.quaternary, in: Capsule())
    }
}

/// Lays its views out in rows, left to right, starting a new row when one is full.
struct FlowLayout: Layout {
    var spacing: CGFloat = 6

    func sizeThatFits(
        proposal: ProposedViewSize,
        subviews: Subviews,
        cache: inout ()
    ) -> CGSize {
        let frames = arrange(subviews, width: proposal.width ?? .infinity)
        let width = frames.map(\.maxX).max() ?? 0
        let height = frames.map(\.maxY).max() ?? 0
        return CGSize(width: width, height: height)
    }

    func placeSubviews(
        in bounds: CGRect,
        proposal: ProposedViewSize,
        subviews: Subviews,
        cache: inout ()
    ) {
        let frames = arrange(subviews, width: bounds.width)
        for (subview, frame) in zip(subviews, frames) {
            subview.place(
                at: CGPoint(x: bounds.minX + frame.minX, y: bounds.minY + frame.minY),
                proposal: ProposedViewSize(frame.size))
        }
    }

    // Each view at its ideal size; a view wider than a row gets a row of its own.
    private func arrange(_ subviews: Subviews, width: CGFloat) -> [CGRect] {
        var frames: [CGRect] = []
        var x: CGFloat = 0
        var y: CGFloat = 0
        var rowHeight: CGFloat = 0
        for subview in subviews {
            let size = subview.sizeThatFits(.unspecified)
            if x > 0 && x + size.width > width {
                x = 0
                y += rowHeight + spacing
                rowHeight = 0
            }
            frames.append(CGRect(origin: CGPoint(x: x, y: y), size: size))
            x += size.width + spacing
            rowHeight = max(rowHeight, size.height)
        }
        return frames
    }
}
