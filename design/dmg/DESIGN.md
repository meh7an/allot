# Allot DMG window: design

Status: light path, locked by Mehran (2026-11-21). Decision log: `design/dmg/_archive/decision-log.yaml`.
Source art: `design/dmg/background.svg`.

The window that opens from `Allot-<version>.dmg` on macOS: Allot.app on the left, the Applications
folder on the right, and one job, dragging the app across. The app inside carries `allot`,
`allotd` and `allot-mcp`, so installing it this way updates the service too.

---

## 1. Decisions

| # | Decision | Choice | Chosen by |
|---|----------|--------|-----------|
| D1 | Build tool | dmgbuild: writes the window layout without scripting Finder, so it runs on a headless CI runner | Mehran |
| D2 | Look | Taken from the app icon: its near-black tile and its stacked capsule bars, violet at the top to sky at the bottom | Mehran |
| D3 | Direction | F3, the live floor: a floor plane of capsules (leases) under a horizon, a few lit, one slot reserved under Applications | Mehran, after directions A to G |
| D4 | Wordmark | None; the window title and the icon carry the name | Mehran |
| D5 | Tone | Graphite `#17181C`, the icon tile's family | Mehran |
| D6 | Labels | A grey pill (`#74757B`) behind each label, because Finder draws labels black in Light Mode and white in Dark Mode over the same picture | Design lead (hard floor) |

## 2. Layout (points, inside the window below the title bar)

| Item | Value |
|------|-------|
| Window content | 640 x 400 |
| Icon size | 128 |
| Allot.app center | (190, 144) |
| Applications center | (450, 144) |
| Label text size | 13 |
| Label text, as Finder draws it | cap top 222.5, baseline 232, descenders to 234 (measured on a Mac) |
| Label pills | Allot: x 160, y 216, 61 x 22; Applications: x 394, y 216, 113 x 22; radius 11; centered on the label text, 6.5 above the caps and 4 below the descenders |
| Arrow | four capsules at y 140, height 8: x 276 w 10, x 296 w 13, x 316 w 16, x 336 w 19 |
| Horizon | y 290, 1 pt `#2A2C33`; floor below it `#1B1C21` |
| Floor rows | y 298, 309, 322, 337, 355; heights 6, 7, 8, 9, 11 (nearer is larger) |
| Allot's lease | front row, x 150 w 80, lit |
| Reserved slot | front row, x 412 w 76, dashed outline |

The slot sits about 117 pt below the Applications label's pill, inside the floor, so it reads as part
of the picture and not as a place to drop; Finder accepts the drop on the folder only.

## 3. Palette

| Role | Value | Notes |
|------|-------|-------|
| Field | `#17181C` | Window background |
| Floor | `#1B1C21` | One step lighter than the field |
| Horizon | `#2A2C33` | 1 pt line |
| Label pill | `#74757B` | 4.57:1 with black, 4.59:1 with white (computed by hand from WCAG relative luminance; `contrast.py` not run) |
| Row 1 (back) | `#9294F6` at 10% | The icon's top bar |
| Row 2 | `#8DA2FA` at 13% | |
| Row 3 | `#87AEFD` at 16% | |
| Row 4 | `#80BCFF` at 19% | |
| Row 5 (front) | `#7DC6FF` at 22% | The icon's bottom bars |
| Lit lease | its row's color at 92 to 95%, with a white 1.2 pt top highlight at 45% | |
| Reserved slot | `#7DC6FF` at 8% fill, 1.5 pt dashed outline (5, 4) at 85% | |

The colors were read from `apps/macos/Allot/Assets.xcassets/AppIcon.appiconset/icon_512x512.png`
by eye, not sampled; check them against the icon file before the first build.

## 4. Capsule anatomy

- Free lease: a fully rounded bar in its row's color, dim.
- Live lease: the same bar nearly opaque, with a thin light line along its top edge.
- Reserved slot: an empty bar, a faint fill and a dashed outline.

## 5. Accessibility and platform

- Label contrast passes in both appearances through the pills (D6). Nothing in the art carries
  meaning a label does not also give; the picture has no text, so nothing needs translating.
- The background ships as a multi-resolution TIFF from `background.png` (640 x 400) and
  `background@2x.png` (1280 x 800), so it is sharp on Retina displays.
- The DMG window hides the toolbar, sidebar, path bar and status bar; icon view, no arrangement.

## 6. Checks still to run (manual, on a Mac)

1. The label pills sit under Finder's labels at text size 13 in both Light and Dark Mode, and
   neither label overflows its pill. The first run, on the 0.6.1 DMG, found the text 7 pt above
   the pills' centers; the pills moved up (change log, 2026-11-22), so run it again.
2. The palette matches the icon file (section 3).
3. The window opens at 640 x 400 with nothing to scroll on a 13-inch display.

## Change log

- 2026-11-21: first version (light path).
- 2026-11-22: label pills up 7 pt, from y 223 to y 216, after Mehran's Mac showed the text over
  their top halves. Measured from his 2x screenshot of the 0.6.1 DMG: cap top 222.5, baseline 232,
  so the text centers on 227.25 and the pills now center on 227. Height stays 22: the text with
  descenders spans 12 pt and keeps at least 4 pt clear on each side.
