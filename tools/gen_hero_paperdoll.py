#!/usr/bin/env python3
"""Generate the hero paperdoll `.ppd` asset.

The output is a tar archive understood by `paperdoll-tar`: `manifest.yml` plus
RGBA PNG layers. The base bodies are deterministic local drawings for human,
elf, and orc. Class identity is represented only by the equipped weapon
fragment, so the battlefield hero and the paperdoll preview stay race-driven
and visually consistent.
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


def blank() -> bytearray:
    return bytearray(CANVAS_SIZE * CANVAS_SIZE * 4)


def clamp(v: int) -> int:
    return max(0, min(255, v))


def blend_px(img: bytearray, x: int, y: int, color: tuple[int, int, int, int]) -> None:
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
    img[i] = clamp(round((r * sa + dr * oa * (1.0 - sa)) / out_a))
    img[i + 1] = clamp(round((g * sa + dg * oa * (1.0 - sa)) / out_a))
    img[i + 2] = clamp(round((b * sa + db * oa * (1.0 - sa)) / out_a))
    img[i + 3] = clamp(round(out_a * 255.0))


def rect(img: bytearray, x0: int, y0: int, x1: int, y1: int, color) -> None:
    sx0 = math.floor(x0 * RENDER_SCALE)
    sy0 = math.floor(y0 * RENDER_SCALE)
    sx1 = math.ceil(x1 * RENDER_SCALE)
    sy1 = math.ceil(y1 * RENDER_SCALE)
    for y in range(sy0, sy1):
        for x in range(sx0, sx1):
            blend_px(img, x, y, color)


def ellipse(img: bytearray, cx: float, cy: float, rx: float, ry: float, color) -> None:
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


def circle(img: bytearray, cx: float, cy: float, r: float, color) -> None:
    ellipse(img, cx, cy, r, r, color)


def point_in_poly(x: float, y: float, pts: list[tuple[float, float]]) -> bool:
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


def poly(img: bytearray, pts: list[tuple[float, float]], color) -> None:
    pts = [(x * RENDER_SCALE, y * RENDER_SCALE) for x, y in pts]
    min_x = math.floor(min(p[0] for p in pts))
    max_x = math.ceil(max(p[0] for p in pts))
    min_y = math.floor(min(p[1] for p in pts))
    max_y = math.ceil(max(p[1] for p in pts))
    for y in range(min_y, max_y + 1):
        for x in range(min_x, max_x + 1):
            if point_in_poly(x + 0.5, y + 0.5, pts):
                blend_px(img, x, y, color)


def line(img: bytearray, x0: float, y0: float, x1: float, y1: float, color, width=2.0) -> None:
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
        circle(img, x, y, width / 2.0, color)


def png_bytes(img: bytearray) -> bytes:
    def chunk(kind: bytes, data: bytes) -> bytes:
        return (
            struct.pack(">I", len(data))
            + kind
            + data
            + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)
        )

    raw = bytearray()
    out = downsample_rgba(img)
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


def downsample_rgba(img: bytearray) -> bytearray:
    if CANVAS_SIZE == OUTPUT_SIZE:
        return img
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
                out[oi] = clamp(round(r / a))
                out[oi + 1] = clamp(round(g / a))
                out[oi + 2] = clamp(round(b / a))
            out[oi + 3] = clamp(round(a / n))
    return out


def body(race: str) -> bytearray:
    img = blank()
    skin, shade, hair, cloth, eye = {
        "human": (
            (225, 168, 112, 255),
            (174, 111, 80, 165),
            (70, 45, 34, 255),
            (156, 93, 63, 245),
            (34, 29, 28, 255),
        ),
        "elf": (
            (234, 201, 137, 255),
            (174, 137, 82, 150),
            (232, 217, 132, 255),
            (82, 128, 82, 245),
            (25, 82, 58, 255),
        ),
        "orc": (
            (93, 171, 86, 255),
            (47, 113, 55, 170),
            (34, 70, 31, 255),
            (104, 77, 48, 245),
            (32, 36, 23, 255),
        ),
    }[race]

    wide = race == "orc"
    slim = race == "elf"
    torso_rx = 19 if wide else 16 if slim else 17
    head_r = 18 if wide else 16
    arm_w = 7 if wide else 5.5 if slim else 6

    ellipse(img, 48, 80, 22 if wide else 20, 5, (0, 0, 0, 62))

    rect(img, 36, 57, 44, 78, skin)
    rect(img, 52, 57, 60, 78, skin)
    rect(img, 34, 76, 45, 81, (44, 36, 31, 255))
    rect(img, 51, 76, 62, 81, (44, 36, 31, 255))

    ellipse(img, 48, 47, torso_rx, 22, skin)
    poly(img, [(48 - torso_rx, 40), (42, 36), (42, 68), (32, 62)], shade)
    poly(img, [(48 + torso_rx, 40), (54, 36), (54, 68), (64, 62)], shade)
    line(img, 48, 39, 48, 61, (255, 222, 169, 72), 1.5)
    ellipse(img, 48, 58, 13, 6, shade)
    rect(img, 35, 57, 61, 61, (64, 44, 33, 210))
    poly(img, [(38, 60), (58, 60), (55, 70), (48, 74), (41, 70)], cloth)

    line(img, 29, 39, 19, 58, skin, arm_w)
    line(img, 67, 39, 78, 58, skin, arm_w)
    circle(img, 19, 59, 4.2 if wide else 3.7, skin)
    circle(img, 78, 59, 4.2 if wide else 3.7, skin)

    if race == "elf":
        poly(img, [(35, 22), (21, 28), (36, 32)], skin)
        poly(img, [(61, 22), (75, 28), (60, 32)], skin)
        poly(img, [(31, 27), (23, 29), (32, 30)], (184, 132, 98, 165))
        poly(img, [(65, 27), (73, 29), (64, 30)], (184, 132, 98, 165))

    circle(img, 48, 27, head_r, skin)
    if race == "orc":
        ellipse(img, 48, 20, 19, 10, hair)
        rect(img, 31, 24, 65, 30, hair)
        poly(img, [(38, 38), (41, 47), (44, 38)], (242, 235, 198, 255))
        poly(img, [(52, 38), (55, 47), (58, 38)], (242, 235, 198, 255))
        ellipse(img, 48, 38, 10, 3, (44, 91, 45, 160))
    elif race == "elf":
        ellipse(img, 48, 17, 18, 8, hair)
        rect(img, 33, 21, 63, 27, hair)
        line(img, 37, 20, 31, 45, hair, 4)
        line(img, 59, 20, 65, 45, hair, 4)
    else:
        ellipse(img, 48, 18, 18, 8, hair)
        rect(img, 33, 22, 63, 27, hair)
        line(img, 36, 22, 34, 42, hair, 3)

    circle(img, 42, 30, 2, eye)
    circle(img, 54, 30, 2, eye)
    rect(img, 43, 38, 54, 40, (93, 47, 39, 190))
    circle(img, 42, 29, 0.9, (255, 255, 235, 230))
    circle(img, 54, 29, 0.9, (255, 255, 235, 230))
    return img


def weapon(kind: str) -> bytearray:
    img = blank()
    if kind == "sword":
        line(img, 61, 62, 82, 25, (215, 235, 244, 255), 5)
        line(img, 62, 61, 83, 24, (78, 103, 122, 255), 1.5)
        line(img, 54, 58, 68, 67, (183, 107, 50, 255), 4)
    elif kind == "staff":
        line(img, 72, 73, 72, 22, (108, 72, 42, 255), 4)
        circle(img, 72, 18, 8, (246, 83, 54, 225))
        circle(img, 72, 18, 4, (255, 206, 91, 255))
    elif kind == "bow":
        for a in range(-55, 56, 4):
            rad = math.radians(a)
            x = 21 + math.cos(rad) * 13
            y = 49 + math.sin(rad) * 32
            circle(img, x, y, 2.4, (164, 108, 55, 255))
        line(img, 33, 23, 33, 75, (237, 232, 197, 210), 1.5)
        line(img, 22, 50, 63, 50, (225, 245, 235, 255), 2)
    elif kind == "shield":
        poly(img, [(24, 40), (43, 35), (48, 53), (41, 73), (24, 66)], (201, 54, 53, 255))
        poly(img, [(27, 43), (39, 41), (42, 54), (37, 66), (27, 62)], (238, 206, 82, 255))
        line(img, 67, 68, 84, 35, (176, 188, 196, 255), 5)
    elif kind == "storm_orb":
        circle(img, 72, 34, 11, (73, 191, 250, 190))
        line(img, 64, 36, 76, 29, (246, 242, 93, 255), 3)
        line(img, 76, 29, 70, 43, (246, 242, 93, 255), 3)
    elif kind == "sentry_bow":
        rect(img, 17, 49, 55, 54, (107, 74, 48, 255))
        line(img, 20, 38, 20, 65, (160, 113, 59, 255), 4)
        line(img, 50, 38, 50, 65, (160, 113, 59, 255), 4)
        line(img, 24, 52, 71, 52, (200, 235, 225, 255), 2)
    elif kind == "dagger":
        line(img, 28, 63, 18, 43, (214, 225, 226, 255), 4)
        line(img, 68, 63, 79, 43, (214, 225, 226, 255), 4)
        line(img, 30, 65, 38, 72, (96, 58, 42, 255), 4)
        line(img, 66, 65, 58, 72, (96, 58, 42, 255), 4)
    elif kind == "censer":
        line(img, 68, 28, 72, 56, (218, 198, 126, 255), 2)
        ellipse(img, 73, 61, 9, 7, (126, 82, 54, 255))
        rect(img, 66, 58, 80, 63, (235, 199, 77, 255))
        circle(img, 76, 55, 5, (255, 243, 153, 120))
    elif kind == "hammer":
        line(img, 65, 70, 78, 38, (116, 76, 45, 255), 5)
        rect(img, 68, 27, 88, 41, (152, 163, 163, 255))
        rect(img, 72, 30, 84, 37, (222, 238, 230, 255))
    return img


def gear(kind: str) -> bytearray:
    img = blank()
    if kind == "vow_plate":
        poly(img, [(34, 34), (46, 30), (50, 44), (44, 58), (35, 54)], (116, 125, 136, 224))
        poly(img, [(62, 34), (50, 30), (46, 44), (52, 58), (61, 54)], (116, 125, 136, 224))
        poly(img, [(32, 36), (21, 41), (29, 49), (38, 45)], (161, 169, 174, 214))
        poly(img, [(64, 36), (75, 41), (67, 49), (58, 45)], (161, 169, 174, 214))
        line(img, 35, 36, 43, 56, (228, 211, 135, 220), 1.8)
        line(img, 61, 36, 53, 56, (228, 211, 135, 220), 1.8)
        circle(img, 48, 44, 3, (255, 229, 108, 245))
        circle(img, 29, 43, 2, (255, 231, 130, 210))
        circle(img, 67, 43, 2, (255, 231, 130, 210))
    elif kind == "starweave_robe":
        poly(img, [(29, 35), (45, 40), (39, 84), (21, 78)], (51, 62, 154, 154))
        poly(img, [(67, 35), (51, 40), (57, 84), (75, 78)], (51, 62, 154, 154))
        line(img, 32, 38, 39, 82, (196, 185, 255, 150), 1.6)
        line(img, 64, 38, 57, 82, (196, 185, 255, 150), 1.6)
        line(img, 37, 43, 59, 43, (252, 222, 110, 210), 2.2)
        for x, y, r in [(40, 50, 1.4), (55, 58, 1.2), (46, 68, 1.1), (61, 73, 1.0)]:
            circle(img, x, y, r, (255, 239, 150, 230))
    elif kind == "windrunner_cloak":
        poly(img, [(29, 33), (46, 38), (35, 88), (13, 75)], (34, 128, 82, 166))
        poly(img, [(67, 33), (50, 38), (61, 88), (83, 75)], (34, 128, 82, 166))
        line(img, 31, 35, 18, 74, (106, 221, 157, 145), 1.7)
        line(img, 65, 35, 78, 74, (106, 221, 157, 145), 1.7)
        line(img, 38, 40, 58, 40, (198, 238, 188, 185), 2)
    elif kind == "thunder_charm":
        line(img, 48, 38, 48, 47, (54, 38, 30, 220), 1.5)
        poly(img, [(50, 45), (43, 57), (49, 56), (45, 67), (56, 52), (50, 53)], (255, 224, 63, 238))
        circle(img, 49, 55, 10, (255, 224, 63, 38))
    elif kind == "saint_bell":
        line(img, 63, 43, 67, 59, (239, 220, 149, 220), 1.6)
        ellipse(img, 68, 64, 6, 8, (227, 186, 61, 240))
        rect(img, 62, 59, 74, 63, (246, 218, 93, 232))
        circle(img, 68, 70, 2, (255, 245, 170, 255))
        circle(img, 68, 62, 11, (255, 245, 170, 32))
    elif kind == "night_mask":
        poly(img, [(39, 26), (57, 26), (62, 30), (56, 34), (40, 34), (34, 30)], (34, 29, 49, 218))
        circle(img, 43, 30, 1.8, (178, 231, 255, 250))
        circle(img, 53, 30, 1.8, (178, 231, 255, 250))
    elif kind == "forge_gauntlet":
        rect(img, 61, 53, 69, 64, (184, 89, 47, 238))
        rect(img, 29, 53, 37, 64, (184, 89, 47, 238))
        line(img, 31, 56, 36, 56, (255, 190, 85, 205), 1.5)
        line(img, 63, 56, 68, 56, (255, 190, 85, 205), 1.5)
        circle(img, 66, 52, 4, (255, 188, 72, 155))
    elif kind == "warden_scope":
        circle(img, 65, 30, 7, (77, 230, 157, 205))
        circle(img, 65, 30, 3.5, (22, 55, 50, 245))
        line(img, 59, 30, 71, 30, (180, 255, 214, 230), 1)
        line(img, 65, 24, 65, 36, (180, 255, 214, 230), 1)
    elif kind == "carrot_halo":
        for a in range(0, 360, 8):
            rad = math.radians(a)
            x = 48 + math.cos(rad) * 16
            y = 7 + math.sin(rad) * 4.0
            circle(img, x, y, 1.8, (255, 218, 74, 142))
        circle(img, 48, 7, 2, (255, 248, 180, 220))
    elif kind == "wayfarer_boots":
        rect(img, 32, 75, 45, 82, (77, 58, 43, 238))
        rect(img, 51, 75, 64, 82, (77, 58, 43, 238))
        line(img, 34, 76, 43, 76, (211, 164, 92, 210), 1.4)
        line(img, 53, 76, 62, 76, (211, 164, 92, 210), 1.4)
    elif kind == "bloodstep_greaves":
        rect(img, 31, 73, 46, 82, (120, 38, 48, 238))
        rect(img, 50, 73, 65, 82, (120, 38, 48, 238))
        line(img, 34, 75, 44, 80, (236, 91, 81, 220), 1.6)
        line(img, 62, 75, 52, 80, (236, 91, 81, 220), 1.6)
        circle(img, 39, 77, 2.0, (255, 122, 91, 130))
    elif kind == "starpath_sandals":
        rect(img, 32, 76, 45, 81, (72, 77, 150, 226))
        rect(img, 51, 76, 64, 81, (72, 77, 150, 226))
        for x, y in [(36, 75), (43, 79), (55, 75), (62, 79)]:
            circle(img, x, y, 1.4, (255, 238, 150, 230))
    elif kind == "engineer_treads":
        rect(img, 30, 73, 47, 83, (78, 71, 60, 240))
        rect(img, 49, 73, 66, 83, (78, 71, 60, 240))
        for x in [35, 42, 54, 61]:
            circle(img, x, 77, 2.0, (228, 163, 64, 235))
        line(img, 31, 82, 46, 82, (34, 32, 30, 210), 1.6)
        line(img, 50, 82, 65, 82, (34, 32, 30, 210), 1.6)
    elif kind == "summoner_greaves":
        rect(img, 31, 74, 46, 82, (59, 45, 91, 232))
        rect(img, 50, 74, 65, 82, (59, 45, 91, 232))
        circle(img, 38, 76, 4, (120, 241, 154, 92))
        circle(img, 58, 76, 4, (120, 241, 154, 92))
        line(img, 34, 74, 43, 81, (115, 255, 172, 190), 1.2)
        line(img, 62, 74, 53, 81, (115, 255, 172, 190), 1.2)
    elif kind == "carrot_wings":
        poly(img, [(30, 76), (16, 72), (26, 84), (40, 81)], (255, 156, 54, 190))
        poly(img, [(66, 76), (80, 72), (70, 84), (56, 81)], (255, 156, 54, 190))
        rect(img, 34, 75, 45, 82, (246, 201, 92, 232))
        rect(img, 51, 75, 62, 82, (246, 201, 92, 232))
        circle(img, 48, 80, 3, (97, 221, 93, 180))
    return img


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
            [
                f"- id: {frag_id}",
                f"  desc: {name}",
                f"  pivot: {point()}",
                f"  path: {path}",
            ]
        )
    for frag_id, name, path, _slot in GEAR:
        lines.extend(
            [
                f"- id: {frag_id}",
                f"  desc: {name}",
                f"  pivot: {point()}",
                f"  path: {path}",
            ]
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

    files: dict[str, bytes] = {
        "manifest.yml": manifest().encode("utf-8"),
    }
    for race in ["human", "elf", "orc"]:
        files[f"body_{race}.png"] = png_bytes(body(race))
    for _frag_id, name, path in WEAPONS:
        files[path] = png_bytes(weapon(name))
    for _frag_id, name, path, _slot in GEAR:
        files[path] = png_bytes(gear(name))

    with tarfile.open(OUT, "w") as tar:
        for name in sorted(files):
            add_file(tar, name, files[name])

    print(f"wrote {OUT} ({OUT.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
