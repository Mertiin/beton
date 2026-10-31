"""Assemble worms sheets. Original pixel art, not a trace of a commercial sprite.

Run from anywhere:

    python games/worms/assets/_draw_worm.py
"""

import math
import struct
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parent

PINK = (255, 186, 168, 255)
PINK_DK = (214, 120, 108, 255)
OUTLINE = (42, 24, 28, 255)
EYE = (255, 252, 246, 255)
PUPIL = (28, 18, 22, 255)
BROW = (36, 22, 26, 255)
CLEAR = (0, 0, 0, 0)
AIM_BLUE = (48, 148, 255, 255)
AIM_BLUE_DK = (24, 88, 200, 255)


def write_png(path, w, h, rgba):
    def chunk(tag, data):
        return (
            struct.pack(">I", len(data))
            + tag
            + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        )

    raw = b"".join(b"\x00" + bytes(rgba[y * w * 4 : (y + 1) * w * 4]) for y in range(h))
    ihdr = struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0)
    png = (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", ihdr)
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )
    path.write_bytes(png)


def blank(w, h, color=CLEAR):
    return [c for _ in range(w * h) for c in color]


def put(buf, w, x, y, color):
    if x < 0 or y < 0 or x >= w:
        return
    h = len(buf) // (w * 4)
    if y >= h:
        return
    i = (y * w + x) * 4
    buf[i : i + 4] = list(color)


def get(buf, w, x, y):
    h = len(buf) // (w * 4)
    if x < 0 or y < 0 or x >= w or y >= h:
        return CLEAR
    i = (y * w + x) * 4
    return tuple(buf[i : i + 4])


def outline(buf, w, h):
    add = []
    for y in range(h):
        for x in range(w):
            if get(buf, w, x, y)[3] > 0:
                continue
            for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                if get(buf, w, x + dx, y + dy)[3] > 200:
                    add.append((x, y))
                    break
    for x, y in add:
        put(buf, w, x, y, OUTLINE)


def blit(dst, dw, src, sw, sh, ox, oy):
    for y in range(sh):
        for x in range(sw):
            c = get(src, sw, x, y)
            if c[3] == 0:
                continue
            put(dst, dw, ox + x, oy + y, c)


def ellipse(buf, w, cx, cy, rx, ry, color, shade_left=True):
    for y in range(int(cy - ry - 1), int(cy + ry + 2)):
        for x in range(int(cx - rx - 1), int(cx + rx + 2)):
            dx = (x + 0.5 - cx) / max(0.1, rx)
            dy = (y + 0.5 - cy) / max(0.1, ry)
            if dx * dx + dy * dy <= 1.0:
                c = PINK_DK if shade_left and x < cx - 0.5 else color
                put(buf, w, x, y, c)


def worm_cell(phase):
    """phase 0 idle, 1 scrunch, 2 extend, 3 airborne. Faces right.

    Drawn for a 10×14 on-screen size over a 6×10 hitbox: clear upright S-curve
    with head, neck, hump, and floor-hugging tail.
    """
    w, h = 16, 24
    buf = blank(w, h)
    air = phase == 3
    # Image Y grows down; feet sit on the bottom row so draw offset matches hitbox.
    if phase == 0:
        head = (12.0, 5.5, 3.8, 4.2)
        neck = (11.0, 11.0, 2.8, 3.2)
        hump = (6.5, 16.0, 3.8, 3.2)
        tail = (2.8, 21.5, 4.8, 2.0)
    elif phase == 1:
        head = (12.0, 5.0, 3.8, 4.2)
        neck = (11.2, 10.5, 2.8, 3.0)
        hump = (8.0, 15.5, 3.4, 3.4)
        tail = (5.5, 21.0, 3.4, 2.2)
    elif phase == 2:
        head = (11.5, 5.5, 3.8, 4.2)
        neck = (10.0, 11.0, 2.6, 3.2)
        hump = (5.0, 15.5, 4.0, 3.0)
        tail = (1.4, 21.5, 5.2, 1.9)
    else:
        head = (11.5, 4.5, 3.6, 4.6)
        neck = (10.5, 10.0, 2.6, 3.0)
        hump = (7.0, 14.0, 3.2, 2.8)
        tail = (4.5, 17.0, 3.2, 2.2)

    for cx, cy, rx, ry in (tail, hump, neck, head):
        ellipse(buf, w, cx, cy, rx, ry, PINK)

    for y in range(6, 23 if not air else 19):
        t = (y - 6) / (16.0 if not air else 12.0)
        if phase == 1:
            cx = 12.0 - t * 5.5
        elif phase == 2:
            cx = 11.5 - t * 9.5
        elif air:
            cx = 11.5 - t * 6.0
        else:
            cx = 12.0 - t * 8.5
        thick = 2.8 if 9 < y < 17 else 2.2
        for x in range(int(cx - thick), int(cx + thick) + 1):
            if 0 <= x < w and get(buf, w, x, y)[3] == 0:
                put(buf, w, x, y, PINK_DK if x < cx else PINK)

    for ex, ey in ((9, 3), (12, 3)):
        for y in range(ey, ey + 5):
            for x in range(ex, ex + 2):
                put(buf, w, x, y, EYE)
        put(buf, w, ex + 1, ey + 2, PUPIL)
        put(buf, w, ex + 1, ey + 3, PUPIL)
        put(buf, w, ex, ey - 1, BROW)
        put(buf, w, ex + 1, ey - 1, BROW)
    put(buf, w, 14, 7, PINK)
    put(buf, w, 15, 8, PINK)
    put(buf, w, 14, 9, BROW)
    put(buf, w, 15, 9, BROW)

    outline(buf, w, h)
    return buf


def circle(buf, w, cx, cy, r, color):
    for y in range(int(cy - r - 1), int(cy + r + 2)):
        for x in range(int(cx - r - 1), int(cx + r + 2)):
            if (x + 0.5 - cx) ** 2 + (y + 0.5 - cy) ** 2 <= r * r:
                put(buf, w, x, y, color)


def ring(buf, w, cx, cy, r, thickness, color):
    r2 = r * r
    ri = max(0.0, r - thickness)
    ri2 = ri * ri
    for y in range(int(cy - r - 1), int(cy + r + 2)):
        for x in range(int(cx - r - 1), int(cx + r + 2)):
            d2 = (x + 0.5 - cx) ** 2 + (y + 0.5 - cy) ** 2
            if ri2 <= d2 <= r2:
                put(buf, w, x, y, color)


def badge(kind):
    buf = blank(32, 32)
    circle(buf, 32, 16, 16, 14, (24, 58, 120, 255))
    circle(buf, 32, 16, 16, 12, (232, 196, 64, 255))
    ink = (32, 36, 42, 255)
    if kind == 0:
        for x in range(8, 24):
            put(buf, 32, x, 15, ink)
            put(buf, 32, x, 16, ink)
        circle(buf, 32, 10, 18, 3, ink)
    elif kind == 1:
        circle(buf, 32, 16, 17, 6, (52, 120, 48, 255))
        put(buf, 32, 16, 8, ink)
        put(buf, 32, 16, 9, ink)
        put(buf, 32, 17, 7, (180, 60, 40, 255))
    elif kind == 2:
        for i, x in enumerate(range(8, 24)):
            y = 20 - i // 3
            put(buf, 32, x, y, (90, 58, 36, 255))
            put(buf, 32, x, y + 1, (90, 58, 36, 255))
        circle(buf, 32, 22, 12, 2, ink)
    elif kind == 3:
        circle(buf, 32, 16, 16, 5, (220, 220, 230, 255))
        circle(buf, 32, 10, 12, 3, (200, 200, 210, 255))
        circle(buf, 32, 21, 20, 3, (180, 180, 190, 255))
    else:
        # Banana badge
        yel = (240, 200, 48, 255)
        for i, (x, y) in enumerate(
            ((10, 10), (12, 12), (14, 14), (16, 16), (18, 18), (20, 19), (22, 18))
        ):
            circle(buf, 32, x, y, 3 if i < 5 else 2, yel)
        put(buf, 32, 10, 8, (60, 140, 50, 255))
    return buf


def weapon_cell(kind):
    buf = blank(16, 16)
    ink = (40, 36, 32, 255)
    if kind == 0:  # bazooka
        for x in range(2, 15):
            put(buf, 16, x, 7, (70, 110, 60, 255))
            put(buf, 16, x, 8, (50, 80, 44, 255))
        circle(buf, 16, 8, 8, 2, ink)
        put(buf, 16, 14, 6, (90, 130, 70, 255))
        put(buf, 16, 14, 9, (90, 130, 70, 255))
    elif kind == 1:  # grenade — clearer disc + pin
        circle(buf, 16, 8, 9, 5, (40, 96, 40, 255))
        circle(buf, 16, 8, 9, 4, (56, 128, 52, 255))
        put(buf, 16, 8, 3, ink)
        put(buf, 16, 8, 4, ink)
        put(buf, 16, 9, 2, (210, 80, 40, 255))
        put(buf, 16, 10, 2, (210, 80, 40, 255))
        put(buf, 16, 6, 8, (30, 70, 30, 255))
    elif kind == 2:  # shotgun — stock left, barrels right
        for x in range(1, 6):
            put(buf, 16, x, 8, (120, 78, 42, 255))
            put(buf, 16, x, 9, (90, 58, 32, 255))
            put(buf, 16, x, 10, (120, 78, 42, 255))
        for x in range(5, 15):
            put(buf, 16, x, 7, (70, 70, 76, 255))
            put(buf, 16, x, 8, (50, 50, 56, 255))
            put(buf, 16, x, 9, (70, 70, 76, 255))
            put(buf, 16, x, 10, (50, 50, 56, 255))
        put(buf, 16, 14, 6, ink)
        put(buf, 16, 14, 11, ink)
        circle(buf, 16, 5, 9, 2, ink)
    elif kind == 3:  # crate
        for y in range(4, 13):
            for x in range(4, 13):
                put(buf, 16, x, y, (176, 132, 64, 255))
        for x in range(4, 13):
            put(buf, 16, x, 4, (96, 64, 32, 255))
            put(buf, 16, x, 12, (96, 64, 32, 255))
        for y in range(4, 13):
            put(buf, 16, 4, y, (96, 64, 32, 255))
            put(buf, 16, 12, y, (96, 64, 32, 255))
    elif kind == 4:  # shell — bright pointed bullet facing right
        circle(buf, 16, 6, 8, 4, (255, 210, 70, 255))
        circle(buf, 16, 6, 8, 2, (255, 240, 160, 255))
        for x in range(8, 15):
            put(buf, 16, x, 7, (255, 180, 40, 255))
            put(buf, 16, x, 8, (255, 230, 90, 255))
            put(buf, 16, x, 9, (255, 180, 40, 255))
        put(buf, 16, 15, 8, (255, 255, 200, 255))
        put(buf, 16, 4, 8, (200, 80, 30, 255))
        outline(buf, 16, 16)
    elif kind == 5:  # cluster
        circle(buf, 16, 8, 8, 4, (230, 230, 236, 255))
        circle(buf, 16, 5, 6, 2, (200, 200, 210, 255))
        circle(buf, 16, 11, 10, 2, (190, 190, 200, 255))
        circle(buf, 16, 10, 5, 2, (210, 210, 220, 255))
    else:  # banana
        yel = (245, 205, 50, 255)
        dk = (200, 150, 30, 255)
        for i, (x, y) in enumerate(((4, 5), (6, 7), (8, 9), (10, 11), (12, 12), (13, 11))):
            circle(buf, 16, x, y, 2.4 if i < 4 else 2.0, yel if i % 2 == 0 else dk)
        put(buf, 16, 4, 3, (50, 120, 40, 255))
        put(buf, 16, 5, 3, (50, 120, 40, 255))
        outline(buf, 16, 16)
    return buf


def tree_cell():
    buf = blank(32, 48)
    for y in range(30, 46):
        for x in range(14, 18):
            put(buf, 32, x, y, (110, 72, 40, 255))
    for i, (top, half, _gy) in enumerate(((4, 10, 34), (10, 12, 28), (16, 14, 22))):
        green = (40, 150, 48, 255) if i != 1 else (56, 170, 60, 255)
        for y in range(top, top + half):
            t = (y - top) / max(1, half - 1)
            span = int(2 + t * (half - 2))
            for x in range(16 - span, 16 + span):
                put(buf, 32, x, y, green)
    return buf


def grave_cell():
    buf = blank(16, 16)
    for y in range(4, 14):
        for x in range(4, 12):
            arch = y > 6 or (x - 8) ** 2 + (y - 7) ** 2 < 16
            if arch and y < 14:
                put(buf, 16, x, y, (170, 170, 176, 255))
    put(buf, 16, 8, 8, (80, 80, 88, 255))
    put(buf, 16, 8, 9, (80, 80, 88, 255))
    return buf


def cloud_cell():
    buf = blank(32, 16)
    circle(buf, 32, 10, 9, 5, (236, 240, 248, 255))
    circle(buf, 32, 16, 8, 6, (246, 248, 252, 255))
    circle(buf, 32, 22, 10, 4, (230, 236, 246, 255))
    return buf


def aim_cursor_cell():
    """Blue ring + cross reticle, 32x32."""
    buf = blank(32, 32)
    cx, cy = 15.5, 15.5
    ring(buf, 32, cx, cy, 12.0, 2.0, AIM_BLUE)
    ring(buf, 32, cx, cy, 12.0, 1.0, AIM_BLUE_DK)
    # Four ticks.
    for x in range(14, 18):
        put(buf, 32, x, 2, AIM_BLUE)
        put(buf, 32, x, 3, AIM_BLUE)
        put(buf, 32, x, 28, AIM_BLUE)
        put(buf, 32, x, 29, AIM_BLUE)
    for y in range(14, 18):
        put(buf, 32, 2, y, AIM_BLUE)
        put(buf, 32, 3, y, AIM_BLUE)
        put(buf, 32, 28, y, AIM_BLUE)
        put(buf, 32, 29, y, AIM_BLUE)
    # Center dot.
    circle(buf, 32, cx, cy, 1.6, (255, 255, 255, 255))
    put(buf, 32, 15, 15, AIM_BLUE_DK)
    return buf


def power_blob_cell():
    """Soft horizontal capsule used as the charge cone body (tint in draw)."""
    buf = blank(32, 16)
    for y in range(16):
        for x in range(32):
            # Stadium: circles at ends + rectangle middle.
            inside = False
            if 6 <= x <= 25 and 4 <= y <= 11:
                inside = True
            if (x - 6) ** 2 + (y - 7.5) ** 2 <= 4.2 ** 2:
                inside = True
            if (x - 25) ** 2 + (y - 7.5) ** 2 <= 4.2 ** 2:
                inside = True
            if inside:
                # Soft edge alpha via brighter center.
                d = abs(y - 7.5) / 4.5
                a = int(255 * (1.0 - d * 0.35))
                put(buf, 32, x, y, (255, 255, 255, a))
    return buf


def ground_tile():
    w = h = 64
    buf = blank(w, h, (156, 78, 40, 255))
    for y in range(h):
        for x in range(w):
            n = (x * 374761393 + y * 668265263) & 0xFFFFFFFF
            n = ((n ^ (n >> 13)) * 1274126177) & 0xFFFFFFFF
            v = (n ^ (n >> 16)) & 0xFF
            if v < 40:
                put(buf, w, x, y, (112, 52, 26, 255))
            elif v > 220:
                put(buf, w, x, y, (186, 104, 54, 255))
            elif v % 11 == 0:
                put(buf, w, x, y, (96, 44, 22, 255))
                put(buf, w, x + 1, y, (130, 64, 32, 255))
                put(buf, w, x, y + 1, (130, 64, 32, 255))
    return buf


def sheet(cells, cw, ch, cols):
    rows = (len(cells) + cols - 1) // cols
    w, h = cols * cw, rows * ch
    buf = blank(w, h)
    for i, cell in enumerate(cells):
        blit(buf, w, cell, cw, ch, (i % cols) * cw, (i // cols) * ch)
    return w, h, buf


def main():
    worms = [worm_cell(i) for i in range(4)]
    w, h, buf = sheet(worms, 16, 24, 4)
    write_png(ROOT / "worm.png", w, h, buf)

    weapons = [weapon_cell(i) for i in range(7)]
    w, h, buf = sheet(weapons, 16, 16, 7)
    write_png(ROOT / "weapons.png", w, h, buf)

    icons = [badge(i) for i in range(5)]
    w, h, buf = sheet(icons, 32, 32, 5)
    write_png(ROOT / "icons.png", w, h, buf)

    # props.png 96x48: tree (0,0), cloud (32,0), grave (32,32), aim (64,0), power (64,32).
    props = blank(96, 48)
    blit(props, 96, tree_cell(), 32, 48, 0, 0)
    blit(props, 96, cloud_cell(), 32, 16, 32, 0)
    blit(props, 96, grave_cell(), 16, 16, 32, 32)
    blit(props, 96, aim_cursor_cell(), 32, 32, 64, 0)
    blit(props, 96, power_blob_cell(), 32, 16, 64, 32)
    write_png(ROOT / "props.png", 96, 48, props)

    g = ground_tile()
    write_png(ROOT / "ground.png", 64, 64, g)
    print("wrote sheets in", ROOT)


if __name__ == "__main__":
    main()
