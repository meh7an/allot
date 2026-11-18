# M0.2 and M0.3 findings: a Settings window on every panel

Throwaway spike for the Settings roadmap (`plan/settings/`). M4.1 deletes `windows/`, M5.1
`macos/`, M6.1 `gnome/` and M6.2 `kde/` and `linux/`, once the real windows exist. Manual
results go in the `CHECKLIST-*.md` file of each platform, and their summary in
`plan/settings/PROGRESS.md`.

## Windows (`windows/`, verified on this PC 2026-11-18)

Setup: `dotnet new winui`, then the tray's own packages and properties (WinAppSDK WinUI 2.3.9,
Foundation 2.3.12, InteractiveExperiences 2.1.9, SDK BuildTools 10.0.28000.2705; unpackaged,
self-contained, PerMonitorV2, `DispatcherShutdownMode.OnExplicitShutdown`, Native AOT for
Release). It has its own exe (`allot-settings-spike.exe`) and its own instance key
(`Allot.SettingsSpike`), so it runs beside the real tray. State goes to
`%LOCALAPPDATA%\AllotSettingsSpike\`.

`allot-settings-spike.exe --selftest` covers the checks that need no hands. It passed on both the
Native AOT publish and the Debug (JIT) build. This PC has one display (1920x1080 at 100 %), so the
selftest says nothing about DPI; the checklist covers that.

1. **A second Window works under Native AOT.** These all work in the AOT publish:
   - opening, closing and opening again
   - NavigationView with `Frame.Navigate` pages
   - the TitleBar control with `ExtendsContentIntoTitleBar`
   - x:Bind on pages (two-way, function bindings, DataTemplates)
   - Mica, Mica Alt, Acrylic and no backdrop, set at runtime

   The publish has no trim or AOT warnings.
2. **x:Bind in a Window's own XAML leaks the Window** (microsoft-ui-xaml#9063, still present in
   WinUI 2.3.9, under JIT and AOT alike). Ten closed windows of each kind were checked:
   - a plain `Window`, a Window with a Grid and a Window with Mica: none stayed alive
   - a Window whose XAML has a single x:Bind: all 10 stayed alive

   Rule for M4.1: no x:Bind in the Window's XAML. Bind inside pages or a root UserControl, and set
   window-level text in code.
3. **Rebuilding the window leaks native memory; hiding it does not.** The full Settings window,
   without window-level x:Bind, was opened and closed through WM_CLOSE (as the title bar's X
   sends):
   - Destroy model: every managed window is collected, yet private memory grows by about
     1.3 MB per open, linearly (147, 173, 198, 224 MB at 20, 40, 60, 80 opens).
   - Hide model (cancel `AppWindow.Closing`, then `Hide`): 40 more opens cost 1 MB (227, 227,
     228).

   Which control leaks was not narrowed; a plain Window does not leak. Rule for M4.1: build the
   window once, cancel the close, and hide it. Its state (page, scroll, unsaved edits) then
   survives a close, which M0.4 has to account for.
4. **Opening cost (rough, not measured under a benchmark lease).** The first Settings window
   loads in about 170 to 450 ms (the first run includes loading XAML). Each later new window
   loads in about 115 to 130 ms. A plain WinUI window takes private memory from 12 MB to about
   78 MB; the Settings window with NavigationView brings it to about 90 MB.
5. **CsWinRT AOT gotchas the real pages will hit:**
   - `ItemsSource` set to a collection expression (`IReadOnlyList<T> x = [..]`) throws
     `ArgumentException` ("Value does not fall within the expected range") under AOT. The
     compiler's own read-only type cannot be marshalled. Use `List<T>` or
     `ObservableCollection<T>`.
   - Types used in an x:Bind DataTemplate cannot have `init` setters: the generated
     `XamlTypeInfo.g.cs` assigns them (CS8852).
   - A function binding (`{x:Bind Summary(A, B)}`) must call an instance method, not a static
     one (CS0176 in the generated code).
6. **The presenter.** In the first selftest run, `AppWindow.Presenter is OverlappedPresenter` was
   false, so the minimum size and the remembered position were skipped without any error. Every
   later run (AOT and JIT) saw true, and the first result was not reproduced. Rule for M4.1: create
   the presenter (`OverlappedPresenter.Create()`, `AppWindow.SetPresenter`) and keep it, as the
   flyout does.
7. **Placement.**
   - Saving: store the top-left corner in physical pixels and the size in DIPs, taken from
     `AppWindow.Changed` while the presenter is Restored, so a maximized window keeps its
     restored rect.
   - Restoring: `DisplayArea.GetFromRect(.., Nearest)` picks the display, the size is computed
     for that monitor's scale (`GetDpiForMonitor`), and the rect is clamped into its work area.
   - The close-and-reopen round trip is exact on one display. A move between monitors with
     different scales is not tested here, and the log reports whether the move changed the size a
     second time.
   - `PreferredMinimumWidth/Height` are physical pixels and start out null. Whether WinUI rescales
     them on a DPI change is a checklist item; the log records the old value before each reapply.
8. **Closing never quits.** With `OnExplicitShutdown`, closing the Settings window leaves the
   process running, under both close models.
9. **Focus.** The selftest does not call `SetForegroundWindow`, so it would not steal focus. A
   process started in the background (as this shell does) does not get the foreground from
   `Activate()` alone. Which open paths do get it (the flyout menu, a second launch with
   AllowSetForegroundWindow, a timer while another app is in front) is checklist work. The
   tray-icon menu path is not built in the spike. The tray calls `SetForegroundWindow` on its own
   window before `TrackPopupMenu`, so a menu choice starts from the same foreground state as the
   flyout path.
10. **Publishing** needs `vswhere.exe` on PATH (in
    `C:\Program Files (x86)\Microsoft Visual Studio\Installer`), as the tray's csproj says.
    Without it, ILCompiler's link step fails with exit code 3.

## macOS (`macos/`, not built yet: needs Mehran's Mac)

What the research found (sources in the M0.2 session note):

- Apple documents no rule for whether a menu bar app's Settings window shows in the Dock and
  Cmd-Tab. The common practice (steipete's write-up, several apps, Developer Forums answers) is
  to switch to `.regular` while the window is open and back to `.accessory` when it closes.
- `NSApp.activate()` (macOS 14+) only asks for activation and does not guarantee it. Forum
  threads report the window opening behind other apps, or not activating about 20 % of the
  time, even after the switch.
- The SwiftUI `Settings` scene needs a SwiftUI `App` (the real app has none), and
  `showSettingsWindow:` stopped working in macOS 14. The spike therefore uses an `NSWindow` with
  an `NSHostingController`, `isReleasedWhenClosed = false`, and `setFrameAutosaveName`.
- Key equivalents (Cmd-W, Cmd-Q, Cmd-C, Cmd-V) come from the main menu, which an accessory app
  does not show. The spike can install one or leave it out, so the checklist can tell whether
  copy and paste work in the window's text field without it.

The spike's stand-in panel is borderless and non-activating, like `AllotPanel`, so Settings is
opened from the same state as in the real app. Its switches compare the two Dock policies and
three activation orders.

## GNOME and KDE (M0.3, 2026-11-18)

The pieces:

- `linux/mock-settingsd.py` stands in for the settings routes M3.1 will add (`GET` and `PATCH
  /settings` with a revision, 409, 422, 401, and slow or down modes).
- `linux/settings-client.mjs` is the one client and field list both panels import. It is written
  in ES2016 syntax by hand, because QML's engine has no class fields, private members or async
  functions.
- `linux/atspi-drive.py` drives a window through AT-SPI for the smoke runs.
- `gnome/` is its own extension with a `prefs.js`.
- `kde/` is its own widget offering both hosts: the config dialog (A) and a window of its own (B).

The research came first; the smoke runs then confirmed or corrected it.

- `gnome/smoke.sh` ran in a headless GNOME Shell 50.1 (WSL Ubuntu 26.04).
- `kde/smoke.sh` ran in a virtual nested Plasma 6.8.3 session (Qt 6.12, KF 6.32) on royalserver.
- Both used throwaway HOMEs and stopped what they started.

Research claims the runs settled are marked "(run)".

### GNOME (`prefs.js`)

- **Versions.** Shell 48 ships libadwaita 1.7, 49 ships 1.8, 50 ships 1.9 and 51 ships 1.10. 51
  (2026-09-16) is the current stable series, and 50.5 is out. Every settings widget the pages need
  is in libadwaita 1.7, so all four supported versions have it: SwitchRow, SpinRow, EntryRow,
  ComboRow, ExpanderRow, ButtonRow (1.6), Banner, ToastOverlay, NavigationView, WrapBox and
  ToggleGroup (1.7). The nested shell used for earlier checks on this PC (WSL Ubuntu 26.04) runs
  50.1.
- **The API is the same on 48 to 51.** `fillPreferencesWindow(window)` may be `async`, which works
  since 47. `window` is the shell's `ExtensionPrefsDialog`, a subclass of `Adw.PreferencesWindow`
  (deprecated since libadwaita 1.6, but the shell still uses it), created with `search_enabled:
  false`. The shell throws "Extension did not provide any UI" when no page is visible after the
  fill. Close the window with `close()`, because `destroy()` shows an error page. `getSettings()`
  needs a GSettings schema, and the extension has none, which the spike does not need either.
- **Process.** `prefs.js` runs in the `org.gnome.Shell.Extensions` D-Bus service, a GJS process on
  the host, not in a sandbox. That holds even when it is opened from the Flatpak Extension Manager,
  which only calls `LaunchExtensionPrefs` on that service. (run) Reading `~/.allot/token` and
  calling 127.0.0.1 with libsoup work from it.
- **Lifecycle.**
  - A new dialog is built for every open, so a Windows-style "hide and reuse" model is not
    possible: each reopen starts fresh, and M0.4 has to plan for that.
  - The module stays cached while the service lives and is loaded fresh after the service exits
    on idle. Module-level state therefore survives some reopens but not others, so it must not be
    relied on. (run) A reopen 1.5 s after a close reused the process and the module; one 3 s
    later got a new process.
  - Nothing remembers the window size. `set_default_size` works, but gnome-shell#7201 (Shell 45)
    reports it ignored on the first open after login. (run) On 50.1 the first open had the asked
    720 x 640.
- **One prefs dialog at a time, for all extensions.** `openPreferences()` while any prefs dialog is
  open fails with "Already showing a prefs dialog", and the open window is neither raised nor
  focused. (run) On 50.1 `openPreferences()` returns `undefined`, and the refusal surfaces as a
  Gjs "Unhandled promise rejection" warning in the shell's log. A second click on a plain
  Settings item therefore does nothing visible.
- **Shared modules.** The review guidelines forbid Clutter, Meta, St and Shell imports in the prefs
  process, and Gtk, Gdk and Adw imports in the shell. `daemon-io.js` imports only Gio, GLib and
  Soup, and `lib/client.js` and `lib/config.js` import nothing, so the prefs process can reuse all
  three. The guidelines say nothing against network access, reading files outside the extension
  folder, or `Gio._promisify`.
- **Two writers of `tray.json`.** The panel's own settings live in `~/.allot/tray.json`, which the
  shell process writes. A prefs page that edits them would be a second process writing the same
  file. The shell would have to watch the file, or the page would have to hand its changes to the
  shell. The spike tries a `Gio.FileMonitor` in the shell.

### KDE (Plasma 6 widget)

- **Versions.** Plasma 6.8 (2026-10-14, now 6.8.3) is current. 6.7.6 and 6.6.7 are still being
  patched. The Qt and Frameworks minimums, read from each tag's `CMakeLists.txt`:

  | Plasma | Qt   | KF   |
  |--------|------|------|
  | 6.0    | 6.6  | 6.0  |
  | 6.4    | 6.8  | 6.14 |
  | 6.5    | 6.9  | 6.18 |
  | 6.6    | 6.10 | 6.22 |
  | 6.7    | 6.10 | 6.26 |
  | 6.8    | 6.11 | 6.30 |

  Plasma 6.8 retires the X11 session, and Kubuntu 26.10 ships 6.7. Other distributions were not
  checked. The widget declares `X-Plasma-API-Minimum-Version` 6.0. royalserver (Manjaro) runs the
  nested Plasma session that earlier checks used.
- **A. The widget's config dialog** (`contents/config/config.qml`, `main.xml`, read from
  plasma-desktop's `AppletConfiguration.qml`):
  - The widget has no `contents/config/` today. Adding one gives it "Configure Allot..." and two
    built-in pages, Keyboard Shortcuts and About.
  - Plasma 6 expects a page root from `org.kde.kcmutils` (`KCM.SimpleKCM` and its relatives).
  - Each `main.xml` key arrives as a `cfg_<key>` property. Apply copies the `cfg_` values to
    `Plasmoid.configuration` and writes the file, after calling the page's own `saveConfig()` if
    it has one. So `saveConfig()` is where a page would send the batch to allotd.
  - Apply turns on when a `cfg_` value differs, or when the page emits `configurationChanged()`.
    That signal is all that 6.0 has. Later versions also take a page property `unsavedChanges` and
    mark the signal for removal in Plasma 7. Which version added `unsavedChanges` was not
    verified, so a page that keeps 6.0 support emits the signal.
  - Closing with Apply on asks Apply, Discard or Cancel; switching pages asks first too. Pages
    seem to be recreated on every switch (`pageStack.replace`), so state belongs in the widget,
    not the page. That was inferred from the code and not tested.
  - The dialog is a window in plasmashell's process and remembers its size. Pages can use
    `Plasmoid`. (run) `Plasmoid.rootItem` is undefined in a page, so a page cannot reach the
    widget's JS objects; XMLHttpRequest (including PATCH) and the `executable` data engine work
    inside a page.
  - It opens with `Plasmoid.internalAction("configure").trigger()`, which also collapses the popup.
    The action is off when widgets are locked. Opening a specific page is not supported for
    widgets.
- **B. A window of the widget's own** (a QML `Window` or `Kirigami.ApplicationWindow` created inside
  plasmashell): no documentation, example widget or KDE guidance was found for or against it.
  KWin is tightening focus-stealing prevention on Wayland (bugs.kde.org 511015 and 500294), so
  such a window may open behind other windows. Its taskbar entry, icon, activation, lifetime and
  crash risk are all for the spike to find out.
- **How each fits the API.** A's Apply model maps naturally onto a single revision-checked `PATCH`
  sent from `saveConfig()`. Two things work against it: allotd's values are not `main.xml` keys,
  so every page has to drive Apply by hand, and live parts such as the fit preview have to live
  inside a page that is rebuilt on every switch. B is free of the dialog's rules, but nothing
  documents it.

### What the smoke runs showed

GNOME (`gnome/smoke.sh`, GNOME Shell 50.1, headless):

1. **It opens in front.** Settings from the panel menu opened focused and on top every time. Its
   window class is `org.gnome.Shell.Extensions` and its title is the extension's name; the GTK
   application id stays unset, and neither is set yet when `window-created` fires.
2. **Raise it yourself.** The extension can find its own prefs window among
   `global.get_window_actors()` (class and title, read after it settles) and
   `Main.activateWindow` it. That raised and focused it. M6.1's Settings item should do this
   first and call `openPreferences()` only when no window is open.
3. **Never wait for allotd in `fillPreferencesWindow`.** The window maps only after the fill
   resolves: with a 4 s allotd, a fill that awaited the load showed nothing at all for 4 s. Without
   the wait, the window appeared at once and loaded in the background. A first load took about
   450 to 550 ms in a new process (about 150 ms on a reused one), and the fill itself about 20 ms.
4. **Instant apply works.** Each burst of edits went out as one `PATCH` with the revision on
   screen, 600 ms after the last change.
   - A refused value came back with the server's reason in a toast and the old value.
   - A conflict showed the file's values with an "Apply Mine" toast, which re-sent the edit on the
     new revision.
   - Reset per field, the restart note for the port, a down allotd (rows greyed, Retry) and a
     refused token (one re-read of the token, then "refused") all behaved.
5. **Toasts queue.** A conflict toast waited behind an earlier six-second toast until the spike
   dismissed the previous toast before adding one. M6.1 should do the same, or use a banner for
   lasting states.
6. **An edit just before closing is sent.** On `close-request` the pending edit was flushed. Its
   `PATCH` completed after the window closed, before the service exited.
7. **The shell sees the panel's file.** A `Gio.FileMonitor` with `WATCH_MOVES` reported the prefs
   process's atomic write (CREATED, CHANGED, CHANGES_DONE_HINT) at once, and the shell's menu
   followed. So `tray.json` can be edited from Settings if the shell watches it.
8. **`destroy()` shows the "Extension Error" window,** as the shell's code says. Close with
   `close()`.
9. **Accessibility: `Adw.SpinRow` is missing from the AT-SPI tree.** On 50.1 (libadwaita 1.9),
   a group's list held only its switch row: the three spin rows and their spin buttons were not
   exposed at all. The same rows built as `Adw.ActionRow` with a `Gtk.SpinButton` suffix were
   exposed as "Headroom, spin button, 15" and could be set. M0.4 and M7.1 should check this with
   Orca before M6.1 picks spin rows.
10. **A shell warning outside the spike.** The headless shell logged `dash.js` "firstIcon.icon is
    null" once a prefs window was open (the Extensions service has no app icon there). It is
    unrelated to the spike.

KDE (`kde/smoke.sh`, Plasma 6.8.3 nested, virtual):

1. **Shared code works in QML.** The ES2016 client module imports into QML as `.mjs`. The token
   is read through the `executable` data engine and `XMLHttpRequest` sends `GET` and `PATCH`,
   when `send()` runs in a QML file.
2. **B (own window).**
   - `Kirigami.ApplicationWindow` created from the widget opened visible and active (from the
     spike's command hook, not a click).
   - All the write paths behaved as on GNOME, with a `Kirigami.InlineMessage` for conflicts.
   - On close it hides, and a reopen showed the same window and values; a pending edit was sent on
     `closing`.
   - It has its own taskbar entry.
   - `Plasmoid.configuration.x = value` from the window was in the appletsrc file at once.
3. **A (config dialog).**
   - Apply stayed off until the page emitted `configurationChanged()` and set `unsavedChanges`.
     Apply then called the page's `saveConfig()`, which sent the `PATCH`.
   - A category switch destroyed the page and built a new one, which loaded from allotd again.
   - Switching or pressing Cancel with unsaved edits asked Apply, Discard or Cancel.
   - Plasma added Keyboard Shortcuts and About pages.
4. **OK loses a direct write.** OK destroys the page right after `saveConfig()`, so the write it
   started (token read, then `PATCH`) never reached the server: no request was logged.
5. **Handing the write to the widget works.** `saveConfig()` set a hidden `main.xml` key
   (`pendingWrite`, JSON with a nonce), and the dialog wrote it on Apply or OK. The widget's
   binding on `Plasmoid.configuration.pendingWrite` sent the `PATCH` after the page was gone.
   Its outcome (ok, invalid, conflict) can then only be shown by the widget, not by the dialog.
6. **Qt and AT-SPI.** `SpinBox.valueModified` is not emitted when a value is set through AT-SPI's
   Value interface, so such an edit would be ignored. The spike reports edits from `valueChanged`,
   guarded while values from allotd are shown and before the first load. Page tabs list
   "Set Focus" before "Press" as their AT-SPI actions.

### The KDE choice (for Mehran)

| | A: config dialog | B: own window |
|---|---|---|
| Apply model | Apply, OK and Cancel; batches edits | Instant, like GNOME and the Windows reference |
| Errors after a write | After OK the page is gone; only the widget can tell the user | Shown in place (inline message), like the other panels |
| Writes on OK | Must go through the widget's config (hand-off) | n/a |
| Live parts (fit preview, command test) | Rebuilt and reloaded on every category switch | Built once and kept while hidden |
| allotd's values | Not `main.xml` keys; every page wires Apply by hand | Plain bindings |
| Comes for free | Keyboard Shortcuts and About pages, the standard dialog look | Nothing; its own layout, taskbar entry and title |
| Opening | "Configure..." in the context menu and `internalAction("configure")`; off when widgets are locked; no deep link to a page | Any menu item; can open a given page |
| Documented by KDE | Yes | No; it works in 6.8.3 nested, while focus from a real click on Wayland is checklist item 1 |
| Plasma 6.0 to 6.7 | Needs `configurationChanged()` (6.0 lacks `unsavedChanges`) | No dialog rules involved |

Recommendation: **B for allotd's settings.** It is the same model as every other panel, it can
show errors and live explanations, and it does not lose a write on OK. The widget's own settings
can stay in a small A dialog, or move into B. Mehran decides; the checklist's items 1 to 4 test
B's one open question, focus on a real Wayland session.

Sources: gjs.guide (anatomy, preferences, review guidelines, upgrading to 49, 50 and 51);
gnome-shell `js/extensions/prefs.js` (branches gnome-45 to gnome-51),
`js/dbusServices/extensions/extensionsService.js`, `extensionPrefsDialog.js` and
`js/ui/extensionSystem.js`; gnome-shell#7201, gjs#598; release.gnome.org/48 to /51 developer
pages; libadwaita class docs; mjakeman/extension-manager `src/local/exm-manager.c` and its Flathub
manifest; plasma-desktop `desktoppackage/contents/configuration/AppletConfiguration.qml` (v6.0.0
and master); libplasma `configview.cpp`, `configmodel.cpp`, `applet.cpp`; plasma-workspace
`CMakeLists.txt` at v6.0.0 to v6.8.0; develop.kde.org widget configuration and KF6 porting pages;
kde.org/announcements; bugs.kde.org 511015 and 500294.
