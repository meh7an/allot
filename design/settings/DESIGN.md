# Allot Settings: design

Status: approved by Mehran (M0.4, 2026-11-20). Mockups: `design/settings/mockups/`
(open `index.html`). Decision log: `design/settings/_archive/decision-log.yaml`.

This is the design every Settings milestone builds from: M1.1 takes its labels, help text, units,
ranges and pages from the key catalog below; M4.1 and M4.2 build the Windows reference; M5, M6.1
and M6.2 adapt it per the platform section.

Places marked **Revisit R<n>** depend on a manual check that has not run yet (M0.2 and M0.3
checklists). The list of them, with what changes if a check fails, is at the end.

---

## 1. Decisions

| # | Decision | Choice | Chosen by |
|---|----------|--------|-----------|
| D1 | KDE host (M0.3) | B: a `Kirigami.ApplicationWindow` of the widget's own, for every setting | Mehran |
| D2 | Apply model | Instant for single values (600 ms after the last change); a class, preset or rule commits as a whole from its edit dialog | Mehran |
| D3 | Pages | 7 task pages: Panel, Admission, Sizes, Commands, Benchmarks, Leases, Service | Mehran |
| D4 | Key names | Shown small, in monospace, at the end of each setting's help line | Mehran |
| D5 | Size tables | Read-only table to compare rows; Edit opens a dialog (Windows, KDE) or sheet (macOS) | Mehran |
| D6 | Test a command | Box at the top of Commands; the matching rule is highlighted below; also inside the rule dialog | Mehran |
| D7 | GNOME navigation | The stock view switcher with all 7 pages (**Revisit R8**) | Mehran |
| D8 | macOS navigation | Toolbar tabs, the classic Mac Settings window | Mehran |
| D9 | Visual style | Native controls and materials on each platform; no Allot-specific style in the window | Design lead |
| D10 | Signature element | Every group with numbers ends in a live "What this means" sentence built from the current values | Design lead |
| D11 | Advanced keys | No Advanced page; rarely changed keys sit last on their page under an "Advanced" group | Design lead |
| D12 | Errors | Inline at the field; the typed value stays with "Not saved" until fixed or undone; the file is untouched | Design lead |
| D13 | Lasting states | Banners (InfoBar, `Adw.Banner`, `InlineMessage`); toasts only for brief notices with Undo | Design lead |
| D14 | Reopen | Hide model (Windows, macOS, KDE): same page and scroll, values reloaded. GNOME: rebuilt, opens on Panel | Design lead, from M0.2 and M0.3 |
| D15 | Menu wording | Each platform's convention: Settings (Windows, GNOME), Settings… (macOS), Configure Allot… (KDE) | Design lead |

---

## 2. Information architecture

Seven pages in this order. The first, Panel, holds the panel's own settings (its own store,
works while allotd is down); the other six hold allotd's `config.toml`.

| Page | Icon idea | Groups (in order) |
|------|-----------|-------------------|
| Panel | gauge | Meters; Notifications; Startup (Windows, macOS); Text size (macOS) |
| Admission | scale | Keep free for your desktop; Learn from past runs; What fits on this machine |
| Sizes | stack | Classes; Presets |
| Commands | terminal | Test a command; Heavy-command rules; Print-only flags |
| Benchmarks | timer | Quiet machine |
| Leases | key | Leases; History (Advanced) |
| Service | server | Service; Connection; Linux (GNOME and KDE only); Files; Reset |

Every page ends with **Reset page** (section 6). The 19 scalar keys, the two tables and the two
lists all have exactly one home:

| Key | Page | Group |
|-----|------|-------|
| `admission.headroom_pct` | Admission | Keep free for your desktop |
| `estimates.lower`, `min_runs`, `margin_pct` | Admission | Learn from past runs |
| `[classes.*]` (with `min_cores`) | Sizes | Classes |
| `[presets.*]` (with `min_cores`) | Sizes | Presets |
| `classifier.rules` | Commands | Heavy-command rules |
| `classifier.light_flags` | Commands | Print-only flags |
| `benchmark.quiet_cpu_pct`, `quiet_gpu_pct`, `quiet_secs`, `max_quiet_wait_secs` | Benchmarks | Quiet machine |
| `leases.heartbeat_ttl_secs`, `idle_release_secs` | Leases | Leases |
| `retention.lease_days`, `history_days`, `keep_runs_per_job`, `max_job_runs` | Leases | History (Advanced) |
| `daemon.port`, `hook.fail_open` | Service | Connection (port under Advanced) |
| `linux.scopes`, `enforce_cpu`, `enforce_memory` | Service | Linux |

The Linux group shows only on GNOME and KDE. M1.1's spec carries a `platforms` field so the
panels hide it without hard-coding.

---

## 3. Anatomy of a setting row

```
[Label]                                              [control] [reset]
[Help sentence. Second sentence if needed.  key.name]
[inline error or "Restart needed" line, when present]
```

- **Label**: a noun phrase, sentence case (title case on macOS buttons only). A switch's label
  describes the on state.
- **Help**: one or two plain sentences, grade 7 to 8, then the TOML key in monospace at a smaller
  size (D4). Screen readers get label as the name and help plus key as the description.
- **Reset**: an icon button shown only when the value differs from the default. Accessible name
  "Reset Headroom to 15 %". It removes the key from `config.toml` (the default applies again).
  No confirmation; the field's old value shows in a brief toast with Undo.
- **Changed mark**: a value that differs from its default shows the reset button only. No dot,
  no color: the button itself is the mark, so nothing relies on color.

### Controls per value type

| Value type | Keys | Windows | macOS | GNOME | KDE |
|------------|------|---------|-------|-------|-----|
| On/off | `lower`, `fail_open`, `linux.*`, panel toggles | ToggleSwitch | Toggle (switch style) | `Adw.SwitchRow` | CheckBox (KDE form convention) |
| Percent | `headroom_pct`, `margin_pct`, `quiet_*_pct`, `gpu_pct` | Slider + NumberBox, % suffix | Slider + TextField with % formatter | `Adw.ActionRow` + `Gtk.SpinButton` (**Revisit R7**) | Slider + SpinBox |
| Count | `min_runs`, `keep_runs_per_job`, `max_job_runs` | NumberBox with spin buttons, unit label after | Stepper + TextField | ActionRow + SpinButton | SpinBox |
| Days | `lease_days`, `history_days` | NumberBox, "days" | Stepper + TextField, "days" | ActionRow + SpinButton | SpinBox, suffix " days" |
| Duration | `*_secs` | NumberBox + unit ComboBox (seconds, minutes, hours) | TextField + Picker | ActionRow + SpinButton + DropDown | SpinBox + ComboBox |
| Duration that can be off | `idle_release_secs` | ToggleSwitch, then the duration row below it (indented) | same | SwitchRow + ActionRow | CheckBox + SpinBox |
| Cores | `cores`, `min_cores` | NumberBox, step 1, decimals allowed (0.25) | TextField + Stepper | SpinButton, 2 digits | SpinBox (decimal) |
| Memory | `ram_mb`, `vram_mb` | NumberBox + unit (MB, GB) | same | SpinButton + DropDown | SpinBox + ComboBox |
| Port | `daemon.port` | NumberBox, no digit grouping | TextField, no grouping | SpinButton, no grouping | SpinBox, no grouping |
| Class | rule's `class` | ComboBox | Picker | `Adw.ComboRow` | ComboBox |
| String list | rule `args`, `light_flags` | Token list: chips with remove buttons + "Add" TextBox | TokenField-like chips | `Adw.WrapBox` of chips + EntryRow | Flow of chips + TextField |
| Table | classes, presets | ListView laid out as a table + Edit dialog | `Table` + sheet | Rows with aligned summary + `Adw.Dialog` | Rows with aligned summary + `Kirigami.Dialog` |
| Ordered list | `classifier.rules` | ListView with row menu + Edit dialog | List + sheet | Rows + `Adw.Dialog` | Rows + `Kirigami.Dialog` |

Rules for every number:

- Display uses the platform's number formatting (Intl equivalent: `NumberFormatter`, .NET
  `CultureInfo`, `Intl.NumberFormat` in GJS and QML), except the port, which is never grouped.
- A number field commits on Enter, on leaving the field, or on a spin step, never per keystroke.
- Units: memory shows GB when the value is a whole multiple of 1024 MB, otherwise MB; switching
  the unit converts the shown value, and the file always gets MB. Durations show the largest unit
  that divides the value evenly (180 s shows as 3 minutes); the file always gets seconds.
- Sliders are a second way in, never the only one: the number box beside each slider is the
  accessible control, and the slider has the same accessible name with "slider".

---

## 4. Key catalog (labels, help, ranges)

M1.1 copies this table into `SettingsSpec`. "Range" is what the panel enforces before sending; it
matches `Config::validate`. Live sentences use placeholders in braces; M2.2's fit preview fills
the machine numbers.

### Admission

**Keep free for your desktop**

| Key | Label | Help | Control, range | Default |
|-----|-------|------|----------------|---------|
| `admission.headroom_pct` | Headroom | Share of cores, memory, GPU and video memory that jobs never get, so your desktop stays responsive. | Percent, 0 to below 100, step 1 | 15 % |

Live: "Keeps {2.4} of {16} cores, {4.8} of {32} GB memory, {15} % of the GPU and {1.2} of {8} GB
video memory free."

**Learn from past runs**

| Key | Label | Help | Control, range | Default |
|-----|-------|------|----------------|---------|
| `estimates.lower` | Lower requests that past runs show are too big | Once a command has used less than it asks for, reserve what it really uses. Requests always rise when runs used more. Benchmarks never lower. | On/off | On |
| `estimates.min_runs` | Runs before lowering | Clean past runs of the same command in the same folder needed before its request lowers. | Count, at least 1, "runs" | 3 |
| `estimates.margin_pct` | Safety margin | Added on top of what past runs used, so a slightly heavier run still fits. | Percent, 0 to 100 | 10 % |

While `lower` is off, `min_runs` and `margin_pct` are disabled with the line "Used only while
lowering is on." (both only act when lowering, `core/src/history.rs`).

Live: "A command that asks for 8 cores but used about 3 in its last {3} runs is reserved {3.3}
cores. Requests never go below 0.25 cores, 256 MB, or their minimum cores, and run time never
lowers." With lowering off: "Requests are reserved as declared, or higher when past runs used
more."

**What fits on this machine** (live, read-only)

A small table from M2.2's fit preview, one row per class, for the values on screen:

| Class | Now | When idle |
|-------|-----|-----------|
| CPU heavy | 1 at full size, 2 more at 4 cores | 2 at full size |

Row text pattern: "{n} at full size" and "{m} more at {min} cores" (ICU plural; "None fit" when
both are 0). Caption: "Based on this machine now ({cpu} % CPU busy) and with every lease ended."
Updates 600 ms after a change on the page. **Needs from M2.2/M3.1:** fit counts per class now
and idle, full and shrunk, plus the kept-free amounts for the headroom sentence.

### Sizes

Intro: "Each heavy command reserves the size of its class. Presets are sizes a session can ask
for by name."

**Classes** table, four fixed rows: Light, CPU heavy, GPU heavy, Mixed (key names `light`,
`cpu_heavy`, `gpu_heavy`, `mixed` under each name in monospace).

Columns: Class · Cores · Minimum cores · Memory · GPU · Video memory · Run time · Fits now.
"Fits now" is the live column ("1, 2 more shrunk"). Row actions: Edit…, Reset (when changed).

**Presets** table, the same columns plus the name. Built-in presets (`cpu_small`, `cpu_medium`,
`gpu_small`, `quick`, `low_impact`) can be edited and reset, not renamed or deleted (the defaults
always come back, `Presets::deserialize`). Presets you add can be edited, renamed and deleted.
Below the table: **Add preset…**

**Edit dialog** (one per class or preset; title "Edit CPU heavy" or "Edit preset cpu_small"):

| Field | Label | Help | Control, range | Default |
|-------|-------|------|----------------|---------|
| name (new or renamed preset only) | Name | Lowercase letters, digits and _. Sessions ask for the preset by this name. | Text, `[a-z0-9_]+`, not a class name, unique | none |
| `cores` | Cores | Cores a job of this size reserves. Fractions such as 0.5 are allowed. | Cores, at least 0 | per class or preset |
| `min_cores` | Minimum cores | When the machine is busy, a job may start with fewer cores, down to this. | Radio: "Automatic ({4}: half the cores, at least 1)" or "Custom" with Cores, above 0 and at most Cores | Automatic |
| `ram_mb` | Memory | Memory a job of this size reserves. | Memory, at least 0 | per class or preset |
| `gpu_pct` | GPU | Share of the GPU's engines a job reserves. | Percent, 0 to 100 | per class or preset |
| `vram_mb` | Video memory | GPU memory a job reserves. | Memory, at least 0 | per class or preset |
| `est_secs` | Expected run time | How long a job of this size usually runs. Jobs of 15 s or less may start while a benchmark waits. | Duration, at least 0 | per class or preset |

The dialog's live line (above its buttons): "With these values: {1} fits now at full size, {2}
more at {4} cores. With every lease ended: {2} at full size." Buttons: **Save** (primary),
**Cancel**; Reset to default in the dialog's footer when the row is changed. A new preset needs
Name, Cores and Memory; the others start at GPU 0 %, Video memory 0 and 5 minutes
(`PRESET_EST_SECS`). Renaming warns in the dialog: "Sessions that ask for {old} get an error until
they use {new}."

Delete a preset: confirmation dialog "Delete preset {name}?", body "Sessions that ask for it by
name get an error. You can add it again later.", buttons **Delete preset** and **Cancel**.

### Commands

**Test a command** (D6)

| Element | Text |
|---------|------|
| Field label | Command line |
| Placeholder | `cargo build --release` (an example value) |
| Help | Shows how the hook would treat this command with the rules below. |

Result patterns, one line per simple command in the line (`a && b` gives two):
- "Heavy: needs a {CPU heavy} lease ({8} cores, {4 GB} memory). Rule {1}: {cargo} with {build,
  test, run or bench}." The rule's row below gets a highlight and "Matches" text (not color only).
- "Print-only: {--help} only prints and exits, so no lease is needed."
- "Not heavy: no rule matches {cargo fmt}."
- Empty field: no result line.

Inside the rule dialog the same box tests the rule being edited, before Save.
**Needs from M2.2:** per simple command, the result kind, the rule index, the class and its size.

**Heavy-command rules**

Status line above the list: "Built-in rules." or "Your rules. They replace the built-in list."
Each row: number, program, arguments joined with " · " (or "any arguments"), arrow, class.
Row menu ("More options for rule {1}"): Edit…, Move up, Move down, Delete. Keyboard: Alt+Up and
Alt+Down move the focused rule; Delete deletes it. Below the list: **Add rule…** and **Use
built-in rules…**.

| Field | Label | Help | Control |
|-------|-------|------|---------|
| `program` | Program | The program's name without folder or extension, such as cargo or python*. * matches any characters, and case is ignored. | Text, not blank |
| `args` | Arguments | Any one of these makes the command heavy; leave empty to match any arguments. A pattern starting with - matches that flag anywhere; others match the first argument that is not a flag. | String list, no blank entries |
| `class` | Lease class | The size a matching command reserves. | Class |

- Delete: immediate, then a toast "Rule {3} deleted." with **Undo** (10 s). The whole list is
  written at once, as always.
- Use built-in rules: confirmation "Use the built-in rules?", body "Your {4} rules are replaced by
  the 6 built-in ones. This can't be undone.", buttons **Use built-in rules** and **Cancel**. It
  removes the key.
- Empty list (written as `rules = []`): empty state "No rules. No command counts as heavy, so the
  hook never asks for a lease." with **Add rule…** and **Use built-in rules…**.

**Print-only flags**

| Key | Label | Help | Control | Default |
|-----|-------|------|---------|---------|
| `classifier.light_flags` | Print-only flags | A command with any of these only prints and exits, so it never needs a lease, even when a rule matches. | String list, no blank entries; adding or removing a chip saves | `--help`, `-h`, `--version`, `-V`, `/?` |

### Benchmarks

**Quiet machine**

| Key | Label | Help | Control, range | Default |
|-----|-------|------|----------------|---------|
| `benchmark.quiet_cpu_pct` | CPU load limit | A benchmark waits until total CPU load stays at or below this. | Percent, 0 to 100 | 10 % |
| `benchmark.quiet_gpu_pct` | GPU load limit | A benchmark waits until GPU load stays at or below this. | Percent, 0 to 100 | 10 % |
| `benchmark.quiet_secs` | Quiet for | How long both loads must stay under their limits, once no other lease is active. | Duration, above 0 | 10 seconds |
| `benchmark.max_quiet_wait_secs` | Longest wait | After this, a waiting benchmark starts anyway, marked not quiet. | Duration, at least Quiet for | 1 minute |

Live: "A benchmark starts once CPU and GPU stay at or below {10 %} for {10 seconds}. If that takes
longer than {1 minute}, it starts anyway, marked not quiet."

### Leases

**Leases**

| Key | Label | Help | Control, range | Default |
|-----|-------|------|----------------|---------|
| `leases.heartbeat_ttl_secs` | Lease timeout | A lease ends when its session stops checking in for this long. | Duration, above 0 | 1 minute |
| `leases.idle_release_secs` (switch) | End leases left idle | Ends a lease once nothing runs under it and its session has been idle, for sessions that forget to release. | On/off; off writes 0 | On |
| `leases.idle_release_secs` (duration) | Idle for | How long the session must be idle first. | Duration, above 0 | 3 minutes |

Turning the switch on restores the last duration used, or 3 minutes.

**History** (Advanced)

| Key | Label | Help | Control, range | Default |
|-----|-------|------|----------------|---------|
| `retention.lease_days` | Keep ended leases for | Ended leases and their audit trail are deleted after this. Cleanup runs every hour. | Days, at least 1 | 30 days |
| `retention.history_days` | Keep job runs for | Job runs are deleted after this, except each command's newest runs below. | Days, at least 1 | 90 days |
| `retention.keep_runs_per_job` | Always keep per command | The newest runs of each command stay whatever their age, so estimates survive long breaks. Estimates read the newest 20. | Count, at least 1, "runs" | 20 runs |
| `retention.max_job_runs` | Most runs kept | Beyond this, the oldest runs are deleted, whatever the command. | Count, at least Always keep per command, "runs" | 5,000 runs |

Live: "Keeps ended leases for {30 days} and job runs for {90 days}. Each command keeps at least
its newest {20} runs, and never more than {5,000} runs are stored."

### Service

**Service** (status row, no key)

| State | Text | Button |
|-------|------|--------|
| Running | "Running · version {0.5.0} · port {47823}" | Stop service |
| Stopped by you | "Stopped. Heavy commands run without leases until you start it." | Start service |
| Starting | "Starting…" | (busy, disabled) |
| Not answering | "Not answering on port {47823}." | Start service, Open logs |

Stop asks only while leases are active: title "Stop the service?", body "{2} jobs hold leases.
They keep running, and new heavy commands run without leases until you start it again.",
buttons **Stop service** and **Cancel**. (M2.1 keeps it stopped.)

**Connection**

| Key | Label | Help | Control, range | Default |
|-----|-------|------|----------------|---------|
| `hook.fail_open` | Let heavy commands run while the service is down | When off, every heavy command is blocked until the service answers again. | On/off | On |
| `daemon.port` (Advanced) | Port | The service listens on this port on 127.0.0.1. Change it only if another app needs the port. | Port, 1 to 65535 | 47823 |

**Linux** (GNOME and KDE only)

| Key | Label | Help | Control | Default |
|-----|-------|------|---------|---------|
| `linux.scopes` | Run each lease in its own systemd scope | Gives exact CPU and memory use, and a kill that ends every process at once. Needs cgroup v2 and a systemd user session. | On/off | On |
| `linux.enforce_cpu` | Cap jobs at their reserved cores | A job that outgrows its lease is slowed down, not stopped. | On/off; disabled with "Needs systemd scopes." while scopes are off | Off |
| `linux.enforce_memory` | Slow down jobs above their reserved memory | Memory above the reservation is throttled. A job is never stopped for memory. | On/off; same | Off |

**Files**: a row "Config file" with the path (`~/.allot/config.toml` in the platform's form) and
**Open file** plus **Show in folder** (Windows: Show in File Explorer; macOS: Show in Finder).

**Reset**: **Reset all service settings…** (section 6).

### Panel (each panel's own store)

**Meters** (help under the group: "Rows shown in the {flyout}.", where {flyout} is flyout on
Windows, panel on macOS and popup on GNOME and KDE)

| Setting | Label | Default |
|---------|-------|---------|
| `meters.cpu` | CPU | On |
| `meters.ram` | RAM | On |
| `meters.gpu` | GPU | On |
| `meters.vram` | VRAM ("GPU memory" where the GPU shares system memory) | On |
| `meters.drives` | Drives | On |
| `meters.drive_activity` | Drive activity (indented under Drives, disabled while Drives is off) | On |

**Notifications**

| Setting | Label | Help | Default |
|---------|-------|------|---------|
| `service_stopped` | Service stopped | When the service stops without you stopping it. | On |
| `running_long` | Running long | When a job runs 1.5 times longer than expected. | On |
| `benchmark_finished` | Benchmark finished | When a benchmark you started ends. | On |
| `job_finished` | Job finished | When any job ends. | Off |
| `killed_outside_flyout` | Killed outside the {flyout} | When a job is stopped by something other than this {flyout}. | Off |

When the OS blocks notifications for Allot, a banner at the top of the group: "Notifications for
Allot are off in {Windows Settings}." with **Open notification settings** (on GNOME and KDE, the
existing do-not-disturb line instead). The toggles stay editable.

**Startup** (Windows, macOS): "Launch at logon" (Windows) / "Launch at Login" (macOS), help
"Start Allot when you sign in." Moves here from the flyout.

**Text size** (macOS): Standard, Large, Larger (segmented control), as today.

Panel settings save at once to the panel's store (no 600 ms batch, no allotd needed). Reset page
on Panel resets only these.

---

## 5. Apply model (D2)

- **Single values** save 600 ms after the last change, as one `PATCH` with the revision for every
  change in that burst (the spikes' model). A number counts as changed when it commits (Enter,
  leaving the field, a spin step), so typing "22" is one change.
- **Classes, presets and rules** save from their dialog's Save: the dialog checks the row, then
  one `PATCH` writes the whole table or list. Cancel and Escape drop the dialog's changes.
- **Chips** (print-only flags) save on add or remove.
- **Save status** sits at the bottom of the navigation pane (Windows, KDE), the window's bottom
  bar (macOS) or the header bar subtitle (GNOME), as text read politely by screen readers:
  "All changes saved" · "Saving…" · "1 change not saved". No spinner, no animation.
- **Closing** sends a pending burst first (proven on GNOME and KDE B; Windows and macOS do the
  same). Closing with a dialog open cancels the dialog.
- **Reopening** (D14): Windows, macOS and KDE keep the window hidden, so the same page and scroll
  come back; values are fetched again on every show. GNOME builds a new window and opens on Panel.
  The window shows at once and loads in the background (GNOME must never wait for allotd in
  `fillPreferencesWindow`).
- **External changes**: while the window is open, a change to the file by hand or by another
  window shows on the next fetch. The panels fetch on focus and every 5 s while visible. A
  changed value updates in place; nothing announces it unless it collides with a pending edit
  (section 7).

---

## 6. Reset

| Level | Where | Confirmation | Effect |
|-------|-------|--------------|--------|
| Field | Reset button in the row, only when changed | None; toast "Headroom reset to 15 %." with Undo | Removes the key |
| Row (class, preset) | Row action and dialog footer | None; toast with Undo | Removes that table's keys |
| Page | **Reset page…** at the bottom of each page | "Reset {3} settings on Admission?", body lists them with their defaults, button **Reset {3} settings** | Removes the page's keys |
| All | Service page, **Reset all service settings…** | "Reset all {12} changed settings?", body "config.toml goes back to the defaults. Your rules and presets are replaced by the built-in ones, and presets you added are deleted. Comments next to these keys are removed too. This can't be undone.", button **Reset {12} settings** | Removes every key allotd owns; the file keeps comments elsewhere |

Reset page and Reset all are disabled, with "Nothing to reset", when no value on them differs
from its default. Reset all leaves panel settings alone; Panel has its own Reset page.

---

## 7. States

All lasting states use a banner at the top of the page area (InfoBar, `Adw.Banner`,
`Kirigami.InlineMessage`, a SwiftUI banner view). Toasts only for short notices with Undo.

| State | Trigger | Shows | Actions |
|-------|---------|-------|---------|
| Loading | First fetch takes over 300 ms | Rows show their labels with empty controls, disabled; status "Loading settings…" | none |
| Field error (client) | Value outside the spec's range | Under the field, error style plus icon: "Enter a number from 0 to 99." Status "1 change not saved" | Fix it, or Escape to restore the saved value |
| Cross-field error | `max_quiet_wait_secs` < `quiet_secs`, `max_job_runs` < `keep_runs_per_job`, `min_cores` > `cores` | Under the dependent field: "Must be at least Quiet for (10 seconds)." | same |
| Field error (allotd) | `PATCH` refused (422) | Under the field: allotd's reason, rewritten by M1.1's field errors ("Headroom must be below 100 %."). The typed value stays; the file is untouched (D12) | Fix it, or **Undo** to restore the saved value |
| Restart needed | `daemon.port` saved | Under Port: "Restart needed. The service still listens on {47823}." Banner on Service: "Restart the service to use port {47999}." | **Restart service** |
| Hand-edit conflict | `PATCH` gets 409 | Banner: "config.toml changed outside Settings. Showing the file's values. Your change to {Safety margin} ({15 %}) was not saved." | **Apply my change**, **Keep file's values** |
| Invalid file | allotd reports the file rejected (it keeps the last good settings) | Banner (error): "config.toml has an error: {reason}. The service keeps using its last good settings. Fix the file to change settings here." allotd pages read-only | **Open file**, **Check again** |
| Stopped by you | M2.1 stop record | Banner: "The service is stopped. Start it to change these settings." allotd pages show the last fetched values, read-only; with none fetched, the page shows only the banner | **Start service** |
| Not answering | Request fails, no stop record | Banner (warning): "The service isn't answering on port {47823}. Trying again every 5 s." Read-only as above | **Try now**, **Open logs** |
| Token refused | 401 after one token re-read | Banner (error): "The service refused this panel's key. Restart the service to make a new one." | **Restart service** |
| Notifications off (Panel) | OS setting | see Panel | **Open notification settings** |

Panel page keeps working in every allotd state.

**Needs from M3.1:** `GET /settings` reports a rejected file with its reason, and the
restart-needed state for the port, so a reopened window shows both.

---

## 8. Live explanations (D10)

Every group with numbers ends in one sentence in the help style, built from the values on screen
(section 4 lists each). They:

- update 600 ms after the last change, from local math (benchmarks, leases, history, estimates
  example) or M2.2's preview (headroom amounts, fits);
- live in a polite live region that announces at most once per committed change, never per
  keystroke;
- use ICU plurals and the platform's number and duration formatting; never concatenate strings;
- say "Based on this machine now" when they use live load, so the number changing on its own is
  expected;
- show "Can't work this out while the service is stopped." while allotd is down (local-math
  sentences still show).

---

## 9. Shrunk menus

More options keeps three things, as decided at kickoff. Meters, Notifications and Launch at
logon move to the Panel page.

| Windows flyout ⋯ | macOS gear | GNOME panel menu | KDE context and popup menu |
|------------------|------------|------------------|----------------------------|
| Settings (Ctrl+,) | Settings… (⌘,) | Settings | Configure Allot… |
| Stop service / Start service | Stop Service / Start Service | Stop service / Start service | Stop Service / Start Service |
| — | — | — | — |
| Open logs | Open Logs | Open logs | Open Logs |
| | — Quit Allot (⌘Q) | | |

- Opening Settings while it is open raises and focuses it (Windows: the hidden window shows;
  macOS: `makeKeyAndOrderFront`; GNOME: find the window by class `org.gnome.Shell.Extensions`
  and title, `Main.activateWindow`, and call `openPreferences()` only when none is open; KDE:
  the same window's `raise()` and `requestActivate()`).
- Errors in the flyout that a setting can fix link to the page ("config.toml has an error" opens
  Service, the token banner opens Service). The flyout's first-run Launch at logon prompt stays
  where it is.
- The Windows tray icon's right-click menu gets **Settings** above its existing items.
- Wording follows each platform (D15): title case on macOS and in KDE menus (as the menus are
  today), sentence case on Windows and GNOME.

---

## 10. Platform adaptation

### Windows (the reference; Fluent)

- A second WinUI `Window`, built once and hidden on close (M0.2 finding 3). No x:Bind in the
  window's XAML; bind inside pages (finding 2). Own `OverlappedPresenter` (finding 6).
- `NavigationView`, left pane, 7 items with Fluent icons and labels; pane collapses to compact
  under 900 px window width and to minimal under 640 px. Default size 920 x 680 DIPs, minimum
  640 x 500 (**Revisit R2** for DPI).
- Backdrop Mica (**Revisit R3**); content on cards (`CardBackgroundFillColorDefaultBrush`,
  `CardStrokeColorDefaultBrush`), one card per group, rows inside. A small `SettingRow` user
  control in `Allot.Tray`, not CommunityToolkit's `SettingsCard` (no new dependency).
- Page header: page title (`TitleTextBlockStyle`), one intro line.
- Dialogs: `ContentDialog`. Banners: `InfoBar` at the top of the page. Brief notices with Undo:
  an `InfoBar` pinned to the bottom of the page that closes after 10 s.
- Opening from the flyout or tray menu brings it to the front (**Revisit R1**).
- Theme: follows Windows light and dark and high contrast through stock theme resources only.

### macOS (SwiftUI, AppKit host)

- `NSWindow` with an `NSHostingController`, `isReleasedWhenClosed = false`, frame autosave
  (M0.2 research). Toolbar tabs (D8) with SF Symbols: Panel `gauge.with.dots.needle.33percent`,
  Admission `scalemass`, Sizes `square.stack.3d.up`, Commands `terminal`, Benchmarks `timer`,
  Leases `key`, Service `server.rack`. The window resizes per page.
- `Form` with `.formStyle(.grouped)`; help text as `.font(.callout)` secondary; keys in
  `.monospaced()`.
- Tables: SwiftUI `Table`; edit in a `.sheet`. Banners: a view at the top of the form.
- Title case for buttons and menu items ("Reset Page…", "Start Service"); sentence case in
  labels and help.
- Dock and Cmd-Tab while open, and the activation order: **Revisit R4**. A main menu for Cmd-W,
  copy and paste in fields: **Revisit R5**.

### GNOME (libadwaita prefs)

- `prefs.js` fills the shell's `ExtensionPrefsDialog`; 7 `Adw.PreferencesPage`s in the stock
  view switcher (D7, **Revisit R8**); default 760 x 680. Groups are `Adw.PreferencesGroup`s with
  their live sentence as the group's description at the bottom (a `Gtk.Label` with `dim-label`).
- Numbers: `Adw.ActionRow` plus `Gtk.SpinButton`, not `Adw.SpinRow` (**Revisit R7**).
- Tables become rows: one `Adw.ActionRow` per class or preset, subtitle "8 cores, at least 4 ·
  4 GB · GPU 0 % · 5 min · fits 1, 2 more shrunk"; Edit opens an `Adw.Dialog`.
- Banners: `Adw.Banner` at the top of the page; toasts: `Adw.Toast` with Undo, dismissing the
  previous one first (M0.3 finding 5).
- Reset page: an `Adw.ButtonRow` at the bottom of each page.
- Reopen starts on Panel (**Revisit R9**). One prefs window for all extensions (**Revisit R6**).
- Panel settings write `tray.json`; the shell follows through its `Gio.FileMonitor` (M0.3
  finding 7).

### KDE (Kirigami window, D1)

- `Kirigami.ApplicationWindow` created by the widget, hidden on close. Left: a list of the 7
  pages (icons from Breeze: `speedometer`, `view-statistics`, `view-list-details`,
  `utilities-terminal`, `chronometer`, `password-copy`, `network-server`), right: the page, a
  `Kirigami.FormLayout` per group, `Kirigami.Separator` between groups. Default 900 x 640.
- CheckBox for on/off (KDE form convention). SpinBox edits reported from `valueChanged` with a
  guard (M0.3 KDE finding 6, **Revisit R11**).
- Tables become rows as on GNOME; Edit opens a `Kirigami.Dialog`. Banners:
  `Kirigami.InlineMessage`; toasts: `showPassiveNotification` with Undo.
- Title case for buttons and menu items, as the widget's menus are today.
- Focus from a real click on Wayland: **Revisit R10**. Plasma 6.0 to 6.7: **Revisit R12**.

---

## 11. Accessibility floor

Checked against the mockups (section 13) and required of every panel:

- **Contrast**: native controls carry the platform's contrast; Allot adds no colors in the
  window. The live sentence uses the secondary text style, at least 4.5:1 on its card.
- **Keyboard**: every control and row action reachable by Tab; arrow keys within the navigation,
  lists and sliders; Alt+Up and Alt+Down reorder rules; Escape closes a dialog; Ctrl+, (⌘, on
  macOS) opens Settings from the flyout. Rules can be reordered without dragging.
- **Focus order**: navigation, page title, banners, then groups top to bottom, row by row, label,
  control, reset. A dialog takes focus on its first field and returns it to the row's Edit
  button.
- **Screen readers**: name = label; description = help plus key; reset buttons say what and to
  what; the matched rule says "Matches"; errors are announced and tied to their field; the save
  status and live sentences are polite live regions.
- **No color-only meaning**: errors have an icon and text; changed values show a reset button;
  the matched rule has text.
- **Targets**: at least 24 x 24 px (the platforms' own controls are larger).
- **Text scaling**: rows wrap at 200 % text; nothing truncates a label.
- **Motion**: no animation beyond the platform's own control transitions.
- Spike findings that bear on it: GNOME spin rows missing from AT-SPI (R7), KDE's
  `valueModified` (R11).

### Checks run on the mockups (2026-11-20)

- Contrast, WCAG 2.2 formula in the browser, every token pair in both themes: all pass. Lowest:
  danger text on the error banner 4.79:1 (light), danger on cards 5.66:1, secondary text on the
  page 5.41:1, input borders 3.45:1 (light) and 5.03:1 (dark).
- Keyboard: rule menu opened with Enter, moved with arrow keys, Move down applied; Alt+Up moved
  the rule back with focus following it; dialogs take focus on their first field, Escape closes.
- All 10 states in the switcher render with no console errors; read-only states disable allotd's
  controls and leave Panel editable.
- No page-level horizontal scroll at 800 px (one escaping hidden header fixed).
- Not run: the wizard's `contrast.py`, `lint_ui.py` and `copy_lint.py` (no Python on this PC),
  screen readers, and 200 % text. M7.1 covers screen readers on each platform.

---

## 12. Copy rules for this window

- Sentence case, except macOS buttons and menus and KDE buttons and menus (title case).
- Buttons name the action: Save, Add rule…, Reset 3 settings, Use built-in rules, Start service.
  "…" only when a dialog follows.
- Errors say what is wrong and what is allowed, next to the field; no codes alone; no blame.
- "Service" in the UI, "allotd" only where the user needs the process name (logs, the help of
  Port). "Lease", "class" and "preset" stay: they are the words the CLI and MCP tools use.
- Numbers, units, plurals and durations through the platform's formatter; ports never grouped.

---

## 13. Mockups

`design/settings/mockups/` (static HTML, no dependencies; open `index.html`):

- `windows.html`: the Windows reference, all 7 pages, the edit and rule dialogs, the test box
  working against the built-in rules, and a state switcher for every state in section 7.
- `platforms.html`: the Admission page on macOS, GNOME and KDE.
- `menus.html`: the shrunk menu on each platform.

They approximate the native look to show structure, copy and states. They are not a style to
copy pixel for pixel; each panel uses its own platform controls.

---

## 14. Revisit list

| ID | Check | Where in this design | If it fails |
|----|-------|----------------------|-------------|
| R1 | Windows focus (CHECKLIST-windows 1 to 6) | 9, 10 Windows | Open from the menu with `AllowSetForegroundWindow` and `SetForegroundWindow`; if still behind, flash the taskbar button and say so in the flyout |
| R2 | Windows DPI (13 to 15) | 10 Windows sizes | Store minimum size in DIPs and reapply per DPI change |
| R3 | Mica (16, 17) | 10 Windows | Mica Alt or solid `SolidBackgroundFillColorBase` |
| R4 | macOS Dock policy and activation order (2, 3, 6, 7, 8) | 10 macOS | Stay `.accessory` if the switch is flaky; pick the order that lands in front |
| R5 | macOS keys without a main menu (9, 10) | 10 macOS, Commands test box | Install a minimal main menu while the window is open |
| R6 | GNOME raise and the one-window rule (2, 3, 4) | 9 | Show a notice "Close the other extension's settings first" when another extension's window is open |
| R7 | GNOME spin rows and Orca (12, Orca) | 3 controls, 10 GNOME | Keep ActionRow + SpinButton (current choice); switch to SpinRow only if Orca reads it |
| R8 | GNOME 7 pages in the view switcher at 760 px (look, 13) | D7, 10 GNOME | Fall back to one page of navigation rows (`push_subpage`) |
| R9 | GNOME reopen (rebuilt window, 13) | 5, D14 | Keep opening on Panel; module state does not survive every reopen (M0.3), so no last-page memory |
| R10 | KDE focus from a real click on Wayland (1 to 4) | D1, 10 KDE | Pass the click's XDG activation token; if focus still fails, reopen D1 |
| R11 | KDE AT-SPI value edits (6) | 3, 10 KDE | Keep the `valueChanged` guard |
| R12 | Plasma 6.0 to 6.7, GNOME 48, 49, 51 (never run) | 10 | Per-version workarounds at M6 |
| R13 | macOS spike never compiled | 10 macOS | Fix at M5.1 |

---

## Change log

- 2026-11-20: First draft (M0.4), approved by Mehran the same day.
