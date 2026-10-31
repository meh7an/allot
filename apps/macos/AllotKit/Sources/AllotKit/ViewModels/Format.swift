import AllotContract
import Foundation

/// Text shared by several view models. Numbers never follow the user's locale: the UI is
/// English, and a decimal comma would read as a list separator next to the handoff's middle dots.
public enum Format {
    private static let shortIdLength = 8

    /// The 8-character prefix the panel shows for a session UUID.
    public static func shortId(_ session: String) -> String {
        String(session.prefix(shortIdLength))
    }

    /// The project folder: the last segment of a working directory, or empty without one.
    /// Both separators count, since a state can come from a Windows daemon.
    public static func project(_ cwd: String?) -> String {
        let segments = (cwd ?? "").split { $0 == "/" || $0 == "\\" }
        return segments.last.map(String.init) ?? ""
    }

    /// Live usage as `7.8 cores · 2.9 GB`.
    public static func usage(_ usage: ResourceUsage) -> String {
        "\(oneDecimal(Double(usage.cores))) cores · \(gigabytes(usage.ramMb)) GB"
    }

    /// Megabytes as gigabytes with one decimal, divided by 1024 as Activity Monitor does.
    public static func gigabytes(_ megabytes: UInt64) -> String {
        oneDecimal(Double(megabytes) / 1024)
    }

    // Halves round away from zero, as the Windows app's .NET formatting does, so both apps
    // print the same figure for the same state.
    private static func oneDecimal(_ value: Double) -> String {
        let rounded = (value * 10).rounded(.toNearestOrAwayFromZero) / 10
        return String(format: "%.1f", rounded)
    }
}
