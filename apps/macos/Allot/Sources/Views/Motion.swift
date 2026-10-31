import SwiftUI

/// Lays out its content at its natural size but reports only `progress` of its height, top
/// aligned, so a row can grow from or collapse to nothing. As a `Layout`, its progress animates
/// frame by frame, and the stack around it moves with it.
private struct RevealLayout: Layout {
    var progress: CGFloat

    var animatableData: CGFloat {
        get { progress }
        set { progress = newValue }
    }

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        guard let content = subviews.first else {
            return .zero
        }
        let size = content.sizeThatFits(ProposedViewSize(width: proposal.width, height: nil))
        return CGSize(width: size.width, height: size.height * progress)
    }

    func placeSubviews(
        in bounds: CGRect,
        proposal: ProposedViewSize,
        subviews: Subviews,
        cache: inout ()
    ) {
        subviews.first?.place(
            at: bounds.origin, anchor: .topLeading,
            proposal: ProposedViewSize(width: bounds.width, height: nil))
    }
}

private struct Reveal: ViewModifier {
    let progress: CGFloat

    func body(content: Content) -> some View {
        RevealLayout(progress: progress) {
            content
        }
        .opacity(progress)
        .clipped()
    }
}

extension AnyTransition {
    /// A row entering or leaving by its height and opacity, as the Windows flyout's rows do.
    static var reveal: AnyTransition {
        .modifier(active: Reveal(progress: 0), identity: Reveal(progress: 1))
    }

    /// ``reveal``, but entering only after the rows leaving at the same time have gone, as the
    /// last job card gives way to Recently finished.
    static var revealAfterLeaving: AnyTransition {
        .asymmetric(
            insertion: .reveal.animation(Tokens.cardMotion.delay(Tokens.cardDuration)),
            removal: .reveal)
    }
}

extension View {
    /// Shows `progress` of this view's height, from the top and clipped, keeping it laid out
    /// even at zero: animating `progress` folds it open or shut without inserting or removing
    /// anything, so nothing can draw outside its container mid-way.
    func folded(_ progress: CGFloat) -> some View {
        RevealLayout(progress: progress) {
            self
        }
        .clipped()
    }

    /// A list row: the 6 pt gap above it belongs to the row, so it collapses with it.
    func listRow(_ transition: AnyTransition = .reveal) -> some View {
        padding(.top, Tokens.rowGap)
            .transition(transition)
    }
}
