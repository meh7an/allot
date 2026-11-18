# M0.3 manual checklist: GNOME

The spike is its own extension (`allot-settings-spike@mehran.tech`). It runs in a nested GNOME
Shell window under the throwaway HOME `~/allot-s03/home`, against the mock settings server on port
47960, so your desktop, `~/.allot` and any real allotd are untouched. On this PC, start it in WSL
Ubuntu (GNOME Shell 50.1). On a GNOME 49 or newer desktop, run the same script from a clone:

```bash
bash /mnt/e/User/Projects/resources-helper-mcp/spikes/settings-window/gnome/nested.sh
```

Close the nested window to stop everything. Use a second terminal for the server's controls:

```bash
echo slow > ~/allot-s03/home/settings-spike-mode
```

The modes are `normal`, `slow` (4 s per answer), `down` and `unauthorized`.

```bash
curl -X POST http://127.0.0.1:47960/spike/hand-edit
```

That command stands in for an edit by hand: headroom moves by 5.

- The log is `~/allot-s03/home/settings-spike.log`. The server's requests are in
  `~/allot-s03/home/server.log`.
- The panel button is "Spike". Its menu has Settings, which raises an open window; Settings
  (openPreferences only), which is the shell's plain call; and Settings in 3 s.
- Already checked headless (`gnome/smoke.sh`, GNOME 50.1): it opens focused and on top, a second
  plain open is refused with an unhandled rejection, Settings raises, and edits, 100 refused,
  conflict and Apply Mine, port restart note, reset, down, token refused, an edit just before
  closing, slow allotd with and without waiting, the panel file seen by the shell, and
  `destroy()`. What is left needs your eyes and hands.

Fill in each Result.

| # | Check | How | Expect | Result |
|---|-------|-----|--------|--------|
| 1 | Open from the panel menu | Spike > Settings | Window in front and focused; Up and Down in Headroom change it without a click first | |
| 2 | Plain second open | Settings open; Spike > Settings (openPreferences only) | Nothing visible happens (the shell refuses a second prefs window) | |
| 3 | Raise | Click the nested desktop or overview so Settings loses focus; Spike > Settings | Settings comes to the front, focused | |
| 4 | Opened while away | Close Settings; Spike > Settings in 3 s; click elsewhere at once | Note: in front and focused, or behind | |
| 5 | Keyboard | Tab and Shift+Tab through a page; Ctrl+F; Escape | Visible focus on every control; search finds "Headroom"; Escape closes or clears search (note which) | |
| 6 | Edits | Headroom: click + three times fast, then type 22 and Tab | One PATCH per burst in server.log, not one per click | |
| 7 | Refused value | Headroom 100 | Toast with the reason; value back to the last saved one | |
| 8 | Conflict | Run hand-edit, then change Estimate margin | Toast "Settings changed outside this window" with Apply Mine; Apply Mine saves the margin; headroom shows the hand edit | |
| 9 | allotd down | Mode `down`; Retry; then mode `normal`; Retry | Status says not answering and the rows grey out; back to Connected afterwards | |
| 10 | Slow allotd | Mode `slow`; close and open Settings; then turn on Panel > Wait for allotd, close, open | Without waiting: window at once, "Connecting..." for 4 s. With waiting: nothing at all for 4 s (the reason M6.1 must not wait) | |
| 11 | Panel setting | Panel > Show CPU off | Spike menu shows "Show CPU: off" at once | |
| 12 | Spin rows | Look at the numbers; Panel > Numbers as spin rows off; reopen | Note which looks right; both should edit the same way | |
| 13 | Size | Resize, close, reopen | Note: opens at 720 x 640 again (nothing remembers the size) | |
| 14 | Dark style | Nested Quick Settings > Dark Style | Window follows at once | |
| 15 | Restart note | Service > Port 47999 | Subtitle "Restart allotd to use the new port." and a reset button; reset brings 47823 back | |

Optional, with Orca (`sudo apt install orca` in WSL, and tell me so it goes on the purge list):
does Orca read the Headroom row in both number styles? Headless AT-SPI shows `Adw.SpinRow` rows
missing from the tree entirely on GNOME 50.1, while an action row with a `Gtk.SpinButton` is read as
"Headroom, spin button, 15".

Not run anywhere: GNOME 48, 49 and 51 (only 50.1 here), and a real GNOME desktop session.

Notes (anything odd, with log lines):
