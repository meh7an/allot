# M0.2 manual checklist: macOS

Build on the Mac (macOS 14 or later, Xcode, xcodegen):

```
cd spikes/settings-window/macos
xcodegen generate
xcodebuild -project AllotSettingsSpike.xcodeproj -scheme AllotSettingsSpike -configuration Debug -derivedDataPath build build
open build/Build/Products/Debug/AllotSettingsSpike.app
```

If it does not compile, send the errors back; this side cannot build Swift. The spike is an
accessory app with a gear status item, separate from Allot.app (its own bundle id
`tech.mehran.allot.settings-spike`), and it never talks to allotd.
- Clicking the gear toggles a non-activating stand-in panel, like Allot's. The panel has
  "Settings…", "Open Settings in 5 s", and the switches the checks compare.
- `open -b tech.mehran.allot.settings-spike` while the spike runs is a "reopen": it opens
  Settings, as a future `allot settings` would.
- The log is `~/Library/Logs/AllotSettingsSpike/spike.log`, and the window's footer shows the
  policy, active and key state.

Defaults: Dock "Dock and Cmd-Tab while open", order "makeKeyAndOrderFront, then activate()",
main menu on. Run each check 3 times; activation is reported to be flaky. Note the macOS version.

| # | Check | How | Expect | Result |
|---|-------|-----|--------|--------|
| 1 | Open from the panel | With another app in front (Safari, Finder), gear > Settings… | Window in front, typing lands in "Type here" without a click; log: `300 ms later: app active true, key true` | |
| 2 | Dock and Cmd-Tab while open | Settings open | A Dock icon and a Cmd-Tab entry; the menu bar shows the spike's menus | |
| 3 | Gone after closing | Close (red button), then Cmd-W | Dock icon and Cmd-Tab entry go; the app in front before comes back in front; the status item stays | |
| 4 | No second copy | Settings open behind another app; gear > Settings…; then `open -b tech.mehran.allot.settings-spike` | The same window comes to the front, never two | |
| 5 | From minimized | Minimize (yellow); open again from the panel, then from the Dock icon | Deminiaturizes, in front, key | |
| 6 | Opened while another app is in front | "Open Settings in 5 s", click another app at once | Note: in front with focus, behind, or bouncing | |
| 7 | Activation order | Repeat 1 and 6 with each order (three) | Note which order lands in front with focus every time | |
| 8 | Stay accessory | Dock: "Stay accessory"; repeat 1, 4, 6 | Note: no Dock icon or Cmd-Tab; does it still come to the front and take keys? | |
| 9 | Keys with a main menu | Main menu on: in "Type here", Cmd-A, Cmd-C, Cmd-V, Cmd-Z; Cmd-W closes; Cmd-Q quits | All work | |
| 10 | Keys without one | Main menu off: same keys | Note which stop working (expected: all of them) | |
| 11 | Closing never quits | Close the window | The spike stays in the menu bar | |
| 12 | Position memory | Move and resize, close, reopen; Quit, relaunch, open | Same frame both times | |
| 13 | Second display | Move to a second display (other scale if possible), close, reopen; then unplug it and reopen | Reopens there; with it gone, fully on the remaining screen; log: backing scale change | |
| 14 | Full screen and Spaces | Another app full screen, or a different Space; open Settings | Note where it opens (current Space, or switches Space) | |
| 15 | Keyboard | Tab through the sidebar and controls (with Full Keyboard Access if needed) | Focus ring visible, every control reachable | |

macOS version:

Notes (anything odd, with log lines):
