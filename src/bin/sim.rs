//! Headless balance simulator for 保卫萝卜.
//!
//! Runs the *real* game simulation systems (no rendering, window, or GPU) with a
//! controlled "sandbox" board — infinite gold, towers placed across every
//! buildable cell — then auto-runs a level's waves and reports per-tower damage.
//!
//! Two modes:
//!   mixed (default) — every tower kind round-robin on one board; reports each
//!                     kind's DAMAGE SHARE of the combined defense. Note: this is
//!                     "kill-credit" share, biased toward burst since fast towers
//!                     land the killing blow before DoT/slow towers contribute.
//!   iso            — one separate run per tower kind (that kind alone fills the
//!                     board); reports each kind's STANDALONE clear power (damage,
//!                     dmg/gold, win/lose, waves survived). Fairer for balance.
//!
//! Usage:  cargo run --bin sim [level_index] [seed] [mixed|iso]
//!   e.g.  cargo run --bin sim 4 12345 iso
//!
//! It reuses `protect_carrot`'s systems verbatim (via the library crate), so the
//! numbers reflect exactly what players experience. Time is stepped at a fixed
//! 1/60s via `TimeUpdateStrategy::ManualDuration` for determinism.

use std::collections::HashMap;
use std::time::Duration;

use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy_sequential_actions::SequentialActionsPlugin;
use bevy_spritesheet_animation::prelude::SpritesheetAnimationPlugin;

use protect_carrot::{
    Levels, audio, bestiary, build, components, data, enemy, equipment as equipment_inv, game,
    hero, hero_gear, i18n, meta, roguelite, states, tower, ui, vfx,
};

use build::spawn_tower;
use data::{BOARD_H, BOARD_W, Behavior, TILE_SIZE, TowerKind, UpgradeMul, cell_center, levels};
use game::{
    CurrentLevel, EncounterRng, GameDifficulty, GameMode, Paused, Rng, RunState, load_level,
    tick_auto_wave, tick_message,
};
use protect_carrot::board::Board;
use tower::{Damage, Tower};

/// Per-tower-kind accumulated stats for the report.
#[derive(Default, Clone, Copy)]
struct KindStat {
    count: u32,
    total_cost: i64,
    damage: f64,
}

#[derive(Resource, Default)]
struct Report {
    per_kind: HashMap<TowerKind, KindStat>,
    hero_damage: f64,
    hero_skill_casts: u32,
    hero_backstabs: u32,
}

#[derive(Resource, Default)]
struct EnemyCount(usize);

/// When `Some(k)`, the board is filled with ONLY tower kind `k` (isolation mode).
#[derive(Resource)]
struct OnlyKind(Option<TowerKind>);

#[derive(Resource)]
struct SimHeroEnabled(bool);

#[derive(Default)]
struct SimSkillTiming {
    wave: i32,
    ready_for: f32,
    pending: bool,
}

fn sim_hero_enabled(enabled: Res<SimHeroEnabled>) -> bool {
    enabled.0
}

fn sim_no_hero() -> bool {
    matches!(
        std::env::var("CARROT_SIM_NO_HERO")
            .ok()
            .as_deref()
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("1" | "true" | "yes" | "on")
    )
}

/// Exercise the same active-skill path a player uses instead of estimating skill
/// damage inside the harness. Skills fire once a useful local target density is
/// present, with a short fallback delay so support and summon weapons still act.
fn sim_auto_cast_hero_skill(
    time: Res<Time>,
    run: Res<RunState>,
    loadout: Res<hero::HeroLoadout>,
    towers: Query<(Entity, &Tower)>,
    enemies: Query<(&components::Enemy, &Transform)>,
    mut actions: MessageWriter<ui::UiActionActivated>,
    mut report: ResMut<Report>,
    mut timing: Local<SimSkillTiming>,
) {
    if timing.pending {
        if loadout.skill_cd > 0 {
            report.hero_skill_casts += 1;
        }
        timing.pending = false;
    }
    if timing.wave != run.wave {
        timing.wave = run.wave;
        timing.ready_for = 0.0;
    }
    if !run.wave_in_progress || loadout.skill_cd > 0 {
        timing.ready_for = 0.0;
        return;
    }

    let Some((hero_entity, hero)) = towers
        .iter()
        .find(|(_, tower)| tower.hero && tower.hp > 0.0)
    else {
        return;
    };
    let hero_pos = hero.center();
    let (radius, desired_targets, max_wait, can_precast) = match loadout.weapon {
        hero::HeroWeapon::BannerSword => (130.0, 3, 2.0, false),
        hero::HeroWeapon::StarfireStaff => (250.0, 4, 1.3, false),
        hero::HeroWeapon::ShadowBow => (280.0, 2, 1.4, false),
        hero::HeroWeapon::OathShield => (180.0, 1, 0.9, false),
        hero::HeroWeapon::StormOrb => (260.0, 4, 0.9, false),
        hero::HeroWeapon::SentryCrossbow => (260.0, 3, 0.8, false),
        // Death Mark is global and can mark six targets in the benchmark build.
        // Waiting for a pack avoids spending the wave's only cast on the opener.
        hero::HeroWeapon::NightDagger => (f32::INFINITY, 5, 1.8, false),
        hero::HeroWeapon::SummonStaff => (300.0, 0, 0.3, true),
        hero::HeroWeapon::ForgeHammer => (170.0, 2, 0.9, false),
    };
    let mut nearby = 0usize;
    let mut boss_in_reach = false;
    let mut any_enemy = false;
    for (enemy, tf) in &enemies {
        if enemy.hp <= 0.0 {
            continue;
        }
        any_enemy = true;
        if tf.translation.truncate().distance(hero_pos) <= radius {
            nearby += 1;
            boss_in_reach |= enemy.boss;
        }
    }
    timing.ready_for += time.delta_secs() * run.game_speed;
    if !any_enemy && (!can_precast || timing.ready_for < max_wait) {
        return;
    }
    if any_enemy && nearby < desired_targets && !boss_in_reach && timing.ready_for < max_wait {
        return;
    }
    timing.ready_for = 0.0;
    timing.pending = true;
    actions.write(ui::UiActionActivated {
        entity: hero_entity,
        action: ui::UiAction::HeroSkill,
    });
}

/// Headless equivalent of a basic player right-clicking the hero toward the
/// current frontline. This keeps balance runs honest: the real game has a free
/// hero, but a stationary headless hero becomes an artificial permanent wall.
fn sim_hero_ai(
    time: Res<Time>,
    run: Res<RunState>,
    board: Res<Board>,
    loadout: Res<hero::HeroLoadout>,
    enemies: Query<(&components::Enemy, &Transform)>,
    mut towers: ParamSet<(Query<&mut Tower>, Query<&Tower>)>,
    mut acc: Local<f32>,
) {
    *acc += time.delta_secs() * run.game_speed;
    let decision_interval = if loadout.weapon == hero::HeroWeapon::NightDagger {
        0.06
    } else {
        0.22
    };
    if *acc < decision_interval {
        return;
    }
    *acc = 0.0;

    let support_weapon = matches!(
        loadout.weapon,
        hero::HeroWeapon::StarfireStaff
            | hero::HeroWeapon::StormOrb
            | hero::HeroWeapon::SentryCrossbow
            | hero::HeroWeapon::SummonStaff
    );
    let hero_aura = towers
        .p1()
        .iter()
        .find(|tower| tower.hero && tower.hp > 0.0)
        .map(|tower| tower.buff_range)
        .unwrap_or(TILE_SIZE * 3.0)
        .max(TILE_SIZE * 2.0);
    let field: Vec<(Vec2, f32, f32)> = towers
        .p1()
        .iter()
        .filter(|tower| !tower.hero && tower.hp > 0.0)
        .map(|tower| {
            let dps =
                tower.damage / tower.cooldown.max(0.05) * behavior_mult(tower.behavior) as f32;
            (tower.center(), tower.range * 0.92, dps)
        })
        .collect();
    let active_enemies = enemies
        .iter()
        .filter(|(enemy, _)| enemy.hp > 0.0)
        .map(|(_, tf)| tf.translation.truncate())
        .collect::<Vec<_>>();
    let hold_line = matches!(
        loadout.weapon,
        hero::HeroWeapon::BannerSword
            | hero::HeroWeapon::OathShield
            | hero::HeroWeapon::ForgeHammer
    );
    let path_len = board.path_world.len();
    let killbox_anchor = board
        .path_world
        .iter()
        .enumerate()
        .filter(|(index, _)| *index > 0 && index.saturating_add(1) < path_len)
        .max_by(|(ai, a), (bi, b)| {
            let score = |index: usize, point: Vec2| {
                let covering = field
                    .iter()
                    .filter(|(center, range, _)| center.distance(point) <= *range)
                    .map(|(_, _, dps)| *dps)
                    .sum::<f32>();
                let commanded = field
                    .iter()
                    .filter(|(center, _, _)| center.distance(point) <= hero_aura)
                    .map(|(_, _, dps)| *dps)
                    .sum::<f32>();
                let depth = index as f32 / path_len.max(1) as f32;
                covering + commanded * 0.8 + depth * 0.001
            };
            score(*ai, **a).total_cmp(&score(*bi, **b))
        })
        .map(|(index, point)| (index, *point));
    let assassin_anchor = board
        .path_world
        .iter()
        .enumerate()
        .filter(|(index, _)| {
            let lower = (path_len / 4).max(1);
            let upper = (path_len * 3 / 4).min(path_len.saturating_sub(2));
            *index >= lower && *index <= upper
        })
        .max_by(|(_, a), (_, b)| {
            let score = |point: Vec2| {
                field
                    .iter()
                    .filter(|(center, range, _)| center.distance(point) <= *range)
                    .map(|(_, _, dps)| *dps)
                    .sum::<f32>()
            };
            score(**a).total_cmp(&score(**b))
        })
        .map(|(index, point)| (index, *point));
    let hold_anchor = hold_line
        .then(|| killbox_anchor.map(|(_, point)| point))
        .flatten();
    let support_anchor = support_weapon
        .then(|| {
            field
                .iter()
                .max_by(|(a, _, _), (b, _, _)| {
                    let score = |candidate: Vec2| {
                        field
                            .iter()
                            .filter(|(center, _, _)| center.distance(candidate) <= hero_aura)
                            .map(|(center, range, dps)| {
                                let active = active_enemies
                                    .iter()
                                    .any(|enemy| center.distance(*enemy) <= *range);
                                let relevance = if active_enemies.is_empty() || active {
                                    1.0
                                } else {
                                    0.08
                                };
                                *dps * relevance
                            })
                            .sum::<f32>()
                    };
                    score(*a).total_cmp(&score(*b))
                })
                .map(|(center, _, _)| *center)
        })
        .flatten();
    let coverage: Vec<(Vec2, f32)> = field
        .iter()
        .map(|(center, range, _)| (*center, *range))
        .collect();
    let covered = |pos: Vec2| {
        coverage.is_empty()
            || coverage
                .iter()
                .any(|(center, r)| center.distance(pos) <= *r)
    };
    let target_enemy = if loadout.weapon == hero::HeroWeapon::NightDagger {
        assassin_anchor.and_then(|(_, anchor)| {
            enemies
                .iter()
                .filter(|(enemy, tf)| {
                    enemy.hp > 0.0
                        && covered(tf.translation.truncate())
                        && tf.translation.truncate().distance(anchor) <= TILE_SIZE * 4.5
                })
                .max_by(|(a, atf), (b, btf)| {
                    let score = |enemy: &components::Enemy, pos: Vec2| {
                        let threat = if enemy.boss {
                            2_000_000.0
                        } else if enemy.elite {
                            700_000.0
                        } else {
                            0.0
                        };
                        threat + enemy.max_hp * 0.5 - pos.distance(anchor) * 40.0
                    };
                    score(a, atf.translation.truncate())
                        .total_cmp(&score(b, btf.translation.truncate()))
                })
        })
    } else {
        enemies
            .iter()
            .filter(|(e, tf)| e.hp > 0.0 && covered(tf.translation.truncate()))
            .max_by(|(a, atf), (b, btf)| {
                let ap = atf.translation.truncate();
                let bp = btf.translation.truncate();
                (a.path_index, (ap.x + ap.y) as i32).cmp(&(b.path_index, (bp.x + bp.y) as i32))
            })
    };
    let fallback = coverage.first().map(|(center, _)| *center);

    let mut heroes = towers.p0();
    let Some(mut hero) = heroes.iter_mut().find(|t| t.hero && t.hp > 0.0) else {
        return;
    };
    let hero_pos = hero.center();
    if let Some(anchor) = hold_anchor {
        hero.move_target = Some(anchor);
        return;
    }
    if let Some(anchor) = support_anchor {
        hero.move_target = Some(anchor);
        return;
    }
    let Some((enemy, tf)) = target_enemy else {
        if loadout.weapon == hero::HeroWeapon::NightDagger {
            if let Some((index, anchor)) = assassin_anchor {
                let facing = board
                    .path_world
                    .get(index.saturating_add(1))
                    .map(|next| (*next - anchor).normalize_or_zero())
                    .filter(|dir| dir.length_squared() > 0.01)
                    .unwrap_or(Vec2::X);
                let reach = (hero.range * 0.78).clamp(TILE_SIZE * 0.42, TILE_SIZE * 0.68);
                hero.move_target = Some(anchor - facing * reach);
                return;
            }
        }
        if let Some(target) = fallback {
            hero.move_target = Some(target);
        }
        return;
    };

    let enemy_pos = tf.translation.truncate();
    let facing = board
        .path_world
        .get(enemy.path_index.saturating_add(1))
        .or_else(|| board.path_world.last())
        .map(|next| (*next - enemy_pos).normalize_or_zero())
        .filter(|dir| dir.length_squared() > 0.01)
        .unwrap_or_else(|| {
            if enemy.facing.length_squared() > 0.01 {
                enemy.facing.normalize()
            } else {
                Vec2::X
            }
        });
    let target = if hold_line {
        // Aim a little behind the enemy so melee heroes intercept the frontline.
        enemy_pos - facing * (TILE_SIZE * 0.35)
    } else if loadout.weapon == hero::HeroWeapon::NightDagger {
        // Stay inside dagger reach on the path-facing rear arc. The gameplay
        // backstab rule suppresses frontal retaliation only while this flank is
        // maintained, so the faster steering cadence is part of competent play.
        let backstab_reach = (hero.range * 0.78).clamp(TILE_SIZE * 0.42, TILE_SIZE * 0.68);
        enemy_pos - facing * backstab_reach
    } else {
        // Ranged weapons should flank the path rather than accidentally becoming
        // melee blockers and compressing a wave into a single leaking pack.
        let normal = Vec2::new(-facing.y, facing.x);
        [
            enemy_pos + normal * (TILE_SIZE * 1.35),
            enemy_pos - normal * (TILE_SIZE * 1.35),
        ]
        .into_iter()
        .filter(|candidate| {
            candidate.x.abs() <= BOARD_W * 0.46 && candidate.y.abs() <= BOARD_H * 0.46
        })
        .max_by(|a, b| {
            let covered_a = coverage
                .iter()
                .filter(|(center, range)| center.distance(*a) <= *range)
                .count();
            let covered_b = coverage
                .iter()
                .filter(|(center, range)| center.distance(*b) <= *range)
                .count();
            covered_a
                .cmp(&covered_b)
                .then_with(|| hero_pos.distance(*b).total_cmp(&hero_pos.distance(*a)))
        })
        .unwrap_or(enemy_pos)
    };
    hero.move_target = Some(Vec2::new(
        target.x.clamp(-BOARD_W * 0.46, BOARD_W * 0.46),
        target.y.clamp(-BOARD_H * 0.46, BOARD_H * 0.46),
    ));
}

/// Sum each tower's cumulative effective damage (`Tower::damage_done`, which the
/// game maintains for both direct hits AND damage-over-time) by kind.
fn collect_damage(towers: Query<&Tower>, mut report: ResMut<Report>) {
    for s in report.per_kind.values_mut() {
        s.damage = 0.0;
    }
    let mut living_hero_damage = 0.0;
    for t in &towers {
        if t.hero {
            living_hero_damage += t.damage_done as f64;
            continue;
        }
        report.per_kind.entry(t.kind).or_default().damage += t.damage_done as f64;
    }
    // A dead hero is despawned before the final report. Preserve the highest
    // cumulative value observed while it existed so weak/suicidal builds do not
    // misleadingly report zero contribution.
    report.hero_damage = report.hero_damage.max(living_hero_damage);
}

fn collect_combat_vfx(mut events: MessageReader<vfx::VfxEvent>, mut report: ResMut<Report>) {
    for event in events.read() {
        if let vfx::VfxEvent::Text { text, .. } = event {
            if text.contains("背击") || text.contains("Backstab") {
                report.hero_backstabs += 1;
            }
        }
    }
}

fn count_enemies(q: Query<(), With<components::Enemy>>, mut c: ResMut<EnemyCount>) {
    c.0 = q.iter().count();
}

/// Fill buildable cells with towers (round-robin across kinds, or a single kind in
/// isolation mode), nearest-the-path first for good coverage.
fn build_full_board(
    mut commands: Commands,
    board: Res<Board>,
    sprites: Res<protect_carrot::sprites::Sprites>,
    talents: Res<meta::Talents>,
    only: Res<OnlyKind>,
    mut run: ResMut<RunState>,
    mut report: ResMut<Report>,
) {
    let dist_to_path = |c: &(i32, i32)| -> i32 {
        board
            .path_cells
            .iter()
            .map(|p| (p.0 - c.0).abs() + (p.1 - c.1).abs())
            .min()
            .unwrap_or(0)
    };
    let mut cells: Vec<(i32, i32)> = board.buildable.iter().copied().collect();
    cells.sort_by(|a, b| dist_to_path(a).cmp(&dist_to_path(b)).then(a.cmp(b)));

    let kinds: Vec<TowerKind> = match only.0 {
        Some(k) => vec![k],
        None => TowerKind::ALL.to_vec(),
    };
    let mut occupied: std::collections::HashSet<(i32, i32)> = std::collections::HashSet::new();
    let mut ki = 0usize;

    for (col, row) in cells {
        if occupied.contains(&(col, row)) {
            continue;
        }
        for _ in 0..kinds.len() {
            let kind = kinds[ki % kinds.len()];
            ki += 1;
            let fp = kind.def().footprint.max(1);
            let fits = (0..fp).all(|dx| {
                (0..fp).all(|dy| {
                    board.buildable.contains(&(col + dx, row + dy))
                        && !occupied.contains(&(col + dx, row + dy))
                })
            });
            let cost = kind.def().cost;
            if fits && run.gold >= cost {
                run.gold -= cost;
                for dx in 0..fp {
                    for dy in 0..fp {
                        occupied.insert((col + dx, row + dy));
                    }
                }
                spawn_tower(&mut commands, kind, col, row, &sprites, &talents);
                let st = report.per_kind.entry(kind).or_default();
                st.count += 1;
                st.total_cost += cost as i64;
                break;
            }
        }
    }
}

/// How the board is populated for a run.
#[derive(Clone, Copy)]
enum RunMode {
    /// Infinite gold, board filled with every kind (or one kind in isolation).
    Sandbox(Option<TowerKind>),
    /// Real economy: a greedy player spends kill-gold to build + upgrade.
    Greedy,
}

/// Behavior weight: AoE/DoT/utility deal more than their per-target number implies.
fn behavior_mult(b: Behavior) -> f64 {
    match b {
        Behavior::Aoe | Behavior::Fire => 3.0,
        Behavior::Summon => 2.5,
        Behavior::Chain => 2.0,
        Behavior::Poison | Behavior::Curse => 1.6,
        Behavior::Slow | Behavior::Freeze | Behavior::Knockback => 1.2,
        Behavior::Heal | Behavior::Detect => 0.05,
        _ => 1.0,
    }
}

/// Rough standalone value of a tower kind: weighted effective DPS per gold.
fn tower_value(kind: TowerKind) -> f64 {
    let d = kind.def();
    let eff = d.damage as f64 / (d.cooldown_ms as f64 / 1000.0).max(0.05);
    eff * behavior_mult(d.behavior) / (d.cost as f64).max(1.0)
}

fn greedy_kind_priority(kind: TowerKind) -> f64 {
    match kind {
        TowerKind::Arrow => 0.72,
        TowerKind::Cannon => 1.24,
        TowerKind::Magic => 1.16,
        TowerKind::Thunder => 1.20,
        TowerKind::Laser | TowerKind::Prism => 1.34,
        TowerKind::Sniper | TowerKind::Missile | TowerKind::Fortress => 1.24,
        TowerKind::Ice | TowerKind::Wind | TowerKind::FrostNova | TowerKind::Shadow => 1.10,
        TowerKind::Poison | TowerKind::Fire => 1.12,
        TowerKind::Summon | TowerKind::Necromancer => 1.18,
        TowerKind::Holy => 0.92,
        TowerKind::Detection => 0.0,
    }
}

fn greedy_tower_plan() -> &'static [TowerKind] {
    &[
        TowerKind::Arrow,
        TowerKind::Cannon,
        TowerKind::Magic,
        TowerKind::Thunder,
        TowerKind::Ice,
        TowerKind::Arrow,
        TowerKind::Fire,
        TowerKind::Wind,
        TowerKind::Poison,
        TowerKind::Laser,
        TowerKind::Cannon,
        TowerKind::Summon,
        TowerKind::Shadow,
        TowerKind::Prism,
        TowerKind::Missile,
        TowerKind::FrostNova,
        TowerKind::Necromancer,
        TowerKind::Fortress,
    ]
}

/// Smarter greedy economy player. Every few frames it picks the single best
/// affordable action:
///   • BUILD the next tower from a mixed real-player plan where it covers the most
///     path, so the harness uses damage, AoE, slow, DoT, summons and endgame towers,
///     not a degenerate all-arrow board, or
///   • UPGRADE an existing tower (each upgrade is +~76% DPS for 0.7×scaled cost —
///     often the best gold sink once the path is covered), or
///   • emergency: if an invisible enemy is on the field and undetected, build a
///     Detection tower covering the most path (otherwise invisibles leak freely).
fn greedy_player(
    mut commands: Commands,
    board: Res<Board>,
    sprites: Res<protect_carrot::sprites::Sprites>,
    talents: Res<meta::Talents>,
    mut run: ResMut<RunState>,
    mut towers: Query<(Entity, &mut Tower)>,
    enemies: Query<(&components::Enemy, &Transform)>,
    time: Res<Time>,
    mut report: ResMut<Report>,
    mut cells: Local<Vec<(i32, i32)>>,
    mut order: Local<Vec<TowerKind>>,
    mut acc: Local<f32>,
) {
    // Decide ~10× per GAME-second (not per frame), so the build cadence is identical
    // at any game_speed — letting the sweep run at 4× speed without weakening play.
    *acc += time.delta_secs() * run.game_speed;
    if *acc < 0.1 {
        return;
    }
    *acc = 0.0;

    if cells.is_empty() {
        let dist = |c: &(i32, i32)| {
            board
                .path_cells
                .iter()
                .map(|p| (p.0 - c.0).abs() + (p.1 - c.1).abs())
                .min()
                .unwrap_or(99)
        };
        let mut cs: Vec<(i32, i32)> = board
            .buildable
            .iter()
            .copied()
            .filter(|p| dist(p) <= 3)
            .collect();
        cs.sort_by(|a, b| dist(a).cmp(&dist(b)).then(a.cmp(b)));
        *cells = cs;
    }

    // Occupancy from existing towers; detectors tracked for the invisible check.
    let mut occupied: std::collections::HashSet<(i32, i32)> = std::collections::HashSet::new();
    let mut detectors: Vec<(Vec2, f32)> = Vec::new();
    let mut tower_count = 0usize;
    for (_, t) in towers.iter() {
        let fp = t.footprint.max(1);
        for dx in 0..fp {
            for dy in 0..fp {
                occupied.insert((t.col + dx, t.row + dy));
            }
        }
        if !t.hero && t.kind != TowerKind::Detection {
            tower_count += 1;
        }
        if t.behavior == Behavior::Detect {
            detectors.push((t.center(), t.range));
        }
    }
    let fits = |kind: TowerKind, col: i32, row: i32| {
        let fp = kind.def().footprint.max(1);
        (0..fp).all(|dx| {
            (0..fp).all(|dy| {
                board.buildable.contains(&(col + dx, row + dy))
                    && !occupied.contains(&(col + dx, row + dy))
            })
        })
    };
    let cell_pos = |col: i32, row: i32, kind: TowerKind| {
        let off = (kind.def().footprint.max(1) - 1) as f32 / 2.0;
        cell_center(col as f32 + off, row as f32 + off)
    };

    // 0) Detection emergency: an invisible enemy nobody can see.
    let invis_uncovered = enemies.iter().any(|(e, tf)| {
        e.invisible && {
            let p = tf.translation.truncate();
            // 隐形分级：探测塔有效射程按 stealth 折扣（与游戏内 is_detected 一致）。
            !detectors
                .iter()
                .any(|(c, r)| c.distance(p) <= *r * e.stealth)
        }
    });
    let detector_cap = (1usize + run.wave.max(1) as usize / 4 + board.level_index / 8).min(5);
    if invis_uncovered && detectors.len() < detector_cap {
        let dk = TowerKind::Detection;
        let dcost = dk.def().cost;
        if run.gold >= dcost {
            let r = dk.def().range * talents.range_mult;
            let mut best: Option<((i32, i32), usize)> = None;
            for &(col, row) in cells.iter() {
                if occupied.contains(&(col, row)) || !fits(dk, col, row) {
                    continue;
                }
                let cen = cell_pos(col, row, dk);
                let n = board
                    .path_world
                    .iter()
                    .filter(|p| cen.distance(**p) <= r)
                    .count();
                if best.map(|(_, b)| n > b).unwrap_or(true) {
                    best = Some(((col, row), n));
                }
            }
            if let Some(((col, row), _)) = best {
                run.gold -= dcost;
                spawn_tower(&mut commands, dk, col, row, &sprites, &talents);
                let st = report.per_kind.entry(dk).or_default();
                st.count += 1;
                st.total_cost += dcost as i64;
                return;
            }
        }
    }

    // Mixed real-player build order (cached). The old value/gold order collapsed
    // into all arrows on late maps, which made the balance harness report false
    // losses for levels that require AoE/control/elemental coverage.
    if order.is_empty() {
        *order = greedy_tower_plan().to_vec();
    }

    // 1) Best BUILD: next planned kind at the free cell covering the most path
    //    (killbox/chokepoint). If that kind cannot fit or is unaffordable, scan
    //    forward through the plan before giving up.
    let mut best_build: Option<(f64, (i32, i32), TowerKind, i32)> = None;
    for offset in 0..order.len() {
        let kind = order[(tower_count + offset) % order.len()];
        let d = kind.def();
        if run.gold < d.cost {
            continue;
        }
        let r = d.range * talents.range_mult;
        let mut bestcell: Option<(i32, i32)> = None;
        let mut bestcov = 0usize;
        for &(col, row) in cells.iter() {
            if occupied.contains(&(col, row)) || !fits(kind, col, row) {
                continue;
            }
            let cen = cell_pos(col, row, kind);
            let n = board
                .path_world
                .iter()
                .filter(|p| cen.distance(**p) <= r)
                .count();
            if n > bestcov {
                bestcov = n;
                bestcell = Some((col, row));
            }
        }
        if let Some(cell) = bestcell {
            let coverage_bonus = 1.0 + bestcov as f64 * 0.08;
            best_build = Some((
                tower_value(kind) * greedy_kind_priority(kind) * coverage_bonus,
                cell,
                kind,
                d.cost,
            ));
            break;
        }
    }

    // 2) Best UPGRADE: marginal weighted-DPS per upgrade gold.
    let up_gain = (UpgradeMul::DAMAGE as f64 / UpgradeMul::COOLDOWN as f64) - 1.0; // ≈0.76
    let mut best_up: Option<(f64, Entity, i32)> = None;
    for (e, t) in towers.iter() {
        if t.hero || t.level >= 9 {
            continue;
        }
        let uc = t.upgrade_cost();
        if uc <= 0 || uc > run.gold {
            continue;
        }
        let cur = (t.damage as f64 / (t.cooldown as f64).max(0.05)) * behavior_mult(t.behavior);
        let score = (cur * up_gain * greedy_kind_priority(t.kind)) / uc as f64;
        if best_up.map(|(s, ..)| score > s).unwrap_or(true) {
            best_up = Some((score, e, uc));
        }
    }

    // 3) Execute the higher-value action. Force enough field presence first; after
    // that, upgrades compete normally against the next planned build.
    let desired_field = (3usize + run.wave.max(1) as usize / 3 + (run.gold.max(0) as usize / 1500))
        .min(cells.len())
        .min(14);
    let upgrade_wins = match (best_build, best_up) {
        (Some(_), Some(_)) if tower_count < desired_field => false,
        (Some((bs, ..)), Some((us, ..))) => us >= bs,
        (None, Some(_)) => true,
        _ => false,
    };
    if upgrade_wins {
        if let Some((_, e, uc)) = best_up {
            if let Ok((_, mut t)) = towers.get_mut(e) {
                run.gold -= uc;
                let kind = t.kind;
                build::upgrade_tower(&mut t);
                t.damage = t.base_damage;
                report.per_kind.entry(kind).or_default().total_cost += uc as i64;
            }
        }
    } else if let Some((_, (col, row), kind, cost)) = best_build {
        run.gold -= cost;
        spawn_tower(&mut commands, kind, col, row, &sprites, &talents);
        let st = report.per_kind.entry(kind).or_default();
        st.count += 1;
        st.total_cost += cost as i64;
    }
}

fn roguelite_choice_score(choice: roguelite::RogueliteTalent, run: &RunState) -> i32 {
    use roguelite::RogueliteTalent::*;
    match choice {
        TowerOverclock => 110,
        GemResonance => 104,
        WeaponSignature => 94,
        WeaponMastery => 88,
        WeaponTempo => 82,
        HumanFormation => 76,
        OrcWarDrum => 74,
        ElfForestSight => 72,
        HumanLogistics => {
            if run.gold < 180 {
                86
            } else {
                68
            }
        }
        CarrotDividend => {
            if run.gold < 160 {
                84
            } else {
                58
            }
        }
        OrcBloodrage => 64,
        ElfMoonstep => 62,
    }
}

/// The real game blocks after each cleared wave until the player picks one of
/// three roguelite talents. The balance harness needs to play that draft, or it
/// will sit at full lives until the frame cap and report a false loss.
fn auto_pick_roguelite(
    mut roguelite_run: ResMut<roguelite::RogueliteRun>,
    mut run: ResMut<RunState>,
    mut loadout: ResMut<hero::HeroLoadout>,
    mut talents: ResMut<meta::Talents>,
    mut towers: Query<(Entity, &mut Tower)>,
) {
    let Some(draft) = roguelite_run.draft.as_ref() else {
        return;
    };
    let best = draft
        .choices
        .iter()
        .enumerate()
        .max_by_key(|(_, choice)| roguelite_choice_score(**choice, &run))
        .map(|(index, _)| index)
        .unwrap_or(0);
    let _ = roguelite_run.pick(best, &mut loadout, &mut talents, &mut run, &mut towers);
}

struct SimResult {
    per_kind: HashMap<TowerKind, KindStat>,
    hero_damage: f64,
    hero_skill_casts: u32,
    hero_backstabs: u32,
    outcome: &'static str,
    wave: i32,
    total_waves: i32,
    lives: i32,
    frames: u32,
    enemies: usize,
    wave_in_progress: bool,
    draft_waiting: bool,
    spawned: i32,
    spawn_target: i32,
    enemy_debug: String,
    hero_debug: String,
    gold: i32,
}

fn use_saved_profile() -> bool {
    matches!(
        std::env::var("CARROT_SIM_USE_SAVE")
            .ok()
            .as_deref()
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("1" | "true" | "yes" | "on")
    )
}

#[derive(Clone, Copy)]
enum HeroScenario {
    Baseline,
    Saved,
    Build {
        label: &'static str,
        race: hero::Race,
        weapon: hero::HeroWeapon,
        level: u8,
        gear: [Option<hero_gear::HeroGear>; hero_gear::HeroGearSlot::COUNT],
    },
}

impl HeroScenario {
    fn from_env() -> Self {
        if use_saved_profile() {
            HeroScenario::Saved
        } else {
            HeroScenario::Baseline
        }
    }

    fn label(self) -> &'static str {
        match self {
            HeroScenario::Baseline => "baseline",
            HeroScenario::Saved => "saved",
            HeroScenario::Build { label, .. } => label,
        }
    }

    fn loadout(self) -> hero::HeroLoadout {
        match self {
            HeroScenario::Saved => hero::HeroLoadout::default(),
            HeroScenario::Baseline => baseline_hero_loadout(),
            HeroScenario::Build {
                race,
                weapon,
                level,
                gear,
                ..
            } => trained_hero_loadout(race, weapon, level, gear),
        }
    }

    fn weapon(self) -> hero::HeroWeapon {
        match self {
            HeroScenario::Baseline | HeroScenario::Saved => hero::HeroWeapon::BannerSword,
            HeroScenario::Build { weapon, .. } => weapon,
        }
    }

    fn gear(self) -> [Option<hero_gear::HeroGear>; hero_gear::HeroGearSlot::COUNT] {
        match self {
            HeroScenario::Build { gear, .. } => gear,
            HeroScenario::Baseline | HeroScenario::Saved => hero_gear::empty_gear(),
        }
    }

    fn field_hero_enabled(self) -> bool {
        true
    }
}

fn baseline_hero_loadout() -> hero::HeroLoadout {
    hero::HeroLoadout {
        race: hero::Race::Human,
        weapon: hero::HeroWeapon::BannerSword,
        level: 1,
        xp: 0,
        talent_points: 0,
        weapon_talents: [[0; hero::HeroLoadout::TALENT_SLOTS]; hero::HeroWeapon::ALL.len()],
        gear: hero_gear::empty_gear(),
        skill_cd: 0,
        run_mods: hero::HeroRunMods::default(),
        alive: false,
        respawn_waves: 0,
    }
}

fn trained_hero_loadout(
    race: hero::Race,
    weapon: hero::HeroWeapon,
    level: u8,
    gear: [Option<hero_gear::HeroGear>; hero_gear::HeroGearSlot::COUNT],
) -> hero::HeroLoadout {
    let mut talents = [[0; hero::HeroLoadout::TALENT_SLOTS]; hero::HeroWeapon::ALL.len()];
    let weapon_index = hero::HeroWeapon::ALL
        .iter()
        .position(|candidate| *candidate == weapon)
        .unwrap_or(0);
    for slot in 0..hero::HeroLoadout::TALENT_SLOTS {
        if slot != weapon.ult_slot() {
            talents[weapon_index][slot] = 3;
        }
    }
    hero::HeroLoadout {
        race,
        weapon,
        level: level.clamp(1, hero::HeroLoadout::MAX_LEVEL),
        xp: 0,
        talent_points: 0,
        weapon_talents: talents,
        gear,
        skill_cd: 0,
        run_mods: hero::HeroRunMods::default(),
        alive: false,
        respawn_waves: 0,
    }
}

fn gear_slots(
    armor: hero_gear::HeroGear,
    charm: hero_gear::HeroGear,
    relic: hero_gear::HeroGear,
    boots: hero_gear::HeroGear,
) -> [Option<hero_gear::HeroGear>; hero_gear::HeroGearSlot::COUNT] {
    let items = [
        (hero_gear::HeroGearSlot::Armor, armor),
        (hero_gear::HeroGearSlot::Charm, charm),
        (hero_gear::HeroGearSlot::Relic, relic),
        (hero_gear::HeroGearSlot::Boots, boots),
    ];
    let mut slots = hero_gear::empty_gear();
    for (expected, item) in items {
        assert_eq!(
            item.def().slot,
            expected,
            "invalid sim build: {} belongs in {}, not {}",
            item.def().name,
            item.def().slot.name(),
            expected.name()
        );
        hero_gear::equip(&mut slots, item);
    }
    slots
}

fn benchmark_hero_level(level_index: usize) -> u8 {
    let last = levels().len().saturating_sub(1).max(1);
    let progress = level_index.min(last) as f32 / last as f32;
    (1.0 + progress * (hero::HeroLoadout::MAX_LEVEL - 1) as f32)
        .round()
        .clamp(1.0, hero::HeroLoadout::MAX_LEVEL as f32) as u8
}

fn hero_build_profiles(build_level: u8) -> [HeroScenario; 10] {
    use hero::HeroWeapon::*;
    use hero_gear::HeroGear::*;
    let build_level = build_level.clamp(1, hero::HeroLoadout::MAX_LEVEL);
    [
        HeroScenario::Baseline,
        HeroScenario::Build {
            label: "banner-blood",
            race: hero::Race::Human,
            weapon: BannerSword,
            level: build_level,
            gear: gear_slots(WarflagTabard, BloodBanner, DragonheartCrown, WayfarerBoots),
        },
        HeroScenario::Build {
            label: "starfire-burst",
            race: hero::Race::Elf,
            weapon: StarfireStaff,
            level: build_level,
            gear: gear_slots(StarweaveRobe, EmberPrayer, MeteorCodex, StarpathSandals),
        },
        HeroScenario::Build {
            label: "shadow-bounty",
            race: hero::Race::Elf,
            weapon: ShadowBow,
            level: build_level,
            gear: gear_slots(WindrunnerCloak, BountyQuiver, BrassCompass, CarrotWings),
        },
        HeroScenario::Build {
            label: "oath-bulwark",
            race: hero::Race::Human,
            weapon: OathShield,
            level: build_level,
            gear: gear_slots(VowPlate, CitadelSeal, DragonheartCrown, WayfarerBoots),
        },
        HeroScenario::Build {
            label: "storm-matrix",
            race: hero::Race::Elf,
            weapon: StormOrb,
            level: build_level,
            gear: gear_slots(MoonthreadVest, ThunderCharm, TempestCore, StarpathSandals),
        },
        HeroScenario::Build {
            label: "sentry-array",
            race: hero::Race::Human,
            weapon: SentryCrossbow,
            level: build_level,
            gear: gear_slots(WindrunnerCloak, BountyQuiver, SentryScope, CarrotWings),
        },
        HeroScenario::Build {
            label: "night-backstab",
            race: hero::Race::Orc,
            weapon: NightDagger,
            level: build_level,
            gear: gear_slots(AssassinWraps, NightMask, BrassCompass, BloodstepGreaves),
        },
        HeroScenario::Build {
            label: "summon-pact",
            race: hero::Race::Elf,
            weapon: SummonStaff,
            level: build_level,
            gear: gear_slots(NullMantle, MythcallerTotem, RiftIdol, SummonerGreaves),
        },
        HeroScenario::Build {
            label: "forge-workshop",
            race: hero::Race::Orc,
            weapon: ForgeHammer,
            level: build_level,
            gear: gear_slots(
                WildhideHarness,
                ClockworkBadge,
                GolemBlueprint,
                EngineerTreads,
            ),
        },
    ]
}

/// Build a fresh headless app, fill the board, run the level's waves to a
/// resolution (win/lose) or a frame cap, and return the collected stats.
fn run_sim(level: usize, seed: u64, mode: RunMode, econ: Option<(i32, f32)>) -> SimResult {
    run_sim_with_hero(level, seed, mode, econ, HeroScenario::from_env())
}

fn run_sim_with_hero(
    level: usize,
    seed: u64,
    mode: RunMode,
    econ: Option<(i32, f32)>,
    hero_profile: HeroScenario,
) -> SimResult {
    let only = match mode {
        RunMode::Sandbox(k) => k,
        RunMode::Greedy => None,
    };
    let greedy = matches!(mode, RunMode::Greedy);
    let field_hero_enabled = greedy && hero_profile.field_hero_enabled() && !sim_no_hero();
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(AssetPlugin {
        meta_check: bevy::asset::AssetMetaCheck::Never,
        ..default()
    });
    app.add_plugins(SequentialActionsPlugin);
    app.add_plugins(SpritesheetAnimationPlugin);
    app.init_asset::<Image>()
        .init_asset::<bevy::audio::AudioSource>()
        .init_asset::<Font>()
        .init_asset::<TextureAtlasLayout>()
        .init_asset::<Mesh>()
        .init_asset::<ColorMaterial>();
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
        1.0 / 60.0,
    )));
    app.add_plugins(bevy::state::app::StatesPlugin);
    app.init_state::<states::GameState>();
    app.insert_resource(ui::UiFont(Handle::default()));

    app.insert_resource(Levels(levels()))
        .insert_resource(CurrentLevel(level))
        .insert_resource(Rng(seed))
        .insert_resource(EncounterRng::seeded(seed))
        .insert_resource(OnlyKind(only))
        .insert_resource(SimHeroEnabled(field_hero_enabled))
        .insert_resource(protect_carrot::lighting::LightingSettings::load())
        .init_resource::<Paused>()
        .init_resource::<GameMode>()
        .init_resource::<GameDifficulty>()
        .init_resource::<tower::Snapshot>()
        .insert_resource(hero_profile.loadout())
        .init_resource::<hero_gear::HeroGearInventory>()
        .init_resource::<meta::Talents>()
        .init_resource::<meta::Abilities>()
        .init_resource::<protect_carrot::roguelite::RogueliteRun>()
        .insert_resource(if matches!(hero_profile, HeroScenario::Saved) {
            equipment_inv::EquipmentInventory::default()
        } else {
            equipment_inv::EquipmentInventory { counts: [0; 20] }
        })
        .init_resource::<bestiary::Bestiary>()
        .init_resource::<build::Selection>()
        .init_resource::<ui::JoystickState>()
        .init_resource::<vfx::ScreenShake>()
        .init_resource::<audio::AudioSettings>()
        .init_resource::<i18n::Language>()
        .init_resource::<Report>()
        .init_resource::<EnemyCount>();

    if matches!(hero_profile, HeroScenario::Saved) {
        app.add_systems(Startup, ui::load_persistent_progress);
    }

    app.add_message::<Damage>()
        .add_message::<tower::Status>()
        .add_message::<tower::BuffTower>()
        .add_message::<tower::HealCarrot>()
        .add_message::<vfx::VfxEvent>()
        .add_message::<audio::SfxEvent>()
        .add_message::<tower::EnemyDied>()
        .add_message::<ui::UiActionActivated>();

    let assets = app.world().resource::<AssetServer>().clone();
    app.insert_resource(protect_carrot::sprites::build_sprites(&assets));
    app.world_mut()
        .run_system_once(protect_carrot::creatures::load_creatures)
        .expect("load_creatures");
    app.world_mut()
        .run_system_once(build::load_hero_walks)
        .expect("load_hero_walks");

    // The real per-frame simulation chain (same order as the game's Playing state),
    // minus rendering/gizmos/UI. Split into two ≤20 tuples (Bevy's tuple cap).
    app.add_systems(
        Update,
        (
            (
                sim_hero_ai.run_if(sim_hero_enabled),
                build::hero_move.run_if(sim_hero_enabled),
                sim_auto_cast_hero_skill.run_if(sim_hero_enabled),
                ui::hero_buttons.run_if(sim_hero_enabled),
                tower::build_snapshot,
                hero::hero_doctrine,
                tower::update_towers,
                tower::update_projectiles,
                tower::update_shot_fx,
                tower::update_summons,
                tower::tick_attack_actions,
                tower::apply_buffs,
                tower::apply_heal,
                tower::apply_status,
                tower::apply_damage,
            )
                .chain(),
            (
                tower::enemy_vs_ally,
                tower::enemy_vs_tower,
                enemy::boss_specials,
                tower::update_fire_grounds,
                enemy::spawn_enemies,
                enemy::update_enemies,
                tower::necromancer_raise,
                enemy::heal_auras,
                enemy::incubation,
                protect_carrot::mutators::starfall,
                protect_carrot::mutators::void_rift,
                build::hero_status,
                build::hero_respawn.run_if(sim_hero_enabled),
                tick_auto_wave,
                tick_message,
            )
                .chain(),
        )
            .chain(),
    );
    app.add_systems(
        Update,
        (
            game::update_carrot_seal,
            tower::compute_synergy,
            count_enemies,
            collect_damage,
            collect_combat_vfx,
        ),
    );
    if greedy {
        app.add_systems(Update, greedy_player);
    }
    app.add_systems(Update, auto_pick_roguelite);

    // Optimizer hook: override this level's starting gold + kill reward at runtime
    // (before load_level reads them) so the Layer-3 search can try economies without
    // recompiling.
    if let Some((gold, reward)) = econ {
        let mut lv = app.world_mut().resource_mut::<Levels>();
        lv.0[level].gold = gold;
        lv.0[level].enemies.reward = reward;
    }
    app.init_resource::<protect_carrot::mutators::MutatorState>();
    app.world_mut()
        .run_system_once(load_level)
        .expect("load_level");
    app.world_mut()
        .run_system_once(protect_carrot::mutators::setup_mutators)
        .expect("setup_mutators");
    if greedy && field_hero_enabled {
        app.world_mut()
            .run_system_once(build::auto_spawn_hero)
            .expect("auto_spawn_hero");
    }
    // Keep game_speed = 1 for accurate physics: higher speeds coarsen the per-frame
    // dt, making projectiles overshoot and towers weaker (distorts difficulty). The
    // headless sim already runs ~8–10× faster than real-time at 1×.
    if greedy {
        // Real economy: keep the level's starting gold; the greedy player earns more
        // from kills. Just auto-advance waves.
        app.world_mut().resource_mut::<RunState>().auto_wave = true;
    } else {
        // Sandbox: infinite gold + a full pre-built board.
        {
            let mut run = app.world_mut().resource_mut::<RunState>();
            run.gold = 1_000_000;
            run.auto_wave = true;
        }
        app.world_mut()
            .run_system_once(build_full_board)
            .expect("build_full_board");
    }

    let total_waves_for_cap = app.world().resource::<RunState>().total_waves.max(1) as u32;
    let cap_minutes = if greedy {
        (12 + total_waves_for_cap).min(36)
    } else {
        20
    };
    let max_frames = std::env::var("CARROT_SIM_MAX_FRAMES")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(60 * 60 * cap_minutes);
    let mut frame = 0u32;
    let outcome;
    loop {
        app.update();
        frame += 1;
        let run = app.world().resource::<RunState>();
        let enemies = app.world().resource::<EnemyCount>().0;
        if run.lives <= 0 {
            outcome = "DEFEAT";
            break;
        }
        if run.wave >= run.total_waves && !run.wave_in_progress && enemies == 0 {
            outcome = "VICTORY";
            break;
        }
        if frame >= max_frames {
            outcome = "TIMEOUT";
            break;
        }
    }
    app.world_mut()
        .run_system_once(collect_damage)
        .expect("collect_damage");

    let enemy_debug = {
        let mut q = app.world_mut().query::<(&components::Enemy, &Transform)>();
        q.iter(app.world())
            .take(3)
            .map(|(e, tf)| {
                let name = protect_carrot::monster::species_by_id(e.species_id)
                    .map(|species| species.name)
                    .unwrap_or("未知");
                format!(
                    "{} hp{:.0}/{:.0} p{} spd={:.2} slow={:.2} blocked={} frozen={} stun={:.2} invis={} fly={} charge={} pos=({:.0},{:.0})",
                    i18n::t(name),
                    e.hp,
                    e.max_hp,
                    e.path_index,
                    e.base_speed,
                    e.slow_timer,
                    e.blocked,
                    e.frozen,
                    e.stun_timer,
                    e.invisible,
                    e.flying,
                    e.charger,
                    tf.translation.x,
                    tf.translation.y
                )
            })
            .collect::<Vec<_>>()
            .join(" | ")
    };
    let hero_debug = {
        let mut q = app.world_mut().query::<&Tower>();
        q.iter(app.world())
            .find(|tower| tower.hero)
            .map(|tower| {
                let pos = tower.center();
                format!(
                    "alive hp={:.0}/{:.0} dmg={:.0} range={:.0} cd={:.2} pos=({:.0},{:.0})",
                    tower.hp, tower.max_hp, tower.damage, tower.range, tower.cooldown, pos.x, pos.y
                )
            })
            .unwrap_or_else(|| "dead".to_string())
    };
    let run = app.world().resource::<RunState>();
    let report = app.world().resource::<Report>();
    let enemies = app.world().resource::<EnemyCount>().0;
    let draft_waiting = app
        .world()
        .resource::<protect_carrot::roguelite::RogueliteRun>()
        .is_waiting();
    SimResult {
        per_kind: report.per_kind.clone(),
        hero_damage: report.hero_damage,
        hero_skill_casts: report.hero_skill_casts,
        hero_backstabs: report.hero_backstabs,
        outcome,
        wave: run.wave,
        total_waves: run.total_waves,
        lives: run.lives,
        frames: frame,
        enemies,
        wave_in_progress: run.wave_in_progress,
        draft_waiting,
        spawned: run.spawned,
        spawn_target: run.spawn_target,
        enemy_debug,
        hero_debug,
        gold: run.gold,
    }
}

/// Run the greedy player over `n` seeds of a level. Returns
/// (wins, timeouts, avg_waves, avg_lives, total_waves, avg hero damage,
/// avg skill casts, tower-usage).
fn greedy_winrate(
    level: usize,
    base_seed: u64,
    n: u64,
    econ: Option<(i32, f32)>,
) -> (u32, u32, f32, f32, i32, f64, f32, HashMap<TowerKind, u64>) {
    greedy_winrate_with_hero(level, base_seed, n, econ, HeroScenario::from_env())
}

fn greedy_winrate_with_hero(
    level: usize,
    base_seed: u64,
    n: u64,
    econ: Option<(i32, f32)>,
    hero_profile: HeroScenario,
) -> (u32, u32, f32, f32, i32, f64, f32, HashMap<TowerKind, u64>) {
    let mut wins = 0u32;
    let mut timeouts = 0u32;
    let mut waves = 0i64;
    let mut lives = 0i64;
    let mut total_waves = 0i32;
    let mut hero_damage = 0.0;
    let mut hero_skill_casts = 0u64;
    let mut usage: HashMap<TowerKind, u64> = HashMap::new();
    for s in 0..n {
        let r = run_sim_with_hero(
            level,
            base_seed.wrapping_add(s),
            RunMode::Greedy,
            econ,
            hero_profile,
        );
        if n == 1 && r.outcome != "VICTORY" {
            eprintln!(
                "[sim/build-debug] {} outcome={} wave={}/{} lives={} gold={} enemies={} spawned={}/{} frames={} active={} draft={} skills={} backstabs={} hero={} front={}",
                hero_profile.label(),
                r.outcome,
                r.wave,
                r.total_waves,
                r.lives,
                r.gold,
                r.enemies,
                r.spawned,
                r.spawn_target,
                r.frames,
                r.wave_in_progress,
                r.draft_waiting,
                r.hero_skill_casts,
                r.hero_backstabs,
                r.hero_debug,
                r.enemy_debug,
            );
        }
        if r.outcome == "VICTORY" {
            wins += 1;
        }
        if r.outcome == "TIMEOUT" {
            timeouts += 1;
        }
        waves += r.wave as i64;
        lives += r.lives.max(0) as i64;
        total_waves = r.total_waves;
        hero_damage += r.hero_damage;
        hero_skill_casts += r.hero_skill_casts as u64;
        for (k, st) in &r.per_kind {
            *usage.entry(*k).or_default() += st.count as u64;
        }
    }
    (
        wins,
        timeouts,
        waves as f32 / n as f32,
        lives as f32 / n as f32,
        total_waves,
        hero_damage / n as f64,
        hero_skill_casts as f32 / n as f32,
        usage,
    )
}

/// Print a per-tower table (damage share + dmg/gold) for one run.
fn print_share(title: &str, level: usize, level_name: &str, seed: u64, r: &SimResult, note: &str) {
    let total_dmg: f64 = r.per_kind.values().map(|s| s.damage).sum();
    let total_spent: i64 = r.per_kind.values().map(|s| s.total_cost).sum();
    let mut rows: Vec<(TowerKind, KindStat)> = r.per_kind.iter().map(|(k, s)| (*k, *s)).collect();
    rows.sort_by(|a, b| b.1.damage.partial_cmp(&a.1.damage).unwrap());

    println!("\n============== {title} ==============");
    println!(
        "level {} ({})  seed {}  {}  waves {}/{}  lives {}  enemies {}  active {}  draft {}  spawned {}/{}  gold-spent {}  sim {:.1}s",
        level,
        level_name,
        seed,
        r.outcome,
        r.wave,
        r.total_waves,
        r.lives,
        r.enemies,
        r.wave_in_progress,
        r.draft_waiting,
        r.spawned,
        r.spawn_target,
        total_spent,
        r.frames as f32 / 60.0
    );
    if r.enemies > 0 && !r.enemy_debug.is_empty() {
        println!("remaining: {}", r.enemy_debug);
    }
    println!("total effective damage: {total_dmg:.0}\n");
    println!(
        "{:<14} {:>4} {:>8} {:>12} {:>7} {:>10}",
        "tower", "cnt", "cost", "damage", "share", "dmg/gold"
    );
    println!("{}", "-".repeat(60));
    for (kind, s) in &rows {
        if s.count == 0 && s.total_cost == 0 {
            continue;
        }
        let share = if total_dmg > 0.0 {
            s.damage / total_dmg * 100.0
        } else {
            0.0
        };
        let dpg = if s.total_cost > 0 {
            s.damage / s.total_cost as f64
        } else {
            0.0
        };
        println!(
            "{:<14} {:>4} {:>8} {:>12.0} {:>6.1}% {:>10.2}",
            format!("{:?}", kind),
            s.count,
            s.total_cost,
            s.damage,
            share,
            dpg
        );
    }
    println!("{}", "=".repeat(60));
    if !note.is_empty() {
        println!("{note}\n");
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let level: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(3);
    let seed: u64 = args
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0x1234_5678);
    let mode = args.next().unwrap_or_else(|| "mixed".into());
    let level_name = i18n::t(levels()[level].name);

    match mode.as_str() {
        "opt" => {
            // Layer 3: per-level economy optimizer. For each level, binary-search the
            // smallest economy scale `s` (gold = HP×s, reward = HP×s×0.05) at which the
            // greedy player's win-rate reaches a target curve — then report the
            // recommended starting gold + kill reward. We never go below the original
            // values (early levels keep their hand-tuned generosity).
            let n: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(6);
            let count = levels().len();
            // Target win-rate: gentle descent — easy early, "hard but fair" late.
            let target = |i: usize| (0.95 - 0.02 * i as f32).clamp(0.55, 0.95);
            eprintln!("[sim] LAYER-3 economy optimizer — {count} levels × {n} seeds/eval...");
            println!("\n===== PER-LEVEL ECONOMY OPTIMIZER (target win-rate curve) =====");
            println!(
                "{:>3}  {:<16} {:>5} {:>10} {:>10} {:>6} {:>6}",
                "lvl", "name", "hp", "gold→", "reward→", "win%", "tgt%"
            );
            println!("{}", "-".repeat(64));
            for lvl in 0..count {
                let name = i18n::t(levels()[lvl].name);
                let hp = levels()[lvl].enemies.hp;
                let orig_gold = levels()[lvl].gold;
                let orig_rew = levels()[lvl].enemies.reward;
                let tgt = target(lvl);
                // Binary-search smallest s in [0.4, 3.0] reaching the target win-rate.
                let (mut lo, mut hi) = (0.4f32, 3.0f32);
                let mut win_at_hi = 0.0f32;
                for _ in 0..4 {
                    let s = (lo + hi) / 2.0;
                    let gold = ((hp * s).round() as i32).max(orig_gold);
                    let rew = (hp * s * 0.05).round().max(orig_rew);
                    let (wins, ..) = greedy_winrate(lvl, seed, n, Some((gold, rew)));
                    let win = wins as f32 / n as f32;
                    if win >= tgt {
                        hi = s;
                        win_at_hi = win;
                    } else {
                        lo = s;
                    }
                }
                let s = hi;
                let gold = ((hp * s).round() as i32).max(orig_gold);
                let rew = (hp * s * 0.05).round().max(orig_rew);
                println!(
                    "{:>3}  {:<16} {:>5.0} {:>4}→{:<5} {:>4.0}→{:<5.0} {:>5.0}% {:>5.0}%",
                    lvl + 1,
                    name,
                    hp,
                    orig_gold,
                    gold,
                    orig_rew,
                    rew,
                    win_at_hi * 100.0,
                    tgt * 100.0
                );
                eprintln!(
                    "  level {} optimized → gold {} reward {:.0}",
                    lvl + 1,
                    gold,
                    rew
                );
            }
            println!("{}", "=".repeat(64));
            println!(
                "(recommended gold/reward to bake into levels(); win% = greedy at that economy.)\n"
            );
        }
        "iso" => {
            eprintln!(
                "[sim] ISOLATION sweep — level {} ({}), seed {} — {} kinds...",
                level,
                level_name,
                seed,
                TowerKind::ALL.len()
            );
            let mut rows: Vec<(TowerKind, KindStat, &'static str, i32, i32, i32)> = Vec::new();
            for kind in TowerKind::ALL {
                let r = run_sim(level, seed, RunMode::Sandbox(Some(kind)), None);
                let st = r.per_kind.get(&kind).copied().unwrap_or_default();
                rows.push((kind, st, r.outcome, r.wave, r.lives, r.total_waves));
            }
            rows.sort_by(|a, b| b.1.damage.partial_cmp(&a.1.damage).unwrap());
            println!(
                "\n========== PER-TOWER ISOLATION REPORT (level {level}, seed {seed}) =========="
            );
            println!(
                "{:<14} {:>4} {:>8} {:>12} {:>9} {:>9} {:>12}",
                "tower", "cnt", "cost", "damage", "dmg/gold", "outcome", "waves/lives"
            );
            println!("{}", "-".repeat(74));
            for (kind, s, outcome, wave, lives, total) in &rows {
                let dpg = if s.total_cost > 0 {
                    s.damage / s.total_cost as f64
                } else {
                    0.0
                };
                println!(
                    "{:<14} {:>4} {:>8} {:>12.0} {:>9.2} {:>9} {:>12}",
                    format!("{:?}", kind),
                    s.count,
                    s.total_cost,
                    s.damage,
                    dpg,
                    outcome,
                    format!("{}/{} L{}", wave, total, lives)
                );
            }
            println!("{}", "=".repeat(74));
            println!(
                "(standalone power: a board of ONLY that kind. DEFEAT/low-wave = too weak alone.)\n"
            );
        }
        "winrate" => {
            // 可选第 4 参数 = 局数（默认 20），如 `sim 99 12345 winrate 8`。
            let n: u64 = std::env::args()
                .nth(4)
                .and_then(|v| v.parse().ok())
                .unwrap_or(20);
            eprintln!(
                "[sim] WIN-RATE — greedy player, level {} ({}), {} seeds...",
                level, level_name, n
            );
            let (wins, timeouts, aw, al, tw, _, skill_casts, usage) =
                greedy_winrate(level, seed, n, None);
            let mut us: Vec<(TowerKind, u64)> = usage.into_iter().filter(|(_, c)| *c > 0).collect();
            us.sort_by(|a, b| b.1.cmp(&a.1));
            println!(
                "\n============== GREEDY WIN-RATE (level {level}: {level_name}) =============="
            );
            println!(
                "win-rate {}/{} = {:.0}%   timeouts {}   avg waves {:.1}/{}   avg lives {:.1}   avg skills {:.1}",
                wins,
                n,
                wins as f32 / n as f32 * 100.0,
                timeouts,
                aw,
                tw,
                al,
                skill_casts
            );
            println!("\ngreedy tower picks (total built across {n} runs):");
            for (k, c) in &us {
                println!("  {:<14} {}", format!("{:?}", k), c);
            }
            println!("{}", "=".repeat(56));
            println!("(smart greedy: spread builds + upgrades + reactive detection.)\n");
        }
        "builds" => {
            let n: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(8);
            let filter = args.next();
            let build_level = benchmark_hero_level(level);
            eprintln!(
                "[sim] HERO BUILDS — level {} ({}), hero Lv{}, {} seeds/profile...",
                level, level_name, build_level, n
            );
            if let Some(filter) = filter.as_deref() {
                eprintln!("[sim] filtering profile: {filter}");
            }
            println!(
                "\n============== HERO BUILD SWEEP (level {level}: {level_name}) =============="
            );
            println!(
                "{:<17} {:<10} {:>6}  {:>11}  {:>9}  {:>6}  {:>10}  {:>6}  {:>6}  {}",
                "profile",
                "weapon",
                "win%",
                "avg waves",
                "avg lives",
                "power",
                "hero dmg",
                "skills",
                "towers",
                "resonance / set"
            );
            println!("{}", "-".repeat(104));
            for profile in hero_build_profiles(build_level) {
                if filter
                    .as_deref()
                    .is_some_and(|wanted| wanted != profile.label())
                {
                    continue;
                }
                let (wins, timeouts, aw, al, tw, hero_damage, skill_casts, usage) =
                    greedy_winrate_with_hero(level, seed, n, None, profile);
                let weapon = profile.weapon();
                let gear = profile.gear();
                let stats = hero_gear::active_stats_for_weapon(&gear, weapon);
                let power = stats.damage_mult / stats.cooldown_mult.max(0.01);
                let average_towers = usage.values().copied().sum::<u64>() as f32 / n as f32;
                let resonance = hero_gear::weapon_resonance_summary(&gear, weapon)
                    .unwrap_or_else(|| "无".to_string());
                let gear_set = hero_gear::gear_set_summary(&gear).unwrap_or_default();
                println!(
                    "{:<17} {:<10} {:>5.0}%  {:>6.1}/{:<4}  {:>9.1}  {:>5.2}  {:>10.0}  {:>6.1}  {:>6.1}  {}{}{}",
                    profile.label(),
                    i18n::t(weapon.name()),
                    wins as f32 / n as f32 * 100.0,
                    aw,
                    tw,
                    al,
                    power,
                    hero_damage,
                    skill_casts,
                    average_towers,
                    resonance.trim(),
                    gear_set,
                    if timeouts > 0 { "  TIMEOUT" } else { "" }
                );
                eprintln!(
                    "  build {} done — {:.0}% win",
                    profile.label(),
                    wins as f32 / n as f32 * 100.0
                );
            }
            println!("{}", "=".repeat(104));
            println!(
                "(builds = campaign-expected Lv{} + rank-3 weapon talents + four matching hero gear pieces; greedy towers still play normally.)\n",
                build_level
            );
        }
        "all" => {
            let n: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(12);
            let count = levels().len();
            eprintln!("[sim] ALL-LEVELS difficulty sweep — {count} levels × {n} seeds...");
            println!("\n===== DIFFICULTY CURVE — greedy player, {n} seeds/level =====");
            println!(
                "{:>3}  {:<18} {:>6}  {:>11}  {:>9}  {:>4}",
                "lvl", "name", "win%", "avg waves", "avg lives", "TO"
            );
            println!("{}", "-".repeat(56));
            for lvl in 0..count {
                let name = i18n::t(levels()[lvl].name);
                let (wins, timeouts, aw, al, tw, _, _, _) = greedy_winrate(lvl, seed, n, None);
                let winpct = wins as f32 / n as f32 * 100.0;
                println!(
                    "{:>3}  {:<18} {:>5.0}%  {:>6.1}/{:<4}  {:>9.1}  {:>4}",
                    lvl + 1,
                    name,
                    winpct,
                    aw,
                    tw,
                    al,
                    timeouts
                );
                eprintln!("  level {} done — {:.0}% win", lvl + 1, winpct);
            }
            println!("{}", "=".repeat(56));
            println!("(low win% or low avg-lives = hard; 100% + high lives = easy/under-tuned.)\n");
        }
        "greedy" => {
            let r = run_sim(level, seed, RunMode::Greedy, None);
            print_share(
                "GREEDY ECONOMY RUN",
                level,
                &level_name,
                seed,
                &r,
                "(realistic economy: greedy player spends kill-gold. usage = what it chose to build.)",
            );
        }
        _ => {
            let r = run_sim(level, seed, RunMode::Sandbox(None), None);
            print_share(
                "TOWER DAMAGE-SHARE (mixed sandbox)",
                level,
                &level_name,
                seed,
                &r,
                "(mixed = kill-credit share; for unbiased per-tower power use `iso`, for economy use `greedy`/`winrate`.)",
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn campaign_build_benchmark_reaches_expected_progression_levels() {
        assert_eq!(benchmark_hero_level(0), 1);
        assert_eq!(benchmark_hero_level(59), 18);
        assert_eq!(benchmark_hero_level(79), 24);
        assert_eq!(benchmark_hero_level(99), hero::HeroLoadout::MAX_LEVEL);
    }

    #[test]
    fn hero_build_profiles_have_weapon_resonance() {
        let profiles = hero_build_profiles(18);
        assert_eq!(profiles.len(), 10);
        let mut labels = std::collections::HashSet::new();
        for profile in profiles {
            assert!(labels.insert(profile.label()));
            if matches!(profile, HeroScenario::Baseline | HeroScenario::Saved) {
                continue;
            }
            let weapon = profile.weapon();
            let gear = profile.gear();
            assert!(
                hero_gear::weapon_affinity_count(&gear, weapon) == 4,
                "{} should activate a full 4-piece weapon resonance",
                profile.label()
            );
            assert!(
                hero_gear::weapon_resonance_summary(&gear, weapon).is_some(),
                "{} should have a resonance summary",
                profile.label()
            );
            assert!(
                hero_gear::active_four_piece_set(&gear).is_some(),
                "{} should exercise a four-piece keystone",
                profile.label()
            );
        }
    }

    #[test]
    fn greedy_tower_plan_covers_core_roles() {
        let plan = greedy_tower_plan();
        let unique = plan
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>();
        assert!(unique.len() >= 12);
        assert!(unique.contains(&TowerKind::Cannon));
        assert!(unique.contains(&TowerKind::Ice));
        assert!(unique.contains(&TowerKind::Thunder));
        assert!(unique.contains(&TowerKind::Summon));
        assert!(unique.contains(&TowerKind::Prism));
        assert!(unique.contains(&TowerKind::Necromancer));
        assert!(unique.contains(&TowerKind::Fortress));
    }
}
