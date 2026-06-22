#!/usr/bin/env python3
"""Verify required sprite assets and write QA manifests.

This is intended for StoryOS/ComfyUI batches and release checks. It derives the
required tower/enemy/species/equipment sprites from Rust source, validates that the files
exist and are nonblank PNGs, and writes:

    tmp/sprite_manifest.json
    tmp/sprite_report.md

Run:
    .venv/bin/python tools/verify_sprite_assets.py
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

from PIL import Image, ImageStat

ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / "src" / "data.rs"
MONSTER = ROOT / "src" / "monster.rs"
EQUIPMENT = ROOT / "src" / "equipment.rs"
OUT = ROOT / "tmp" / "sprite_manifest.json"
REPORT = ROOT / "tmp" / "sprite_report.md"


def parse_sprite_names(enum_name: str) -> list[str]:
    src = DATA.read_text(encoding="utf-8")
    impl_start = src.index(f"impl {enum_name}")
    fn_start = src.index("pub fn sprite_name", impl_start)
    body_start = src.index("{", fn_start)
    depth = 0
    body_end = body_start
    for i, char in enumerate(src[body_start:], start=body_start):
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                body_end = i + 1
                break
    block = src[fn_start:body_end]
    names = re.findall(r'=>\s*"([^"]+)"', block)
    if not names:
        raise SystemExit(f"could not parse {enum_name} sprite names")
    return names


def parse_equipment_sprite_names() -> list[str]:
    src = EQUIPMENT.read_text(encoding="utf-8")
    impl_start = src.index("impl Equipment")
    fn_start = src.index("pub fn sprite_name", impl_start)
    body_start = src.index("{", fn_start)
    depth = 0
    body_end = body_start
    for i, char in enumerate(src[body_start:], start=body_start):
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                body_end = i + 1
                break
    block = src[fn_start:body_end]
    names = re.findall(r'=>\s*"([^"]+)"', block)
    if len(names) != 20:
        raise SystemExit(f"expected 20 equipment sprite names, parsed {len(names)}")
    return names


def parse_species_ids() -> list[str]:
    lines = MONSTER.read_text(encoding="utf-8").splitlines()
    out: list[str] = []
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
            out.append(f"{int(vals[1]):03}")
            block = None
    if len(out) != 100:
        raise SystemExit(f"expected 100 species, parsed {len(out)}")
    return out


def alpha_nonblank(img: Image.Image) -> bool:
    rgba = img.convert("RGBA")
    alpha = rgba.getchannel("A")
    return alpha.getbbox() is not None


def visible_mean(img: Image.Image) -> float:
    rgba = img.convert("RGBA")
    alpha = rgba.getchannel("A")
    bbox = alpha.getbbox()
    if bbox is None:
        return 0.0
    crop = rgba.crop(bbox).convert("RGB")
    stat = ImageStat.Stat(crop)
    return sum(stat.mean) / 3.0


def inspect(path: Path, group: str, key: str) -> dict[str, object]:
    item: dict[str, object] = {
        "group": group,
        "key": key,
        "path": str(path.relative_to(ROOT)),
        "exists": path.exists(),
        "ok": False,
        "errors": [],
        "warnings": [],
    }
    errors: list[str] = item["errors"]  # type: ignore[assignment]
    warnings: list[str] = item["warnings"]  # type: ignore[assignment]
    if not path.exists():
        errors.append("missing")
        return item
    try:
        with Image.open(path) as img:
            item["width"], item["height"] = img.size
            item["mode"] = img.mode
            if img.format != "PNG":
                errors.append(f"not_png:{img.format}")
            if img.width < 32 or img.height < 32:
                errors.append("too_small")
            if abs(img.width - img.height) > max(2, min(img.size) * 0.05):
                errors.append("not_square")
            if not alpha_nonblank(img):
                errors.append("blank_alpha")
            elif visible_mean(img) < 3.0:
                warnings.append("very_dark")
    except Exception as exc:  # noqa: BLE001 - validation should report all failures.
        errors.append(f"unreadable:{exc}")
    item["ok"] = not errors
    return item


def expected() -> list[tuple[str, str, Path]]:
    rows: list[tuple[str, str, Path]] = []
    rows.extend(
        ("towers", name, ROOT / "assets" / "sprites" / "towers" / f"{name}.png")
        for name in parse_sprite_names("TowerKind")
    )
    rows.extend(
        ("enemies", name, ROOT / "assets" / "sprites" / "enemies" / f"{name}.png")
        for name in parse_sprite_names("EnemyKind")
    )
    rows.extend(
        ("species", sid, ROOT / "assets" / "sprites" / "species" / f"{sid}.png")
        for sid in parse_species_ids()
    )
    rows.extend(
        (
            "equipment",
            name,
            ROOT / "assets" / "sprites" / "equipment" / f"{name}.png",
        )
        for name in parse_equipment_sprite_names()
    )
    return rows


def write_report(manifest: dict[str, object]) -> None:
    groups: dict[str, dict[str, int]] = manifest["groups"]  # type: ignore[assignment]
    entries: list[dict[str, object]] = manifest["entries"]  # type: ignore[assignment]
    failures = [entry for entry in entries if not entry["ok"]]
    warnings = [entry for entry in entries if entry.get("warnings")]
    lines = [
        "# Sprite QA Report",
        "",
        f"Total: {manifest['ok']}/{manifest['total']} required sprites OK",
        f"Failures: {manifest['failed']}",
        f"Warnings: {len(warnings)}",
        "",
        "## Groups",
        "",
    ]
    for group, stats in groups.items():
        lines.append(f"- {group}: {stats['ok']}/{stats['total']} OK")
    lines.extend(["", "## Failures", ""])
    if failures:
        for entry in failures:
            lines.append(f"- `{entry['path']}`: {', '.join(entry['errors'])}")
    else:
        lines.append("- None")
    lines.extend(["", "## Warnings", ""])
    if warnings:
        for entry in warnings:
            lines.append(f"- `{entry['path']}`: {', '.join(entry['warnings'])}")
    else:
        lines.append("- None")
    REPORT.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> int:
    entries = [inspect(path, group, key) for group, key, path in expected()]
    groups: dict[str, dict[str, int]] = {}
    for entry in entries:
        group = str(entry["group"])
        groups.setdefault(group, {"total": 0, "ok": 0, "failed": 0})
        groups[group]["total"] += 1
        groups[group]["ok" if entry["ok"] else "failed"] += 1
    manifest = {
        "total": len(entries),
        "ok": sum(1 for entry in entries if entry["ok"]),
        "failed": sum(1 for entry in entries if not entry["ok"]),
        "groups": groups,
        "entries": entries,
    }
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding="utf-8")
    write_report(manifest)
    print(OUT.relative_to(ROOT))
    print(REPORT.relative_to(ROOT))
    for group, stats in groups.items():
        print(f"{group}: {stats['ok']}/{stats['total']} ok")
    failures = [entry for entry in entries if not entry["ok"]]
    if failures:
        for entry in failures[:20]:
            print(f"FAIL {entry['path']}: {', '.join(entry['errors'])}", file=sys.stderr)
        if len(failures) > 20:
            print(f"... {len(failures) - 20} more failures", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
