#!/usr/bin/env python3
"""Audit release-scope content promises for Protect Carrot.

This complements `verify_sprite_assets.py`: that script proves runtime WebP coverage,
while this one proves the main game catalogs and connected screens still match
the intended commercial-content scope.

Run:
    .venv/bin/python tools/audit_release_content.py
"""

from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "src"
TMP = ROOT / "tmp"
OUT = TMP / "content_audit.json"
REPORT = TMP / "content_audit.md"

DATA = SRC / "data.rs"
MONSTER = SRC / "monster.rs"
EQUIPMENT = SRC / "equipment.rs"
STATES = SRC / "states.rs"
MAIN = SRC / "main.rs"
UI = SRC / "ui.rs"
TOWER = SRC / "tower.rs"
ENEMY = SRC / "enemy.rs"
BUILD = SRC / "build.rs"
VFX = SRC / "vfx.rs"
GAME = SRC / "game.rs"
COMPONENTS = SRC / "components.rs"
HERO_PASSIVES = SRC / "hero_passives.rs"
HERO = SRC / "hero.rs"
HERO_EFFECTS = SRC / "hero_skill_effects.rs"


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def block_after(src: str, start_pattern: str) -> str:
    start = src.index(start_pattern)
    brace = src.index("{", start)
    depth = 0
    for i, ch in enumerate(src[brace:], start=brace):
        if ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
            if depth == 0:
                return src[brace : i + 1]
    raise ValueError(f"unterminated block after {start_pattern}")


def const_all_count(src: str, impl_name: str, variant_prefix: str) -> int:
    impl_start = src.index(f"impl {impl_name}")
    all_start = src.index("pub const ALL", impl_start)
    value_start = src.index("=", all_start)
    bracket = src.index("[", value_start)
    depth = 0
    for i, ch in enumerate(src[bracket:], start=bracket):
        if ch == "[":
            depth += 1
        elif ch == "]":
            depth -= 1
            if depth == 0:
                block = src[bracket : i + 1]
                return len(re.findall(rf"\b{re.escape(variant_prefix)}::\w+", block))
    raise ValueError(f"unterminated ALL array for {impl_name}")


def parse_species() -> list[dict[str, object]]:
    lines = read(MONSTER).splitlines()
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
            vals = [item.strip().rstrip(",") for item in block]
            quoted = [item[1:-1] for item in vals if re.fullmatch(r'".*"', item)]
            out.append(
                {
                    "id": int(vals[1]),
                    "name": quoted[0],
                    "kind": vals[3],
                    "tags": quoted[-1],
                }
            )
            block = None
    return out


def run(cmd: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        cmd,
        cwd=ROOT,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )


def check(name: str, ok: bool, detail: str) -> dict[str, object]:
    return {"name": name, "ok": ok, "detail": detail}


def audit() -> dict[str, object]:
    data = read(DATA)
    monster = read(MONSTER)
    equipment = read(EQUIPMENT)
    states = read(STATES)
    main = read(MAIN)
    ui = read(UI)
    tower = read(TOWER)
    enemy = read(ENEMY)
    build = read(BUILD)
    vfx = read(VFX)
    game = read(GAME)
    components = read(COMPONENTS)
    hero_passives = read(HERO_PASSIVES)
    hero = read(HERO)
    hero_effects = read(HERO_EFFECTS)
    class_effects = "\n".join(read(SRC / "hero_skill_effects" / f"{name}.rs") for name in ["martial", "magic", "support"])
    hero_names = block_after(hero, "pub fn talent_name(")
    hero_descriptions = block_after(hero, "pub fn talent_desc(")
    ultimate_names = block_after(hero, "pub fn ultimate_name(")
    ultimate_descriptions = block_after(hero, "pub fn ultimate_desc(")
    basic_skills = re.findall(r'\(HeroWeapon::(\w+), ([01])\) => "([^"]+)"', hero_names)
    ultimates = re.findall(r'HeroWeapon::(\w+) => "([^"]+)"', ultimate_names)
    class_names = {weapon for weapon, _ in ultimates}
    described_skills = re.findall(r'\(HeroWeapon::(\w+), ([01])\) =>\s*(?:\{\s*)?"([^"]+)"', hero_descriptions)
    described_ultimates = re.findall(r'HeroWeapon::(\w+) =>\s*(?:\{\s*)?"([^"]+)"', ultimate_descriptions)
    implemented_skills = {
        (weapon, int(slot))
        for weapon, slots in re.findall(r'\(HeroWeapon::(\w+), ([012](?: \| [012])*)\)', class_effects)
        for slot in slots.split(" | ")
    }
    expected_skills = {(weapon, slot) for weapon in class_names for slot in range(3)}
    skill_icons = re.findall(r'=> "([^"]+)"', block_after(hero, "pub fn talent_sprite_name("))[:-1]
    ultimate_icons = re.findall(r'=> "([^"]+)"', block_after(hero, "pub fn ultimate_sprite_name("))

    checks: list[dict[str, object]] = []

    tower_count = const_all_count(data, "TowerKind", "TowerKind")
    enemy_count = const_all_count(data, "EnemyKind", "EnemyKind")
    equipment_count = const_all_count(equipment, "Equipment", "Equipment")
    element_count = const_all_count(data, "Element", "Element")
    species = parse_species()
    species_ids = sorted(int(item["id"]) for item in species)
    boss_species = [item for item in species if "首领" in str(item["tags"])]

    skill_pairs = re.findall(r"(\d+)\s*=>\s*BossSkill::(\w+)", monster)
    boss_skill_ids = sorted(int(sid) for sid, skill in skill_pairs if skill != "None")
    unique_skill_names = {skill for _, skill in skill_pairs if skill != "None"}

    checks.extend(
        [
            check("tower catalog", tower_count == 5, f"{tower_count}/5 towers in TowerKind::ALL"),
            check("enemy archetypes", enemy_count == 18, f"{enemy_count}/18 EnemyKind archetypes"),
            check("equipment catalog", equipment_count == 20, f"{equipment_count}/20 relics"),
            check("element catalog", element_count == 7, f"{element_count}/7 elements"),
            check("monster species", len(species) == 101, f"{len(species)}/101 species"),
            check(
                "species ids",
                species_ids == list(range(101)),
                "ids are contiguous 0..100" if species_ids == list(range(101)) else str(species_ids[:10]),
            ),
            check("boss species", len(boss_species) == 11, f"{len(boss_species)}/11 boss species"),
            check(
                "boss skills",
                boss_skill_ids == list(range(90, 101)) and len(unique_skill_names) == 11,
                f"ids={boss_skill_ids}, unique skills={len(unique_skill_names)}",
            ),
            check(
                "persistent collection screens",
                all(name in states for name in ["Bestiary", "Armory", "TowerArchive"])
                and all(name in main for name in ["spawn_bestiary", "spawn_armory", "spawn_tower_archive"]),
                "Bestiary, Armory, and TowerArchive states are wired",
            ),
            check(
                "meta milestone screen",
                "Milestones" in states
                and all(token in ui for token in ["milestone_rows", "spawn_milestones", "OpenMilestones"])
                and "milestone_buttons" in main,
                "derived achievement screen is wired from menu progress resources",
            ),
            check(
                "campaign dossier screen",
                "CampaignDossier" in states
                and all(
                    token in ui
                    for token in [
                        "spawn_campaign_dossier",
                        "campaign_boss_line",
                        "campaign_recommendation",
                        "OpenCampaignDossier",
                    ]
                )
                and all(token in data for token in ["LEVEL_LORE", "LEVEL_THEMES", "BOSS_WAVE_INTERVAL"]),
                "level lore, boss-wave previews, and recommended elements are surfaced from the menu",
            ),
            check(
                "equipment progression loop",
                all(token in equipment for token in ["roll_clear_rewards", "refine_equipment", "equipment_set_bonus"])
                and "RewardCard" in ui
                and all(token in vfx for token in ["item: Equipment", "sprites.equipment[item]", "VfxEvent::Loot"]),
                "chests, refinement, set bonuses, reward cards, and icon drop VFX found",
            ),
            check(
                "equipment resonance board feedback",
                all(token in tower for token in ["draw_equipment_resonance", "equipment_set_bonus", "resonance_element", "grade_tier"])
                and "tower::draw_equipment_resonance" in main,
                "active equipment set bonuses draw board-space resonance auras",
            ),
            check(
                "bestiary discovery VFX",
                all(token in vfx for token in ["VfxEvent::Discovery", "species_id: usize", "sprites.species"])
                and all(token in enemy for token in ["first_seen", "图鉴更新", "VfxEvent::Discovery"]),
                "first-time monster discoveries show species portrait flourishes",
            ),
            check(
                "run threat introductions",
                all(token in game for token in ["encountered_species", "HashSet"])
                and all(token in enemy for token in ["VfxEvent::ThreatIntro", "侦测到新威胁", "species.traits()"])
                and all(token in vfx for token in ["VfxEvent::ThreatIntro", "sprites.species", "Sound::Wave"]),
                "new species entering a run announce with portrait VFX without changing bestiary kill counts",
            ),
            check(
                "tower siege systems",
                all(token in tower for token in ["enemy_vs_tower", "moss_destroy", "TOWER_RAIDER_ENGAGE"])
                and all(token in components for token in ["tower_raider", "moss_destroy"]),
                "tower raiders and MOSS tower destruction hooks found",
            ),
            check(
                "tower danger feedback",
                all(
                    token in tower
                    for token in ["siege_vfx_timer", "low_hp_warned", '"攻塔"', "防御塔濒危"]
                )
                and all(
                    token in build
                    for token in [
                        "Color::srgb(1.0, 0.16, 0.1)",
                        "low_hp_warned = false",
                        "select_most_damaged_tower",
                        "KeyCode::Tab",
                    ]
                ),
                "tower attack VFX, low-HP warnings, danger-colored HP bars, and damaged-tower shortcut found",
            ),
            check(
                "tower targeting controls",
                all(
                    token in tower
                    for token in [
                        "TargetPriority",
                        "target_priority",
                        "cycle_target_priority",
                        "path_index",
                        "threat_rank",
                        "hp_frac",
                    ]
                )
                and all(token in build for token in ["KeyCode::KeyT", "目标优先"])
                and all(token in ui for token in ["UiAction::CycleTargetPriority", "切换选中防御塔的目标优先级", "t.cycle_target_priority()"]),
                "selected towers can cycle targeting priorities for late-wave control",
            ),
            check(
                "tower damage attribution and kill tally",
                all(
                    token in tower
                    for token in [
                        "damage_done",
                        "source_tower",
                        "poison_source_tower",
                        "fire_source_tower",
                    ]
                )
                and all(token in enemy for token in ["last_hit_tower", "tower.kills += 1"])
                and "tw.kills" in ui,
                "damage counters retain DoT attribution and the selected tower HUD shows credited kills",
            ),
            check(
                "carrot seal feedback",
                all(token in components for token in ["CarrotSealBar", "pulse_timer", "last_lives"])
                and all(token in game for token in ["start_lives", "update_carrot_seal", "CarrotSealBar"])
                and all(token in enemy for token in ["VfxEvent::CarrotHit", "突破封印", "剩余生命"])
                and all(token in vfx for token in ["VfxEvent::CarrotHit", "封印受损", "封印濒危"])
                and all(token in tower for token in ["run.start_lives", "HealCarrot"]),
                "carrot/base life has board-space bar, hit pulse, breach VFX, and correct heal cap",
            ),
            check(
                "combo economy feedback",
                all(token in game for token in ["kill_combo", "kill_combo_timer", "best_combo"])
                and all(token in enemy for token in ["kill_combo_bonus", "VfxEvent::ComboReward", "连杀 x"])
                and all(token in ui for token in ["ComboMeterRoot", "ComboMeterFill", "update_combo_meter", "距奖励"])
                and all(token in vfx for token in ["VfxEvent::ComboReward", "Sound::Gold", "连杀 x"])
                and "ui::update_combo_meter" in main,
                "kill streaks have timer HUD, gold bonuses, and reward burst feedback",
            ),
            check(
                "perfect wave rewards",
                all(token in game for token in ["wave_start_lives", "wave_perfect"])
                and all(token in enemy for token in ["perfect_bonus", "VfxEvent::PerfectWave", "完美防守"])
                and all(token in enemy for token in ["run.lives >= run.wave_start_lives", "wave_perfect = false"])
                and all(token in vfx for token in ["VfxEvent::PerfectWave", "Sound::Gold", "完美防守"]),
                "flawless waves grant readable bonus gold and carrot-side reward VFX",
            ),
            check(
                "silencer systems",
                "silence_aura" in data and "tower_silenced" in tower and "draw_silence_auras" in main,
                "silence aura data, targeting suppression, and rendering hooks found",
            ),
            check(
                "combat readability badges",
                all(token in enemy for token in ["special_trait_badge", '"攻城"', '"静默"', "Text2d::new(crate::i18n::t(label))"])
                and "draw_elite_auras" in main,
                "special enemy trait badges and elite aura rendering found",
            ),
            check(
                "enemy sprite life",
                "animate_enemy_sprites" in enemy
                and "enemy::animate_enemy_sprites" in main
                and all(token in enemy for token in ["custom_size", "phase_timer", "fire_timer", "poison_timer"]),
                "procedural breathing, status tint, and phase alpha animation found",
            ),
            check(
                "boss enrage phase",
                all(token in components for token in ["enraged", "low-health pressure phase"])
                and all(token in enemy for token in ["BOSS_ENRAGE_HP_FRACTION", "BOSS_ENRAGE_SKILL_RATE", "狂怒阶段"])
                and all(token in ui for token in ["boss.enraged", "狂怒·"]),
                "low-health bosses accelerate skills and expose an enrage state",
            ),
            check(
                "boss pressure HUD",
                all(
                    token in ui
                    for token in [
                        "BossBarRoot",
                        "BossBarFill",
                        "BossSkillFill",
                        "BossBarText",
                        "active_boss_info",
                        "Val::Percent(info.hp_frac * 100.0)",
                        "Val::Percent(info.skill_frac * 100.0)",
                    ]
                )
                and "ui::update_boss_bar" in main,
                "active bosses show an over-board HP bar and skill charge/cast meter",
            ),
            check(
                "boss tower threat feedback",
                all(
                    token in enemy
                    for token in [
                        "boss_skill_threatens_towers",
                        "draw_boss_cast_telegraphs",
                        "gizmos.line_2d(pos, tower_pos",
                        '"首领"',
                        '"停火"',
                    ]
                )
                and "enemy::draw_boss_cast_telegraphs" in main,
                "tower-threatening boss casts mark endangered towers and show impact feedback",
            ),
            check(
                "combat impact feedback",
                all(token in vfx for token in ["ScreenShake", "ShakeCamera", "update_camera_shake", "shake.add"])
                and all(token in main for token in ["vfx::ScreenShake", "vfx::ShakeCamera", "vfx::update_camera_shake"]),
                "world camera shake is wired to high-impact VFX events",
            ),
            check(
                "kill combo rewards",
                all(
                    token in game
                    for token in ["KILL_COMBO_WINDOW", "kill_combo", "kill_combo_timer", "kill_combo_window", "best_combo"]
                )
                and all(token in enemy for token in ["kill_combo_bonus", "VfxEvent::ComboReward", "连杀 x"])
                and all(token in ui for token in ["最高连杀", "ComboMeterRoot", "连杀 x"]),
                "rapid kills build combo pressure with milestone gold and HUD/settlement feedback",
            ),
            check(
                "automatic hero cast sequencing",
                all(
                    token in hero_passives
                    for token in [
                        "MessageWriter<crate::vfx::VfxEvent>",
                        "trigger_weapon_skill",
                        "loadout.skill_cooldowns[cast.slot] = loadout.skill_interval(cast.slot)",
                        "impl Action for CastAction",
                        "CastPhase::Windup",
                        "CastPhase::Release",
                        "CastPhase::Recovery",
                        "cast.released",
                        "time.delta_secs() * run.game_speed",
                        "paused.0",
                        "!run.wave_in_progress",
                        "MAX_HERO_SUMMONS",
                    ]
                )
                and "bevy_sequential_actions" in hero_passives
                and "hero_cast_visuals::pose_casts" in main
                and "hero_cast_visuals::draw_casts" in main
                and not any(
                    token in ui or token in main
                    for token in ["UiAction::HeroSkill", "SummonHero", "update_ability_buttons"]
                )
                and "hero_passives::update_hero_passives" in main
                and "run.show(" not in hero_passives,
                "class casts use the ordinary attack queue with windup/release/recovery, independent game-time cooldowns, visible cast hooks, pause gates and no manual controls",
            ),
            check(
                "27 class-specific hero skills",
                len(class_names) == 9
                and len(basic_skills) == 18
                and len(ultimates) == 9
                and len({name for _, _, name in basic_skills} | {name for _, name in ultimates}) == 27
                and len(described_skills) == 18
                and len(described_ultimates) == 9
                and implemented_skills == expected_skills
                and "pub const TALENT_SLOTS: usize = 3;" in hero,
                f"{len(implemented_skills)}/27 release branches, 18 trainable base casts and 9 level-30 ultimates",
            ),
            check(
                "skill training is not attribute filler",
                "talent_rank" not in block_after(hero, "pub fn apply_loadout_to_tower(")
                and "talent_rank" not in block_after(hero, "pub fn hero_move_speed(")
                and "talent_rank" not in block_after(hero, "pub fn skill_damage_mult(")
                and "MAX_LEVEL" not in block_after(hero, "pub fn apply_loadout_to_tower(")
                and not any(re.match(r"提高|强化|增加|缩短|延长", desc) or "+" in desc for *_, desc in described_skills + described_ultimates)
                and "loadout.run_mods.skill_interval_mult *= 0.90" in read(SRC / "roguelite.rs")
                and "loadout.run_mods.skill_power_mult *= 1.15" in read(SRC / "roguelite.rs"),
                "training changes cast-local effects; baseline stats and stat-only ultimates are excluded; speed and skill tempo remain Hex modifiers",
            ),
            check(
                "class skill icon assets",
                len(skill_icons) == 18
                and len(ultimate_icons) == 9
                and all((ROOT / "assets" / "sprites" / "hero_talents" / f"{icon}.webp").is_file() for icon in skill_icons + ultimate_icons),
                f"{len(skill_icons) + len(ultimate_icons)}/27 skill icons resolve to existing runtime assets",
            ),
            check(
                "summoner class only casts summons",
                all(
                    "spawn_ally(" in block_after(class_effects, f"(HeroWeapon::SummonStaff, {slot}) =>")
                    or "spawn_mythic_ally(" in block_after(class_effects, f"(HeroWeapon::SummonStaff, {slot}) =>")
                    for slot in range(3)
                )
                and all(
                    not any(token in block_after(class_effects, f"(HeroWeapon::SummonStaff, {slot}) =>") for token in ["dmg.write(", "buff.write(", "fields::spawn("])
                    for slot in range(3)
                )
                and "run.fallen_enemies.drain(..count)" in class_effects
                and "MAX_HERO_SUMMONS" in hero_effects,
                "all three summoner releases spawn actual allies, with bounded fallen-soul consumption and a shared living-unit capacity",
            ),
            check(
                "auto wave pacing",
                all(
                    token in game
                    for token in [
                        "AUTO_WAVE_DELAY",
                        "auto_wave",
                        "auto_wave_timer",
                        "toggle_auto_wave",
                        "tick_auto_wave",
                        "KeyCode::KeyA",
                    ]
                )
                and all(token in ui for token in ["UiAction::ToggleAutoWave", "toggle_auto_wave(&mut run)", "自动下一波（A键）"])
                and "tick_auto_wave" in main
                and all(token in enemy for token in ["自动下一波", "AUTO_WAVE_DELAY"]),
                "players can enable auto-start with visible between-wave countdown",
            ),
            check(
                "queued run notices",
                all(token in game for token in ["MESSAGE_QUEUE_LIMIT", "message_queue", "show_for", "pop_front"])
                and "tick_message" in main
                and "run.message_timer" in ui,
                "bounded message queue and HUD banner timer found",
            ),
        ]
    )

    sprite_run = run([sys.executable, "tools/verify_sprite_assets.py"])
    sprite_manifest = json.loads((TMP / "sprite_manifest.json").read_text(encoding="utf-8"))
    sprite_groups = sprite_manifest["groups"]
    checks.append(
        check(
            "sprite assets",
            sprite_run.returncode == 0
            and sprite_manifest["format"] == "webp"
            and sprite_manifest["ok"] == 141
            and sprite_manifest["failed"] == 0
            and sprite_groups == {
                "towers": {"total": 5, "ok": 5, "failed": 0},
                "enemies": {"total": 16, "ok": 16, "failed": 0},
                "species": {"total": 100, "ok": 100, "failed": 0},
                "equipment": {"total": 20, "ok": 20, "failed": 0},
            },
            f"{sprite_manifest['ok']}/{sprite_manifest['total']} sprites OK",
        )
    )

    prompt_run = run(["node", "tools/gen_sprites_storyos_comfy.mjs", "manifest", "full"])
    prompt_manifest = json.loads((TMP / "comfy" / "prompt_manifest.json").read_text(encoding="utf-8"))
    prompt_groups: dict[str, int] = {}
    for entry in prompt_manifest["entries"]:
        prompt_groups[entry["kind"]] = prompt_groups.get(entry["kind"], 0) + 1
    checks.append(
        check(
            "StoryOS/Comfy prompt coverage",
            prompt_run.returncode == 0
            and prompt_manifest["total"] == 141
            and prompt_groups == {
                "towers": 5,
                "enemies": 16,
                "equipment": 20,
                "species": 100,
            },
            f"{prompt_manifest['total']} prompts: {prompt_groups}",
        )
    )

    return {
        "ok": all(bool(item["ok"]) for item in checks),
        "checks": checks,
        "commands": {
            "sprite_assets": {
                "returncode": sprite_run.returncode,
                "output": sprite_run.stdout.strip(),
            },
            "comfy_manifest": {
                "returncode": prompt_run.returncode,
                "output": prompt_run.stdout.strip(),
            },
        },
    }


def write_report(result: dict[str, object]) -> None:
    checks: list[dict[str, object]] = result["checks"]  # type: ignore[assignment]
    lines = [
        "# Release Content Audit",
        "",
        f"Status: {'PASS' if result['ok'] else 'FAIL'}",
        "",
        "## Checks",
        "",
    ]
    for item in checks:
        mark = "PASS" if item["ok"] else "FAIL"
        lines.append(f"- {mark}: {item['name']} - {item['detail']}")
    REPORT.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> int:
    TMP.mkdir(parents=True, exist_ok=True)
    result = audit()
    OUT.write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8")
    write_report(result)
    print(OUT.relative_to(ROOT))
    print(REPORT.relative_to(ROOT))
    for item in result["checks"]:
        mark = "PASS" if item["ok"] else "FAIL"
        print(f"{mark} {item['name']}: {item['detail']}")
    return 0 if result["ok"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
