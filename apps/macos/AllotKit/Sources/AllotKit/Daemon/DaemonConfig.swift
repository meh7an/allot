import Foundation

/// The one value the app needs from `config.toml`: `[daemon] port`.
public enum DaemonConfig {
    /// allotd's port when the config does not set one, as in `DaemonConfig::default`.
    public static let defaultPort: UInt16 = 47823

    public static func readPort(_ data: DataDirectory) -> UInt16 {
        guard let toml = try? String(contentsOf: data.configFile, encoding: .utf8) else {
            return defaultPort
        }
        return port(from: toml)
    }

    /// Finds the port as either `port` under `[daemon]` or a top-level `daemon.port`. Not a TOML
    /// parser: allotd validates the whole file and refuses to start on a bad one, so anything
    /// this cannot read falls back to the default.
    public static func port(from toml: String) -> UInt16 {
        var table = ""
        // `\r\n` is a single Character in Swift, which a split on "\n" would not see.
        for rawLine in toml.split(whereSeparator: \.isNewline) {
            let line = stripComment(rawLine).trimmingCharacters(in: .whitespacesAndNewlines)
            if line.isEmpty {
                continue
            }
            if line.hasPrefix("[") && line.hasSuffix("]") {
                table = line.trimmingCharacters(in: CharacterSet(charactersIn: "[]"))
                    .trimmingCharacters(in: .whitespaces)
                continue
            }
            guard let equals = line.firstIndex(of: "=") else {
                continue
            }
            let key = line[..<equals].trimmingCharacters(in: .whitespaces)
            let isPort = (table == "daemon" && key == "port")
                || (table.isEmpty && key == "daemon.port")
            if isPort {
                return parsePort(line[line.index(after: equals)...])
            }
        }
        return defaultPort
    }

    // The port is an integer, so a '#' always starts a comment.
    private static func stripComment(_ line: Substring) -> Substring {
        line.firstIndex(of: "#").map { line[..<$0] } ?? line
    }

    // TOML allows underscores between digits, as in the 47_823 allotd's own default uses.
    private static func parsePort(_ value: Substring) -> UInt16 {
        let digits = value.trimmingCharacters(in: .whitespaces)
            .replacingOccurrences(of: "_", with: "")
        guard digits.allSatisfy(\.isASCIIDigit), let port = UInt16(digits), port != 0 else {
            return defaultPort
        }
        return port
    }
}

private extension Character {
    var isASCIIDigit: Bool {
        isASCII && isNumber
    }
}
