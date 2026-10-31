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

    /// Debug builds: the handoff scenarios More options lists, and showing one.
    let scenarios: [String]
    let showScenario: (String) -> Void

    /// The panel's natural height changed; the window follows it.
    let heightChanged: (CGFloat) -> Void
}

/// The tallest the panel may be on its screen, set by the controller before each open.
@MainActor
@Observable
final class PanelLayout {
    var maxHeight = Tokens.panelMaxHeight
}

/// The panel's content: the header, then the job list, which scrolls once the panel would pass
/// its maximum height. Its height is its content's, never the window's.
struct PanelView: View {
    let model: PanelModel
    let layout: PanelLayout
    let actions: PanelActions

    @State private var headerHeight: CGFloat = 0
    @State private var listHeight: CGFloat = 0

    var body: some View {
        VStack(spacing: 0) {
            HeaderView(model: model, actions: actions)
                .onGeometryChange(for: CGFloat.self, of: \.size.height) { headerHeight = $0 }
            ScrollView {
                JobListView(snapshot: model.snapshot, actions: cardActions)
                    .onGeometryChange(for: CGFloat.self, of: \.size.height) { listHeight = $0 }
            }
            .scrollBounceBehavior(.basedOnSize)
            .frame(height: min(listHeight, max(0, layout.maxHeight - headerHeight)))
        }
        .frame(width: Tokens.panelWidth)
        .fixedSize(horizontal: false, vertical: true)
        .onGeometryChange(for: CGFloat.self, of: \.size.height) { actions.heightChanged($0) }
    }

    private var cardActions: CardActions {
        CardActions(
            kill: { _ = model.pressKill($0) },
            remove: { _ = model.pressRemove($0) })
    }
}
