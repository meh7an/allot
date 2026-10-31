import AllotKit
import SwiftUI

/// The red banner while the service is down, with Start Service, and the "Last known" caption
/// over the dimmed cards.
struct ServiceBannerView: View {
    let banner: ServiceBanner
    let startService: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(alignment: .center, spacing: 8) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(banner.text)
                        .font(Tokens.Font.action)
                    if !banner.problem.isEmpty {
                        Text(banner.problem)
                            .font(Tokens.Font.small)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }
                .foregroundStyle(Tokens.bannerText)
                .frame(maxWidth: .infinity, alignment: .leading)
                switch banner.button {
                case .start:
                    Button("Start Service", action: startService)
                        .buttonStyle(.borderedProminent)
                        .controlSize(.regular)
                case .starting:
                    Button("Starting…") {}
                        .buttonStyle(.borderedProminent)
                        .controlSize(.regular)
                        .disabled(true)
                case .none:
                    EmptyView()
                }
            }
            .padding(EdgeInsets(top: 6, leading: 10, bottom: 6, trailing: 6))
            .background(RoundedRectangle(cornerRadius: 6).fill(Tokens.bannerFill))
            .overlay(
                RoundedRectangle(cornerRadius: 6).strokeBorder(Tokens.bannerBorder, lineWidth: 1))
            .accessibilityElement(children: .contain)
            if !banner.caption.isEmpty {
                Text(banner.caption)
                    .font(Tokens.Font.small)
                    .foregroundStyle(.secondary)
                    .padding(.horizontal, 6)
            }
        }
    }
}

/// The first-run card: the welcome text, Launch at login and Done.
struct WelcomeCardView: View {
    let card: WelcomeCard
    let setLaunchAtLogin: (Bool) -> Void
    let openLoginItems: () -> Void
    let done: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(WelcomeCard.title)
                .font(Tokens.Font.summary)
                .accessibilityAddTraits(.isHeader)
            Text(WelcomeCard.body)
                .font(Tokens.Font.body)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            VStack(alignment: .leading, spacing: 2) {
                Toggle(isOn: Binding(
                    get: { card.login.isOn },
                    set: { setLaunchAtLogin($0) })
                ) {
                    Text("Launch at login")
                        .font(Tokens.Font.body)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                .toggleStyle(.switch)
                .controlSize(.small)
                .disabled(!card.login.canToggle)
                if card.login == .needsApproval {
                    HStack(spacing: 4) {
                        Text(card.login.hint)
                        Button("Open Login Items", action: openLoginItems)
                            .buttonStyle(.link)
                    }
                    .font(Tokens.Font.small)
                    .foregroundStyle(.secondary)
                } else if !card.login.hint.isEmpty {
                    Text(card.login.hint)
                        .font(Tokens.Font.small)
                        .foregroundStyle(.secondary)
                }
            }
            .frame(minHeight: 40)
            .overlay(alignment: .top) {
                Rectangle().fill(Tokens.separator).frame(height: 1)
            }
            HStack {
                Spacer()
                Button("Done", action: done)
                    .controlSize(.regular)
            }
        }
        .padding(12)
        .cardFrame()
        .accessibilityElement(children: .contain)
    }
}
