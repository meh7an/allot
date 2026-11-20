import AllotKit
import SwiftUI

/// A page of allotd's settings, built from the spec once `GET /settings` has answered: its
/// groups, each ending in its live sentence, then an Advanced section with the rarely changed
/// keys (DESIGN.md D11), then Reset. A size table or the rule list is a group's section of its
/// own. The Service page adds its status row above the groups and Files after them.
struct AllotdPage: View {
    let model: SettingsWindowModel
    let pageId: String

    var body: some View {
        let session = model.session
        let page = session.pages.first { $0.spec.id == pageId }
        VStack(alignment: .leading, spacing: 20) {
            if pageId == SettingsBanners.servicePage {
                ServiceStatusSection(model: model)
            }
            if let page {
                if let intro = page.spec.intro, !intro.isEmpty {
                    Text(intro)
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
                ForEach(page.groups, id: \.spec.id) { group in
                    let normal = group.settings.filter { !$0.spec.advanced }
                    if let table = group.settings.first as? SizeTableModel {
                        SizeTableSection(window: model, table: table, title: group.spec.label)
                    } else if let rules = group.settings.first as? RuleListModel {
                        RuleListSections(window: model, rules: rules, title: group.spec.label)
                    } else if !normal.isEmpty {
                        section(group.spec.label, normal, live: live(group.spec.id))
                    }
                    if pageId == "admission" && group.spec.id == "learn" {
                        WhatFitsSection(model: model)
                    }
                }
                if pageId == SettingsBanners.servicePage {
                    FilesSection(data: model.data)
                }
                let advanced = page.groups.filter { group in
                    group.settings.contains { $0.spec.advanced }
                }
                if !advanced.isEmpty {
                    Text("Advanced")
                        .font(.title3.weight(.semibold))
                        .accessibilityAddTraits(.isHeader)
                    ForEach(advanced, id: \.spec.id) { group in
                        let rare = group.settings.filter { $0.spec.advanced }
                        // The live sentence goes with the group's last part.
                        let allRare = rare.count == group.settings.count
                        section(group.spec.label, rare, live: allRare ? live(group.spec.id) : nil)
                    }
                }
                ResetSection(items: resetItems(page))
            } else if session.status == .loading {
                Text(SettingsText.loading)
                    .foregroundStyle(.secondary)
            }
        }
    }

    private func section(_ title: String, _ settings: [ValueModel], live: String?) -> some View {
        SettingsSection(title: title, live: live) {
            ForEach(settings.indices, id: \.self) { index in
                if index > 0 {
                    Divider()
                }
                SettingRows(setting: settings[index], window: model)
            }
        }
    }

    // The live sentence of a group, from the values on screen or the fit preview.
    private func live(_ groupId: String) -> String? {
        let number = model.number
        switch (pageId, groupId) {
        case ("admission", "keep_free"):
            if let unavailable = model.fits.unavailable {
                return unavailable
            }
            return model.fits.latest.map { LiveText.headroom($0.keptFree, number: number) }
                ?? FitPreviews.notYet
        case ("admission", "learn"):
            let lower = (model.session.setting("estimates.lower") as? SwitchModel)?.isOn ?? true
            return LiveText.estimates(
                lower: lower, minRuns: value("estimates.min_runs"),
                marginPct: value("estimates.margin_pct"), number: number)
        case ("benchmarks", "quiet"):
            return LiveText.benchmarks(
                cpuPct: value("benchmark.quiet_cpu_pct"),
                gpuPct: value("benchmark.quiet_gpu_pct"),
                quietSecs: value("benchmark.quiet_secs"),
                maxWaitSecs: value("benchmark.max_quiet_wait_secs"), number: number)
        case ("leases", "history"):
            return LiveText.history(
                leaseDays: value("retention.lease_days"),
                historyDays: value("retention.history_days"),
                keepRuns: value("retention.keep_runs_per_job"),
                maxRuns: value("retention.max_job_runs"), number: number)
        default:
            return nil
        }
    }

    // The value on screen; a field with an issue still shows the user's number.
    private func value(_ key: String) -> Double {
        (model.session.setting(key) as? NumberModel)?.number ?? 0
    }

    private func resetItems(_ page: PageModel) -> [ResetSection.Item] {
        let session = model.session
        let changedHere = session.readOnly ? [] : session.changedOn(pageId)
        var items = [
            ResetSection.Item(title: "Reset Page…", isEnabled: !changedHere.isEmpty) {
                Task { await resetPage(page.spec.label) }
            },
        ]
        if pageId == SettingsBanners.servicePage {
            let changedAll = session.readOnly ? [] : session.changedSettings()
            items.append(
                ResetSection.Item(
                    title: "Reset All Service Settings…", isEnabled: !changedAll.isEmpty
                ) {
                    Task { await resetAll() }
                })
        }
        return items
    }

    private func resetPage(_ label: String) async {
        let changed = model.session.changedOn(pageId)
        guard !changed.isEmpty else {
            return
        }
        let confirmation = SettingsText.resetPage(label, changed: changed, number: model.number)
        if await model.confirm(confirmation) {
            model.session.resetChanged(pageId)
        }
    }

    private func resetAll() async {
        let changed = model.session.changedSettings()
        guard !changed.isEmpty else {
            return
        }
        if await model.confirm(SettingsText.resetAll(changed.count)) {
            model.session.resetChanged(nil)
        }
    }
}

/// What fits on this machine (DESIGN.md section 4, Admission): one row per class from allotd's
/// fit preview for the values on screen.
private struct WhatFitsSection: View {
    let model: SettingsWindowModel

    var body: some View {
        let fits = model.fits
        SettingsSection(title: "What fits on this machine", live: caption) {
            if fits.unavailable == nil, let preview = fits.latest {
                Grid(alignment: .leading, horizontalSpacing: 24, verticalSpacing: 6) {
                    GridRow {
                        Text("Class")
                        Text("Now")
                        Text("When idle")
                    }
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    ForEach(preview.classes, id: \.name) { fit in
                        let minCores = Double(fit.minCores)
                        GridRow {
                            Text(label(fit.name))
                            Text(LiveText.fits(fit.now, minCores: minCores, number: model.number))
                            Text(LiveText.fits(fit.idle, minCores: minCores, number: model.number))
                        }
                    }
                }
                .padding(.vertical, 6)
            }
        }
    }

    private var caption: String {
        let fits = model.fits
        if let unavailable = fits.unavailable {
            return unavailable
        }
        guard let preview = fits.latest else {
            return FitPreviews.notYet
        }
        return LiveText.fitsCaption(Double(preview.cpuBusyPct), number: model.number)
    }

    private func label(_ name: String) -> String {
        let classes = model.session.setting("classes") as? SizeTableModel
        return classes?.rows.first { $0.row.name == name }?.label ?? name
    }
}
