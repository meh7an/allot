# M0.2 findings: a Settings window on Windows and macOS

Throwaway spike for the Settings roadmap (`plan/settings/`). M4.1 deletes `windows/` and M5.1
deletes `macos/` once the real windows exist. Manual results go in `CHECKLIST-windows.md` and
`CHECKLIST-macos.md`, and their summary in `plan/settings/PROGRESS.md`.

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
