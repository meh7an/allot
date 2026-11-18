# M0.3 manual checklist: KDE Plasma

The spike is its own widget (`tech.mehran.allot.settingsspike`). It offers both hosts:

- **A**, the widget's config dialog with an Admission page (Apply model);
- **B**, a window of its own (instant apply).

It runs on royalserver (Plasma 6.8.3, Qt 6.12, KF 6.32) in a throwaway Plasma session in a window
on your desktop, with its own KWin, HOME, D-Bus and runtime folder under `~/allot-s03`. Your panel,
config, `~/.allot` and any real allotd are untouched. The spike is already copied to
`~/allot-s03/spike`. In a terminal on royalserver:

```bash
N="bash $HOME/allot-s03/spike/kde/nested.sh"
$N stop; $N unserve; $N widget; $N start window; $N serve; sleep 5; $N panel
```

Click the Allot Settings Spike icon in the nested panel; its popup has both Settings buttons.

- Server modes: `$N mode slow` (also `normal`, `down`, `unauthorized`).
- An edit by hand: `$N hand-edit`.
- The widget's log: `$N log SettingsSpike`. Requests: `~/allot-s03/server.log`.
- Stop with `$N stop; $N unserve`.

Already checked in a virtual nested session (`kde/smoke.sh`), driven through AT-SPI:

- B opens active, every write path works, a change just before closing is sent, and the hidden
  window comes back with its values.
- A: Apply turns on with an edit, a category switch rebuilds the page, and a switch or Cancel with
  unsaved edits asks Apply, Discard or Cancel.
- A: OK loses a write sent straight from the page, while a write handed to the widget arrives.

What is left needs your eyes and hands.

Fill in each Result.

| # | Check | How | Expect | Result |
|---|-------|-----|--------|--------|
| 1 | B from a click | Popup > Settings: Own Window | Window in front and focused; arrows in Headroom work without a click first | |
| 2 | B from the context menu | Right-click the icon > Settings (Own Window) | Same as 1 | |
| 3 | B raised | Click the desktop or another window; open B again from the popup | The same window comes to the front, focused (not a second one) | |
| 4 | B in the taskbar | While open, then after closing | Own taskbar entry and Alt+Tab entry with a sensible icon and title while open; none after | |
| 5 | B keeps state | Change a value, close with X, reopen | Same values, same size; note whether the position is kept | |
| 6 | B edits | Headroom: click up three times fast, then type 22 and Tab | One PATCH per burst in server.log | |
| 7 | B conflict | `$N hand-edit`, then change Estimate margin | Inline message with Apply Mine, which saves the margin | |
| 8 | B down | `$N mode down`; Retry; `$N mode normal`; Retry | "not answering" and greyed fields, then Connected | |
| 9 | A from the popup | Popup > Settings: Config Dialog | Dialog in front and focused, with Admission, Panel, Keyboard Shortcuts and About | |
| 10 | A from the menu | Right-click the icon > Configure Allot Settings Spike... | Same dialog | |
| 11 | A Apply | Change Headroom; Apply | Apply is off until the change; one PATCH; status shows the new revision | |
| 12 | A switch | Change Headroom; click Panel | Apply, Discard or Cancel prompt; Discard drops the change | |
| 13 | A OK, direct | Panel: "Send allotd's changes through the widget" off; Apply; Admission: change Headroom; OK | No PATCH in server.log: the write is lost | |
| 14 | A OK, through the widget | Same with it on | PATCH arrives; the popup shows "Last handed write: ok" | |
| 15 | A errors | With it on: Headroom 100, OK | The dialog is gone, so the refusal shows only in the popup ("invalid") | |
| 16 | Look | Both hosts, light and dark (`$N run plasma-apply-colorscheme BreezeDark`, then `BreezeLight`) | Note which feels right for Allot | |
| 17 | Panel setting | B: Show CPU (saved at once); A: Panel > Show CPU, Apply | Popup line follows each | |

Not run anywhere: Plasma 6.0 to 6.7 (6.0 has no `unsavedChanges`, so A relies on
`configurationChanged()` there); a real Plasma session outside the nested one; X11 (Plasma 6.8
retired it).

Notes (anything odd, with log lines):
