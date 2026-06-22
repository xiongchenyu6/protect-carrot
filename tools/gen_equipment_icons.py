#!/usr/bin/env python3
"""Generate readable fallback equipment icons.

StoryOS/ComfyUI can replace these with painterly relic icons later. This script
keeps the game shippable and the asset verifier green when no remote generator is
available.
"""

from __future__ import annotations

import math
from pathlib import Path

from PIL import Image, ImageDraw

S = 64
SS = 4
Z = S * SS
ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "assets" / "sprites" / "equipment"


def rgb(h: int) -> tuple[int, int, int]:
    return ((h >> 16) & 255, (h >> 8) & 255, h & 255)


def lighten(c: tuple[int, int, int], f: float) -> tuple[int, int, int]:
    return tuple(min(255, int(v + (255 - v) * f)) for v in c)


def darken(c: tuple[int, int, int], f: float) -> tuple[int, int, int]:
    return tuple(max(0, int(v * (1 - f))) for v in c)


def rarity_color(rarity: str) -> tuple[int, int, int]:
    return {
        "common": rgb(0xB8C0AA),
        "uncommon": rgb(0x45D483),
        "rare": rgb(0x4AA3FF),
        "epic": rgb(0xB66DFF),
        "legendary": rgb(0xFFA93D),
        "mythic": rgb(0xFF4F6D),
    }[rarity]


ITEMS = [
    ("rusty_sight", "common", "sight"),
    ("carrot_sigil", "common", "sigil"),
    ("bone_fletching", "common", "feather"),
    ("saltpeter_keg", "uncommon", "keg"),
    ("prism_shard", "uncommon", "crystal"),
    ("frost_lens", "uncommon", "lens"),
    ("ember_core", "rare", "flame"),
    ("venom_vial", "rare", "vial"),
    ("thunder_coil", "rare", "coil"),
    ("shadow_seal", "rare", "seal"),
    ("bulwark_plate", "epic", "plate"),
    ("clockwork_trigger", "epic", "gear"),
    ("witch_salt", "epic", "salt"),
    ("deep_one_scale", "epic", "scale"),
    ("cultist_manual", "legendary", "book"),
    ("star_metal_barrel", "legendary", "barrel"),
    ("void_capacitor", "legendary", "capacitor"),
    ("sainted_gear", "legendary", "relic_gear"),
    ("kraken_heart", "mythic", "heart"),
    ("azathoth_eye", "mythic", "eye"),
]


def new_icon(rarity: str) -> tuple[Image.Image, ImageDraw.ImageDraw]:
    img = Image.new("RGBA", (Z, Z), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    c = Z // 2
    edge = rarity_color(rarity)
    fill = darken(edge, 0.62)
    r = int(Z * 0.43)
    d.rounded_rectangle(
        [c - r, c - r, c + r, c + r],
        radius=int(Z * 0.13),
        fill=fill + (230,),
        outline=edge + (255,),
        width=int(Z * 0.045),
    )
    d.rounded_rectangle(
        [c - int(r * 0.78), c - int(r * 0.78), c + int(r * 0.78), c + int(r * 0.78)],
        radius=int(Z * 0.10),
        outline=lighten(edge, 0.18) + (150,),
        width=int(Z * 0.018),
    )
    return img, d


def poly_points(cx: int, cy: int, radius: float, sides: int, rot: float = -90.0):
    return [
        (
            cx + math.cos(math.radians(rot + i * 360 / sides)) * radius,
            cy + math.sin(math.radians(rot + i * 360 / sides)) * radius,
        )
        for i in range(sides)
    ]


def draw_symbol(d: ImageDraw.ImageDraw, shape: str, col: tuple[int, int, int]) -> None:
    c = Z // 2
    w = int(Z * 0.055)
    bright = lighten(col, 0.32)
    dark = darken(col, 0.35)
    if shape == "sight":
        r = int(Z * 0.20)
        d.ellipse([c - r, c - r, c + r, c + r], outline=bright + (255,), width=w)
        d.line([c - r * 2, c, c + r * 2, c], fill=bright + (255,), width=w)
        d.line([c, c - r * 2, c, c + r * 2], fill=bright + (255,), width=w)
    elif shape == "sigil":
        d.polygon(poly_points(c, c, Z * 0.22, 5), fill=bright + (255,), outline=dark + (255,))
        d.ellipse([c - Z * 0.08, c - Z * 0.08, c + Z * 0.08, c + Z * 0.08], fill=dark + (255,))
    elif shape == "feather":
        d.ellipse([c - Z * 0.12, c - Z * 0.30, c + Z * 0.18, c + Z * 0.24], fill=bright + (255,))
        d.line([c - Z * 0.18, c + Z * 0.28, c + Z * 0.16, c - Z * 0.25], fill=dark + (255,), width=w)
    elif shape == "keg":
        d.rounded_rectangle([c - Z * 0.20, c - Z * 0.24, c + Z * 0.20, c + Z * 0.24], radius=w, fill=bright + (255,), outline=dark + (255,), width=w)
        d.line([c - Z * 0.23, c - Z * 0.08, c + Z * 0.23, c - Z * 0.08], fill=dark + (255,), width=w)
        d.line([c - Z * 0.23, c + Z * 0.10, c + Z * 0.23, c + Z * 0.10], fill=dark + (255,), width=w)
    elif shape == "crystal":
        d.polygon(poly_points(c, c, Z * 0.29, 6), fill=bright + (255,), outline=dark + (255,))
        d.line([c, c - Z * 0.26, c, c + Z * 0.26], fill=lighten(bright, 0.25) + (210,), width=w)
    elif shape == "lens":
        d.ellipse([c - Z * 0.25, c - Z * 0.18, c + Z * 0.25, c + Z * 0.18], fill=bright + (230,), outline=dark + (255,), width=w)
        d.ellipse([c - Z * 0.10, c - Z * 0.07, c + Z * 0.10, c + Z * 0.07], fill=(255, 255, 255, 180))
    elif shape == "flame":
        d.polygon([(c, c - Z * 0.30), (c + Z * 0.22, c + Z * 0.08), (c, c + Z * 0.30), (c - Z * 0.22, c + Z * 0.08)], fill=bright + (255,), outline=dark + (255,))
        d.polygon([(c, c - Z * 0.10), (c + Z * 0.09, c + Z * 0.10), (c, c + Z * 0.20), (c - Z * 0.09, c + Z * 0.10)], fill=(255, 236, 120, 245))
    elif shape == "vial":
        d.rounded_rectangle([c - Z * 0.13, c - Z * 0.25, c + Z * 0.13, c + Z * 0.26], radius=w, fill=bright + (235,), outline=dark + (255,), width=w)
        d.rectangle([c - Z * 0.08, c - Z * 0.34, c + Z * 0.08, c - Z * 0.22], fill=dark + (255,))
    elif shape == "coil":
        for i in range(4):
            y = c - int(Z * 0.20) + i * int(Z * 0.13)
            d.arc([c - Z * 0.24, y - Z * 0.08, c + Z * 0.24, y + Z * 0.08], 0, 300, fill=bright + (255,), width=w)
        d.polygon([(c + Z * 0.04, c - Z * 0.29), (c - Z * 0.04, c), (c + Z * 0.10, c), (c - Z * 0.02, c + Z * 0.29)], fill=(255, 245, 120, 245))
    elif shape == "seal":
        d.ellipse([c - Z * 0.22, c - Z * 0.22, c + Z * 0.22, c + Z * 0.22], fill=bright + (255,), outline=dark + (255,), width=w)
        d.line([c - Z * 0.13, c, c + Z * 0.13, c], fill=dark + (255,), width=w)
    elif shape == "plate":
        d.polygon(poly_points(c, c, Z * 0.28, 6, -30), fill=bright + (255,), outline=dark + (255,))
        d.line([c, c - Z * 0.25, c, c + Z * 0.25], fill=dark + (255,), width=w)
    elif shape in ("gear", "relic_gear"):
        for a in range(0, 360, 45):
            ax = math.cos(math.radians(a))
            ay = math.sin(math.radians(a))
            d.rectangle([c + ax * Z * 0.20 - w, c + ay * Z * 0.20 - w, c + ax * Z * 0.20 + w, c + ay * Z * 0.20 + w], fill=dark + (255,))
        d.ellipse([c - Z * 0.23, c - Z * 0.23, c + Z * 0.23, c + Z * 0.23], fill=bright + (255,), outline=dark + (255,), width=w)
        d.ellipse([c - Z * 0.09, c - Z * 0.09, c + Z * 0.09, c + Z * 0.09], fill=dark + (255,))
    elif shape == "salt":
        for i in range(6):
            a = i * 60
            ax = math.cos(math.radians(a))
            ay = math.sin(math.radians(a))
            d.line([c, c, c + ax * Z * 0.25, c + ay * Z * 0.25], fill=bright + (255,), width=w)
        d.ellipse([c - Z * 0.08, c - Z * 0.08, c + Z * 0.08, c + Z * 0.08], fill=(255, 255, 255, 230))
    elif shape == "scale":
        for i in range(3):
            x = c - int(Z * 0.16) + i * int(Z * 0.16)
            d.pieslice([x - Z * 0.13, c - Z * 0.18, x + Z * 0.13, c + Z * 0.18], 180, 360, fill=bright + (255,), outline=dark + (255,))
    elif shape == "book":
        d.rounded_rectangle([c - Z * 0.24, c - Z * 0.28, c + Z * 0.22, c + Z * 0.25], radius=w, fill=bright + (255,), outline=dark + (255,), width=w)
        d.line([c - Z * 0.03, c - Z * 0.26, c - Z * 0.03, c + Z * 0.24], fill=dark + (255,), width=w)
    elif shape == "barrel":
        d.rounded_rectangle([c - Z * 0.26, c - Z * 0.12, c + Z * 0.28, c + Z * 0.12], radius=w, fill=bright + (255,), outline=dark + (255,), width=w)
        d.ellipse([c + Z * 0.18, c - Z * 0.12, c + Z * 0.33, c + Z * 0.12], fill=dark + (255,))
    elif shape == "capacitor":
        d.rectangle([c - Z * 0.18, c - Z * 0.24, c + Z * 0.18, c + Z * 0.24], fill=bright + (255,), outline=dark + (255,))
        d.line([c - Z * 0.30, c - Z * 0.16, c - Z * 0.18, c - Z * 0.16], fill=bright + (255,), width=w)
        d.line([c + Z * 0.18, c + Z * 0.16, c + Z * 0.30, c + Z * 0.16], fill=bright + (255,), width=w)
    elif shape == "heart":
        d.ellipse([c - Z * 0.23, c - Z * 0.24, c, c + Z * 0.02], fill=bright + (255,), outline=dark + (255,), width=w)
        d.ellipse([c, c - Z * 0.24, c + Z * 0.23, c + Z * 0.02], fill=bright + (255,), outline=dark + (255,), width=w)
        d.polygon([(c - Z * 0.24, c - Z * 0.04), (c + Z * 0.24, c - Z * 0.04), (c, c + Z * 0.30)], fill=bright + (255,), outline=dark + (255,))
    elif shape == "eye":
        d.ellipse([c - Z * 0.30, c - Z * 0.17, c + Z * 0.30, c + Z * 0.17], fill=bright + (255,), outline=dark + (255,), width=w)
        d.ellipse([c - Z * 0.12, c - Z * 0.12, c + Z * 0.12, c + Z * 0.12], fill=dark + (255,))
        d.ellipse([c - Z * 0.04, c - Z * 0.04, c + Z * 0.04, c + Z * 0.04], fill=(255, 255, 255, 230))


def save_icon(name: str, rarity: str, shape: str) -> None:
    img, d = new_icon(rarity)
    draw_symbol(d, shape, rarity_color(rarity))
    out = OUT / f"{name}.png"
    out.parent.mkdir(parents=True, exist_ok=True)
    img.resize((S, S), Image.Resampling.LANCZOS).save(out)


def main() -> int:
    for name, rarity, shape in ITEMS:
        save_icon(name, rarity, shape)
    print(f"generated {len(ITEMS)} equipment icons into {OUT.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
