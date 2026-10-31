/// Where a process tile's icon comes from. Command-line tools all share Finder's generic
/// executable icon, which says nothing, so only a process inside an app bundle (Xcode's
/// toolchain, LM Studio's runtime) shows an icon: the bundle's own. Others keep their initials.
public enum ProcessIcon {
    /// The outermost `.app` bundle that contains `exe`, or `nil` when there is none.
    public static func bundlePath(forExe exe: String?) -> String? {
        guard let exe, exe.hasPrefix("/") else {
            return nil
        }
        let components = exe.split(separator: "/", omittingEmptySubsequences: true)
        // The executable's own name never counts, even when it ends in `.app`.
        guard let index = components.dropLast().firstIndex(where: { $0.hasSuffix(".app") })
        else {
            return nil
        }
        return "/" + components[...index].joined(separator: "/")
    }
}
