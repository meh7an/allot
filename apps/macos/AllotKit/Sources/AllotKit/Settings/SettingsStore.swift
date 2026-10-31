import Foundation

/// Reads and writes `tray.json`. A missing or unreadable file gives the defaults, and the next
/// save replaces it.
public struct SettingsStore: Sendable {
    public let file: URL

    public init(file: URL) {
        self.file = file
    }

    public func load() -> TraySettings {
        guard let data = try? Data(contentsOf: file) else {
            return TraySettings()
        }
        let decoder = JSONDecoder()
        // The Windows app accepts comments and trailing commas in the same file.
        decoder.allowsJSON5 = true
        return (try? decoder.decode(TraySettings.self, from: data)) ?? TraySettings()
    }

    /// Writes indented snake_case JSON. `.atomic` writes beside the file and renames it over the
    /// old one, so a crash mid-write never leaves half a file.
    public func save(_ settings: TraySettings) throws {
        try FileManager.default.createDirectory(
            at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys, .withoutEscapingSlashes]
        var data = try encoder.encode(settings)
        data.append(UInt8(ascii: "\n"))
        try data.write(to: file, options: .atomic)
    }
}
