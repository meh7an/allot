import SwiftUI

/// The Allot mark as a shape: seven rounded bars on a 16-unit grid, as in
/// `assets/brand/build.py`.
struct AllotMark: Shape {
    /// (left, right, row) in mark units; row n spans y = 1 + 3n to 3 + 3n.
    private static let bars: [(CGFloat, CGFloat, CGFloat)] = [
        (6.5, 9.5, 0),
        (5, 11, 1),
        (3.5, 6.5, 2),
        (9.5, 12.5, 2),
        (2, 14, 3),
        (1, 4.5, 4),
        (11.5, 15, 4),
    ]

    func path(in rect: CGRect) -> Path {
        let unit = min(rect.width, rect.height) / 16
        var path = Path()
        for (left, right, row) in Self.bars {
            let bar = CGRect(
                x: rect.minX + left * unit,
                y: rect.minY + (1 + 3 * row) * unit,
                width: (right - left) * unit,
                height: 2 * unit)
            path.addRoundedRect(in: bar, cornerSize: CGSize(width: unit, height: unit))
        }
        return path
    }
}
