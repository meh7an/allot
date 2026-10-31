import Foundation

/// The names Finder shows for mounted volumes, for the drive rows; allotd names drives by
/// mount point only.
@MainActor
enum VolumeNames {
    private static var cache: [String: String?] = [:]

    /// The volume's name, or `nil` when nothing readable is mounted there. Cached for the app's
    /// life: a renamed volume shows its new name after a restart.
    static func name(mountedAt mountPoint: String) -> String? {
        if let cached = cache[mountPoint] {
            return cached
        }
        let values = try? URL(fileURLWithPath: mountPoint, isDirectory: true)
            .resourceValues(forKeys: [.volumeNameKey])
        let name = values?.volumeName
        cache[mountPoint] = name
        return name
    }
}
