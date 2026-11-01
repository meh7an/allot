import AllotKit
import Observation
import SwiftUI

/// What the panel's controls do; the panel controller supplies them.
@MainActor
struct PanelActions {
    let toggleStayOpen: () -> Void
    let togglePinned: () -> Void
    let openLogs: () -> Void
    let quit: () -> Void
    let startService: () -> Void
    let setLaunchAtLogin: (Bool) -> Void
    let openLoginItems: () -> Void

    /// System Settings › Notifications › Allot, while notifications are off there.
    let openNotificationSettings: () -> Void

    /// Debug builds: the handoff scenarios More options lists, and showing one.
    let scenarios: [String]
    let showScenario: (String) -> Void

    /// Debug builds: posts one of each notification; `nil` where the app posts none.
    let testNotifications: (() -> Void)?

    /// The header's or the body's natural height changed; the window follows their sum.
    let headerHeightChanged: (CGFloat) -> Void
    let bodyHeightChanged: (CGFloat) -> Void
}

/// The list's visible area: all of it but the bottom `depth` under the Resources card, plus the
/// two corners its rounded top leaves open (`inset` in from the sides, `radius` wide).
private struct NotchClip: Shape {
    // A cubic's control distance for a quarter circle.
    private static let kappa: CGFloat = 0.5523

    let depth: CGFloat
    let inset: CGFloat
    let radius: CGFloat

    func path(in rect: CGRect) -> Path {
        var path = Path(CGRect(
            x: rect.minX, y: rect.minY, width: rect.width, height: rect.height - depth))
        guard depth > 0 else {
            return path
        }
        let top = rect.maxY - depth
        let control = radius * (1 - Self.kappa)
        // Each notch runs from the card's corner point along its edge and back over the arc.
        let left = rect.minX + inset
        path.move(to: CGPoint(x: left, y: top))
        path.addLine(to: CGPoint(x: left + radius, y: top))
        path.addCurve(
            to: CGPoint(x: left, y: top + radius),
            control1: CGPoint(x: left + control, y: top),
            control2: CGPoint(x: left, y: top + control))
        path.closeSubpath()
        let right = rect.maxX - inset
        path.move(to: CGPoint(x: right, y: top))
        path.addLine(to: CGPoint(x: right, y: top + radius))
        path.addCurve(
            to: CGPoint(x: right - radius, y: top),
            control1: CGPoint(x: right, y: top + control),
            control2: CGPoint(x: right - control, y: top))
        path.closeSubpath()
        return path
    }
}

/// The list above the docked Resources card: the card (second subview) at its own height, which
/// changes frame by frame while it expands or collapses, and the list (first subview) at its
/// natural height, cut to what the panel's maximum leaves above the card, running `overlap`
/// under the card. The card draws over the list.
private struct DockLayout: Layout {
    let listHeight: CGFloat
    let maxHeight: CGFloat
    let overlap: CGFloat

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let width = proposal.width ?? Tokens.panelWidth
        let (list, card) = heights(width: width, subviews)
        return CGSize(width: width, height: list + card)
    }

    func placeSubviews(
        in bounds: CGRect,
        proposal: ProposedViewSize,
        subviews: Subviews,
        cache: inout ()
    ) {
        guard subviews.count == 2 else {
            return
        }
        let (list, card) = heights(width: bounds.width, subviews)
        subviews[0].place(
            at: bounds.origin, anchor: .topLeading,
            proposal: ProposedViewSize(width: bounds.width, height: list + overlap))
        subviews[1].place(
            at: CGPoint(x: bounds.minX, y: bounds.minY + list), anchor: .topLeading,
            proposal: ProposedViewSize(width: bounds.width, height: card))
    }

    private func heights(width: CGFloat, _ subviews: Subviews) -> (list: CGFloat, card: CGFloat) {
        guard subviews.count == 2 else {
            return (0, 0)
        }
        let card = subviews[1].sizeThatFits(ProposedViewSize(width: width, height: nil)).height
        return (min(listHeight, max(0, maxHeight - card)), card)
    }
}

/// The tallest the panel's body may be: the panel's maximum on its screen, set by the
/// controller before each open, less the header.
@MainActor
@Observable
final class PanelLayout {
    var maxHeight = Tokens.panelMaxHeight
    var headerHeight: CGFloat = 0

    var bodyMaxHeight: CGFloat {
        max(0, maxHeight - headerHeight)
    }
}

/// The header row on its own, hosted apart from the body and pinned to the window's top, so
/// nothing the body does while it animates can move it.
struct PanelHeaderView: View {
    let model: PanelModel
    let actions: PanelActions

    var body: some View {
        HeaderView(model: model, actions: actions)
            .frame(width: Tokens.panelWidth)
            .fixedSize(horizontal: false, vertical: true)
            .onGeometryChange(for: CGFloat.self, of: \.size.height) {
                actions.headerHeightChanged($0)
            }
            .frame(maxHeight: .infinity, alignment: .top)
    }
}

/// The panel's body under the header: the job list, which scrolls once the panel would pass
/// its maximum height, then the docked Resources card. Its height is its content's, never the
/// window's.
struct PanelView: View {
    private static let top = "top"

    let model: PanelModel
    let layout: PanelLayout
    let actions: PanelActions

    @State private var listHeight: CGFloat = 0

    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    // The list and the docked Resources card are placed in one layout pass from the card's
    // height as it is drawn, so they never drift apart while it animates: the rows keep their
    // place from the top, and the card stays right under the list. The list runs `notch` under
    // the card, clipped to the two notches beside its rounded top corners, so scrolled rows
    // show there but never through the card.
    var body: some View {
        let notch = model.resources.isVisible ? Tokens.notchDepth : 0
        ScrollViewReader { proxy in
            DockLayout(listHeight: listHeight, maxHeight: layout.bodyMaxHeight, overlap: notch) {
                ScrollView {
                    VStack(spacing: 0) {
                        Color.clear.frame(height: 0).id(Self.top)
                        JobListView(
                            snapshot: model.snapshot, banner: model.banner,
                            actions: cardActions)
                    }
                    .onGeometryChange(for: CGFloat.self, of: \.size.height) { height in
                        follow { listHeight = height }
                    }
                }
                .scrollBounceBehavior(.basedOnSize)
                .defaultScrollAnchor(.top)
                .clipShape(NotchClip(depth: notch, inset: 8, radius: Tokens.cardRadius))
                VStack(spacing: 0) {
                    if model.resources.isVisible {
                        ResourcesCard(view: model.resources, toggle: { model.toggleResources() })
                            .padding(EdgeInsets(top: 0, leading: 8, bottom: 8, trailing: 8))
                    }
                }
            }
            .onChange(of: model.scrollTarget) { _, target in
                scroll(proxy, to: target)
            }
        }
        .frame(width: Tokens.panelWidth)
        .fixedSize(horizontal: false, vertical: true)
        .onGeometryChange(for: CGFloat.self, of: \.size.height) { actions.bodyHeightChanged($0) }
        // The window takes the final height at once while a card animates its own; centred in
        // the window, the rows would jump and slide back.
        .frame(maxHeight: .infinity, alignment: .top)
    }

    // The list's measured height arrives at its final value while its rows still animate toward
    // it, so the list's frame follows on the rows' curve.
    private func follow(_ change: () -> Void) {
        if reduceMotion {
            change()
        } else {
            withAnimation(Tokens.cardMotion, change)
        }
    }

    private var cardActions: CardActions {
        CardActions(
            kill: { _ = model.pressKill($0) },
            remove: { _ = model.pressRemove($0) },
            cancelConfirm: { model.cancelConfirm($0) },
            startService: actions.startService,
            setLaunchAtLogin: actions.setLaunchAtLogin,
            openLoginItems: actions.openLoginItems,
            openNotificationSettings: actions.openNotificationSettings,
            finishFirstRun: { model.finishFirstRun() })
    }

    // A row that is not shown (a job that has ended since) scrolls to the top instead. Laid
    // out first, since a panel opened for the target has just been sized.
    private func scroll(_ proxy: ScrollViewProxy, to target: String?) {
        guard let target else {
            return
        }
        let snapshot = model.snapshot
        let rows = snapshot.groups.flatMap { $0.jobs.map { NotificationText.jobTarget($0.id) } }
            + (snapshot.recent.isEmpty ? [] : [NotificationText.recentTarget])
        let row = rows.contains(target) ? target : Self.top
        Task { @MainActor in
            proxy.scrollTo(row, anchor: .top)
            model.scrollTarget = nil
        }
    }
}
