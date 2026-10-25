"""Builds Allot's icons from the mark's geometry.

Writes the Windows tray and app icons into the tray app, and the prepared Linux, macOS and
web assets into this folder. Standard library only; the output is the same on every run:

    python assets/brand/build.py

The mark is seven rounded bars on a 16-unit grid (svg/allot-mark-*.svg). Bitmaps snap the
bar edges to whole pixels, so small sizes stay sharp: rows keep a whole-pixel height and gap,
and horizontal edges round away from the centre to keep the mark symmetric.
"""

import math
import struct
import zlib
from pathlib import Path

BRAND = Path(__file__).resolve().parent
ROOT = BRAND.parents[1]
TRAY_ASSETS = ROOT / "tray" / "src" / "Allot.Tray" / "Assets"


def rgb(hex_color):
    value = hex_color.lstrip("#")
    return tuple(int(value[i : i + 2], 16) / 255 for i in (0, 2, 4))


# Bars as (left, right, row) in mark units; row n spans y = 1 + 3n to 3 + 3n.
BARS = [
    (6.5, 9.5, 0),
    (5, 11, 1),
    (3.5, 6.5, 2),
    (9.5, 12.5, 2),
    (2, 14, 3),
    (1, 4.5, 4),
    (11.5, 15, 4),
]

# Row colours, top to bottom, for light and dark backgrounds.
PALETTE = {
    "light": ["#5550C0", "#485DC5", "#3C69C7", "#2B75C8", "#1380C7"],
    "dark": ["#8E93F3", "#87A0FA", "#84ADFD", "#80B9FF", "#7FC5FF"],
}
INK = "#16171B"
WHITE = "#F4F4F6"

# The tray glyph follows the taskbar theme: "light" is a light taskbar, so a dark glyph.
TASKBAR = {
    "light": {"glyph": INK, "queued": "#C27A00", "down": "#C42B1C", "cross": "#FFFFFF"},
    "dark": {"glyph": WHITE, "queued": "#FCE100", "down": "#FF6B6B", "cross": "#1C1C1C"},
}
STATE_ALPHA = {"idle": 0.5, "busy": 1.0, "queued": 1.0, "down": 0.35}
STATES = list(STATE_ALPHA)

# The status badge sits in the empty top-right corner of the A, cut out of the mark by a
# one-unit ring.
BADGE = {"cx": 13.5, "cy": 2.5, "r": 2.5, "ring": 1.0, "arm": 0.95, "stroke": 0.95}

TRAY_SIZES = [16, 20, 24, 32, 40, 48, 64, 256]
APP_SIZES = [16, 20, 24, 32, 40, 48, 64, 256]
FAVICON_SIZES = [16, 32, 48]
HICOLOR_SIZES = [16, 22, 24, 32, 48, 64, 128, 256, 512]


# ---------------------------------------------------------------------------------------------
# Shapes, as signed distance functions in pixels
# ---------------------------------------------------------------------------------------------


class RoundRect:
    def __init__(self, x0, y0, x1, y1, radius):
        self.x0, self.y0, self.x1, self.y1 = x0, y0, x1, y1
        self.radius = min(radius, (x1 - x0) / 2, (y1 - y0) / 2)

    def bounds(self):
        return self.x0, self.y0, self.x1, self.y1

    def distance(self, x, y):
        half_w, half_h = (self.x1 - self.x0) / 2, (self.y1 - self.y0) / 2
        qx = abs(x - self.x0 - half_w) - half_w + self.radius
        qy = abs(y - self.y0 - half_h) - half_h + self.radius
        outside = math.hypot(max(qx, 0), max(qy, 0))
        return outside + min(max(qx, qy), 0) - self.radius


class Circle:
    def __init__(self, cx, cy, radius):
        self.cx, self.cy, self.radius = cx, cy, radius

    def bounds(self):
        r = self.radius
        return self.cx - r, self.cy - r, self.cx + r, self.cy + r

    def distance(self, x, y):
        return math.hypot(x - self.cx, y - self.cy) - self.radius


class Capsule:
    def __init__(self, x0, y0, x1, y1, width):
        self.x0, self.y0, self.x1, self.y1 = x0, y0, x1, y1
        self.half = width / 2

    def bounds(self):
        h = self.half
        return (min(self.x0, self.x1) - h, min(self.y0, self.y1) - h,
                max(self.x0, self.x1) + h, max(self.y0, self.y1) + h)

    def distance(self, x, y):
        dx, dy = self.x1 - self.x0, self.y1 - self.y0
        t = ((x - self.x0) * dx + (y - self.y0) * dy) / (dx * dx + dy * dy)
        t = max(0.0, min(1.0, t))
        return math.hypot(x - self.x0 - t * dx, y - self.y0 - t * dy) - self.half


# ---------------------------------------------------------------------------------------------
# Rasterizer
# ---------------------------------------------------------------------------------------------

SAMPLES = 16
OFFSETS = [(k + 0.5) / SAMPLES for k in range(SAMPLES)]


def coverage(shape, i, j):
    """The share of pixel (i, j) inside a convex shape: exact away from edges, sampled near them."""
    d = shape.distance(i + 0.5, j + 0.5)
    if d <= -0.75:
        return 1.0
    if d >= 0.75:
        return 0.0
    hits = sum(1 for oy in OFFSETS for ox in OFFSETS if shape.distance(i + ox, j + oy) <= 0)
    return hits / (SAMPLES * SAMPLES)


def render(size, ops):
    """Draws ("paint", shape, colour, alpha) and ("clear", shape) ops in order; straight RGBA."""
    pixels = [[0.0, 0.0, 0.0, 0.0] for _ in range(size * size)]
    for op in ops:
        shape = op[1]
        x0, y0, x1, y1 = shape.bounds()
        for j in range(max(0, math.floor(y0)), min(size, math.ceil(y1))):
            for i in range(max(0, math.floor(x0)), min(size, math.ceil(x1))):
                c = coverage(shape, i, j)
                if c == 0:
                    continue
                dst = pixels[j * size + i]
                if op[0] == "clear":
                    dst[3] *= 1 - c
                    continue
                src_a = op[3] * c
                out_a = src_a + dst[3] * (1 - src_a)
                for k in range(3):
                    dst[k] = (op[2][k] * src_a + dst[k] * dst[3] * (1 - src_a)) / out_a
                dst[3] = out_a
    return pixels


# ---------------------------------------------------------------------------------------------
# The mark, snapped to pixels
# ---------------------------------------------------------------------------------------------


def row_metrics(scale):
    """Whole-pixel row height and gap closest to the mark's 14-unit stack at 2:1."""
    target = 14 * scale
    best = None
    for height in range(1, math.ceil(2 * scale) + 3):
        for gap in range(1, height + 1):
            total = 5 * height + 4 * gap
            cost = (abs(total - target) + 1.5 * abs(height - 2 * gap), total)
            if best is None or cost < best[0]:
                best = (cost, height, gap)
    return best[1], best[2]


def snap(x, centre):
    offset = x - centre
    return centre + math.copysign(math.floor(abs(offset) + 0.5), offset)


def bar_rects(scale, left, top):
    """The bars of a mark whose 16-unit box starts at (left, top) at `scale` pixels per unit."""
    height, gap = row_metrics(scale)
    cx, cy = left + 8 * scale, top + 8 * scale
    first = math.floor(cy - (5 * height + 4 * gap) / 2 + 0.5)
    rects = []
    for x0, x1, row in BARS:
        sx0, sx1 = snap(left + x0 * scale, cx), snap(left + x1 * scale, cx)
        if sx1 - sx0 < 1:
            sx1 = sx0 + 1
        y = first + row * (height + gap)
        rects.append((RoundRect(sx0, y, sx1, y + height, height / 2), row))
    return rects


def badge_ops(scale, left, top, state, colours):
    cx, cy = left + BADGE["cx"] * scale, top + BADGE["cy"] * scale
    r = BADGE["r"] * scale
    ops = [("clear", Circle(cx, cy, r + BADGE["ring"] * scale))]
    ops.append(("paint", Circle(cx, cy, r), rgb(colours[state]), 1.0))
    if state == "down":
        arm, stroke = BADGE["arm"] * scale, BADGE["stroke"] * scale
        cross = rgb(colours["cross"])
        ops.append(("paint", Capsule(cx - arm, cy - arm, cx + arm, cy + arm, stroke), cross, 1.0))
        ops.append(("paint", Capsule(cx + arm, cy - arm, cx - arm, cy + arm, stroke), cross, 1.0))
    return ops


def tray_icon(size, state, taskbar):
    colours = TASKBAR[taskbar]
    scale = size / 16
    glyph = rgb(colours["glyph"])
    ops = [("paint", rect, glyph, STATE_ALPHA[state]) for rect, _ in bar_rects(scale, 0, 0)]
    if state in ("queued", "down"):
        ops += badge_ops(scale, 0, 0, state, colours)
    return render(size, ops)


def tile_ops(size, body, scale):
    """The dark-background mark: bright bars on an ink body, the mark centred."""
    left = (size - 16 * scale) / 2
    ops = [("paint", body, rgb(INK), 1.0)]
    ops += [("paint", rect, rgb(PALETTE["dark"][row]), 1.0)
            for rect, row in bar_rects(scale, left, left)]
    return ops


def app_icon(size):
    """The ink square tile. The mark fills 16 / 24 of it as drawn, and a larger share below
    24 px so the bars stay at least a pixel tall."""
    scale = 1.0 if size <= 24 else size / 24
    return render(size, tile_ops(size, RoundRect(0, 0, size, size, 0), scale))


def macos_app_icon():
    """1024 px canvas with Apple's 824 px rounded body; the mark keeps its 16 / 24 share."""
    body = RoundRect(100, 100, 924, 924, 185.4)
    return render(1024, tile_ops(1024, body, 824 / 24))


# ---------------------------------------------------------------------------------------------
# PNG and ICO
# ---------------------------------------------------------------------------------------------


def png_bytes(pixels, size):
    raw = bytearray()
    for j in range(size):
        raw.append(0)
        for r, g, b, a in pixels[j * size : (j + 1) * size]:
            raw.extend(max(0, min(255, round(v * 255))) for v in (r, g, b, a))

    def chunk(tag, data):
        body = tag + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body))

    header = struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header)
            + chunk(b"IDAT", zlib.compress(bytes(raw), 9)) + chunk(b"IEND", b""))


def ico_bytes(frames):
    """PNG-compressed ICO frames, smallest first; `frames` maps size to PNG bytes."""
    sizes = sorted(frames)
    out = struct.pack("<HHH", 0, 1, len(sizes))
    offset = 6 + 16 * len(sizes)
    for size in sizes:
        side = 0 if size >= 256 else size
        out += struct.pack("<BBBBHHII", side, side, 0, 0, 1, 32, len(frames[size]), offset)
        offset += len(frames[size])
    return out + b"".join(frames[size] for size in sizes)


def write(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    if isinstance(data, str):
        data = data.encode("utf-8")
    path.write_bytes(data)
    print(path.relative_to(ROOT).as_posix())


# ---------------------------------------------------------------------------------------------
# SVG (unsnapped; vectors scale cleanly)
# ---------------------------------------------------------------------------------------------


def svg_bars(fill_for_row, extra=""):
    return "".join(
        f'<rect x="{x0:g}" y="{1 + 3 * row}" width="{x1 - x0:g}" height="2" rx="1" '
        f'{fill_for_row(row)}{extra}/>'
        for x0, x1, row in BARS
    )


def favicon_svg():
    """Colour mark that follows the browser's light or dark scheme."""
    rules = "".join(f".r{n}{{fill:{c}}}" for n, c in enumerate(PALETTE["light"]))
    dark = "".join(f".r{n}{{fill:{c}}}" for n, c in enumerate(PALETTE["dark"]))
    bars = svg_bars(lambda row: f'class="r{row}"')
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16">'
            f"<style>{rules}@media (prefers-color-scheme:dark){{{dark}}}</style>{bars}</svg>\n")


def tile_svg():
    """The app icon as a vector: the dark-background mark on its ink square."""
    bars = svg_bars(lambda row: f'fill="{PALETTE["dark"][row]}"')
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="-4 -4 24 24">'
            f'<rect x="-4" y="-4" width="24" height="24" fill="{INK}"/>{bars}</svg>\n')


def state_svg(state, glyph, badge_fills):
    """A tray state as a vector. `badge_fills` maps queued and down to (class, colour)."""
    b = BADGE
    opacity = STATE_ALPHA[state]
    bars = svg_bars(lambda _: f'fill="{glyph}"')
    group = f'<g opacity="{opacity:g}">{bars}</g>' if opacity < 1 else bars
    if state not in ("queued", "down"):
        return f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16">{group}</svg>\n'

    notch = (f'<mask id="notch"><rect width="16" height="16" fill="#fff"/>'
             f'<circle cx="{b["cx"]:g}" cy="{b["cy"]:g}" r="{b["r"] + b["ring"]:g}"/></mask>')
    css_class, colour = badge_fills[state]
    dot = f'<circle class="{css_class}" cx="{b["cx"]:g}" cy="{b["cy"]:g}" r="{b["r"]:g}" fill="{colour}"'
    defs = notch
    if state == "down":
        a = b["arm"]
        cross = (f'<path d="M{b["cx"] - a:g} {b["cy"] - a:g}L{b["cx"] + a:g} {b["cy"] + a:g}'
                 f'M{b["cx"] + a:g} {b["cy"] - a:g}L{b["cx"] - a:g} {b["cy"] + a:g}" '
                 f'stroke="#000" stroke-width="{b["stroke"]:g}" stroke-linecap="round"/>')
        defs += f'<mask id="cross"><rect width="16" height="16" fill="#fff"/>{cross}</mask>'
        dot += ' mask="url(#cross)"'
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><defs>{defs}</defs>'
            f'<g mask="url(#notch)">{group}</g>{dot}/></svg>\n')


# ---------------------------------------------------------------------------------------------
# Outputs
# ---------------------------------------------------------------------------------------------


def build_windows():
    for state in STATES:
        for taskbar in TASKBAR:
            name = f"allot-{state}-on-{taskbar}-taskbar"
            frames = {size: png_bytes(tray_icon(size, state, taskbar), size) for size in TRAY_SIZES}
            write(TRAY_ASSETS / "Tray" / f"{name}.ico", ico_bytes(frames))
            # Toast logos: toast images cannot be ICO files.
            write(TRAY_ASSETS / "Tray" / f"{name}-256.png", frames[256])
    app = {size: png_bytes(app_icon(size), size) for size in APP_SIZES}
    write(TRAY_ASSETS / "AppIcon.ico", ico_bytes(app))


def build_web():
    favicon = {size: png_bytes(app_icon(size), size) for size in FAVICON_SIZES}
    write(BRAND / "web" / "favicon.ico", ico_bytes(favicon))
    write(BRAND / "web" / "favicon.svg", favicon_svg())


def build_linux():
    for size in HICOLOR_SIZES:
        write(BRAND / "linux" / "hicolor" / f"{size}x{size}" / "apps" / "allot.png",
              png_bytes(app_icon(size), size))
    write(BRAND / "linux" / "hicolor" / "scalable" / "apps" / "allot.svg", tile_svg())
    # Symbolic icons: GTK recolours #2e3436 and the warning and error classes to the theme.
    badges = {"queued": ("warning", "#f57900"), "down": ("error", "#cc0000")}
    for state in STATES:
        write(BRAND / "linux" / "symbolic" / f"allot-{state}-symbolic.svg",
              state_svg(state, "#2e3436", badges))


def build_macos():
    write(BRAND / "macos" / "AppIcon-1024.png", png_bytes(macos_app_icon(), 1024))
    # Template images are black with alpha; the app tints them and draws badges itself.
    for state in ("idle", "busy"):
        write(BRAND / "macos" / "menubar" / f"allot-{state}-template.svg",
              state_svg(state, "#000000", {}))


if __name__ == "__main__":
    build_windows()
    build_web()
    build_linux()
    build_macos()
