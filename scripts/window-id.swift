// Prints the CGWindowID of the first normal on-screen window owned by a PID.
// Used by scripts/screenshot. Owner PIDs are readable without Screen Recording
// permission; window titles are not, so we match by PID.
import CoreGraphics
import Foundation

guard CommandLine.arguments.count == 2, let pid = Int(CommandLine.arguments[1]) else {
    FileHandle.standardError.write("usage: window-id.swift <pid>\n".data(using: .utf8)!)
    exit(2)
}

let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]] ?? []
for window in windows {
    guard (window[kCGWindowOwnerPID as String] as? Int) == pid,
          (window[kCGWindowLayer as String] as? Int) == 0,
          let id = window[kCGWindowNumber as String] as? Int
    else { continue }
    print(id)
    exit(0)
}
exit(1)
