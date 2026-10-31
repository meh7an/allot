# Allot brand assets

The Allot mark: seven rounded bars on a 16-unit grid, stacked into an A like a Gantt chart of
allotted jobs. Ultramarine palette.

## Sources

The mark as designed. Edit these, never the generated files.

| Folder | Contents |
|--------|----------|
| `svg/` | Vector, view box 16 (transparent) or 24 (with background) |
| `png/` | `-1x` 128, `-2x` 256, `-3x` 384, `-4x` 512, `-8x` 1024 px |
| `lottie/` | Appearance animation, 512 x 512, 60 fps, 110 frames (1.83 s), shapes only |

Names are `allot-mark-{color|mono}-{light|dark}[-bg]`:

- `light` is for light backgrounds (ink or deep ultramarine bars), `dark` for dark ones (white or
  bright ultramarine bars).
- `-bg` adds a square background (white `#FFFFFF` or ink `#16171B`) with the mark at 58%.
- Without `-bg` the background is transparent and the mark fills its 16-unit square with one unit
  of margin.

| Row (top to bottom) | Light backgrounds | Dark backgrounds |
|---------------------|-------------------|------------------|
| 1 | `#5550C0` | `#8E93F3` |
| 2 | `#485DC5` | `#87A0FA` |
| 3 | `#3C69C7` | `#84ADFD` |
| 4 | `#2B75C8` | `#80B9FF` |
| 5 | `#1380C7` | `#7FC5FF` |

Mono: ink `#16171B`, white `#F4F4F6`.

## Generated

`build.py` (Python 3, standard library only) draws every bitmap from the mark's geometry. Rows
snap to a whole-pixel height and gap and edges round to whole pixels, so 16 to 48 px stay sharp.
Run it after changing the geometry or colours:

```bash
python assets/brand/build.py
```

| Output | Use |
|--------|-----|
| `tray/src/Allot.Tray/Assets/Tray/allot-{idle,busy,benchmark,queued,down}-on-{light,dark}-taskbar.ico` | Windows tray icon, 16 to 256 px frames; the glyph follows the taskbar theme |
| `tray/src/Allot.Tray/Assets/Tray/allot-*-256.png` | Toast logos for the same states |
| `tray/src/Allot.Tray/Assets/AppIcon.ico` | Windows app: exe, setup, Start menu, toast registration |
| `web/favicon.ico`, `web/favicon.svg` | Favicon; the SVG follows the light or dark scheme |
| `linux/hicolor/` | Linux app icon, PNG sizes and scalable SVG |
| `linux/symbolic/allot-{state}-symbolic.svg` | Panel icons for GNOME and KDE; badges use the theme's warning and error colours |
| `macos/AppIcon-1024.png` | macOS app icon source on Apple's 824 px body |
| `macos/menubar/allot-{idle,busy}-template.svg` | macOS menu bar template images |

Tray states: idle is the mark at half opacity, busy is solid, queued adds an amber badge, and down
is the mark at 35% with a red badge. The badge sits in the empty top-right corner of the A. The
Windows tray also has a benchmark state, and its badges carry the tray flyout's icons in its
colours: busy a green play badge, benchmark a purple stopwatch, queued an amber clock and down a
red cross. The Linux and macOS panel icons keep the four states with plain badges.

`social-preview.ps1` renders `github/social-preview.png` (1280 x 640) with System.Drawing and
Segoe UI, so it runs on Windows only:

```bash
powershell -NoProfile -File assets/brand/social-preview.ps1
```

GitHub takes the social preview through the web UI only: Settings, General, Social preview, Edit,
Upload an image.
