#!/usr/bin/env python3
"""Import the bundled monster packs into the 100 species sprite slots.

The Bevy port loads `assets/sprites/species/000.png` through `099.png` based on
the species IDs in `src/monster.rs`. This script normalizes the three external
packs currently checked into the workspace:

- `Pixel Monster Pack/64x64 monsters`: species 000-039
- `388047a56004d755b63a89962f1e7207/Original Size 1x`: species 040-069
- `3b6d1905030ceb9cf2098df75de737ac`: species 070-099, cropped from 3x3 sheets

It also mirrors species 090-099 into `assets/sprites/bosses/` so the boss HUD
uses the same art as the enemy on the board.

Run:
    python3 tools/import_monster_pack_sprites.py
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "assets" / "sprites" / "species"
BOSS_OUT = ROOT / "assets" / "sprites" / "bosses"
MANIFEST = ROOT / "tmp" / "monster_pack_species_manifest.json"
SIZE = 128
HASH_BG = (0, 128, 128)


@dataclass(frozen=True)
class Source:
    species_id: int
    path: Path
    crop_cell: int | None = None


UNDEAD_NAMES = [
    "Banshee",
    "Cursed armor",
    "Ghost",
    "Ghoul",
    "Lich",
    "Monster",
    "Skeleton",
    "Vampire",
    "Wendigo",
    "Zombie",
]

HASH_CROPS = [
    (70, 1, 1),
    (71, 3, 2),
    (72, 6, 4),
    (73, 12, 3),
    (74, 22, 6),
    (75, 24, 9),
    (76, 45, 2),
    (77, 37, 1),
    (78, 33, 9),
    (79, 27, 2),
    (80, 18, 4),
    (81, 59, 8),
    (82, 51, 9),
    (83, 50, 6),
    (84, 63, 7),
    (85, 52, 1),
    (86, 58, 3),
    (87, 40, 8),
    (88, 5, 2),
    (89, 62, 9),
    (90, 4, 5),
    (91, 3, 2),
    (92, 22, 2),
    (93, 6, 7),
    (94, 16, 1),
    (95, 59, 6),
    (96, 5, 4),
    (97, 47, 2),
    (98, 1, 9),
    (99, 64, 2),
]

BOSS_OUTPUTS = {
    90: "serpent",
    91: "abyssal",
    92: "yellow",
    93: "storm",
    94: "furnace",
    95: "brood",
    96: "void",
    97: "starforged",
    98: "moss",
    99: "dream",
}


def trim_alpha(img: Image.Image) -> Image.Image:
    bbox = img.getchannel("A").getbbox()
    if bbox is None:
        raise ValueError("source image has no visible pixels")
    return img.crop(bbox)


def make_square(img: Image.Image, pad: int = 6) -> Image.Image:
    side = max(img.width, img.height) + pad * 2
    square = Image.new("RGBA", (side, side), (0, 0, 0, 0))
    square.alpha_composite(img, ((side - img.width) // 2, (side - img.height) // 2))
    return square


def normalize(img: Image.Image) -> Image.Image:
    img = trim_alpha(img.convert("RGBA"))
    img = make_square(img)
    return img.resize((SIZE, SIZE), Image.Resampling.NEAREST)


def remove_hash_background(img: Image.Image) -> Image.Image:
    rgba = img.convert("RGBA")
    pixels = rgba.load()
    for y in range(rgba.height):
        for x in range(rgba.width):
            r, g, b, a = pixels[x, y]
            if abs(r - HASH_BG[0]) + abs(g - HASH_BG[1]) + abs(b - HASH_BG[2]) <= 20:
                pixels[x, y] = (r, g, b, 0)
            elif a > 0:
                pixels[x, y] = (r, g, b, 255)
    return rgba


def crop_hash_sheet(img: Image.Image, cell: int) -> Image.Image:
    if not 1 <= cell <= 9:
        raise ValueError(f"3b6d crop cell must be 1..9, got {cell}")
    w, h = img.size
    cw, ch = w // 3, h // 3
    index = cell - 1
    x = (index % 3) * cw
    y = (index // 3) * ch
    crop = img.crop((x, y, x + cw, y + ch))
    return remove_hash_background(crop)


def build_sources() -> list[Source]:
    sources: list[Source] = []

    pixel_dir = ROOT / "Pixel Monster Pack" / "64x64 monsters"
    for species_id, path in enumerate(sorted(pixel_dir.glob("*.png"))):
        sources.append(Source(species_id, path))

    undead_root = ROOT / "388047a56004d755b63a89962f1e7207"
    original_dirs = sorted(p for p in undead_root.glob("Original Size */Pallete *") if p.is_dir())
    for palette_index, palette_dir in enumerate(original_dirs):
        for monster_index, name in enumerate(UNDEAD_NAMES):
            species_id = 40 + palette_index * len(UNDEAD_NAMES) + monster_index
            sources.append(Source(species_id, palette_dir / f"{name}.png"))

    hash_dir = ROOT / "3b6d1905030ceb9cf2098df75de737ac"
    for species_id, sheet_id, cell in HASH_CROPS:
        sources.append(Source(species_id, hash_dir / f"monster_{sheet_id:02}.png", cell))

    expected = set(range(100))
    actual = {source.species_id for source in sources}
    missing = sorted(expected - actual)
    duplicate_count = len(sources) - len(actual)
    if missing or duplicate_count:
        raise SystemExit(f"bad species mapping: missing={missing}, duplicates={duplicate_count}")
    return sorted(sources, key=lambda source: source.species_id)


def convert(source: Source) -> Image.Image:
    img = Image.open(source.path)
    if source.crop_cell is not None:
        img = crop_hash_sheet(img, source.crop_cell)
    return normalize(img)


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    BOSS_OUT.mkdir(parents=True, exist_ok=True)
    rows = []
    for source in build_sources():
        if not source.path.exists():
            raise SystemExit(f"missing source: {source.path.relative_to(ROOT)}")
        img = convert(source)
        out = OUT / f"{source.species_id:03}.png"
        img.save(out)
        boss_name = BOSS_OUTPUTS.get(source.species_id)
        boss_out = None
        if boss_name is not None:
            boss_out = BOSS_OUT / f"{boss_name}.png"
            img.save(boss_out)
        rows.append(
            {
                "species_id": source.species_id,
                "output": str(out.relative_to(ROOT)),
                "boss_output": str(boss_out.relative_to(ROOT)) if boss_out else None,
                "source": str(source.path.relative_to(ROOT)),
                "crop_cell": source.crop_cell,
            }
        )

    MANIFEST.parent.mkdir(parents=True, exist_ok=True)
    MANIFEST.write_text(json.dumps(rows, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"wrote {len(rows)} species sprites")
    print(f"manifest: {MANIFEST.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
