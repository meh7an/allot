import AllotContract
import AllotKit
import SwiftUI

/// The Classes or Presets table of the Sizes page (DESIGN.md D5): read-only rows to compare, each
/// with Edit… and a reset or delete button; the Fits now column follows allotd's fit preview. A
/// preset table ends in Add Preset…. Editing happens in a sheet, whose Save writes the whole row.
struct SizeTableSection: View {
    private static let headers = [
        "Cores", "Minimum cores", "Memory", "GPU", "Video memory", "Run time", "Fits now",
    ]

    let window: SettingsWindowModel
    let table: SizeTableModel
    let title: String

    @State private var sheet: SizeSheet?

    var body: some View {
        let editable = !window.session.readOnly
        SettingsSection(title: title, help: table.spec.help) {
            ScrollView(.horizontal) {
                Grid(alignment: .leading, horizontalSpacing: 14, verticalSpacing: 8) {
                    // Each row reads as one element with a summary, so the headers add nothing
                    // for a screen reader.
                    GridRow {
                        Text(table.canAdd ? "Preset" : "Class")
                            .accessibilityHidden(true)
                        ForEach(Self.headers, id: \.self) { header in
                            Text(header)
                                .accessibilityHidden(true)
                        }
                        Color.clear
                            .gridCellUnsizedAxes([.horizontal, .vertical])
                            .accessibilityHidden(true)
                    }
                    .font(SettingsFont.callout)
                    .foregroundStyle(.secondary)
                    ForEach(table.rows, id: \.row.name) { view in
                        row(view, editable: editable)
                    }
                }
                .fixedSize()
                .padding(.vertical, 6)
            }
            .accessibilityLabel(table.label)
            if table.canAdd {
                Button("Add Preset…") {
                    open(nil)
                }
                .disabled(!editable)
                .padding(.vertical, 6)
            }
        }
        .sheet(item: $sheet) { shown in
            SizeSheetView(
                window: window, table: table, draft: shown.draft, offerReset: shown.offerReset
            ) { outcome in
                sheet = nil
                if outcome == .reset, let name = shown.draft.original, let view = current(name) {
                    remove(view)
                }
            }
        }
    }

    private func row(_ view: SizeRowView, editable: Bool) -> some View {
        let number = window.number
        let cells = SizesText.cells(view.row, number: number)
        let fit = window.fits.fit(table.key, view.row.name)
        let fits = SizesText.fitsNow(fit, number: number)
        let summary = SizesText.rowSummary(table, view, fit: fit, number: number)
        return GridRow {
            // The name cell speaks the whole row; the value cells and their sentence parts
            // are hidden, and the buttons stay separate and named.
            nameCell(view)
                .accessibilityElement(children: .ignore)
                .accessibilityLabel(summary)
            hiddenCell(cells.cores)
            hiddenCell(cells.minCores)
            hiddenCell(cells.memory)
            hiddenCell(cells.gpu)
            hiddenCell(cells.videoMemory)
            hiddenCell(cells.runTime)
            hiddenCell(fits)
            HStack(spacing: 4) {
                Button("Edit…") {
                    open(view.row.name)
                }
                .accessibilityLabel(SizesText.editName(table, view))
                if view.builtIn {
                    iconButton("arrow.counterclockwise", SizesText.resetName(table, view)) {
                        remove(view)
                    }
                    .opacity(view.changed ? 1 : 0)
                    .disabled(!view.changed)
                    .accessibilityHidden(!view.changed)
                } else {
                    iconButton("trash", SizesText.deleteName(view)) {
                        Task { await delete(view) }
                    }
                }
            }
            .disabled(!editable)
        }
        .lineLimit(1)
    }

    private func hiddenCell(_ text: String) -> some View {
        Text(text)
            .accessibilityHidden(true)
    }

    // A class shows its label over its key; a preset its name over "built in" or "yours".
    private func nameCell(_ view: SizeRowView) -> some View {
        VStack(alignment: .leading, spacing: 1) {
            if table.canAdd {
                Text(view.row.name)
                    .font(SettingsFont.mono)
                Text(view.builtIn ? "built in" : "yours")
                    .font(SettingsFont.caption)
                    .foregroundStyle(.secondary)
            } else {
                Text(view.label)
                Text(view.row.name)
                    .font(SettingsFont.captionMono)
                    .foregroundStyle(.secondary)
            }
        }
    }

    private func iconButton(
        _ symbol: String,
        _ name: String,
        action: @escaping @MainActor () -> Void
    ) -> some View {
        Button {
            action()
        } label: {
            Image(systemName: symbol)
                .accessibilityHidden(true)
        }
        .buttonStyle(.borderless)
        .help(name)
        .accessibilityLabel(name)
    }

    private func current(_ name: String) -> SizeRowView? {
        table.rows.first { $0.row.name == name }
    }

    private func open(_ name: String?) {
        let draft: RowDraft? = if let name { table.edit(name) } else { table.add() }
        guard let draft else {
            return
        }
        let view = name.flatMap { current($0) }
        sheet = SizeSheet(draft: draft, offerReset: view.map { $0.builtIn && $0.changed } ?? false)
    }

    private func delete(_ view: SizeRowView) async {
        if await window.confirm(SizesText.deletePreset(view.row.name)) {
            remove(view)
        }
    }

    // A reset of a built-in row or the delete of the user's own, with Undo.
    private func remove(_ view: SizeRowView) {
        let old = view.row
        table.remove(old.name)
        window.showNotice(SizesText.rowRemoved(table, view)) {
            table.restore(old)
        }
    }
}

/// The edit sheet being shown.
private struct SizeSheet: Identifiable {
    let id = UUID()
    let draft: RowDraft

    /// Whether the sheet offers Reset to Built-in Size: a built-in row that was changed.
    let offerReset: Bool
}

/// How an edit sheet ended.
private enum SizeSheetOutcome {
    /// Cancelled.
    case closed

    /// The row was handed over to be written.
    case saved

    /// The user asked to reset the row to its built-in size.
    case reset
}

/// The edit sheet of a class or preset, and Add Preset (DESIGN.md section 4, Sizes): the name for
/// a preset the user adds or renames, every size field, the live line from allotd's fit preview
/// for the values in the sheet, and Save, which writes the whole row at once.
private struct SizeSheetView: View {
    let window: SettingsWindowModel
    let table: SizeTableModel
    let draft: RowDraft
    let offerReset: Bool
    let done: @MainActor (SizeSheetOutcome) -> Void

    @State private var name = ""
    @State private var live = ""
    @FocusState private var nameFocused: Bool

    var body: some View {
        let proposal = table.proposal(draft)
        VStack(alignment: .leading, spacing: 12) {
            Text(SizesText.dialogTitle(table, draft))
                .font(SettingsFont.headline)
                .accessibilityAddTraits(.isHeader)
            VStack(alignment: .leading, spacing: 0) {
                if draft.canRename, let spec = draft.nameSpec {
                    SettingRow(label: spec.label, help: spec.help, notes: notes(draft.nameIssue)) {
                        TextField(spec.label, text: $name)
                            .labelsHidden()
                            .font(SettingsFont.mono)
                            .autocorrectionDisabled()
                            .frame(width: 200 * SettingsFont.scale)
                            .rowHint()
                            .focused($nameFocused)
                            .onChange(of: name) {
                                draft.setName(name)
                            }
                    }
                    Divider()
                }
                ForEach(draft.fields.indices, id: \.self) { index in
                    if index > 0 {
                        Divider()
                    }
                    fieldRow(draft.fields[index], first: index == 0)
                }
            }
            if let warning = SizesText.renameWarning(draft) {
                HStack(alignment: .firstTextBaseline, spacing: 4) {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .accessibilityHidden(true)
                    Text(warning)
                        .fixedSize(horizontal: false, vertical: true)
                }
                .font(SettingsFont.callout)
                .accessibilityElement(children: .ignore)
                .accessibilityLabel("Warning: \(warning)")
            }
            Text(live)
                .font(SettingsFont.callout)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            HStack {
                if offerReset {
                    Button("Reset to Built-in Size") {
                        done(.reset)
                    }
                }
                Spacer()
                Button("Cancel", role: .cancel) {
                    done(.closed)
                }
                .keyboardShortcut(.cancelAction)
                Button("Save") {
                    if table.save(draft) {
                        done(.saved)
                    }
                }
                .buttonStyle(.borderedProminent)
            }
        }
        .padding(20)
        .font(SettingsFont.body)
        .frame(width: 600)
        .onAppear {
            name = draft.name
            // The first field takes focus: the name when the sheet has one, else the first size.
            if hasNameField {
                Task { @MainActor in
                    nameFocused = true
                }
            }
        }
        .task(id: proposal) {
            // Asks again once the values have been still for the save delay.
            if !live.isEmpty {
                try? await Task.sleep(for: SettingsSession.saveDelay)
            }
            guard !Task.isCancelled else {
                return
            }
            let line = await liveLine(proposal)
            if !Task.isCancelled {
                live = line
            }
        }
    }

    @ViewBuilder private func fieldRow(_ field: NumberModel, first: Bool) -> some View {
        if field.key == "min_cores" {
            SettingRow(label: field.label, help: field.spec.help, notes: notes(field.issue)) {
                MinCoresEditor(field: field, automaticCores: automaticCores, number: window.number)
            }
        } else {
            SettingRow(label: field.label, help: field.spec.help, notes: notes(field.issue)) {
                NumberEditor(model: field, autofocus: first && !hasNameField)
            }
        }
    }

    private var hasNameField: Bool {
        draft.canRename && draft.nameSpec != nil
    }

    private var automaticCores: Double {
        SizesText.automaticMinCores(draft.field("cores")?.number ?? 0)
    }

    private func notes(_ issue: ValueIssue?) -> [RowNote] {
        issue.map { [.issue(issueText($0, number: window.number))] } ?? []
    }

    private func liveLine(_ proposal: [Change]?) async -> String {
        guard window.session.status == .ready else {
            return LiveText.serviceStopped
        }
        guard let proposal, case .value(let preview)? = try? await window.session.preview(proposal)
        else {
            return ""
        }
        let fits = table.key == "classes" ? preview.classes : preview.presets
        let fit = fits.first { $0.name == draft.name }
        return fit.map { SizesText.dialogFits($0, number: window.number) } ?? ""
    }
}

/// Minimum cores: Automatic ("half the cores, at least 1") or Custom with its own number.
private struct MinCoresEditor: View {
    let field: NumberModel
    let automaticCores: Double
    let number: (Double) -> String

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Picker(field.label, selection: automatic) {
                Text("Automatic (\(number(automaticCores)): \(field.control.automatic ?? ""))")
                    .tag(true)
                Text("Custom")
                    .tag(false)
            }
            .pickerStyle(.radioGroup)
            .labelsHidden()
            NumberEditor(model: field)
                .padding(.leading, 20)
                .disabled(field.automatic)
        }
    }

    private var automatic: Binding<Bool> {
        Binding(
            get: { field.automatic },
            set: { isAutomatic in
                if isAutomatic != field.automatic {
                    field.setAutomatic(isAutomatic, value: automaticCores)
                }
            })
    }
}
