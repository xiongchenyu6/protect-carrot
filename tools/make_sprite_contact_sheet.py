#!/usr/bin/env python3
"""Build contact sheets for generated sprites.

Bulk StoryOS/Comfy runs are hard to QA one PNG at a time. This tool lays out the
current sprites with labels so bad generations, missing files, or duplicated
silhouettes can be spotted quickly.

Examples:
    .venv/bin/python tools/make_sprite_contact_sheet.py species
    .venv/bin/python tools/make_sprite_contact_sheet.py all

Outputs:
    tmp/species_contact_sheet.png
    tmp/towers_contact_sheet.png
    tmp/enemies_contact_sheet.png
    tmp/equipment_contact_sheet.png
"""

from __future__ import annotations

import math
import re
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[1]
TMP = ROOT / "tmp"
FONT_PATH = ROOT / "assets" / "fonts" / "wqy-microhei.ttc"
MONSTER_SRC = ROOT / "src" / "monster.rs"


def font(size: int) -> ImageFont.FreeTypeFont | ImageFont.ImageFont:
    if FONT_PATH.exists():
        return ImageFont.truetype(str(FONT_PATH), size)
    return ImageFont.load_default()


def parse_species() -> list[dict[str, str]]:
    lines = MONSTER_SRC.read_text(encoding="utf-8").splitlines()
    out: list[dict[str, str]] = []
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
                    "id": vals[1],
                    "key": f"{int(vals[1]):03}",
                    "name": quoted[0],
                    "kind": vals[3],
                    "tags": quoted[-1],
                    "path": str(ROOT / "assets" / "sprites" / "species" / f"{int(vals[1]):03}.png"),
                }
            )
            block = None
    if len(out) != 100:
        raise SystemExit(f"expected 100 species, parsed {len(out)}")
    return out


def directory_targets(kind: str) -> list[dict[str, str]]:
    base = ROOT / "assets" / "sprites" / kind
    return [
        {
            "id": str(i),
            "key": path.stem,
            "name": path.stem,
            "kind": kind,
            "tags": "",
            "path": str(path),
        }
        for i, path in enumerate(sorted(base.glob("*.png")))
    ]


def fit_text(text: str, max_chars: int) -> str:
    return text if len(text) <= max_chars else text[: max_chars - 1] + "…"


def make_sheet(name: str, targets: list[dict[str, str]], columns: int) -> Path:
    TMP.mkdir(parents=True, exist_ok=True)
    sprite_size = 96
    cell_w = 184
    cell_h = 142
    pad = 12
    title_h = 46
    rows = math.ceil(len(targets) / columns)
    out = Image.new("RGB", (columns * cell_w, title_h + rows * cell_h), (22, 26, 24))
    draw = ImageDraw.Draw(out)
    title_font = font(24)
    label_font = font(16)
    small_font = font(12)

    missing = 0
    draw.text(
        (pad, 10),
        f"{name} sprite contact sheet · {len(targets)} targets",
        fill=(225, 236, 220),
        font=title_font,
    )

    for i, target in enumerate(targets):
        col = i % columns
        row = i // columns
        x = col * cell_w
        y = title_h + row * cell_h
        path = Path(target["path"])
        exists = path.exists()
        border = (74, 170, 108) if exists else (210, 70, 70)
        draw.rounded_rectangle(
            [x + 4, y + 4, x + cell_w - 4, y + cell_h - 4],
            radius=8,
            outline=border,
            width=2,
            fill=(31, 36, 34),
        )

        if exists:
            img = Image.open(path).convert("RGBA")
            img.thumbnail((sprite_size, sprite_size), Image.Resampling.LANCZOS)
            px = x + (cell_w - img.width) // 2
            py = y + 8 + (sprite_size - img.height) // 2
            out.paste(img, (px, py), img)
        else:
            missing += 1
            draw.line(
                [x + 50, y + 28, x + cell_w - 50, y + 86],
                fill=(210, 70, 70),
                width=4,
            )
            draw.line(
                [x + cell_w - 50, y + 28, x + 50, y + 86],
                fill=(210, 70, 70),
                width=4,
            )

        label = f"{target['key']} {target['name']}"
        draw.text((x + 10, y + 108), fit_text(label, 18), fill=(245, 240, 210), font=label_font)
        detail = target["tags"] or target["kind"]
        draw.text((x + 10, y + 126), fit_text(detail, 25), fill=(160, 176, 166), font=small_font)

    if missing:
        draw.text(
            (out.width - 220, 14),
            f"missing: {missing}",
            fill=(255, 120, 120),
            font=label_font,
        )

    path = TMP / f"{name}_contact_sheet.png"
    out.save(path)
    return path


def main() -> None:
    mode = sys.argv[1] if len(sys.argv) > 1 else "species"
    modes = ["species", "towers", "enemies", "equipment"] if mode == "all" else [mode]
    written: list[Path] = []
    for item in modes:
        if item == "species":
            written.append(make_sheet("species", parse_species(), 10))
        elif item in {"towers", "enemies", "equipment"}:
            written.append(make_sheet(item, directory_targets(item), 8))
        else:
            raise SystemExit(
                "usage: make_sprite_contact_sheet.py [species|towers|enemies|equipment|all]"
            )
    for path in written:
        print(path.relative_to(ROOT))


if __name__ == "__main__":
    main()
