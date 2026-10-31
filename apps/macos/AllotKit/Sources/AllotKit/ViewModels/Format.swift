import AllotContract
import Foundation

/// Text shared by several view models. Numbers never follow the user's locale: the UI is
/// English, and a decimal comma would read as a list separator next to the handoff's middle dots.
public enum Format {
    private static let shortIdLength = 8
    private static let initialsLength = 2

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

    /// A reservation as `8 cores · 4 GB` or `2 cores · 4 GB · GPU 80% · 4 GB VRAM`. Cores always
    /// show; RAM, GPU and VRAM only when reserved.
    public static func reservation(_ profile: JobProfile) -> String {
        var parts = [cores(profile.cores)]
        if profile.ramMb > 0 {
            parts.append("\(size(profile.ramMb)) GB")
        }
        if profile.gpuPct > 0 {
            parts.append("GPU \(whole(Double(profile.gpuPct)))%")
        }
        if profile.vramMb > 0 {
            parts.append("\(size(profile.vramMb)) GB VRAM")
        }
        return parts.joined(separator: " · ")
    }

    /// `1 core`, `8 cores`, `2.5 cores`.
    public static func cores(_ cores: Float) -> String {
        let number = upToOneDecimal(Double(cores))
        return number == "1" ? "1 core" : "\(number) cores"
    }

    /// A duration as `3:06`, or `1:02:03` from one hour.
    public static func duration(_ seconds: Int64) -> String {
        let seconds = max(0, seconds)
        let hours = seconds / 3600
        let minutes = seconds / 60 % 60
        let rest = seconds % 60
        return hours > 0
            ? "\(hours):\(twoDigits(minutes)):\(twoDigits(rest))"
            : "\(seconds / 60):\(twoDigits(rest))"
    }

    /// A rough wait as `2 min` or `1 h 20 min`, rounded to whole minutes and never less than
    /// one.
    public static func minutes(_ seconds: UInt64) -> String {
        let minutes = max(1, Int64((Double(seconds) / 60).rounded(.toNearestOrAwayFromZero)))
        guard minutes >= 60 else {
            return "\(minutes) min"
        }
        let hours = minutes / 60
        let rest = minutes % 60
        return rest == 0 ? "\(hours) h" : "\(hours) h \(rest) min"
    }

    /// How long ago something ended: `just now`, `4 min ago`, `2 h ago`, `3 d ago`.
    public static func ago(_ seconds: Int64) -> String {
        switch seconds {
        case ..<60: "just now"
        case ..<3600: "\(seconds / 60) min ago"
        case ..<86_400: "\(seconds / 3600) h ago"
        default: "\(seconds / 86_400) d ago"
        }
    }

    /// The two-letter placeholder for a process tile: `cargo` gives `CA`. Paths, quotes and a
    /// Windows `.exe` suffix are ignored; empty when nothing is left.
    public static func initials(_ program: String) -> String {
        var name = Substring(program.trimmingCharacters(in: .whitespaces))
        name = name.drop { $0 == "\"" }
        while name.last == "\"" {
            name = name.dropLast()
        }
        if let separator = name.lastIndex(where: { $0 == "/" || $0 == "\\" }) {
            name = name[name.index(after: separator)...]
        }
        if name.lowercased().hasSuffix(".exe") {
            name = name.dropLast(4)
        }
        let letters = name.filter { $0.isLetter || $0.isNumber }.prefix(initialsLength)
        return String(letters).uppercased()
    }

    /// Megabytes as gigabytes, whole ones without decimals and others with one: `4`, `1.5`.
    public static func size(_ megabytes: UInt64) -> String {
        upToOneDecimal(Double(megabytes) / 1024)
    }

    /// A whole number, as `GPU 80%` and `3% CPU` print it.
    public static func whole(_ value: Double) -> String {
        String(format: "%.0f", value.rounded(.toNearestOrAwayFromZero))
    }

    /// One decimal. Halves round away from zero, as the Windows app's .NET formatting does, so
    /// both apps print the same figure for the same state.
    public static func oneDecimal(_ value: Double) -> String {
        let rounded = (value * 10).rounded(.toNearestOrAwayFromZero) / 10
        return String(format: "%.1f", rounded)
    }

    // .NET's `0.#`: one decimal, dropped when it is zero.
    private static func upToOneDecimal(_ value: Double) -> String {
        let rounded = (value * 10).rounded(.toNearestOrAwayFromZero) / 10
        return rounded == rounded.rounded()
            ? String(format: "%.0f", rounded)
            : String(format: "%.1f", rounded)
    }

    private static func twoDigits(_ value: Int64) -> String {
        value < 10 ? "0\(value)" : "\(value)"
    }
}
