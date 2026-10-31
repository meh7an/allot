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

    /// Debug builds: the handoff scenarios More options lists, and showing one.
    let scenarios: [String]
    let showScenario: (String) -> Void

    /// Debug builds: posts one of each notification; `nil` where the app posts none.
    let testNotifications: (() -> Void)?

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
/// its maximum height, then the docked Resources card. Its height is its content's, never the
/// window's.
struct PanelView: View {
    private static let top = "top"

    let model: PanelModel
    let layout: PanelLayout
    let actions: PanelActions

    @State private var headerHeight: CGFloat = 0
    @State private var listHeight: CGFloat = 0
    @State private var resourcesHeight: CGFloat = 0

    var body: some View {
        VStack(spacing: 0) {
            HeaderView(model: model, actions: actions)
                .onGeometryChange(for: CGFloat.self, of: \.size.height) { headerHeight = $0 }
            ScrollViewReader { proxy in
                ScrollView {
                    VStack(spacing: 0) {
                        Color.clear.frame(height: 0).id(Self.top)
                        JobListView(
                            snapshot: model.snapshot, banner: model.banner,
                            actions: cardActions)
                    }
                    .onGeometryChange(for: CGFloat.self, of: \.size.height) { listHeight = $0 }
                }
                .scrollBounceBehavior(.basedOnSize)
                .frame(height: min(
                    listHeight, max(0, layout.maxHeight - headerHeight - resourcesHeight)))
                .onChange(of: model.scrollTarget) { _, target in
                    scroll(proxy, to: target)
                }
            }
            VStack(spacing: 0) {
                if model.resources.isVisible {
                    ResourcesCard(view: model.resources, toggle: { model.toggleResources() })
                        .padding(EdgeInsets(top: 0, leading: 8, bottom: 8, trailing: 8))
                }
            }
            .onGeometryChange(for: CGFloat.self, of: \.size.height) { resourcesHeight = $0 }
        }
        .frame(width: Tokens.panelWidth)
        .fixedSize(horizontal: false, vertical: true)
        .onGeometryChange(for: CGFloat.self, of: \.size.height) { actions.heightChanged($0) }
    }

    private var cardActions: CardActions {
        CardActions(
            kill: { _ = model.pressKill($0) },
            remove: { _ = model.pressRemove($0) },
            startService: actions.startService,
            setLaunchAtLogin: actions.setLaunchAtLogin,
            openLoginItems: actions.openLoginItems,
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
