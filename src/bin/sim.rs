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

use protect_carrot::{
    audio, bestiary, build, components, data, enemy, equipment as equipment_inv, game, hero, i18n,
    meta, states, tower, ui, vfx, Levels,
};

use build::spawn_tower;
use data::{cell_center, levels, Behavior, TowerKind, UpgradeMul};
use game::{
    load_level, tick_auto_wave, tick_message, CurrentLevel, GameDifficulty, GameMode, Paused, Rng,
    RunState,
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
}

#[derive(Resource, Default)]
struct EnemyCount(usize);

/// When `Some(k)`, the board is filled with ONLY tower kind `k` (isolation mode).
#[derive(Resource)]
struct OnlyKind(Option<TowerKind>);

/// Sum each tower's cumulative effective damage (`Tower::damage_done`, which the
/// game maintains for both direct hits AND damage-over-time) by kind.
fn collect_damage(towers: Query<&Tower>, mut report: ResMut<Report>) {
    for s in report.per_kind.values_mut() {
        s.damage = 0.0;
    }
    for t in &towers {
        report.per_kind.entry(t.kind).or_default().damage += t.damage_done as f64;
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

/// Smarter greedy economy player. Every few frames it picks the single best
/// affordable action by **marginal weighted-DPS per gold**:
///   • BUILD a tower where it covers the most *under-covered* path (spreads the
///     line + naturally diversifies kinds), or
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
        let mut cs: Vec<(i32, i32)> =
            board.buildable.iter().copied().filter(|p| dist(p) <= 3).collect();
        cs.sort_by(|a, b| dist(a).cmp(&dist(b)).then(a.cmp(b)));
        *cells = cs;
    }

    // Occupancy from existing towers; detectors tracked for the invisible check.
    let mut occupied: std::collections::HashSet<(i32, i32)> = std::collections::HashSet::new();
    let mut detectors: Vec<(Vec2, f32)> = Vec::new();
    for (_, t) in towers.iter() {
        let fp = t.footprint.max(1);
        for dx in 0..fp {
            for dy in 0..fp {
                occupied.insert((t.col + dx, t.row + dy));
            }
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
            !detectors.iter().any(|(c, r)| c.distance(p) <= *r)
        }
    });
    if invis_uncovered {
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
                let n = board.path_world.iter().filter(|p| cen.distance(**p) <= r).count();
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

    // Kinds by value/gold, best first (cached).
    if order.is_empty() {
        let mut o: Vec<TowerKind> =
            TowerKind::ALL.iter().copied().filter(|k| k.def().behavior != Behavior::Detect).collect();
        o.sort_by(|a, b| tower_value(*b).partial_cmp(&tower_value(*a)).unwrap());
        *order = o;
    }

    // 1) Best BUILD: best value/gold kind at the free cell covering the most path
    //    (killbox/chokepoint), saturating the board as gold allows.
    let mut best_build: Option<(f64, (i32, i32), TowerKind, i32)> = None;
    for &kind in order.iter() {
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
            let n = board.path_world.iter().filter(|p| cen.distance(**p) <= r).count();
            if n > bestcov {
                bestcov = n;
                bestcell = Some((col, row));
            }
        }
        if let Some(cell) = bestcell {
            best_build = Some((tower_value(kind), cell, kind, d.cost));
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
        let score = (cur * up_gain) / uc as f64;
        if best_up.map(|(s, ..)| score > s).unwrap_or(true) {
            best_up = Some((score, e, uc));
        }
    }

    // 3) Execute the higher-value action.
    let upgrade_wins = match (best_build, best_up) {
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

struct SimResult {
    per_kind: HashMap<TowerKind, KindStat>,
    outcome: &'static str,
    wave: i32,
    total_waves: i32,
    lives: i32,
    frames: u32,
}

/// Build a fresh headless app, fill the board, run the level's waves to a
/// resolution (win/lose) or a frame cap, and return the collected stats.
fn run_sim(level: usize, seed: u64, mode: RunMode, econ: Option<(i32, f32)>) -> SimResult {
    let only = match mode {
        RunMode::Sandbox(k) => k,
        RunMode::Greedy => None,
    };
    let greedy = matches!(mode, RunMode::Greedy);
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(AssetPlugin {
        meta_check: bevy::asset::AssetMetaCheck::Never,
        ..default()
    });
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
        .insert_resource(OnlyKind(only))
        .init_resource::<Paused>()
        .init_resource::<GameMode>()
        .init_resource::<GameDifficulty>()
        .init_resource::<tower::Snapshot>()
        .init_resource::<hero::HeroLoadout>()
        .init_resource::<meta::Talents>()
        .init_resource::<meta::Abilities>()
        .init_resource::<equipment_inv::EquipmentInventory>()
        .init_resource::<bestiary::Bestiary>()
        .init_resource::<build::Selection>()
        .init_resource::<ui::Progress>()
        .init_resource::<vfx::ScreenShake>()
        .init_resource::<audio::AudioSettings>()
        .init_resource::<i18n::Language>()
        .init_resource::<Report>()
        .init_resource::<EnemyCount>();

    app.add_message::<Damage>()
        .add_message::<tower::Status>()
        .add_message::<tower::BuffTower>()
        .add_message::<tower::HealCarrot>()
        .add_message::<vfx::VfxEvent>()
        .add_message::<audio::SfxEvent>()
        .add_message::<tower::EnemyDied>();

    let assets = app.world().resource::<AssetServer>().clone();
    app.insert_resource(protect_carrot::sprites::build_sprites(&assets));
    app.world_mut()
        .run_system_once(protect_carrot::creatures::load_creatures)
        .expect("load_creatures");

    // The real per-frame simulation chain (same order as the game's Playing state),
    // minus rendering/gizmos/UI. Split into two ≤20 tuples (Bevy's tuple cap).
    app.add_systems(
        Update,
        (
            (
                tower::build_snapshot,
                hero::hero_doctrine,
                tower::update_towers,
                tower::update_projectiles,
                tower::update_shot_fx,
                tower::update_summons,
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
                tick_auto_wave,
                tick_message,
            )
                .chain(),
        )
            .chain(),
    );
    app.add_systems(
        Update,
        (game::update_carrot_seal, tower::compute_synergy, count_enemies),
    );
    if greedy {
        app.add_systems(Update, greedy_player);
    }

    // Optimizer hook: override this level's starting gold + kill reward at runtime
    // (before load_level reads them) so the Layer-3 search can try economies without
    // recompiling.
    if let Some((gold, reward)) = econ {
        let mut lv = app.world_mut().resource_mut::<Levels>();
        lv.0[level].gold = gold;
        lv.0[level].enemies.reward = reward;
    }
    app.world_mut().run_system_once(load_level).expect("load_level");
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

    let max_frames = 60 * 60 * 20; // 20 sim-minutes cap
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

    let run = app.world().resource::<RunState>();
    let report = app.world().resource::<Report>();
    SimResult {
        per_kind: report.per_kind.clone(),
        outcome,
        wave: run.wave,
        total_waves: run.total_waves,
        lives: run.lives,
        frames: frame,
    }
}

/// Run the greedy player over `n` seeds of a level. Returns
/// (wins, avg_waves, avg_lives, total_waves, tower-usage).
fn greedy_winrate(
    level: usize,
    base_seed: u64,
    n: u64,
    econ: Option<(i32, f32)>,
) -> (u32, f32, f32, i32, HashMap<TowerKind, u64>) {
    let mut wins = 0u32;
    let mut waves = 0i64;
    let mut lives = 0i64;
    let mut total_waves = 0i32;
    let mut usage: HashMap<TowerKind, u64> = HashMap::new();
    for s in 0..n {
        let r = run_sim(level, base_seed.wrapping_add(s), RunMode::Greedy, econ);
        if r.outcome == "VICTORY" {
            wins += 1;
        }
        waves += r.wave as i64;
        lives += r.lives.max(0) as i64;
        total_waves = r.total_waves;
        for (k, st) in &r.per_kind {
            *usage.entry(*k).or_default() += st.count as u64;
        }
    }
    (
        wins,
        waves as f32 / n as f32,
        lives as f32 / n as f32,
        total_waves,
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
        "level {} ({})  seed {}  {}  waves {}/{}  lives {}  gold-spent {}  sim {:.1}s",
        level,
        level_name,
        seed,
        r.outcome,
        r.wave,
        r.total_waves,
        r.lives,
        total_spent,
        r.frames as f32 / 60.0
    );
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
    let seed: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(0x1234_5678);
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
                eprintln!("  level {} optimized → gold {} reward {:.0}", lvl + 1, gold, rew);
            }
            println!("{}", "=".repeat(64));
            println!("(recommended gold/reward to bake into levels(); win% = greedy at that economy.)\n");
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
            println!("(standalone power: a board of ONLY that kind. DEFEAT/low-wave = too weak alone.)\n");
        }
        "winrate" => {
            let n = 20u64;
            eprintln!(
                "[sim] WIN-RATE — greedy player, level {} ({}), {} seeds...",
                level, level_name, n
            );
            let (wins, aw, al, tw, usage) = greedy_winrate(level, seed, n, None);
            let mut us: Vec<(TowerKind, u64)> = usage.into_iter().filter(|(_, c)| *c > 0).collect();
            us.sort_by(|a, b| b.1.cmp(&a.1));
            println!("\n============== GREEDY WIN-RATE (level {level}: {level_name}) ==============");
            println!(
                "win-rate {}/{} = {:.0}%   avg waves {:.1}/{}   avg lives {:.1}",
                wins,
                n,
                wins as f32 / n as f32 * 100.0,
                aw,
                tw,
                al
            );
            println!("\ngreedy tower picks (total built across {n} runs):");
            for (k, c) in &us {
                println!("  {:<14} {}", format!("{:?}", k), c);
            }
            println!("{}", "=".repeat(56));
            println!("(smart greedy: spread builds + upgrades + reactive detection.)\n");
        }
        "all" => {
            let n: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(12);
            let count = levels().len();
            eprintln!("[sim] ALL-LEVELS difficulty sweep — {count} levels × {n} seeds...");
            println!("\n===== DIFFICULTY CURVE — greedy player, {n} seeds/level =====");
            println!(
                "{:>3}  {:<18} {:>6}  {:>11}  {:>9}",
                "lvl", "name", "win%", "avg waves", "avg lives"
            );
            println!("{}", "-".repeat(56));
            for lvl in 0..count {
                let name = i18n::t(levels()[lvl].name);
                let (wins, aw, al, tw, _) = greedy_winrate(lvl, seed, n, None);
                let winpct = wins as f32 / n as f32 * 100.0;
                println!(
                    "{:>3}  {:<18} {:>5.0}%  {:>6.1}/{:<4}  {:>9.1}",
                    lvl + 1,
                    name,
                    winpct,
                    aw,
                    tw,
                    al
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
