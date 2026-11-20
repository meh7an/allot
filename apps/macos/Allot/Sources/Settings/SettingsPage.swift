import AllotKit
import SwiftUI

/// One toolbar tab of the Settings window: its banners, the page's content in a scroll view as
/// tall as the content up to a limit, the notice, and the save status in the bottom bar. Its
/// size is the window's: the window fits each page (DESIGN.md section 10, macOS).
struct SettingsPage: View {
    static let width: CGFloat = 640

    // Sizes shows every column of its tables; the window widens while it is selected.
    private static let wideWidth: CGFloat = 900

    // Beyond this the page scrolls; it keeps the window on a 13-inch screen.
    private static let maxContentHeight: CGFloat = 560

    let model: SettingsWindowModel
    let pageId: String

    @State private var contentHeight: CGFloat = 120

    var body: some View {
        let banners = SettingsBanners.banners(
            model.session, pageId: pageId, port: model.port, number: model.number)
        VStack(spacing: 0) {
            if !banners.isEmpty {
                VStack(spacing: 8) {
                    ForEach(Array(banners.enumerated()), id: \.offset) { _, banner in
                        BannerView(banner: banner, run: model.run)
                    }
                }
                .padding(EdgeInsets(top: 16, leading: 20, bottom: 0, trailing: 20))
            }
            ScrollView {
                content
                    .padding(20)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .onGeometryChange(for: CGFloat.self, of: \.size.height) {
                        contentHeight = $0
                    }
            }
            .frame(height: min(contentHeight, Self.maxContentHeight))
            .overlay(alignment: .bottom) {
                if let notice = model.notice {
                    NoticeView(notice: notice, undo: model.undoNotice)
                        .padding(.bottom, 12)
                }
            }
            Divider()
            Text(model.saveStatus)
                .font(.callout)
                .foregroundStyle(.secondary)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(EdgeInsets(top: 8, leading: 20, bottom: 8, trailing: 20))
        }
        .frame(width: pageId == "sizes" ? Self.wideWidth : Self.width)
    }

    @ViewBuilder private var content: some View {
        if pageId == SettingsBanners.panelPage {
            PanelPage(model: model)
        } else {
            AllotdPage(model: model, pageId: pageId)
        }
    }
}

/// A lasting state at the top of a page (DESIGN.md D13).
private struct BannerView: View {
    let banner: SettingsBanner
    let run: @MainActor (BannerAction) -> Void

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            Image(systemName: symbol)
                .foregroundStyle(tint)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 8) {
                Text(banner.message)
                    .fixedSize(horizontal: false, vertical: true)
                HStack(spacing: 8) {
                    ForEach(Array(banner.actions.enumerated()), id: \.offset) { index, action in
                        if index == 0 {
                            Button(action.label) { run(action) }
                                .buttonStyle(.borderedProminent)
                        } else {
                            Button(action.label) { run(action) }
                        }
                    }
                }
            }
            Spacer(minLength: 0)
        }
        .padding(12)
        .background(tint.opacity(0.12), in: RoundedRectangle(cornerRadius: 8))
        .accessibilityElement(children: .contain)
    }

    private var symbol: String {
        switch banner.severity {
        case .informational: "info.circle.fill"
        case .warning: "exclamationmark.triangle.fill"
        case .error: "xmark.octagon.fill"
        }
    }

    private var tint: Color {
        switch banner.severity {
        case .informational: .accentColor
        case .warning: .orange
        case .error: .red
        }
    }
}

/// A brief notice with Undo, closed after 10 s by the model.
private struct NoticeView: View {
    let notice: SettingsNotice
    let undo: @MainActor () -> Void

    var body: some View {
        HStack(spacing: 12) {
            Text(notice.message)
            if notice.undo != nil {
                Button("Undo") { undo() }
            }
        }
        .padding(EdgeInsets(top: 8, leading: 14, bottom: 8, trailing: 10))
        .background(.regularMaterial, in: Capsule())
        .shadow(radius: 4, y: 1)
    }
}

/// A group of rows under its heading, ending in the group's live sentence (DESIGN.md D10).
struct SettingsSection<Content: View>: View {
    let title: String
    var help = ""
    var live: String?
    @ViewBuilder let content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(title)
                .font(.headline)
                .accessibilityAddTraits(.isHeader)
            if !help.isEmpty {
                Text(help)
                    .font(.callout)
                    .foregroundStyle(.secondary)
            }
            GroupBox {
                VStack(alignment: .leading, spacing: 0) {
                    content
                    if let live, !live.isEmpty {
                        Divider()
                            .padding(.vertical, 4)
                        Text(live)
                            .font(.callout)
                            .foregroundStyle(.secondary)
                            .fixedSize(horizontal: false, vertical: true)
                            .padding(.vertical, 4)
                    }
                }
                .padding(.horizontal, 4)
            }
        }
    }
}

/// The Reset group at the end of a page: its buttons, disabled with "Nothing to reset" while
/// nothing differs from its default.
struct ResetSection: View {
    struct Item: Identifiable {
        let title: String
        let isEnabled: Bool
        let action: @MainActor () -> Void

        var id: String {
            title
        }
    }

    let items: [Item]

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("Reset")
                .font(.headline)
                .accessibilityAddTraits(.isHeader)
            HStack(spacing: 8) {
                ForEach(items) { item in
                    Button(item.title) { item.action() }
                        .disabled(!item.isEnabled)
                }
            }
            if !items.contains(where: \.isEnabled) {
                Text(SettingsText.nothingToReset)
                    .font(.callout)
                    .foregroundStyle(.secondary)
            }
        }
    }
}
