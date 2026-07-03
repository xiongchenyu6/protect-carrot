#!/usr/bin/env python3
"""Generate the hero paperdoll `.ppd` asset.

The output is a tar archive understood by `paperdoll-tar`: `manifest.yml` plus
RGBA PNG layers. Every layer is a *procedurally drawn* chibi fragment — no
external art. The v2 renderer aims to match the glossy, saturated look of the
AI class portraits: cel-shaded volumes, a dark silhouette outline on every
layer, warm rim light and specular highlights, and gear that is registered to
the body region it clothes (torso / feet / head / back).

Layout contract (must stay stable — `hero_paperdoll.rs` and the fragment ids in
the manifest depend on it): 128x128 dolls, slots [20 armor, 23 boots, 10 weapon,
21 charm, 22 relic], fragment ids as listed in WEAPONS / GEAR.
"""

from __future__ import annotations

import io
import math
import struct
import tarfile
import zlib
from pathlib import Path

DESIGN_SIZE = 96
OUTPUT_SIZE = 128
CANVAS_SIZE = 384
RENDER_SCALE = CANVAS_SIZE / DESIGN_SIZE
OUT = Path("assets/paperdoll/hero.ppd")

# Light comes from the upper-left; shadows fall to the lower-right.
OUTLINE = (26, 20, 34, 255)


# ---------------------------------------------------------------------------
# low-level raster primitives (operate in DESIGN coordinates, 0..96)
# ---------------------------------------------------------------------------
def blank() -> bytearray:
    return bytearray(CANVAS_SIZE * CANVAS_SIZE * 4)


def clamp(v) -> int:
    return max(0, min(255, int(round(v))))


def mul(c, f: float):
    return (clamp(c[0] * f), clamp(c[1] * f), clamp(c[2] * f), c[3])


def mix(c, d, t: float):
    return (
        clamp(c[0] + (d[0] - c[0]) * t),
        clamp(c[1] + (d[1] - c[1]) * t),
        clamp(c[2] + (d[2] - c[2]) * t),
        clamp(c[3] + (d[3] - c[3]) * t),
    )


def blend_px(img: bytearray, x: int, y: int, color) -> None:
    if x < 0 or y < 0 or x >= CANVAS_SIZE or y >= CANVAS_SIZE:
        return
    r, g, b, a = color
    if a <= 0:
        return
    i = (y * CANVAS_SIZE + x) * 4
    dr, dg, db, da = img[i], img[i + 1], img[i + 2], img[i + 3]
    sa = a / 255.0
    oa = da / 255.0
    out_a = sa + oa * (1.0 - sa)
    if out_a <= 0.0:
        return
    img[i] = clamp((r * sa + dr * oa * (1.0 - sa)) / out_a)
    img[i + 1] = clamp((g * sa + dg * oa * (1.0 - sa)) / out_a)
    img[i + 2] = clamp((b * sa + db * oa * (1.0 - sa)) / out_a)
    img[i + 3] = clamp(out_a * 255.0)


def rect(img, x0, y0, x1, y1, color) -> None:
    sx0 = math.floor(x0 * RENDER_SCALE)
    sy0 = math.floor(y0 * RENDER_SCALE)
    sx1 = math.ceil(x1 * RENDER_SCALE)
    sy1 = math.ceil(y1 * RENDER_SCALE)
    for y in range(sy0, sy1):
        for x in range(sx0, sx1):
            blend_px(img, x, y, color)


def ellipse(img, cx, cy, rx, ry, color) -> None:
    cx *= RENDER_SCALE
    cy *= RENDER_SCALE
    rx *= RENDER_SCALE
    ry *= RENDER_SCALE
    x0 = math.floor(cx - rx)
    x1 = math.ceil(cx + rx)
    y0 = math.floor(cy - ry)
    y1 = math.ceil(cy + ry)
    for y in range(y0, y1 + 1):
        for x in range(x0, x1 + 1):
            nx = (x + 0.5 - cx) / rx
            ny = (y + 0.5 - cy) / ry
            if nx * nx + ny * ny <= 1.0:
                blend_px(img, x, y, color)


def circle(img, cx, cy, r, color) -> None:
    ellipse(img, cx, cy, r, r, color)


def point_in_poly(x, y, pts) -> bool:
    inside = False
    j = len(pts) - 1
    for i, (xi, yi) in enumerate(pts):
        xj, yj = pts[j]
        crosses = (yi > y) != (yj > y)
        if crosses:
            at_x = (xj - xi) * (y - yi) / ((yj - yi) or 1e-6) + xi
            if x < at_x:
                inside = not inside
        j = i
    return inside


def poly(img, pts, color) -> None:
    pts = [(x * RENDER_SCALE, y * RENDER_SCALE) for x, y in pts]
    min_x = math.floor(min(p[0] for p in pts))
    max_x = math.ceil(max(p[0] for p in pts))
    min_y = math.floor(min(p[1] for p in pts))
    max_y = math.ceil(max(p[1] for p in pts))
    for y in range(min_y, max_y + 1):
        for x in range(min_x, max_x + 1):
            if point_in_poly(x + 0.5, y + 0.5, pts):
                blend_px(img, x, y, color)


def line(img, x0, y0, x1, y1, color, width=2.0) -> None:
    x0 *= RENDER_SCALE
    y0 *= RENDER_SCALE
    x1 *= RENDER_SCALE
    y1 *= RENDER_SCALE
    width *= RENDER_SCALE
    steps = max(1, int(max(abs(x1 - x0), abs(y1 - y0)) * 2))
    for i in range(steps + 1):
        t = i / steps
        x = x0 + (x1 - x0) * t
        y = y0 + (y1 - y0) * t
        circle_px(img, x, y, width / 2.0, color)


def circle_px(img, cx, cy, r, color) -> None:
    x0 = math.floor(cx - r)
    x1 = math.ceil(cx + r)
    y0 = math.floor(cy - r)
    y1 = math.ceil(cy + r)
    for y in range(y0, y1 + 1):
        for x in range(x0, x1 + 1):
            dx = x + 0.5 - cx
            dy = y + 0.5 - cy
            if dx * dx + dy * dy <= r * r:
                blend_px(img, x, y, color)


# ---------------------------------------------------------------------------
# high-level shading helpers
# ---------------------------------------------------------------------------
def celblob(img, cx, cy, rx, ry, base, light=1.28, shade=0.7, spec=True) -> None:
    """A cel-shaded rounded volume: shadow tone, base, lit cap, spec dot."""
    ellipse(img, cx, cy, rx, ry, mul(base, shade))  # shadow underlayer
    ellipse(img, cx - rx * 0.10, cy - ry * 0.12, rx * 0.92, ry * 0.9, base)
    ellipse(img, cx - rx * 0.26, cy - ry * 0.30, rx * 0.52, ry * 0.46, mul(base, light))
    if spec:
        ellipse(
            img,
            cx - rx * 0.34,
            cy - ry * 0.4,
            rx * 0.2,
            ry * 0.17,
            mix(base, (255, 255, 255, base[3]), 0.75),
        )


def spec(img, cx, cy, r, a=210) -> None:
    circle(img, cx, cy, r, (255, 255, 255, a))


def rim(img, x0, y0, x1, y1, color, width=1.4) -> None:
    line(img, x0, y0, x1, y1, color, width)


# ---------------------------------------------------------------------------
# encode: supersample down to 128, add a silhouette outline, write PNG
# ---------------------------------------------------------------------------
def downsample_rgba(img: bytearray) -> bytearray:
    factor = CANVAS_SIZE // OUTPUT_SIZE
    out = bytearray(OUTPUT_SIZE * OUTPUT_SIZE * 4)
    for oy in range(OUTPUT_SIZE):
        for ox in range(OUTPUT_SIZE):
            r = g = b = a = 0
            for dy in range(factor):
                for dx in range(factor):
                    sx = ox * factor + dx
                    sy = oy * factor + dy
                    i = (sy * CANVAS_SIZE + sx) * 4
                    alpha = img[i + 3]
                    r += img[i] * alpha
                    g += img[i + 1] * alpha
                    b += img[i + 2] * alpha
                    a += alpha
            n = factor * factor
            oi = (oy * OUTPUT_SIZE + ox) * 4
            if a:
                out[oi] = clamp(r / a)
                out[oi + 1] = clamp(g / a)
                out[oi + 2] = clamp(b / a)
            out[oi + 3] = clamp(a / n)
    return out


def add_outline(buf: bytearray, thickness: int = 2, color=OUTLINE) -> None:
    """Paint a dark rim on transparent pixels that hug the opaque silhouette."""
    W = OUTPUT_SIZE
    alpha = [buf[i * 4 + 3] for i in range(W * W)]
    edge = bytearray(W * W)
    t2 = thickness * thickness
    for y in range(W):
        for x in range(W):
            if alpha[y * W + x] >= 90:
                continue
            hit = False
            for dy in range(-thickness, thickness + 1):
                yy = y + dy
                if yy < 0 or yy >= W:
                    continue
                base = yy * W
                for dx in range(-thickness, thickness + 1):
                    if dx * dx + dy * dy > t2:
                        continue
                    xx = x + dx
                    if 0 <= xx < W and alpha[base + xx] >= 150:
                        hit = True
                        break
                if hit:
                    break
            if hit:
                edge[y * W + x] = 1
    for idx in range(W * W):
        if edge[idx]:
            i = idx * 4
            # only fills transparent pixels, so it sits *behind* the art
            buf[i], buf[i + 1], buf[i + 2], buf[i + 3] = color


def encode(img: bytearray, outline: bool = True, thickness: int = 2) -> bytes:
    small = downsample_rgba(img)
    if outline:
        add_outline(small, thickness)
    return png_from_128(small)


def png_from_128(out: bytearray) -> bytes:
    def chunk(kind: bytes, data: bytes) -> bytes:
        return (
            struct.pack(">I", len(data))
            + kind
            + data
            + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)
        )

    raw = bytearray()
    row_len = OUTPUT_SIZE * 4
    for y in range(OUTPUT_SIZE):
        raw.append(0)
        raw.extend(out[y * row_len : (y + 1) * row_len])

    header = struct.pack(">IIBBBBB", OUTPUT_SIZE, OUTPUT_SIZE, 8, 6, 0, 0, 0)
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", header)
        + chunk(b"IDAT", zlib.compress(bytes(raw), 9))
        + chunk(b"IEND", b"")
    )


# ---------------------------------------------------------------------------
# bodies
# ---------------------------------------------------------------------------
PALETTE = {
    "human": {
        "skin": (241, 194, 141, 255),
        "hair": (98, 63, 44, 255),
        "hair_hi": (150, 104, 70, 255),
        "cloth": (176, 96, 66, 255),
        "cloth_hi": (214, 140, 96, 255),
        "belt": (74, 47, 36, 255),
        "eye": (60, 42, 36, 255),
    },
    "elf": {
        "skin": (240, 214, 165, 255),
        "hair": (232, 214, 138, 255),
        "hair_hi": (255, 246, 196, 255),
        "cloth": (86, 150, 104, 255),
        "cloth_hi": (139, 205, 150, 255),
        "belt": (54, 92, 66, 255),
        "eye": (36, 104, 78, 255),
    },
    "orc": {
        "skin": (112, 178, 96, 255),
        "hair": (40, 66, 34, 255),
        "hair_hi": (74, 104, 56, 255),
        "cloth": (132, 96, 60, 255),
        "cloth_hi": (176, 134, 88, 255),
        "belt": (60, 42, 28, 255),
        "eye": (40, 46, 26, 255),
    },
}


def body(race: str) -> bytearray:
    img = blank()
    p = PALETTE[race]
    skin = p["skin"]
    wide = race == "orc"
    slim = race == "elf"
    torso_rx = 18 if wide else 14 if slim else 16

    # ground shadow
    ellipse(img, 48, 84, 21 if wide else 18, 4.5, (0, 0, 0, 70))

    # legs + simple shoes
    leg_col = mul(skin, 0.9)
    for lx in (41.5, 54.5):
        rect(img, lx - 3.2, 66, lx + 3.2, 79, leg_col)
    shoe = (58, 42, 33, 255)
    for lx in (41.5, 54.5):
        ellipse(img, lx, 80, 5.2, 3.4, shoe)
        ellipse(img, lx - 1.2, 79, 2.4, 1.6, mul(shoe, 1.4))

    # torso tunic (cel-shaded)
    cloth = p["cloth"]
    ellipse(img, 48, 52, torso_rx, 17, mul(cloth, 0.68))
    ellipse(img, 48, 50, torso_rx * 0.94, 16, cloth)
    ellipse(img, 44, 46, torso_rx * 0.5, 9, p["cloth_hi"])
    # collar V + center seam + belt
    poly(img, [(41, 38), (55, 38), (52, 47), (48, 51), (44, 47)], mul(cloth, 0.55))
    line(img, 48, 40, 48, 62, mul(cloth, 0.5), 1.2)
    rect(img, 48 - torso_rx * 0.92, 60, 48 + torso_rx * 0.92, 64, p["belt"])
    circle(img, 48, 62, 2.2, mix(p["belt"], (255, 220, 120, 255), 0.7))
    # rim light on the shaded side
    rim(img, 48 + torso_rx * 0.8, 44, 48 + torso_rx * 0.72, 60, (255, 236, 200, 90), 1.3)

    # arms + hands
    arm_w = 6.5 if wide else 5.0 if slim else 5.6
    line(img, 40, 41, 22, 56, cloth, arm_w)
    line(img, 56, 41, 74, 56, cloth, arm_w)
    line(img, 40, 41, 23, 55, p["cloth_hi"], arm_w * 0.4)
    celblob(img, 20.5, 58, 4.2, 4.2, skin, spec=False)
    celblob(img, 75.5, 58, 4.2, 4.2, skin, spec=False)

    # ears (elf) drawn before head so head overlaps the base
    if race == "elf":
        poly(img, [(33, 24), (22, 28), (34, 32)], skin)
        poly(img, [(63, 24), (74, 28), (62, 32)], skin)
        poly(img, [(31, 27), (25, 28.5), (33, 30)], mul(skin, 0.8))
        poly(img, [(65, 27), (71, 28.5), (63, 30)], mul(skin, 0.8))

    # head
    head_r = 17 if wide else 15.5
    celblob(img, 48, 27, head_r, head_r * 0.98, skin, light=1.18, shade=0.78)

    # hair per race
    hair, hair_hi = p["hair"], p["hair_hi"]
    if race == "orc":
        # scalp band + tusks + brow
        ellipse(img, 48, 17, head_r, 8.5, hair)
        rect(img, 48 - head_r, 15, 48 + head_r, 21, hair)
        ellipse(img, 40, 15, 5, 4, hair_hi)
        # tusks
        poly(img, [(40, 36), (42.5, 45), (44.5, 36)], (238, 232, 205, 255))
        poly(img, [(52, 36), (54.5, 45), (56, 36)], (238, 232, 205, 255))
        line(img, 40, 22, 45, 24, (30, 44, 26, 200), 1.4)
        line(img, 56, 22, 51, 24, (30, 44, 26, 200), 1.4)
    elif race == "elf":
        ellipse(img, 48, 15, head_r + 1, 8, hair)
        rect(img, 48 - head_r, 13, 48 + head_r, 20, hair)
        line(img, 48 - head_r + 1, 16, 48 - head_r - 1, 44, hair, 4.5)
        line(img, 48 + head_r - 1, 16, 48 + head_r + 1, 44, hair, 4.5)
        ellipse(img, 42, 13, 6, 3.4, hair_hi)
        # circlet
        line(img, 38, 22, 58, 22, (245, 224, 130, 235), 1.4)
        spec(img, 48, 22, 1.4)
    else:
        ellipse(img, 48, 15, head_r, 8, hair)
        rect(img, 48 - head_r, 14, 48 + head_r, 20, hair)
        line(img, 48 - head_r + 1, 18, 48 - head_r, 34, hair, 3.5)
        ellipse(img, 42, 13.5, 6, 3.2, hair_hi)

    # face
    eye = p["eye"]
    for ex in (43, 53):
        ellipse(img, ex, 30, 2.4, 3.0, (255, 255, 255, 235))
        circle(img, ex + 0.3, 30.6, 1.7, eye)
        spec(img, ex - 0.4, 29.2, 0.8, 235)
        line(img, ex - 2.4, 26.4, ex + 1.8, 26.0, mul(hair, 0.8), 1.1)  # brow
    # blush + smile
    ellipse(img, 39, 34, 2.6, 1.5, (255, 150, 120, 90))
    ellipse(img, 57, 34, 2.6, 1.5, (255, 150, 120, 90))
    line(img, 44.5, 35.5, 51.5, 35.5, (150, 78, 66, 210), 1.1)
    return img


# ---------------------------------------------------------------------------
# weapons (held on the hero's right, near the hand at ~x76,y58)
# ---------------------------------------------------------------------------
def weapon(kind: str) -> bytearray:
    img = blank()
    steel = (206, 224, 238, 255)
    steel_hi = (245, 251, 255, 255)
    steel_sh = (120, 146, 166, 255)
    wood = (150, 96, 52, 255)
    if kind == "sword":
        line(img, 74, 60, 84, 22, steel_sh, 6)
        line(img, 73.4, 60, 83.4, 22, steel, 4.2)
        line(img, 72.8, 59, 82.4, 24, steel_hi, 1.4)
        line(img, 68, 56, 80, 63, (196, 150, 70, 255), 4)  # guard
        rect(img, 74.5, 60, 78.5, 68, wood)  # grip
        spec(img, 82, 26, 1.4)
    elif kind == "staff":
        line(img, 74, 74, 74, 24, wood, 4)
        line(img, 73, 73, 73, 26, mul(wood, 1.35), 1.3)
        celblob(img, 74, 18, 8, 8, (250, 96, 60, 255), light=1.4)
        circle(img, 74, 18, 3.4, (255, 220, 120, 255))
        spec(img, 71.6, 15.6, 1.6)
    elif kind == "bow":
        for a in range(-58, 59, 3):
            rad = math.radians(a)
            x = 70 + math.cos(rad) * 6 - abs(math.sin(rad)) * 8
            y = 46 + math.sin(rad) * 30
            circle(img, x, y, 2.3, wood)
        for a in range(-58, 59, 6):
            rad = math.radians(a)
            x = 70 + math.cos(rad) * 6 - abs(math.sin(rad)) * 8
            y = 46 + math.sin(rad) * 30
            circle(img, x - 0.6, y, 1.0, mul(wood, 1.4))
        line(img, 62, 18, 62, 74, (240, 236, 205, 210), 1.4)
    elif kind == "shield":
        poly(img, [(60, 40), (84, 34), (86, 56), (73, 74), (60, 60)], steel_sh)
        poly(img, [(62, 42), (82, 37), (84, 55), (72, 70), (62, 58)], (198, 62, 58, 255))
        poly(img, [(66, 45), (78, 42), (79, 55), (72, 64), (66, 55)], (232, 200, 84, 255))
        spec(img, 70, 46, 2.0)
    elif kind == "storm_orb":
        celblob(img, 76, 34, 11, 11, (86, 196, 250, 255), light=1.3)
        line(img, 70, 36, 80, 28, (250, 244, 110, 255), 2.6)
        line(img, 80, 28, 74, 42, (250, 244, 110, 255), 2.6)
        spec(img, 72.5, 30, 2.0)
    elif kind == "sentry_bow":
        rect(img, 66, 48, 86, 54, (120, 84, 52, 255))
        rect(img, 66, 48, 86, 50, (168, 122, 66, 255))
        line(img, 70, 40, 70, 62, (150, 108, 60, 255), 3.4)
        line(img, 82, 40, 82, 62, (150, 108, 60, 255), 3.4)
        line(img, 72, 51, 84, 51, (230, 244, 236, 235), 1.6)
        spec(img, 76, 49, 1.4)
    elif kind == "dagger":
        line(img, 74, 64, 84, 44, steel, 4)
        line(img, 73.4, 63, 83, 45, steel_hi, 1.2)
        line(img, 72, 66, 79, 71, wood, 4)
        spec(img, 82, 47, 1.2)
    elif kind == "censer":
        line(img, 72, 24, 76, 52, (222, 200, 128, 255), 2)
        celblob(img, 77, 58, 8, 7, (196, 150, 78, 255), light=1.35)
        rect(img, 70, 54, 84, 58, (238, 202, 82, 255))
        circle(img, 79, 52, 4, (255, 244, 158, 150))
        spec(img, 74.5, 56, 1.4)
    elif kind == "hammer":
        line(img, 70, 72, 82, 34, wood, 5)
        rect(img, 72, 26, 92, 42, steel_sh)
        rect(img, 73, 27, 90, 40, steel)
        rect(img, 74, 28, 84, 33, steel_hi)
        spec(img, 78, 30, 1.6)
    return img


# ---------------------------------------------------------------------------
# gear (registered to body regions; light upper-left)
# ---------------------------------------------------------------------------
def _plate(img, base, trim):
    """Chest cuirass over the torso."""
    ellipse(img, 48, 51, 17, 15, mul(base, 0.62))
    ellipse(img, 48, 50, 16, 14, base)
    ellipse(img, 43, 46, 8, 7, mul(base, 1.28))
    poly(img, [(40, 39), (56, 39), (52, 47), (48, 50), (44, 47)], mul(base, 0.5))
    # pauldrons
    for sx, s in ((32, 1), (64, -1)):
        celblob(img, sx, 43, 7.5, 6, base, light=1.25)
    line(img, 40, 40, 44, 56, trim, 1.6)
    line(img, 56, 40, 52, 56, trim, 1.6)
    spec(img, 42, 45, 1.8)


def gear(kind: str) -> bytearray:
    img = blank()
    if kind == "vow_plate":
        _plate(img, (150, 162, 176, 255), (238, 214, 128, 235))
        circle(img, 48, 50, 3, (255, 226, 108, 250))  # emblem
        spec(img, 47, 49, 1.2)
    elif kind == "starweave_robe":
        base = (70, 84, 176, 255)
        ellipse(img, 48, 58, 20, 26, mul(base, 0.6))
        poly(img, [(30, 40), (48, 46), (44, 86), (24, 80)], base)
        poly(img, [(66, 40), (48, 46), (52, 86), (72, 80)], base)
        ellipse(img, 42, 48, 7, 9, mul(base, 1.3))
        line(img, 38, 44, 58, 44, (252, 224, 116, 235), 2.4)  # gold hem
        for x, y, r in [(42, 54, 1.5), (56, 60, 1.3), (46, 70, 1.3), (60, 76, 1.1)]:
            circle(img, x, y, r, (255, 244, 166, 240))
            spec(img, x - 0.3, y - 0.3, 0.7, 240)
    elif kind == "windrunner_cloak":
        base = (46, 150, 100, 255)
        # side panels hang from the shoulders and frame the torso (center stays open)
        poly(img, [(34, 37), (40, 41), (30, 88), (16, 80)], mul(base, 0.72))
        poly(img, [(62, 37), (56, 41), (66, 88), (80, 80)], base)
        line(img, 33, 40, 22, 78, (150, 230, 176, 150), 1.6)
        line(img, 63, 40, 74, 78, mul(base, 1.3), 1.6)
        # shoulder collar + gold clasp at the throat
        poly(img, [(40, 36), (56, 36), (54, 43), (42, 43)], mul(base, 0.85))
        celblob(img, 48, 40, 3.6, 3.0, (232, 212, 120, 255))
    elif kind == "thunder_charm":
        line(img, 48, 40, 48, 48, (60, 42, 32, 235), 1.6)
        poly(
            img,
            [(51, 46), (43, 59), (49, 58), (44, 70), (57, 53), (50, 54)],
            (255, 226, 70, 255),
        )
        circle(img, 49, 56, 11, (255, 226, 70, 46))
        spec(img, 49, 52, 1.2)
    elif kind == "saint_bell":
        line(img, 62, 42, 66, 58, (240, 222, 150, 235), 1.8)
        celblob(img, 68, 63, 7, 8, (232, 192, 74, 255), light=1.35)
        rect(img, 61, 58, 75, 62, (248, 220, 96, 240))
        circle(img, 68, 69, 2, (255, 246, 176, 255))
        circle(img, 68, 61, 12, (255, 246, 176, 40))
        spec(img, 65.5, 60, 1.4)
    elif kind == "night_mask":
        poly(img, [(38, 25), (58, 25), (63, 30), (56, 35), (40, 35), (33, 30)],
             (36, 30, 54, 240))
        poly(img, [(38, 25), (58, 25), (60, 28), (38, 28)], (64, 54, 92, 240))
        circle(img, 43, 30, 2.0, (150, 224, 255, 255))
        circle(img, 53, 30, 2.0, (150, 224, 255, 255))
        spec(img, 42.3, 29.4, 0.8, 255)
        spec(img, 52.3, 29.4, 0.8, 255)
    elif kind == "forge_gauntlet":
        for gx in (20.5, 75.5):
            celblob(img, gx, 58, 5.6, 5.2, (196, 96, 50, 255), light=1.3)
            circle(img, gx, 56, 2.4, (255, 190, 84, 220))
        spec(img, 74, 56, 1.2)
    elif kind == "warden_scope":
        celblob(img, 66, 29, 7.5, 7.5, (70, 90, 100, 255), light=1.2)
        circle(img, 66, 29, 4.5, (60, 226, 156, 235))
        circle(img, 66, 29, 2, (18, 46, 42, 245))
        line(img, 60, 29, 72, 29, (200, 255, 224, 235), 1.0)
        line(img, 66, 23, 66, 35, (200, 255, 224, 235), 1.0)
        spec(img, 63.5, 26.5, 1.2)
    elif kind == "carrot_halo":
        for a in range(0, 360, 6):
            rad = math.radians(a)
            x = 48 + math.cos(rad) * 15
            y = 8 + math.sin(rad) * 3.6
            circle(img, x, y, 2.0, (255, 214, 78, 200))
        for a in range(0, 360, 12):
            rad = math.radians(a)
            x = 48 + math.cos(rad) * 15
            y = 8 + math.sin(rad) * 3.6
            spec(img, x, y, 0.9, 230)
    elif kind == "wayfarer_boots":
        _boots(img, (92, 66, 46, 255), (216, 170, 96, 235))
    elif kind == "bloodstep_greaves":
        _boots(img, (132, 44, 52, 255), (240, 104, 92, 235))
        for lx in (41.5, 54.5):
            spec(img, lx, 76, 1.2, 150)
    elif kind == "starpath_sandals":
        _boots(img, (78, 84, 158, 255), (198, 208, 255, 235))
        for lx in (41.5, 54.5):
            circle(img, lx, 75, 1.4, (255, 240, 158, 235))
    elif kind == "engineer_treads":
        _boots(img, (86, 78, 66, 255), (232, 168, 70, 240))
        for lx in (41.5, 54.5):
            circle(img, lx - 2, 78, 1.6, (236, 172, 72, 240))
            circle(img, lx + 2, 78, 1.6, (236, 172, 72, 240))
    elif kind == "summoner_greaves":
        _boots(img, (74, 56, 116, 255), (150, 118, 214, 235))
        for lx in (41.5, 54.5):
            circle(img, lx, 74, 3.4, (128, 244, 160, 90))
    elif kind == "carrot_wings":
        # a fan of tapering feathers arcing outward+down from each shoulder
        for sx, s in ((36, 1), (60, -1)):
            for k in range(4):
                t = k / 3.0
                cx = sx - s * (5 + t * 15)
                cy = 41 + t * 20
                r = 6.2 - t * 1.9
                ellipse(img, cx, cy, r, r * 1.4, mul((255, 172, 72, 255), 0.82))
                ellipse(img, cx - s * 0.8, cy - 1.2, r * 0.92, r * 1.2, (255, 178, 76, 235))
                ellipse(img, cx - s * 1.6, cy - 2.2, r * 0.42, r * 0.7, (255, 226, 150, 230))
        celblob(img, 48, 82, 3.2, 2.4, (110, 220, 96, 220))
    return img


def _boots(img, base, trim):
    for lx in (41.5, 54.5):
        rect(img, lx - 3.6, 70, lx + 3.6, 79, mul(base, 0.7))
        rect(img, lx - 3.2, 70, lx + 3.2, 78, base)
        ellipse(img, lx, 80.5, 5.6, 3.6, mul(base, 0.8))
        ellipse(img, lx - 1.2, 79.5, 2.6, 1.7, mul(base, 1.35))
        line(img, lx - 3, 73, lx + 3, 73, trim, 1.3)


WEAPONS = [
    (100, "sword", "weapon_sword.png"),
    (101, "staff", "weapon_staff.png"),
    (102, "bow", "weapon_bow.png"),
    (103, "shield", "weapon_shield.png"),
    (104, "storm_orb", "weapon_storm_orb.png"),
    (105, "sentry_bow", "weapon_sentry_bow.png"),
    (106, "dagger", "weapon_dagger.png"),
    (107, "censer", "weapon_censer.png"),
    (108, "hammer", "weapon_hammer.png"),
]

GEAR = [
    (200, "vow_plate", "gear_vow_plate.png", 20),
    (201, "starweave_robe", "gear_starweave_robe.png", 20),
    (202, "windrunner_cloak", "gear_windrunner_cloak.png", 20),
    (210, "thunder_charm", "gear_thunder_charm.png", 21),
    (211, "saint_bell", "gear_saint_bell.png", 21),
    (212, "night_mask", "gear_night_mask.png", 21),
    (220, "forge_gauntlet", "gear_forge_gauntlet.png", 22),
    (221, "warden_scope", "gear_warden_scope.png", 22),
    (222, "carrot_halo", "gear_carrot_halo.png", 22),
    (230, "wayfarer_boots", "gear_wayfarer_boots.png", 23),
    (231, "bloodstep_greaves", "gear_bloodstep_greaves.png", 23),
    (232, "starpath_sandals", "gear_starpath_sandals.png", 23),
    (233, "engineer_treads", "gear_engineer_treads.png", 23),
    (234, "summoner_greaves", "gear_summoner_greaves.png", 23),
    (235, "carrot_wings", "gear_carrot_wings.png", 23),
]


def point(x: float = 0.0, y: float = 0.0) -> str:
    return f"{{x: {x:.1f}, y: {y:.1f}}}"


def manifest() -> str:
    lines: list[str] = [
        "meta:",
        "  name: protect-carrot-hero-paperdoll",
        "  version: 1",
        "dolls:",
    ]
    for idx, race in enumerate(["human", "elf", "orc"]):
        lines.extend(
            [
                f"- id: {idx}",
                f"  desc: {race}",
                f"  width: {OUTPUT_SIZE}",
                f"  height: {OUTPUT_SIZE}",
                f"  offset: {point()}",
                "  slots: [20, 23, 10, 21, 22]",
                f"  path: body_{race}.png",
            ]
        )

    lines.extend(
        [
            "slots:",
            "- id: 10",
            "  desc: class weapon",
            "  required: true",
            "  constrainted: false",
            f"  positions: [{point()}]",
            f"  anchor: {point()}",
            "  candidates: [100, 101, 102, 103, 104, 105, 106, 107, 108]",
            "- id: 20",
            "  desc: armor",
            "  required: false",
            "  constrainted: false",
            f"  positions: [{point()}]",
            f"  anchor: {point()}",
            "  candidates: [200, 201, 202]",
            "- id: 21",
            "  desc: charm",
            "  required: false",
            "  constrainted: false",
            f"  positions: [{point()}]",
            f"  anchor: {point()}",
            "  candidates: [210, 211, 212]",
            "- id: 22",
            "  desc: relic",
            "  required: false",
            "  constrainted: false",
            f"  positions: [{point()}]",
            f"  anchor: {point()}",
            "  candidates: [220, 221, 222]",
            "- id: 23",
            "  desc: boots",
            "  required: false",
            "  constrainted: false",
            f"  positions: [{point()}]",
            f"  anchor: {point()}",
            "  candidates: [230, 231, 232, 233, 234, 235]",
            "fragments:",
        ]
    )

    for frag_id, name, path in WEAPONS:
        lines.extend(
            [f"- id: {frag_id}", f"  desc: {name}", f"  pivot: {point()}", f"  path: {path}"]
        )
    for frag_id, name, path, _slot in GEAR:
        lines.extend(
            [f"- id: {frag_id}", f"  desc: {name}", f"  pivot: {point()}", f"  path: {path}"]
        )
    return "\n".join(lines) + "\n"


def add_file(tar: tarfile.TarFile, name: str, data: bytes) -> None:
    info = tarfile.TarInfo(name)
    info.size = len(data)
    info.mtime = 0
    info.mode = 0o644
    tar.addfile(info, io.BytesIO(data))


def main() -> None:
    OUT.parent.mkdir(parents=True, exist_ok=True)

    files: dict[str, bytes] = {"manifest.yml": manifest().encode("utf-8")}
    for race in ["human", "elf", "orc"]:
        files[f"body_{race}.png"] = encode(body(race), outline=True, thickness=2)
    for _frag_id, name, path in WEAPONS:
        files[path] = encode(weapon(name), outline=True, thickness=2)
    for _frag_id, name, path, _slot in GEAR:
        files[path] = encode(gear(name), outline=True, thickness=1)

    with tarfile.open(OUT, "w") as tar:
        for name in sorted(files):
            add_file(tar, name, files[name])

    print(f"wrote {OUT} ({OUT.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
