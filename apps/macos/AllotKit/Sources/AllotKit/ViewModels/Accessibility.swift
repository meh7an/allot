/// How large the panel's text is. macOS's own Text Size setting reaches only the apps it lists, so
/// the panel has its own, under More options.
public enum TextSize: String, Codable, Sendable, Equatable, CaseIterable, Identifiable {
    case standard
    case large
    case larger

    public var id: Self {
        self
    }

    public var title: String {
        switch self {
        case .standard: "Default"
        case .large: "Large"
        case .larger: "Larger"
        }
    }

    /// The multiple of the handoff's text sizes.
    public var scale: Double {
        switch self {
        case .standard: 1
        case .large: 1.18
        case .larger: 1.35
        }
    }
}

/// The widths of a Resources row's outer columns, alike in every row so the bars line up.
public struct MeterColumns: Sendable, Equatable {
    // The panel's 360 less the card's margins, border and padding and the gaps between columns.
    private static let room = 302.0
    private static let label = 48.0
    private static let value = 108.0
    private static let minBar = 48.0

    public let label: Double
    public let value: Double

    /// Grows with the text. The value gives way first, so the bar always keeps its minimum.
    public init(scale: Double) {
        let wider = max(1, scale)
        label = (Self.label * wider).rounded()
        value = max(0, min((Self.value * wider).rounded(), Self.room - Self.minBar - label))
    }
}

extension MeterRow {
    /// What VoiceOver reads after the label: the value, then what the tooltip shows a pointer.
    public var spokenValue: String {
        tooltip.isEmpty ? value : "\(value), \(tooltip)"
    }
}

extension DriveRow {
    /// What VoiceOver reads after the label: the space, the rates, and where it is mounted.
    public var spokenValue: String {
        [value, activity, id].filter { !$0.isEmpty }.joined(separator: ", ")
    }
}

/// Decides what VoiceOver announces as snapshots go by while the panel is open: the summary when it
/// changes, and each card's stop outcome once.
public struct PanelAnnouncer: Sendable {
    private var summary: String?
    private var spoken: [Int64: String] = [:]

    public init() {}

    /// The panel opened: what it shows now is read by the cursor, not announced.
    public mutating func reset() {
        summary = nil
        spoken = [:]
    }

    /// What to announce now, in order.
    public mutating func observe(_ snapshot: PanelSnapshot) -> [String] {
        let isFirst = summary == nil
        var texts: [String] = []
        var next: [Int64: String] = [:]
        for job in snapshot.groups.flatMap(\.jobs) {
            let text = job.card.announcement
            next[job.id] = text
            if !isFirst && !text.isEmpty && text != spoken[job.id] {
                texts.append(text)
            }
        }
        if !isFirst && snapshot.header.summary != summary {
            texts.append(snapshot.header.summary)
        }
        summary = snapshot.header.summary
        spoken = next
        return texts
    }
}
