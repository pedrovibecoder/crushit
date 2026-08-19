// Debugging aid: lists macOS status items so you can see whether the tray
// icon exists but is being hidden for want of menu-bar space.
// Usage: swift preview/windows.swift [owner-substring]
import CoreGraphics
import Foundation

// Status items live at the status-bar window level; list every on-screen window
// so we can see whether the app owns one at all.
let options = CGWindowListOption(arrayLiteral: .optionOnScreenOnly, .excludeDesktopElements)
guard let windows = CGWindowListCopyWindowInfo(options, kCGNullWindowID) as? [[String: Any]] else {
    print("could not read the window list"); exit(1)
}
let needle = CommandLine.arguments.count > 1 ? CommandLine.arguments[1].lowercased() : ""
for window in windows {
    let owner = (window[kCGWindowOwnerName as String] as? String) ?? "?"
    if !needle.isEmpty && !owner.lowercased().contains(needle) { continue }
    let layer = (window[kCGWindowLayer as String] as? Int) ?? -1
    let name = (window[kCGWindowName as String] as? String) ?? ""
    let bounds = window[kCGWindowBounds as String] as? [String: Any] ?? [:]
    let x = bounds["X"] ?? "?", y = bounds["Y"] ?? "?"
    let w = bounds["Width"] ?? "?", h = bounds["Height"] ?? "?"
    print("owner=\(owner) layer=\(layer) name=\(name) bounds=(\(x),\(y) \(w)x\(h))")
}
