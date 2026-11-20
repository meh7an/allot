import AllotContract
import Foundation

/// The cells of a class or preset row in the Sizes tables, each with its unit.
public struct SizeCells: Sendable, Equatable {
    /// "8".
    public let cores: String

    /// "4", or "Automatic (4)" while the row names none.
    public let minCores: String

    /// "4 GB".
    public let memory: String

    /// "50 %".
    public let gpu: String

    /// "0 MB".
    public let videoMemory: String

    /// "5 minutes".
    public let runTime: String

    public init(
        cores: String,
        minCores: String,
        memory: String,
        gpu: String,
        videoMemory: String,
        runTime: String
    ) {
        self.cores = cores
        self.minCores = minCores
        self.memory = memory
        self.gpu = gpu
        self.videoMemory = videoMemory
        self.runTime = runTime
    }
}

/// The Sizes page's sentences: table cells, the Fits now column, the edit sheet's title and live
/// line, and the row actions' names, notices and confirmations (DESIGN.md section 4, Sizes).
public enum SizesText {
    /// The Fits now cell while no preview is known, such as while the service is stopped.
    public static let fitsUnknown = "—"

    public static let addPresetTitle = "Add preset"

    /// The fewest cores a row that names no minimum starts with: half the cores, at least one,
    /// or all of them when there are fewer (allotd's `default_min_cores`).
    public static func automaticMinCores(_ cores: Double) -> Double {
        max(cores / 2, min(cores, 1))
    }

    public static func cells(_ row: SizeRow, number: (Double) -> String) -> SizeCells {
        let profile = row.profile
        let cores = double(profile.cores)
        let minimum = row.minCores.map { number(double($0)) }
            ?? "Automatic (\(number(automaticMinCores(cores))))"
        return SizeCells(
            cores: number(cores),
            minCores: minimum,
            memory: quantityText(Double(profile.ramMb), .megabytes, number: number),
            gpu: quantityText(double(profile.gpuPct), .percent, number: number),
            videoMemory: quantityText(Double(profile.vramMb), .megabytes, number: number),
            runTime: quantityText(Double(profile.estSecs), .seconds, number: number))
    }

    /// The Fits now cell: "1, 2 more shrunk", "2 shrunk", "3", "at least 99" or "None".
    ///
    /// - Parameter fit: The row's fit preview, or `nil` while none is known.
    public static func fitsNow(_ fit: SizeFit?, number: (Double) -> String) -> String {
        guard let fits = fit?.now else {
            return fitsUnknown
        }
        if fits.full == 0 && fits.shrunk == 0 {
            return "None"
        }
        // Counts stop at allotd's limit; the larger count then reads "at least 99".
        let capped = fits.more ? "at least " : ""
        if fits.shrunk == 0 {
            return "\(capped)\(number(Double(fits.full)))"
        }
        let shrunk = "\(capped)\(number(Double(fits.shrunk)))"
        return fits.full == 0
            ? "\(shrunk) shrunk"
            : "\(number(Double(fits.full))), \(shrunk) more shrunk"
    }

    /// The edit sheet's live line: "With these values, now: 1 at full size, 2 more at 4 cores.
    /// With every lease ended: 2 at full size."
    public static func dialogFits(_ fit: SizeFit, number: (Double) -> String) -> String {
        let minCores = double(fit.minCores)
        let now = lowered(LiveText.fits(fit.now, minCores: minCores, number: number))
        let idle = lowered(LiveText.fits(fit.idle, minCores: minCores, number: number))
        return "With these values, now: \(now). With every lease ended: \(idle)."
    }

    /// The edit sheet's title: "Edit CPU heavy", "Edit preset cpu_small" or "Add preset".
    @MainActor
    public static func dialogTitle(_ table: SizeTableModel, _ draft: RowDraft) -> String {
        guard let name = draft.original else {
            return addPresetTitle
        }
        if table.canAdd {
            return "Edit preset \(name)"
        }
        return "Edit \(table.rows.first { $0.row.name == name }?.label ?? name)"
    }

    /// The sheet's warning while a preset is renamed; `nil` otherwise.
    @MainActor
    public static func renameWarning(_ draft: RowDraft) -> String? {
        guard let original = draft.original, draft.canRename, !draft.name.isEmpty,
            draft.name != original
        else {
            return nil
        }
        return "Sessions that ask for \(original) get an error until they use \(draft.name)."
    }

    /// The accessible name of a row's Edit button: "Edit CPU heavy", "Edit preset cpu_small".
    @MainActor
    public static func editName(_ table: SizeTableModel, _ row: SizeRowView) -> String {
        "Edit \(subject(table, row))"
    }

    /// The accessible name of a row's reset button: "Reset CPU heavy to its built-in size".
    @MainActor
    public static func resetName(_ table: SizeTableModel, _ row: SizeRowView) -> String {
        "Reset \(subject(table, row)) to its built-in size"
    }

    /// The accessible name of a preset's delete button: "Delete preset render".
    public static func deleteName(_ row: SizeRowView) -> String {
        "Delete preset \(row.row.name)"
    }

    /// The notice after a row's reset or a preset's delete, both with Undo.
    @MainActor
    public static func rowRemoved(_ table: SizeTableModel, _ row: SizeRowView) -> String {
        row.builtIn
            ? "\(capitalized(subject(table, row))) reset to its built-in size."
            : "Preset \(row.row.name) deleted."
    }

    public static func deletePreset(_ name: String) -> Confirmation {
        Confirmation(
            title: "Delete preset \(name)?",
            body: "Sessions that ask for it by name get an error. You can add it again later.",
            button: "Delete Preset")
    }

    // "CPU heavy" for a class, "preset cpu_small" for a preset.
    @MainActor
    private static func subject(_ table: SizeTableModel, _ row: SizeRowView) -> String {
        table.canAdd ? "preset \(row.row.name)" : row.label
    }

    private static func capitalized(_ text: String) -> String {
        text.prefix(1).uppercased() + text.dropFirst()
    }

    // "None fit" reads lowercase inside a sentence.
    private static func lowered(_ fits: String) -> String {
        fits == LiveText.noneFit ? "none fit" : fits
    }
}
