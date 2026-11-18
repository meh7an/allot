import Foundation
import os

/// Appends to `~/Library/Logs/AllotSettingsSpike/spike.log` and to the unified log
/// (`log stream --predicate 'subsystem == "tech.mehran.allot.settings-spike"'`).
enum SpikeLog {
    private static let logger = Logger(subsystem: "tech.mehran.allot.settings-spike", category: "spike")

    static let file: URL = FileManager.default
        .homeDirectoryForCurrentUser
        .appending(path: "Library/Logs/AllotSettingsSpike/spike.log")

    static func write(_ message: String) {
        logger.log("\(message, privacy: .public)")
        let stamp = Date.now.formatted(.dateTime.hour().minute().second().secondFraction(.fractional(3)))
        let line = "\(stamp) \(message)\n"
        let folder = file.deletingLastPathComponent()
        try? FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        if let handle = try? FileHandle(forWritingTo: file) {
            defer { try? handle.close() }
            _ = try? handle.seekToEnd()
            try? handle.write(contentsOf: Data(line.utf8))
        } else {
            try? Data(line.utf8).write(to: file)
        }
    }
}
