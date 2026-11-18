# M0.2 manual checklist: Windows

Run `spikes\settings-window\windows\publish\allot-settings-spike.exe`, the Native AOT build.
`publish\` is not committed. To build it again, run this with `vswhere.exe` on PATH:

```
dotnet publish SettingsSpike.csproj -c Release -o publish
```

The spike runs beside the real tray and never touches `~\.allot`.
- A small "Stand-in flyout" window opens. Its More options menu (the ⋯ button) has the Settings
  item, which is what the real flyout will get.
- `allot-settings-spike.exe` run again, while the spike runs, is a "second launch": it opens
  Settings in the running spike. `--launcher` shows the stand-in again, and `--no-grant` skips
  AllowSetForegroundWindow.
- The log is `%LOCALAPPDATA%\AllotSettingsSpike\spike.log` (its last lines also show in the
  stand-in), and the Settings footer shows live scale and placement.

Fill in each Result. "Log" means the matching lines from spike.log.

| # | Check | How | Expect | Result |
|---|-------|-----|--------|--------|
| 1 | Open from the flyout menu | ⋯ > Settings | Settings opens in front, the stand-in hides, typing lands in "Type here" without a click | |
| 2 | No second copy | Settings open; click another app; second launch | The same window (same # in the title) comes to the front, focused; log: `SetForegroundWindow True, foreground now True` | |
| 3 | From minimized | Minimize Settings; second launch | Restores, in front, focused | |
| 4 | Without the foreground grant | Settings open behind another app; run with `--no-grant` | Note whether it comes to the front or only flashes in the taskbar (tells M4.1 whether `allot settings` must grant) | |
| 5 | Activate alone | Turn off "Call SetForegroundWindow"; repeat 2 | Note whether `Activate()` alone brings it to the front | |
| 6 | Opened while another app is in front | "Open in 5 s", then click another app at once; repeat with the switch off | Note: in front, or a taskbar flash | |
| 7 | Closing never quits | Close with X, then Alt+F4, under "Destroy" and under "Hide" | The process stays (Task Manager); a second launch opens it again | |
| 8 | Hide model keeps state | "Hide it and reuse it"; change the switch and page; close; reopen | Same # in the title, same page and values | |
| 9 | Taskbar and Alt+Tab | While Settings is open, and after closing it | Its own taskbar button and Alt+Tab entry with the icon while open; none once closed; the stand-in never has one | |
| 10 | Position memory | Move and resize, close, reopen; Quit, relaunch, open | Same place and size both times | |
| 11 | Maximized | Maximize, close, reopen, then restore | Reopens maximized; restore gives the earlier size | |
| 12 | Display gone | On a second monitor, close, disconnect or disable it, reopen | Opens fully inside the remaining monitor's work area | |
| 13 | DPI change in place | Settings open; Settings > Display > Scale to 125 % (then back) | Footer scale follows, text crisp, window keeps its size in DIP; log: `scale 1 -> 1.25`, the minimum size line and its "was" value | |
| 14 | DPI across monitors | If two monitors have different scales: drag Settings across | Same as 13 on crossing; no jump or wrong size | |
| 15 | Restore at another scale | Close at 100 %, change scale, reopen | Same size in DIP (bigger in pixels); log: `restore saved`, "asked" equal to "got" | |
| 16 | Mica | Look at the backdrop while active and inactive; try the footer's Mica Alt, Acrylic and None; switch light and dark mode; turn off transparency effects | Mica tints by wallpaper when active and goes solid when inactive or with transparency off; theme follows | |
| 17 | Title bar | Drag by the title bar; double-click it; caption buttons | Drags, maximizes, buttons work; Mica reaches the title bar | |
| 18 | Keyboard | Tab and the arrow keys through the nav and controls | Visible focus, every control reachable | |
| 19 | Leak (optional, automated already) | "Cycle 20 times" in each model, then "GC and count" | Log: `Alive = 0` in Destroy; private MB grows in Destroy, flat in Hide | |

Notes (anything odd, with log lines):
