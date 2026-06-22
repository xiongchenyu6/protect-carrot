#!/usr/bin/env python3
"""Generate 100 species-level monster sprite placeholders.

These are not meant to replace StoryOS/ComfyUI output. They make every species
visible immediately and give the runtime stable files to load:

    .venv/bin/python tools/gen_species_sprites.py

StoryOS/Comfy can overwrite the same files later:

    node tools/gen_sprites_storyos_comfy.mjs species

Output: assets/sprites/species/000.png ... 099.png
"""

from __future__ import annotations

import hashlib
import math
import os
import re
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "src" / "monster.rs"
OUT = ROOT / "assets" / "sprites" / "species"

S = 96
SS = 4
Z = S * SS

KIND_SHAPES = {
    "Normal": "blob",
    "Fast": "dart",
    "Tank": "square",
    "Flying": "wing",
    "Invisible": "wraith",
    "Regenerating": "blob",
    "Armored": "hex",
    "Swarmer": "bug",
    "Boss": "boss",
    "Shielded": "shield",
    "Splitter": "split",
    "Healer": "healer",
    "Charger": "dart",
    "Climber": "claw",
    "Silencer": "wraith",
    "Moss": "moss",
}

TAG_COLORS = {
    "火焰": (232, 86, 34),
    "冰霜": (93, 190, 245),
    "雷风": (241, 211, 53),
    "暗影": (105, 75, 145),
    "剧毒": (84, 190, 88),
    "秘法": (166, 109, 255),
    "虚空": (68, 60, 112),
    "深海": (48, 130, 165),
    "重甲": (120, 128, 136),
    "护盾": (86, 174, 232),
    "治疗": (80, 210, 138),
    "攻塔": (178, 115, 34),
    "静默": (92, 76, 145),
    "吞塔": (27, 116, 63),
}


def parse_species() -> list[dict[str, object]]:
    lines = SRC.read_text(encoding="utf-8").splitlines()
    out: list[dict[str, object]] = []
    block: list[str] | None = None
    for line in lines:
        if line.strip() == "sp!(":
            block = [line]
            continue
        if block is None:
            continue
        block.append(line)
        if line.strip() == "),":
            vals = [l.strip().rstrip(",") for l in block]
            quoted = [v[1:-1] for v in vals if re.fullmatch(r'".*"', v)]
            out.append(
                {
                    "id": int(vals[1]),
                    "name": quoted[0],
                    "kind": vals[3],
                    "tags": quoted[-1].split("/"),
                }
            )
            block = None
    if len(out) != 100:
        raise SystemExit(f"expected 100 species, parsed {len(out)}")
    return out


def mix(a: tuple[int, int, int], b: tuple[int, int, int], t: float) -> tuple[int, int, int]:
    return tuple(int(x + (y - x) * t) for x, y in zip(a, b))


def dark(c: tuple[int, int, int], t: float) -> tuple[int, int, int]:
    return tuple(max(0, int(v * (1.0 - t))) for v in c)


def light(c: tuple[int, int, int], t: float) -> tuple[int, int, int]:
    return mix(c, (255, 255, 255), t)


def base_color(spec: dict[str, object]) -> tuple[int, int, int]:
    for tag in spec["tags"]:
        if tag in TAG_COLORS:
            return TAG_COLORS[tag]
    h = hashlib.sha256(f"{spec['id']}:{spec['name']}".encode()).digest()
    return (80 + h[0] % 150, 70 + h[1] % 150, 70 + h[2] % 150)


def ellipse(draw: ImageDraw.ImageDraw, cx: float, cy: float, rx: float, ry: float, fill, outline, width: int):
    draw.ellipse([cx - rx, cy - ry, cx + rx, cy + ry], fill=fill, outline=outline, width=width)


def poly(draw: ImageDraw.ImageDraw, pts, fill, outline):
    draw.polygon(pts, fill=fill, outline=outline)


def draw_eyes(draw: ImageDraw.ImageDraw, c: int, r: int, count: int, color: tuple[int, int, int]):
    er = max(4, int(r * 0.14))
    if count == 1:
        positions = [(c, c - int(r * 0.12))]
    elif count == 3:
        positions = [(c, c - int(r * 0.28)), (c - int(r * 0.32), c), (c + int(r * 0.32), c)]
    else:
        positions = [(c - int(r * 0.32), c - int(r * 0.10)), (c + int(r * 0.32), c - int(r * 0.10))]
    for ex, ey in positions:
        draw.ellipse([ex - er, ey - er, ex + er, ey + er], fill=(245, 245, 230, 255))
        draw.ellipse([ex - er // 2, ey - er // 2, ex + er // 2, ey + er // 2], fill=dark(color, 0.65) + (255,))


def draw_marks(draw: ImageDraw.ImageDraw, c: int, r: int, spec: dict[str, object], color, outline, width: int):
    tags = set(spec["tags"])
    if "护盾" in tags:
        ellipse(draw, c, c, r * 1.13, r * 1.13, (255, 255, 255, 22), light(color, 0.45) + (210,), width)
    if "重甲" in tags or "甲壳" in tags:
        for i in range(-2, 3):
            x = c + i * r * 0.22
            draw.line([x, c - r * 0.6, x, c + r * 0.55], fill=outline + (190,), width=width)
    if "分裂" in tags:
        for a in (30, 145, 265):
            ax, ay = math.cos(math.radians(a)), math.sin(math.radians(a))
            draw.line([c, c, c + ax * r * 0.8, c + ay * r * 0.8], fill=(255, 245, 220, 220), width=width)
    if "治疗" in tags:
        g = int(r * 0.38)
        draw.line([c - g, c, c + g, c], fill=(230, 255, 220, 230), width=width * 2)
        draw.line([c, c - g, c, c + g], fill=(230, 255, 220, 230), width=width * 2)
    if "静默" in tags:
        draw.line([c - r * 0.45, c + r * 0.38, c + r * 0.45, c + r * 0.38], fill=outline + (240,), width=width * 2)
    if "攻塔" in tags:
        for sx in (-1, 1):
            draw.line([c + sx * r * 0.45, c + r * 0.15, c + sx * r * 0.85, c + r * 0.55], fill=outline + (240,), width=width)


def render(spec: dict[str, object]) -> Image.Image:
    img = Image.new("RGBA", (Z, Z), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    c = Z // 2
    r = int(Z * 0.34)
    width = int(Z * 0.035)
    color = base_color(spec)
    outline = dark(color, 0.55)
    shape = KIND_SHAPES.get(spec["kind"], "blob")
    fill = color + (255,)
    stroke = outline + (255,)

    if shape in {"blob", "healer", "shield", "split", "boss", "moss"}:
        ellipse(draw, c, c, r, r * (0.95 if shape != "moss" else 0.78), fill, stroke, width)
    elif shape == "dart":
        poly(draw, [(c + r * 1.08, c), (c - r * 0.78, c - r * 0.82), (c - r * 0.72, c + r * 0.82)], fill, stroke)
    elif shape == "square":
        draw.rounded_rectangle([c - r, c - r, c + r, c + r], radius=int(r * 0.25), fill=fill, outline=stroke, width=width)
    elif shape == "hex":
        pts = [(c + r * math.cos(math.radians(a)), c + r * math.sin(math.radians(a))) for a in range(0, 360, 60)]
        poly(draw, pts, fill, stroke)
    elif shape == "wing":
        for sx in (-1, 1):
            poly(draw, [(c, c), (c + sx * r * 1.18, c - r * 0.68), (c + sx * r * 0.62, c + r * 0.05)], light(color, 0.12) + (215,), stroke)
        ellipse(draw, c, c, r * 0.48, r * 0.62, fill, stroke, width)
    elif shape == "bug":
        for sx in (-1, 1):
            for dy in (-0.35, 0.0, 0.35):
                draw.line([c, c + r * dy, c + sx * r * 0.95, c + r * (dy + 0.2)], fill=stroke, width=width)
        ellipse(draw, c, c, r * 0.72, r * 0.54, fill, stroke, width)
    elif shape == "wraith":
        ellipse(draw, c, c, r * 0.78, r, color + (145,), stroke[:3] + (190,), width)
        for a in range(210, 330, 30):
            ax, ay = math.cos(math.radians(a)), math.sin(math.radians(a))
            draw.line([c + ax * r * 0.45, c + ay * r * 0.55, c + ax * r, c + ay * r * 1.05], fill=stroke[:3] + (170,), width=width)
    elif shape == "claw":
        poly(draw, [(c + r, c), (c - r * 0.70, c - r * 0.78), (c - r * 0.70, c + r * 0.78)], fill, stroke)
        for sx in (-0.45, 0.0, 0.45):
            draw.line([c - r * 0.55, c + r * sx, c - r, c + r * sx * 1.15], fill=stroke, width=width)

    if shape in {"boss", "moss"}:
        for a in range(0, 360, 30):
            ax, ay = math.cos(math.radians(a)), math.sin(math.radians(a))
            draw.line([c + ax * r * 0.72, c + ay * r * 0.72, c + ax * r * 1.15, c + ay * r * 1.15], fill=stroke, width=width)
    if shape == "moss":
        for a in range(20, 360, 45):
            ax, ay = math.cos(math.radians(a)), math.sin(math.radians(a))
            draw.arc([c - r * 1.25, c - r * 1.25, c + r * 1.25, c + r * 1.25], a, a + 45, fill=light(color, 0.2) + (220,), width=width)

    eye_count = 3 if spec["id"] % 7 == 0 else (1 if spec["kind"] in {"Moss", "Boss"} else 2)
    draw_eyes(draw, c, r, eye_count, color)
    draw_marks(draw, c, r, spec, color, outline, width)
    return img.resize((S, S), Image.LANCZOS)


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    for spec in parse_species():
        img = render(spec)
        img.save(OUT / f"{spec['id']:03}.png")
    print(f"generated 100 species sprites into {OUT}")


if __name__ == "__main__":
    main()
