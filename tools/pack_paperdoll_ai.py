#!/usr/bin/env python3
"""Pack AI-generated paperdoll layers into `assets/paperdoll/hero.ppd`.

Inputs (generated via codex image_gen; see tools/gen_paperdoll_ai.sh):
  tmp/paperdoll_ai/body_{human,elf,orc}.png — full-canvas character art,
    resized straight to SIZE x SIZE.
  tmp/paperdoll_ai/raw/<layer>.png — STANDALONE item props on transparency.
    Each is trimmed, scaled and composited onto a SIZE x SIZE canvas at the
    anchor from PLACEMENTS, so alignment is deterministic and tunable here —
    never dependent on the image model.

The tar layout (slots/fragment ids) matches tools/gen_hero_paperdoll.py; only
the canvas size differs (256 for crisper UI previews).

Usage:
  python3 tools/pack_paperdoll_ai.py            # pack (requires all layers)
  python3 tools/pack_paperdoll_ai.py --allow-missing  # fall back to current ppd
"""

from __future__ import annotations

import io
import subprocess
import sys
import tarfile
from pathlib import Path

SIZE = 256
SRC = Path("tmp/paperdoll_ai")
OUT = Path("assets/paperdoll/hero.ppd")
CUR = OUT  # fallback source for missing layers

BODIES = ["human", "elf", "orc"]

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

# 高级武器辉光变体：Lv20+ 英雄的武器带元素光晕（fragment id = 基础 id + 50）。
# 光晕颜色按武器的元素气质配色。
GLOW_WEAPONS = [
    (150, "sword_glow", "weapon_sword_glow.png", "weapon_sword.png", "#7fd4ff"),
    (151, "staff_glow", "weapon_staff_glow.png", "weapon_staff.png", "#ff9a4d"),
    (152, "bow_glow", "weapon_bow_glow.png", "weapon_bow.png", "#8dff7a"),
    (153, "shield_glow", "weapon_shield_glow.png", "weapon_shield.png", "#ffd45e"),
    (154, "storm_orb_glow", "weapon_storm_orb_glow.png", "weapon_storm_orb.png", "#66c7ff"),
    (155, "sentry_bow_glow", "weapon_sentry_bow_glow.png", "weapon_sentry_bow.png", "#ffc46b"),
    (156, "dagger_glow", "weapon_dagger_glow.png", "weapon_dagger.png", "#c98bff"),
    (157, "censer_glow", "weapon_censer_glow.png", "weapon_censer.png", "#ffe9a3"),
    (158, "hammer_glow", "weapon_hammer_glow.png", "weapon_hammer.png", "#ff8a5c"),
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


# item placement anchors: layer -> (cx, cy, scale[, mirror_to_cx])
# cx/cy = center of the item as a fraction of the canvas; scale = the item's
# max bounding-box side as a fraction of the canvas. mirror_to_cx composites a
# horizontally-flipped copy at the given cx (for paired hand items).
PLACEMENTS: dict[str, tuple] = {
    "weapon_sword.png": (0.81, 0.52, 0.48),
    "weapon_staff.png": (0.86, 0.48, 0.72),
    "weapon_bow.png": (0.85, 0.50, 0.60),
    "weapon_shield.png": (0.15, 0.55, 0.42),
    "weapon_storm_orb.png": (0.85, 0.42, 0.30),
    "weapon_sentry_bow.png": (0.84, 0.52, 0.45),
    "weapon_dagger.png": (0.84, 0.50, 0.38),
    "weapon_censer.png": (0.84, 0.50, 0.38),
    "weapon_hammer.png": (0.85, 0.46, 0.55),
    "gear_vow_plate.png": (0.50, 0.54, 0.48),
    "gear_starweave_robe.png": (0.50, 0.64, 0.55),
    "gear_windrunner_cloak.png": (0.50, 0.63, 0.54),
    "gear_thunder_charm.png": (0.50, 0.42, 0.14),
    "gear_saint_bell.png": (0.66, 0.58, 0.16),
    "gear_night_mask.png": (0.50, 0.22, 0.30),
    "gear_forge_gauntlet.png": (0.80, 0.57, 0.15, 0.20),
    "gear_warden_scope.png": (0.60, 0.21, 0.14),
    "gear_carrot_halo.png": (0.50, 0.055, 0.38),
    "gear_wayfarer_boots.png": (0.50, 0.87, 0.30),
    "gear_bloodstep_greaves.png": (0.50, 0.86, 0.30),
    "gear_starpath_sandals.png": (0.50, 0.87, 0.30),
    "gear_engineer_treads.png": (0.50, 0.87, 0.30),
    "gear_summoner_greaves.png": (0.50, 0.86, 0.30),
    "gear_carrot_wings.png": (0.50, 0.45, 0.95),
}


def point(x: float = 0.0, y: float = 0.0) -> str:
    return f"{{x: {x:.1f}, y: {y:.1f}}}"


def manifest() -> str:
    lines: list[str] = [
        "meta:",
        "  name: protect-carrot-hero-paperdoll",
        "  version: 1",
        "dolls:",
    ]
    for idx, race in enumerate(BODIES):
        lines += [
            f"- id: {idx}",
            f"  desc: {race}",
            f"  width: {SIZE}",
            f"  height: {SIZE}",
            f"  offset: {point()}",
            "  slots: [20, 23, 10, 21, 22]",
            f"  path: body_{race}.png",
        ]
    lines += [
        "slots:",
        "- id: 10",
        "  desc: class weapon",
        "  required: true",
        "  constrainted: false",
        f"  positions: [{point()}]",
        f"  anchor: {point()}",
        "  candidates: [100, 101, 102, 103, 104, 105, 106, 107, 108, "
        "150, 151, 152, 153, 154, 155, 156, 157, 158]",
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
    for frag_id, name, path in WEAPONS:
        lines += [f"- id: {frag_id}", f"  desc: {name}", f"  pivot: {point()}", f"  path: {path}"]
    for frag_id, name, path, _base, _color in GLOW_WEAPONS:
        lines += [f"- id: {frag_id}", f"  desc: {name}", f"  pivot: {point()}", f"  path: {path}"]
    for frag_id, name, path, _slot in GEAR:
        lines += [f"- id: {frag_id}", f"  desc: {name}", f"  pivot: {point()}", f"  path: {path}"]
    return "\n".join(lines) + "\n"


def load_body(path: Path) -> bytes:
    """Full-canvas body art: resize to SIZE x SIZE, preserving transparency."""
    out = subprocess.run(
        [
            "magick", str(path),
            "-background", "none",
            "-resize", f"{SIZE}x{SIZE}",
            "-gravity", "center",
            "-extent", f"{SIZE}x{SIZE}",
            "PNG32:-",
        ],
        capture_output=True,
        check=True,
    )
    return out.stdout


def place_item(path: Path, cx: float, cy: float, scale: float,
               mirror_to_cx: float | None = None) -> bytes:
    """Trim a standalone prop, scale it, and composite at the anchor."""
    box = int(SIZE * scale)
    args = [
        "magick",
        "-size", f"{SIZE}x{SIZE}", "xc:none",
        "(", str(path), "-trim", "+repage",
        "-background", "none", "-resize", f"{box}x{box}", ")",
    ]

    def offset(target_cx: float) -> list[str]:
        # -gravity center + -geometry offsets shift the overlay's center
        dx = int(round((target_cx - 0.5) * SIZE))
        dy = int(round((cy - 0.5) * SIZE))
        sx = f"+{dx}" if dx >= 0 else str(dx)
        sy = f"+{dy}" if dy >= 0 else str(dy)
        return ["-gravity", "center", "-geometry", f"{sx}{sy}", "-composite"]

    args += offset(cx)
    if mirror_to_cx is not None:
        args += [
            "(", str(path), "-trim", "+repage",
            "-background", "none", "-resize", f"{box}x{box}", "-flop", ")",
        ] + offset(mirror_to_cx)
    args += ["PNG32:-"]
    out = subprocess.run(args, capture_output=True, check=True)
    return out.stdout


def add_glow(placed_png: bytes, color: str) -> bytes:
    """Element-colored halo under an already-placed weapon layer.

    Halo = the weapon's alpha silhouette, dilated + blurred + tinted, composited
    UNDER the weapon so the blade itself stays crisp while radiating light.
    """
    halo = subprocess.run(
        [
            "magick",
            "(", "-size", f"{SIZE}x{SIZE}", f"xc:{color}", ")",
            "(", "png:-", "-alpha", "extract",
            "-morphology", "Dilate", "Disk:3",
            "-blur", "0x7", "-auto-level",
            "-evaluate", "Multiply", "0.9", ")",
            "-alpha", "off", "-compose", "CopyOpacity", "-composite",
            "PNG32:-",
        ],
        input=placed_png,
        capture_output=True,
        check=True,
    )
    import tempfile

    with tempfile.NamedTemporaryFile(suffix=".png") as h, tempfile.NamedTemporaryFile(
        suffix=".png"
    ) as p:
        h.write(halo.stdout)
        h.flush()
        p.write(placed_png)
        p.flush()
        out = subprocess.run(
            ["magick", h.name, p.name, "-compose", "Over", "-composite", "PNG32:-"],
            capture_output=True,
            check=True,
        )
        return out.stdout


def fallback_layers() -> dict[str, bytes]:
    """Current ppd layers (resized to SIZE) used when an AI layer is missing."""
    layers: dict[str, bytes] = {}
    if not CUR.exists():
        return layers
    with tarfile.open(CUR) as tar:
        for member in tar.getmembers():
            if not member.name.endswith(".png"):
                continue
            data = tar.extractfile(member).read()
            out = subprocess.run(
                [
                    "magick",
                    "png:-",
                    "-background",
                    "none",
                    "-filter",
                    "point",
                    "-resize",
                    f"{SIZE}x{SIZE}",
                    "PNG32:-",
                ],
                input=data,
                capture_output=True,
                check=True,
            )
            layers[member.name] = out.stdout
    return layers


def add_file(tar: tarfile.TarFile, name: str, data: bytes) -> None:
    info = tarfile.TarInfo(name)
    info.size = len(data)
    info.mtime = 0
    info.mode = 0o644
    tar.addfile(info, io.BytesIO(data))


def main() -> None:
    allow_missing = "--allow-missing" in sys.argv
    names = [f"body_{r}.png" for r in BODIES]
    names += [p for _, _, p in WEAPONS]
    names += [p for _, _, p, _ in GEAR]

    fallback = fallback_layers() if allow_missing else {}
    files: dict[str, bytes] = {"manifest.yml": manifest().encode()}
    missing: list[str] = []
    used_ai = 0
    for name in names:
        body_src = SRC / name
        raw_src = SRC / "raw" / name
        if name.startswith("body_") and body_src.exists():
            files[name] = load_body(body_src)
            used_ai += 1
        elif name in PLACEMENTS and raw_src.exists():
            files[name] = place_item(raw_src, *PLACEMENTS[name])
            used_ai += 1
        elif name in fallback:
            files[name] = fallback[name]
        else:
            missing.append(name)

    # 辉光武器变体：基于已对位的武器图层加元素光晕。
    for _fid, _name, glow_path, base_path, color in GLOW_WEAPONS:
        if base_path in files:
            files[glow_path] = add_glow(files[base_path], color)
        else:
            missing.append(glow_path)

    if missing:
        sys.exit(f"missing layers (and no fallback): {missing}")

    OUT.parent.mkdir(parents=True, exist_ok=True)
    with tarfile.open(OUT, "w") as tar:
        for name in sorted(files):
            add_file(tar, name, files[name])
    print(f"wrote {OUT} ({OUT.stat().st_size} bytes), {used_ai} AI layers, "
          f"{len(names) - used_ai} fallback")


if __name__ == "__main__":
    main()
