//! Towers, projectiles, summons, and the combat resolution pipeline.
//!
//! Design (idiomatic Bevy): rather than mutating enemies in-place from many
//! places (as the JS does), towers/projectiles/summons **emit events**
//! (`Damage`, `Status`) that dedicated systems apply. This keeps each system's
//! data access simple and is a good ECS pattern to learn.
//!
//! Flow each frame:
//!   build_snapshot -> update_towers -> update_projectiles -> update_summons
//!   -> apply_status -> apply_damage   (then enemy::update_enemies handles death)

use crate::board::Board;
use crate::components::{Enemy, LevelEntity, Particle, SummonHpBarFg};
use crate::data::{
    cell_center, Behavior, Category, Element, TowerDef, TowerKind, MOSS_TOWER_SENSE, TILE_SIZE,
    TOWER_RAIDER_ENGAGE, TOWER_RAIDER_SENSE,
};
use crate::equipment::{
    equipment_set_bonus, return_equipment_to_inventory, Equipment, EquipmentInventory,
    EquipmentVisual, Rarity,
};
use crate::game::RunState;
use bevy::prelude::*;
use bevy::sprite::Anchor;
use std::collections::{HashMap, HashSet};

// ============================ Components ============================

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TargetPriority {
    Nearest,
    Front,
    Strongest,
    Weakest,
    Threat,
}

impl TargetPriority {
    pub fn label(self) -> &'static str {
        match self {
            TargetPriority::Nearest => "近身",
            TargetPriority::Front => "前锋",
            TargetPriority::Strongest => "强者",
            TargetPriority::Weakest => "残血",
            TargetPriority::Threat => "威胁",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            TargetPriority::Nearest => "优先攻击离塔最近的敌人",
            TargetPriority::Front => "优先攻击走得最远的敌人",
            TargetPriority::Strongest => "优先攻击当前生命最高的敌人",
            TargetPriority::Weakest => "优先补刀当前生命最低的敌人",
            TargetPriority::Threat => "优先攻击首领、MOSS、攻城怪",
        }
    }

    pub fn next(self) -> Self {
        match self {
            TargetPriority::Nearest => TargetPriority::Front,
            TargetPriority::Front => TargetPriority::Strongest,
            TargetPriority::Strongest => TargetPriority::Weakest,
            TargetPriority::Weakest => TargetPriority::Threat,
            TargetPriority::Threat => TargetPriority::Nearest,
        }
    }
}

/// A placed tower. Stats are copied from the `TowerDef` at build time so upgrades
/// can mutate them per-instance. Durations are stored in **seconds**.
#[derive(Component)]
pub struct Tower {
    pub kind: TowerKind,
    /// Top-left (min) cell of the tower's footprint.
    pub col: i32,
    pub row: i32,
    /// Footprint side in cells (1 or 2).
    pub footprint: i32,
    pub level: i32,
    pub base_cost: i32,
    /// Equipped relics (max 3).
    pub equipment: [Option<Equipment>; 3],
    pub max_hp: f32,
    pub hp: f32,
    pub armor: f32,
    pub armor_pierce: f32,
    /// Throttles visual/audio feedback when tower-raiders continuously chew this tower.
    pub siege_vfx_timer: f32,
    pub low_hp_warned: bool,
    pub element: Element,
    /// Base damage (from def, talents, upgrades). `damage` is the effective value
    /// after adjacency synergy is applied each frame by `compute_synergy`.
    pub base_damage: f32,
    pub damage: f32,
    /// Per-run contribution stats shown in the selected-tower HUD.
    pub damage_done: f32,
    pub kills: i32,
    /// Current adjacency synergy bonus fraction (0.0 = none), for the HUD.
    pub synergy: f32,
    /// Transient per-frame buffs projected by a nearby hero's doctrine aura
    /// (see `hero::hero_doctrine`). `aura_damage` folds into the `compute_synergy`
    /// damage formula; `aura_haste` speeds the cooldown tick. Reset to 0 each frame
    /// when no hero is in range, so they never permanently compound.
    pub aura_damage: f32,
    pub aura_haste: f32,
    /// +range fraction from a nearby Warden hero's 戍卫结界 doctrine (applied at
    /// targeting time in `Snapshot::target`). Reset to 0 each frame when out of aura.
    pub aura_range: f32,
    pub range: f32,
    pub cooldown: f32,
    pub cooldown_timer: f32,
    pub target_priority: TargetPriority,
    pub magic: bool,
    pub behavior: Behavior,
    pub color: Color,
    pub angle: f32,
    /// Transient muzzle-recoil offset (world px); set on fire, decays in
    /// `rotate_towers` so the sprite kicks back when it shoots.
    pub recoil: Vec2,
    /// True for the unique movable hero tower (see `hero.rs`).
    pub hero: bool,
    /// Reveals invisible enemies in range (detection towers, and the Warden hero's
    /// built-in 反隐形). Used to build `Snapshot::detectors`.
    pub detector: bool,
    /// Hero's free-floating world position (ignored for grid towers).
    pub hero_pos: Vec2,
    /// Where the hero is walking toward, if commanded (hero only).
    pub move_target: Option<Vec2>,
    /// Laser focus ramp: seconds the beam has dwelled on its current target, and
    /// which target that is. DPS grows exponentially with dwell (boss-shredder).
    pub laser_charge: f32,
    pub laser_target: Option<Entity>,
    // behavior params
    pub aoe_radius: f32,
    pub chain_count: i32,
    pub chain_range: f32,
    pub slow_duration: f32,
    pub knock_dist: f32,
    pub stun_duration: f32,
    pub freeze_duration: f32,
    pub armor_reduce: f32,
    pub curse_duration: f32,
    pub heal_amount: f32,
    pub buff_range: f32,
    pub dot_damage: f32,
    pub poison_duration: f32,
    pub fire_duration: f32,
    pub summon_hp: f32,
    pub summon_speed: f32,
    pub max_summons: i32,
}

impl Tower {
    pub fn from_def(def: &TowerDef, col: i32, row: i32) -> Self {
        Tower {
            kind: def.kind,
            detector: def.behavior == Behavior::Detect,
            col,
            row,
            footprint: def.footprint.max(1),
            level: 1,
            base_cost: def.cost,
            equipment: [None, None, None],
            max_hp: def.max_hp,
            hp: def.max_hp,
            armor: def.armor,
            armor_pierce: 0.0,
            siege_vfx_timer: 0.0,
            low_hp_warned: false,
            element: def.element,
            base_damage: def.damage,
            damage: def.damage,
            damage_done: 0.0,
            kills: 0,
            synergy: 0.0,
            aura_damage: 0.0,
            aura_haste: 0.0,
            aura_range: 0.0,
            range: def.range,
            cooldown: def.cooldown_ms / 1000.0,
            cooldown_timer: 0.0,
            target_priority: TargetPriority::Nearest,
            magic: def.magic,
            behavior: def.behavior,
            color: def.color,
            angle: 0.0,
            recoil: Vec2::ZERO,
            hero: false,
            hero_pos: Vec2::ZERO,
            move_target: None,
            laser_charge: 0.0,
            laser_target: None,
            aoe_radius: def.aoe_radius,
            chain_count: def.chain_count,
            chain_range: def.chain_range,
            slow_duration: def.slow_duration / 1000.0,
            knock_dist: def.knock_dist,
            stun_duration: def.stun_duration / 1000.0,
            freeze_duration: def.freeze_duration / 1000.0,
            armor_reduce: def.armor_reduce,
            curse_duration: def.curse_duration / 1000.0,
            heal_amount: def.heal_amount,
            buff_range: def.buff_range,
            dot_damage: def.dot_damage,
            poison_duration: def.poison_duration / 1000.0,
            fire_duration: def.fire_duration / 1000.0,
            summon_hp: def.summon_hp,
            summon_speed: def.summon_speed,
            max_summons: def.max_summons,
        }
    }

    /// World center of the (possibly multi-cell) footprint. The hero floats freely,
    /// so it returns its live `hero_pos` instead of a grid cell.
    pub fn center(&self) -> Vec2 {
        if self.hero {
            return self.hero_pos;
        }
        let off = (self.footprint - 1) as f32 / 2.0;
        cell_center(self.col as f32 + off, self.row as f32 + off)
    }

    /// Does this tower's footprint cover grid cell `(col,row)`?
    pub fn covers(&self, col: i32, row: i32) -> bool {
        col >= self.col
            && col < self.col + self.footprint
            && row >= self.row
            && row < self.row + self.footprint
    }

    /// Cost to upgrade to the next level (original formula).
    pub fn upgrade_cost(&self) -> i32 {
        (self.base_cost as f32 * 1.6f32.powi(self.level - 1) * 0.7).floor() as i32
    }

    /// Refund when selling (original: 60% of scaled cost).
    pub fn refund(&self) -> i32 {
        (self.base_cost as f32 * 1.6f32.powi(self.level - 1) * 0.6).floor() as i32
    }

    pub fn repair_cost(&self) -> i32 {
        let missing = (self.max_hp - self.hp).max(0.0);
        if missing <= 0.5 {
            0
        } else {
            ((missing * 0.35).ceil() as i32).max(5)
        }
    }

    pub fn equipment_count(&self) -> usize {
        self.equipment.iter().filter(|slot| slot.is_some()).count()
    }

    pub fn cycle_target_priority(&mut self) -> TargetPriority {
        self.target_priority = self.target_priority.next();
        self.target_priority
    }

    /// Can this tower's attack reach flying enemies? (original `canHitFlying`).
    pub fn can_hit_flying(&self) -> bool {
        matches!(
            self.kind,
            TowerKind::Arrow
                | TowerKind::Magic
                | TowerKind::Sniper
                | TowerKind::Thunder
                | TowerKind::Laser
                | TowerKind::Missile
                | TowerKind::Poison
                | TowerKind::Fire
                | TowerKind::Holy
                | TowerKind::Detection
        )
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ProjKind {
    Normal,
    Missile,
    Slow,
    Poison,
    Curse,
    Knockback,
}

/// A flying projectile homing on a target entity.
#[derive(Component)]
pub struct Projectile {
    pub source_tower: Option<Entity>,
    pub target: Entity,
    pub speed: f32, // px/sec
    pub damage: f32,
    pub magic: bool,
    pub element: Element,
    pub armor_pierce: f32,
    pub kind: ProjKind,
    pub aoe_radius: f32,
    pub slow_duration: f32,
    pub dot_damage: f32,
    pub poison_duration: f32,
    pub armor_reduce: f32,
    pub curse_duration: f32,
    pub knock_dist: f32,
    pub stun_duration: f32,
}

#[derive(Component)]
pub struct ProjectileVisual {
    pub tower_kind: TowerKind,
    pub trail_timer: f32,
}

#[derive(Component)]
pub struct ShotFx {
    pub life: f32,
    pub max_life: f32,
    pub alpha: f32,
    pub shrink_x: bool,
}

/// A friendly unit (summon-tower skeleton, or a necromancer-raised enemy) that
/// walks to the nearest enemy and fights it. Enemies fight back (`enemy_vs_ally`).
#[derive(Component)]
pub struct Summon {
    pub hp: f32,
    pub max_hp: f32,
    pub damage: f32,
    pub speed: f32, // px/sec
    pub target: Option<Entity>,
    pub attack_timer: f32,
    pub owner: Entity,
    /// Visual = this creature's animated sprite (blue-tinted).
    pub kind: crate::data::EnemyKind,
    /// Seconds before the unit crumbles (f32::INFINITY = permanent skeletons).
    pub lifetime: f32,
    /// Transient +damage fraction from the Priest's 圣疗领域 doctrine (召唤物联动),
    /// refreshed each frame by `hero::hero_doctrine`. 0 when no Priest hero is alive.
    pub buff: f32,
}

/// Minion archetype a summon tower conjures at a given level. Stronger tiers
/// unlock as the tower upgrades (all three have creature sprite sheets).
pub fn summon_minion_kind(level: i32) -> crate::data::EnemyKind {
    use crate::data::EnemyKind::*;
    match level {
        0 | 1 => Armored, // skeleton
        2 => Charger,     // fireworm — faster, aggressive
        _ => Tank,        // mimic — beefy bruiser
    }
}

/// The Engineer's level-30 ultimate: a single summoned 神之塔 (god tower) that fuses
/// every tower attribute. Marker so only one is ever spawned per run.
#[derive(Component)]
pub struct GodTower;

/// Emitted when an enemy dies, so necromancer towers can raise it.
#[derive(Message)]
pub struct EnemyDied {
    pub pos: Vec2,
    pub kind: crate::data::EnemyKind,
    pub max_hp: f32,
}

// ============================ Events ============================

#[derive(Message)]
pub struct Damage {
    pub source_tower: Option<Entity>,
    pub target: Entity,
    pub amount: f32,
    pub magic: bool,
    pub element: Element,
    pub armor_pierce: f32,
}

#[derive(Clone, Copy)]
pub enum StatusKind {
    Slow {
        duration: f32,
    },
    Freeze {
        duration: f32,
    },
    Poison {
        dmg: f32,
        duration: f32,
    },
    Fire {
        dmg: f32,
        duration: f32,
        element: Element,
    },
    Curse {
        reduce: f32,
        duration: f32,
    },
    Knockback {
        dist: f32,
        stun: f32,
    },
}

#[derive(Message)]
pub struct Status {
    pub source_tower: Option<Entity>,
    pub target: Entity,
    pub kind: StatusKind,
}

#[derive(Message)]
pub struct BuffTower {
    pub target: Entity,
}

// ============================ Snapshot ============================

#[derive(Clone, Copy)]
pub struct EnemySnap {
    pub entity: Entity,
    pub pos: Vec2,
    pub hp: f32,
    pub max_hp: f32,
    pub path_index: usize,
    pub flying: bool,
    pub invisible: bool,
    pub boss: bool,
    pub tower_raider: bool,
    pub moss_destroy: bool,
    pub facing: Vec2,
}

/// Read-only view of the battlefield, rebuilt each frame so the mutating combat
/// systems can make targeting decisions without aliasing the enemy/tower queries.
#[derive(Resource, Default)]
pub struct Snapshot {
    pub enemies: Vec<EnemySnap>,
    pub detectors: Vec<(Vec2, f32)>,
    pub silencers: Vec<(Vec2, f32)>,
    pub summon_counts: HashMap<Entity, usize>,
}

pub fn build_snapshot(
    mut snap: ResMut<Snapshot>,
    enemies: Query<(Entity, &Enemy, &Transform)>,
    towers: Query<(&Tower, &Transform)>,
    summons: Query<&Summon>,
) {
    snap.enemies.clear();
    snap.silencers.clear();
    for (e, enemy, tf) in &enemies {
        let pos = tf.translation.truncate();
        snap.enemies.push(EnemySnap {
            entity: e,
            pos,
            hp: enemy.hp,
            max_hp: enemy.max_hp,
            path_index: enemy.path_index,
            flying: enemy.flying,
            invisible: enemy.invisible,
            boss: enemy.boss,
            tower_raider: enemy.tower_raider,
            moss_destroy: enemy.moss_destroy,
            facing: enemy.facing,
        });
        if enemy.silence_aura > 0.0 {
            snap.silencers.push((pos, enemy.silence_aura));
        }
    }
    snap.detectors.clear();
    for (t, _) in &towers {
        if t.detector {
            snap.detectors.push((t.center(), t.range));
        }
    }
    snap.summon_counts.clear();
    for s in &summons {
        *snap.summon_counts.entry(s.owner).or_insert(0) += 1;
    }
}

impl Snapshot {
    fn is_detected(&self, e: &EnemySnap) -> bool {
        if !e.invisible {
            return true;
        }
        self.detectors
            .iter()
            .any(|(pos, range)| pos.distance(e.pos) <= *range)
    }

    fn can_target(&self, tower: &Tower, e: &EnemySnap) -> bool {
        if e.flying && !tower.can_hit_flying() {
            return false;
        }
        if e.invisible && !self.is_detected(e) {
            return false;
        }
        true
    }

    pub fn tower_silenced(&self, pos: Vec2) -> bool {
        self.silencers
            .iter()
            .any(|(spos, r)| pos.distance(*spos) <= *r)
    }

    fn threat_rank(e: &EnemySnap) -> i32 {
        if e.moss_destroy {
            4
        } else if e.boss {
            3
        } else if e.tower_raider {
            2
        } else {
            0
        }
    }

    fn hp_frac(e: &EnemySnap) -> f32 {
        if e.max_hp > 0.0 {
            (e.hp / e.max_hp).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    fn better_target(
        &self,
        tower: &Tower,
        c: Vec2,
        candidate: &EnemySnap,
        best: &EnemySnap,
    ) -> bool {
        let cd = c.distance(candidate.pos);
        let bd = c.distance(best.pos);
        match tower.target_priority {
            TargetPriority::Nearest => cd < bd,
            TargetPriority::Front => {
                candidate.path_index > best.path_index
                    || (candidate.path_index == best.path_index && cd < bd)
            }
            TargetPriority::Strongest => {
                candidate.hp > best.hp || (candidate.hp == best.hp && cd < bd)
            }
            TargetPriority::Weakest => {
                let cf = Self::hp_frac(candidate);
                let bf = Self::hp_frac(best);
                cf < bf || (cf == bf && cd < bd)
            }
            TargetPriority::Threat => {
                let cr = Self::threat_rank(candidate);
                let br = Self::threat_rank(best);
                cr > br
                    || (cr == br
                        && (candidate.path_index > best.path_index
                            || (candidate.path_index == best.path_index && cd < bd)))
            }
        }
    }

    /// Targetable enemy within range, selected by this tower's priority mode.
    fn target(&self, tower: &Tower) -> Option<EnemySnap> {
        let c = tower.center();
        let effective_range = tower.range * (1.0 + tower.aura_range);
        let mut best: Option<EnemySnap> = None;
        for e in &self.enemies {
            if !self.can_target(tower, e) {
                continue;
            }
            let d = c.distance(e.pos);
            if d > effective_range {
                continue;
            }
            if best
                .as_ref()
                .map(|current| self.better_target(tower, c, e, current))
                .unwrap_or(true)
            {
                best = Some(*e);
            }
        }
        best
    }
}

// ============================ Helpers ============================

/// Distance from point `p` to the segment `a`-`b` (original `lineDistToPoint`).
fn seg_dist(a: Vec2, b: Vec2, p: Vec2) -> f32 {
    let ab = b - a;
    let len_sq = ab.length_squared();
    let t = if len_sq == 0.0 {
        0.0
    } else {
        ((p - a).dot(ab) / len_sq).clamp(0.0, 1.0)
    };
    p.distance(a + ab * t)
}

/// Projectile pixel speed: JS moved `speed*gameSpeed` per ~16ms frame.
fn proj_px_s(js_speed: f32) -> f32 {
    js_speed * 1000.0 / 16.0
}

fn fire_wall_angle(target: EnemySnap, board: &Board, fallback_from: Vec2) -> f32 {
    let path = &board.path_world;
    let idx = target.path_index.min(path.len().saturating_sub(1));
    let path_dir = if path.len() >= 2 {
        if idx + 1 < path.len() {
            path[idx + 1] - path[idx]
        } else if idx > 0 {
            path[idx] - path[idx - 1]
        } else {
            target.pos - fallback_from
        }
    } else {
        target.pos - fallback_from
    };
    if path_dir.length_squared() > 0.0 {
        path_dir.to_angle() + std::f32::consts::FRAC_PI_2
    } else {
        0.0
    }
}

fn point_in_fire_wall(
    point: Vec2,
    center: Vec2,
    angle: f32,
    half_len: f32,
    half_width: f32,
) -> bool {
    let rel = point - center;
    let dir = Vec2::from_angle(angle);
    let normal = Vec2::new(-dir.y, dir.x);
    rel.dot(dir).abs() <= half_len && rel.dot(normal).abs() <= half_width
}

fn projectile_tint(kind: TowerKind, proj: &Projectile, tower_color: Color) -> Color {
    use TowerKind::*;
    match kind {
        Arrow => Color::srgb(1.0, 0.82, 0.42),
        Sniper => Color::srgb(0.56, 1.0, 0.58),
        Magic => Color::srgb(0.82, 0.52, 1.0),
        Missile | Fortress | Cannon => Color::srgb(1.0, 0.46, 0.16),
        Ice | FrostNova => Color::srgb(0.52, 0.9, 1.0),
        Wind => Color::srgb(0.36, 1.0, 0.92),
        Shadow | Necromancer => Color::srgb(0.58, 0.42, 0.95),
        Poison => Color::srgb(0.42, 1.0, 0.48),
        Fire => Color::srgb(1.0, 0.34, 0.12),
        Laser | Prism | Holy | Detection | Summon | Thunder => proj.element.color(),
    }
    .mix(&tower_color, 0.18)
    .mix(&Color::WHITE, 0.16)
}

fn projectile_radius(kind: TowerKind, proj: &Projectile) -> f32 {
    use TowerKind::*;
    match (kind, proj.kind) {
        (Missile, _) => 5.8,
        (_, ProjKind::Missile) => 5.2,
        (Magic, _) => 5.0,
        (Sniper, _) => 2.8,
        (Arrow, _) => 3.2,
        (_, ProjKind::Curse) => 5.0,
        (_, ProjKind::Poison) => 4.5,
        (_, ProjKind::Slow) => 4.5,
        (_, ProjKind::Knockback) => 4.0,
        _ => 3.6,
    }
}

fn projectile_tail(kind: TowerKind, proj: &Projectile) -> (f32, f32, f32) {
    use TowerKind::*;
    match (kind, proj.kind) {
        (Missile, _) | (_, ProjKind::Missile) => (34.0, 9.0, 0.34),
        (Sniper, _) => (42.0, 3.0, 0.42),
        (Arrow, _) => (25.0, 4.0, 0.30),
        (Magic, _) => (18.0, 10.0, 0.24),
        (_, ProjKind::Curse) => (22.0, 9.0, 0.26),
        (_, ProjKind::Poison) => (20.0, 8.0, 0.28),
        (_, ProjKind::Slow) => (22.0, 8.0, 0.28),
        (_, ProjKind::Knockback) => (26.0, 7.0, 0.30),
        _ => (18.0, 5.0, 0.24),
    }
}

fn spawn_beam(
    commands: &mut Commands,
    from: Vec2,
    to: Vec2,
    color: Color,
    width: f32,
    alpha: f32,
    life: f32,
    z: f32,
    shrink_x: bool,
) {
    let delta = to - from;
    let len = delta.length();
    if len <= 1.0 {
        return;
    }
    commands.spawn((
        Sprite {
            color: color.with_alpha(alpha),
            custom_size: Some(Vec2::new(len, width.max(1.0))),
            ..default()
        },
        Transform::from_translation(((from + to) * 0.5).extend(z))
            .with_rotation(Quat::from_rotation_z(delta.to_angle())),
        ShotFx {
            life,
            max_life: life,
            alpha,
            shrink_x,
        },
        LevelEntity,
    ));
}

fn spawn_layered_beam(
    commands: &mut Commands,
    from: Vec2,
    to: Vec2,
    color: Color,
    width: f32,
    life: f32,
    z: f32,
    shrink_x: bool,
) {
    spawn_beam(
        commands,
        from,
        to,
        color,
        width * 3.0,
        0.14,
        life,
        z - 0.1,
        shrink_x,
    );
    spawn_beam(
        commands,
        from,
        to,
        color.mix(&Color::WHITE, 0.48),
        width,
        0.78,
        life * 0.82,
        z,
        shrink_x,
    );
}

fn spawn_projectile_trail(
    commands: &mut Commands,
    kind: TowerKind,
    proj: &Projectile,
    from: Vec2,
    to: Vec2,
) {
    let tint = projectile_tint(kind, proj, proj.element.color());
    let (_, width, alpha) = projectile_tail(kind, proj);
    spawn_beam(
        commands,
        from,
        to,
        tint,
        width * 0.72,
        alpha,
        0.18,
        7.5,
        false,
    );
    // A fading glow bead at the head turns the streak into a comet.
    commands.spawn((
        Sprite {
            color: tint.mix(&Color::WHITE, 0.25),
            custom_size: Some(Vec2::splat(width * 1.05)),
            ..default()
        },
        Transform::from_translation(to.extend(7.4)),
        Particle {
            vel: Vec2::ZERO,
            life: 0.16,
            max_life: 0.16,
        },
        LevelEntity,
    ));
}

fn spawn_projectile(
    commands: &mut Commands,
    from: Vec2,
    to: Vec2,
    tower_kind: TowerKind,
    proj: Projectile,
    color: Color,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
) {
    let tint = projectile_tint(tower_kind, &proj, color);
    let radius = projectile_radius(tower_kind, &proj);
    let (tail_len, tail_w, tail_alpha) = projectile_tail(tower_kind, &proj);
    let delta = to - from;
    let angle = if delta.length_squared() > 0.0 {
        delta.to_angle()
    } else {
        0.0
    };
    let core = tint.mix(&Color::WHITE, 0.22);
    let glow = tint;
    let hot = Color::WHITE.mix(&tint, 0.28);
    let missile_like = tower_kind == TowerKind::Missile || proj.kind == ProjKind::Missile;
    commands
        .spawn((
            proj,
            ProjectileVisual {
                tower_kind,
                trail_timer: 0.0,
            },
            Mesh2d(meshes.add(Circle::new(radius))),
            MeshMaterial2d(materials.add(core)),
            Transform::from_translation(from.extend(8.0))
                .with_rotation(Quat::from_rotation_z(angle)),
            LevelEntity,
        ))
        .with_children(|p| {
            p.spawn((
                Sprite {
                    color: glow.with_alpha(tail_alpha),
                    custom_size: Some(Vec2::new(tail_len, tail_w)),
                    ..default()
                },
                Transform::from_xyz(-tail_len * 0.58, 0.0, -0.3),
            ));
            p.spawn((
                Sprite {
                    color: hot.with_alpha(0.62),
                    custom_size: Some(Vec2::new(tail_len * 0.42, tail_w * 0.38)),
                    ..default()
                },
                Transform::from_xyz(-tail_len * 0.28, 0.0, -0.2),
            ));
            if missile_like {
                p.spawn((
                    Sprite {
                        color: Color::srgb(1.0, 0.24, 0.06).with_alpha(0.72),
                        custom_size: Some(Vec2::new(12.0, 7.0)),
                        ..default()
                    },
                    Transform::from_xyz(-radius - 7.0, 0.0, -0.1),
                ));
            } else if matches!(
                tower_kind,
                TowerKind::Arrow | TowerKind::Sniper | TowerKind::Wind
            ) {
                p.spawn((
                    Sprite {
                        color: hot.with_alpha(0.86),
                        custom_size: Some(Vec2::new(radius * 5.2, radius * 0.95)),
                        ..default()
                    },
                    Transform::from_xyz(radius * 0.5, 0.0, 0.1),
                ));
            }
        });
}

// ============================ Tower behavior ============================

pub fn update_towers(
    mut commands: Commands,
    time: Res<Time>,
    snap: Res<Snapshot>,
    run: Res<RunState>,
    board: Res<Board>,
    mut towers: Query<(Entity, &mut Tower, &Transform)>,
    mut dmg: MessageWriter<Damage>,
    mut status: MessageWriter<Status>,
    mut buff: MessageWriter<BuffTower>,
    mut heal: MessageWriter<HealCarrot>,
    mut vfx: MessageWriter<crate::vfx::VfxEvent>,
    creatures: Res<crate::creatures::Creatures>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let dt = time.delta_secs() * run.game_speed;

    // Precompute tower entity+pos for the holy buff (read from snapshot-free list).
    let buff_positions: Vec<(Entity, Vec2)> =
        towers.iter().map(|(e, t, _)| (e, t.center())).collect();

    for (entity, mut tower, _) in &mut towers {
        if tower.cooldown_timer > 0.0 {
            // Engineer/Warden doctrine haste speeds the cooldown tick (= faster attacks).
            tower.cooldown_timer -= dt * (1.0 + tower.aura_haste);
        }
        let c = tower.center();
        if snap.tower_silenced(c) {
            continue;
        }

        match tower.behavior {
            // Passive towers: cooldown already ticked above; no direct attack.
            Behavior::Detect | Behavior::Necromancer => continue,

            Behavior::Heal => {
                if tower.cooldown_timer <= 0.0 {
                    tower.cooldown_timer = tower.cooldown;
                    heal.write(HealCarrot {
                        amount: tower.heal_amount as i32,
                    });
                    vfx.write(crate::vfx::VfxEvent::Heal { pos: c });
                    for (other, pos) in &buff_positions {
                        if *other != entity && pos.distance(c) <= tower.buff_range {
                            buff.write(BuffTower { target: *other });
                            spawn_layered_beam(
                                &mut commands,
                                c,
                                *pos,
                                tower.element.color(),
                                3.0,
                                0.28,
                                9.0,
                                true,
                            );
                        }
                    }
                }
                continue;
            }

            Behavior::Laser => {
                if let Some(t) = snap.target(&tower) {
                    tower.angle = (t.pos - c).to_angle();
                    // The Laser tower is the anti-boss weapon: dwelling on one target
                    // ramps its DPS *exponentially* (doubling ~every 1.2s focus) up to a
                    // hard 1000-dps cap. Switching target resets the charge, so it only
                    // pays off against something that stays in the beam (i.e. bosses).
                    let is_laser = tower.kind == TowerKind::Laser;
                    let dps = if is_laser {
                        if tower.laser_target == Some(t.entity) {
                            tower.laser_charge += dt;
                        } else {
                            tower.laser_target = Some(t.entity);
                            tower.laser_charge = 0.0;
                        }
                        (tower.damage * 2f32.powf(tower.laser_charge / 1.2)).min(1000.0)
                    } else {
                        tower.damage
                    };
                    let charge_frac = if is_laser {
                        (dps / 1000.0).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    let base_width = if tower.kind == TowerKind::Prism { 8.5 } else { 5.5 };
                    let width = base_width + charge_frac * 7.0; // thickens as it charges
                    spawn_layered_beam(
                        &mut commands,
                        c,
                        t.pos,
                        tower
                            .element
                            .color()
                            .mix(&tower.color, 0.22)
                            .mix(&Color::WHITE, charge_frac * 0.5),
                        width,
                        0.075,
                        11.5,
                        false,
                    );
                    let dmg_amt = dps * dt; // continuous: dps * sec
                    for e in &snap.enemies {
                        if !snap.can_target(&tower, e) {
                            continue;
                        }
                        if seg_dist(c, t.pos, e.pos) <= 15.0 {
                            dmg.write(Damage {
                                source_tower: Some(entity),
                                target: e.entity,
                                amount: dmg_amt,
                                magic: true,
                                element: tower.element,
                                armor_pierce: tower.armor_pierce,
                            });
                        }
                    }
                } else {
                    // Beam off — reset the focus ramp so it must build up again.
                    tower.laser_target = None;
                    tower.laser_charge = 0.0;
                }
                continue;
            }

            Behavior::Summon => {
                if tower.cooldown_timer <= 0.0 {
                    let count = snap.summon_counts.get(&entity).copied().unwrap_or(0);
                    if (count as i32) < tower.max_summons {
                        tower.cooldown_timer = tower.cooldown;
                        // The minion tier and its damage scale with the tower level
                        // (skeleton → charger → mimic-bruiser).
                        let minion = summon_minion_kind(tower.level);
                        let dmg_mult = 1.0 + 0.4 * (tower.level - 1).max(0) as f32;
                        spawn_ally(
                            &mut commands,
                            &creatures,
                            minion,
                            c,
                            tower.summon_hp,
                            (tower.damage * dmg_mult).max(20.0),
                            tower.summon_speed * TILE_SIZE / 60.0 * (1000.0 / 16.0),
                            f32::INFINITY,
                            entity,
                        );
                        vfx.write(crate::vfx::VfxEvent::ElementPulse {
                            pos: c,
                            color: tower.element.color(),
                            strong: false,
                        });
                    }
                }
                continue;
            }
            _ => {}
        }

        // Standard targeted attacks.
        let Some(target) = snap.target(&tower) else {
            continue;
        };
        tower.angle = (target.pos - c).to_angle();
        if tower.cooldown_timer > 0.0 {
            continue;
        }
        tower.cooldown_timer = tower.cooldown;

        // Attack flourish (skip non-attacking support behaviors).
        if !matches!(tower.behavior, Behavior::Heal | Behavior::Detect) {
            let dir = Vec2::from_angle(tower.angle);
            if tower.hero && tower.range < 100.0 {
                // Melee class (warrior/guardian/assassin): a slash arc at the target
                // plus a forward lunge, instead of a muzzle flash.
                let poison = matches!(tower.behavior, Behavior::Poison | Behavior::Curse);
                vfx.write(crate::vfx::VfxEvent::Slash {
                    pos: target.pos,
                    angle: tower.angle,
                    color: tower.element.color().mix(&Color::WHITE, 0.28),
                    poison,
                });
                tower.recoil = dir * 6.0; // lunge toward the target
            } else {
                let muzzle = c + dir * (TILE_SIZE * (0.34 + 0.22 * tower.footprint as f32));
                vfx.write(crate::vfx::VfxEvent::Muzzle {
                    pos: muzzle,
                    dir,
                    color: tower.element.color().mix(&Color::WHITE, 0.2),
                });
                tower.recoil = -dir * (3.0 + 1.4 * tower.footprint as f32);
            }
        }

        // Melee heroes strike instantly (no projectile) — the slash VFX is the hit.
        // Warrior(Aoe)/Fire already hit directly; only single-target/poison melee
        // classes (guardian/assassin) would otherwise fire a visible bullet.
        if tower.hero
            && tower.range < 100.0
            && matches!(tower.behavior, Behavior::Single | Behavior::Poison)
        {
            // 背击 (backstab): the assassin (toxic melee hero) deals bonus damage when
            // striking an enemy from behind its facing — strongest against bosses, who
            // walk in a straight line, so it rewards repositioning the hero. This is the
            // assassin's signature anti-boss play.
            let mut amount = tower.damage;
            if tower.element == Element::Toxic {
                let to_attacker = c - target.pos;
                // Hero is behind when the enemy faces away from it.
                if to_attacker.length_squared() > 1.0
                    && target.facing.length_squared() > 0.01
                    && target.facing.dot(to_attacker.normalize()) < -0.2
                {
                    amount *= if target.boss { 2.6 } else { 1.8 };
                    vfx.write(crate::vfx::VfxEvent::Text {
                        pos: target.pos + Vec2::new(0.0, 26.0),
                        text: crate::i18n::t(if target.boss { "背击 致命!" } else { "背击!" }),
                        color: Color::srgb(1.0, 0.35, 0.85),
                        size: if target.boss { 18.0 } else { 14.0 },
                        life: 0.7,
                    });
                }
            }
            dmg.write(Damage {
                source_tower: Some(entity),
                target: target.entity,
                amount,
                magic: tower.magic,
                element: tower.element,
                armor_pierce: tower.armor_pierce,
            });
            if tower.behavior == Behavior::Poison {
                if tower.poison_duration > 0.0 {
                    status.write(Status {
                        source_tower: Some(entity),
                        target: target.entity,
                        kind: StatusKind::Poison {
                            dmg: tower.dot_damage,
                            duration: tower.poison_duration,
                        },
                    });
                }
                if tower.armor_reduce > 0.0 && tower.curse_duration > 0.0 {
                    status.write(Status {
                        source_tower: Some(entity),
                        target: target.entity,
                        kind: StatusKind::Curse {
                            reduce: tower.armor_reduce,
                            duration: tower.curse_duration,
                        },
                    });
                }
            }
            continue;
        }

        match tower.behavior {
            Behavior::Aoe => {
                // A melee cleave hero (warrior) hits the group around the target with
                // its slash arc — no ranged beam. Ranged AoE towers/mages keep the beam.
                let melee_hero = tower.hero && tower.range < 100.0;
                if !melee_hero {
                    spawn_layered_beam(
                        &mut commands,
                        c,
                        target.pos,
                        tower.element.color().mix(&tower.color, 0.35),
                        if tower.kind == TowerKind::Fortress {
                            7.5
                        } else {
                            5.5
                        },
                        0.20,
                        9.5,
                        true,
                    );
                }
                vfx.write(crate::vfx::VfxEvent::Explosion {
                    pos: target.pos,
                    radius: tower.aoe_radius,
                    color: tower.element.color(),
                });
                for e in &snap.enemies {
                    // Invisible enemies that no detector reveals are immune to splash
                    // too — otherwise any AoE trivially bypasses stealth. Detection
                    // towers (or a detected state) make them takeable again.
                    if e.invisible && !snap.is_detected(e) {
                        continue;
                    }
                    if e.pos.distance(target.pos) <= tower.aoe_radius {
                        dmg.write(Damage {
                            source_tower: Some(entity),
                            target: e.entity,
                            amount: tower.damage,
                            magic: false,
                            element: tower.element,
                            armor_pierce: tower.armor_pierce,
                        });
                        // AoE towers carrying extra attributes (notably the 神之塔)
                        // also apply slow/poison to everything caught in the blast.
                        if tower.slow_duration > 0.0 {
                            status.write(Status {
                                source_tower: Some(entity),
                                target: e.entity,
                                kind: StatusKind::Slow {
                                    duration: tower.slow_duration,
                                },
                            });
                        }
                        if tower.dot_damage > 0.0 && tower.poison_duration > 0.0 {
                            status.write(Status {
                                source_tower: Some(entity),
                                target: e.entity,
                                kind: StatusKind::Poison {
                                    dmg: tower.dot_damage,
                                    duration: tower.poison_duration,
                                },
                            });
                        }
                    }
                }
            }
            Behavior::Fire => {
                // Characteristic: lay a roaring wall of flame across the path. It
                // burns longer and reaches wider than a plain AoE, and its look is
                // animated in `update_fire_grounds` (flicker, white-hot core, embers).
                // Equipment can convert this tower, so the wall carries the element.
                let life = (tower.fire_duration.max(2.0) * 1.6).max(3.0);
                let angle = fire_wall_angle(target, &board, c);
                let half_len = (tower.aoe_radius * 1.42).max(TILE_SIZE * 2.2);
                let half_width = (TILE_SIZE * 0.62).max(24.0);
                let elem = tower.element.color();
                let axis = Vec2::from_angle(angle);
                spawn_layered_beam(&mut commands, c, target.pos, elem, 8.0, 0.24, 9.5, true);
                // Outer flame body (FireGround = hitbox + animated base).
                commands.spawn((
                    Sprite {
                        color: elem.mix(&Color::WHITE, 0.12).with_alpha(0.46),
                        custom_size: Some(Vec2::new(half_len * 2.0, half_width * 2.0)),
                        ..default()
                    },
                    Transform::from_translation(target.pos.extend(3.5))
                        .with_rotation(Quat::from_rotation_z(angle)),
                    crate::components::FireGround {
                        half_len,
                        half_width,
                        angle,
                        dps: tower.dot_damage.max(8.0),
                        element: tower.element,
                        source_tower: Some(entity),
                        life,
                        max_life: life,
                        ember_timer: 0.0,
                    },
                    LevelEntity,
                ));
                // White-hot inner core (visual only) — fades with the wall via its
                // own short-lived FireGround sibling carrying no damage.
                commands.spawn((
                    Sprite {
                        color: Color::srgb(1.0, 0.93, 0.66).with_alpha(0.5),
                        custom_size: Some(Vec2::new(half_len * 1.7, half_width * 0.9)),
                        ..default()
                    },
                    Transform::from_translation(target.pos.extend(3.62))
                        .with_rotation(Quat::from_rotation_z(angle)),
                    crate::components::FireGround {
                        half_len,
                        half_width: half_width * 0.45,
                        angle,
                        dps: 0.0,
                        element: tower.element,
                        source_tower: None,
                        life: life * 0.85,
                        max_life: life * 0.85,
                        ember_timer: 0.5,
                    },
                    LevelEntity,
                ));
                // A row of flame bursts along the wall so it ignites as a line of
                // fire rather than a single flat puff.
                for k in [-0.66_f32, -0.22, 0.22, 0.66] {
                    vfx.write(crate::vfx::VfxEvent::ElementPulse {
                        pos: target.pos + axis * k * half_len,
                        color: elem,
                        strong: k.abs() < 0.4,
                    });
                }
                for e in &snap.enemies {
                    if point_in_fire_wall(e.pos, target.pos, angle, half_len, half_width) {
                        dmg.write(Damage {
                            source_tower: Some(entity),
                            target: e.entity,
                            amount: tower.damage,
                            magic: true,
                            element: tower.element,
                            armor_pierce: tower.armor_pierce,
                        });
                        status.write(Status {
                            source_tower: Some(entity),
                            target: e.entity,
                            kind: StatusKind::Fire {
                                dmg: tower.dot_damage.max(8.0),
                                duration: tower.fire_duration.max(1.0),
                                element: tower.element,
                            },
                        });
                    }
                }
            }
            Behavior::Freeze => {
                spawn_layered_beam(
                    &mut commands,
                    c,
                    target.pos,
                    tower.element.color().mix(&Color::WHITE, 0.24),
                    6.0,
                    0.18,
                    9.5,
                    true,
                );
                vfx.write(crate::vfx::VfxEvent::Explosion {
                    pos: target.pos,
                    radius: tower.aoe_radius,
                    color: tower.element.color(),
                });
                for e in &snap.enemies {
                    if e.pos.distance(target.pos) <= tower.aoe_radius {
                        dmg.write(Damage {
                            source_tower: Some(entity),
                            target: e.entity,
                            amount: tower.damage,
                            magic: true,
                            element: tower.element,
                            armor_pierce: tower.armor_pierce,
                        });
                        status.write(Status {
                            source_tower: Some(entity),
                            target: e.entity,
                            kind: StatusKind::Freeze {
                                duration: tower.freeze_duration,
                            },
                        });
                    }
                }
            }
            Behavior::Chain => {
                chain_lightning(entity, &tower, target, &snap, &mut dmg, &mut commands)
            }
            Behavior::Homing => spawn_projectile(
                &mut commands,
                c,
                target.pos,
                tower.kind,
                Projectile {
                    source_tower: Some(entity),
                    target: target.entity,
                    speed: proj_px_s(5.0),
                    damage: tower.damage,
                    magic: false,
                    element: tower.element,
                    armor_pierce: tower.armor_pierce,
                    kind: ProjKind::Missile,
                    aoe_radius: tower.aoe_radius,
                    slow_duration: 0.0,
                    dot_damage: 0.0,
                    poison_duration: 0.0,
                    armor_reduce: 0.0,
                    curse_duration: 0.0,
                    knock_dist: 0.0,
                    stun_duration: 0.0,
                },
                tower.color,
                &mut meshes,
                &mut materials,
            ),
            Behavior::Knockback => spawn_projectile(
                &mut commands,
                c,
                target.pos,
                tower.kind,
                Projectile {
                    source_tower: Some(entity),
                    target: target.entity,
                    speed: proj_px_s(10.0),
                    damage: tower.damage,
                    magic: false,
                    element: tower.element,
                    armor_pierce: tower.armor_pierce,
                    kind: ProjKind::Knockback,
                    aoe_radius: 0.0,
                    slow_duration: 0.0,
                    dot_damage: 0.0,
                    poison_duration: 0.0,
                    armor_reduce: 0.0,
                    curse_duration: 0.0,
                    knock_dist: tower.knock_dist,
                    stun_duration: tower.stun_duration,
                },
                tower.color,
                &mut meshes,
                &mut materials,
            ),
            Behavior::Poison => spawn_projectile(
                &mut commands,
                c,
                target.pos,
                tower.kind,
                Projectile {
                    source_tower: Some(entity),
                    target: target.entity,
                    speed: proj_px_s(7.0),
                    damage: tower.damage,
                    magic: false,
                    element: tower.element,
                    armor_pierce: tower.armor_pierce,
                    kind: ProjKind::Poison,
                    aoe_radius: 0.0,
                    slow_duration: 0.0,
                    dot_damage: tower.dot_damage,
                    poison_duration: tower.poison_duration,
                    armor_reduce: 0.0,
                    curse_duration: 0.0,
                    knock_dist: 0.0,
                    stun_duration: 0.0,
                },
                tower.color,
                &mut meshes,
                &mut materials,
            ),
            Behavior::Curse => spawn_projectile(
                &mut commands,
                c,
                target.pos,
                tower.kind,
                Projectile {
                    source_tower: Some(entity),
                    target: target.entity,
                    speed: proj_px_s(8.0),
                    damage: tower.damage,
                    magic: true,
                    element: tower.element,
                    armor_pierce: tower.armor_pierce,
                    kind: ProjKind::Curse,
                    aoe_radius: 0.0,
                    slow_duration: 0.0,
                    dot_damage: 0.0,
                    poison_duration: 0.0,
                    armor_reduce: tower.armor_reduce,
                    curse_duration: tower.curse_duration,
                    knock_dist: 0.0,
                    stun_duration: 0.0,
                },
                tower.color,
                &mut meshes,
                &mut materials,
            ),
            Behavior::Slow => spawn_projectile(
                &mut commands,
                c,
                target.pos,
                tower.kind,
                Projectile {
                    source_tower: Some(entity),
                    target: target.entity,
                    speed: proj_px_s(8.0),
                    damage: tower.damage,
                    magic: false,
                    element: tower.element,
                    armor_pierce: tower.armor_pierce,
                    kind: ProjKind::Slow,
                    aoe_radius: 0.0,
                    slow_duration: tower.slow_duration,
                    dot_damage: 0.0,
                    poison_duration: 0.0,
                    armor_reduce: 0.0,
                    curse_duration: 0.0,
                    knock_dist: 0.0,
                    stun_duration: 0.0,
                },
                tower.color,
                &mut meshes,
                &mut materials,
            ),
            // Single-target physical/magic (arrow/magic/sniper).
            _ => spawn_projectile(
                &mut commands,
                c,
                target.pos,
                tower.kind,
                Projectile {
                    source_tower: Some(entity),
                    target: target.entity,
                    speed: proj_px_s(9.0),
                    damage: tower.damage,
                    magic: tower.magic,
                    element: tower.element,
                    armor_pierce: tower.armor_pierce,
                    kind: ProjKind::Normal,
                    aoe_radius: 0.0,
                    slow_duration: 0.0,
                    dot_damage: 0.0,
                    poison_duration: 0.0,
                    armor_reduce: 0.0,
                    curse_duration: 0.0,
                    knock_dist: 0.0,
                    stun_duration: 0.0,
                },
                tower.color,
                &mut meshes,
                &mut materials,
            ),
        }
    }
}

/// Thunder tower: hit target, then jump to nearest un-hit enemies (original
/// `chainLightning`). Subsequent jumps deal 70% damage.
fn chain_lightning(
    source_tower: Entity,
    tower: &Tower,
    first: EnemySnap,
    snap: &Snapshot,
    dmg: &mut MessageWriter<Damage>,
    commands: &mut Commands,
) {
    let mut chained: Vec<Entity> = vec![first.entity];
    let mut current = first;
    let mut remaining = tower.chain_count;
    let tower_pos = tower.center();
    spawn_layered_beam(
        commands,
        tower_pos,
        first.pos,
        tower.element.color(),
        4.5,
        0.18,
        10.5,
        true,
    );

    dmg.write(Damage {
        source_tower: Some(source_tower),
        target: first.entity,
        amount: tower.damage,
        magic: true,
        element: tower.element,
        armor_pierce: tower.armor_pierce,
    });

    while remaining > 1 {
        let mut next: Option<EnemySnap> = None;
        let mut best = tower.chain_range;
        for e in &snap.enemies {
            if chained.contains(&e.entity) || !snap.can_target(tower, e) {
                continue;
            }
            let d = current.pos.distance(e.pos);
            if d <= best {
                best = d;
                next = Some(*e);
            }
        }
        let Some(n) = next else { break };
        chained.push(n.entity);
        spawn_layered_beam(
            commands,
            current.pos,
            n.pos,
            tower.element.color().mix(&Color::WHITE, 0.14),
            3.6,
            0.16,
            10.5,
            true,
        );
        dmg.write(Damage {
            source_tower: Some(source_tower),
            target: n.entity,
            amount: tower.damage * 0.7,
            magic: true,
            element: tower.element,
            armor_pierce: tower.armor_pierce,
        });
        current = n;
        remaining -= 1;
    }
}

// ============================ Projectiles ============================

pub fn update_projectiles(
    mut commands: Commands,
    time: Res<Time>,
    run: Res<RunState>,
    snap: Res<Snapshot>,
    mut projectiles: Query<(
        Entity,
        &mut Projectile,
        &mut ProjectileVisual,
        &mut Transform,
    )>,
    enemy_tf: Query<&Transform, (With<Enemy>, Without<Projectile>)>,
    mut dmg: MessageWriter<Damage>,
    mut status: MessageWriter<Status>,
    mut vfx: MessageWriter<crate::vfx::VfxEvent>,
) {
    let dt = time.delta_secs() * run.game_speed;

    for (entity, mut p, mut visual, mut tf) in &mut projectiles {
        let pos = tf.translation.truncate();
        visual.trail_timer = (visual.trail_timer - dt).max(0.0);

        // Resolve target position; retarget missiles, drop others if target gone.
        let target_pos = match enemy_tf.get(p.target) {
            Ok(t) => t.translation.truncate(),
            Err(_) => {
                if p.kind == ProjKind::Missile {
                    // Find a new nearest enemy.
                    let mut best: Option<EnemySnap> = None;
                    let mut bd = f32::INFINITY;
                    for e in &snap.enemies {
                        let d = pos.distance(e.pos);
                        if d < bd {
                            bd = d;
                            best = Some(*e);
                        }
                    }
                    match best {
                        Some(e) => {
                            p.target = e.entity;
                            e.pos
                        }
                        None => {
                            commands.entity(entity).despawn();
                            continue;
                        }
                    }
                } else {
                    commands.entity(entity).despawn();
                    continue;
                }
            }
        };

        let delta = target_pos - pos;
        let dist = delta.length();
        if dist > 0.0 {
            tf.rotation = Quat::from_rotation_z(delta.to_angle());
        }
        let hit_radius = if p.kind == ProjKind::Missile {
            12.0
        } else {
            10.0
        };

        if dist < hit_radius {
            on_hit(&p, pos, target_pos, &snap, &mut dmg, &mut status, &mut vfx);
            commands.entity(entity).despawn();
        } else {
            let step = delta / dist * p.speed * dt;
            let next = pos + step;
            if visual.trail_timer <= 0.0 {
                spawn_projectile_trail(&mut commands, visual.tower_kind, &p, pos, next);
                visual.trail_timer = if p.kind == ProjKind::Missile {
                    0.035
                } else {
                    0.025
                };
            }
            tf.translation.x += step.x;
            tf.translation.y += step.y;
        }
    }
}

fn on_hit(
    p: &Projectile,
    pos: Vec2,
    target_pos: Vec2,
    snap: &Snapshot,
    dmg: &mut MessageWriter<Damage>,
    status: &mut MessageWriter<Status>,
    vfx: &mut MessageWriter<crate::vfx::VfxEvent>,
) {
    match p.kind {
        ProjKind::Missile => {
            vfx.write(crate::vfx::VfxEvent::Explosion {
                pos,
                radius: p.aoe_radius,
                color: p.element.color(),
            });
            for e in &snap.enemies {
                if e.pos.distance(pos) <= p.aoe_radius {
                    dmg.write(Damage {
                        source_tower: p.source_tower,
                        target: e.entity,
                        amount: p.damage,
                        magic: false,
                        element: p.element,
                        armor_pierce: p.armor_pierce,
                    });
                }
            }
        }
        ProjKind::Slow => {
            dmg.write(Damage {
                source_tower: p.source_tower,
                target: p.target,
                amount: p.damage,
                magic: false,
                element: p.element,
                armor_pierce: p.armor_pierce,
            });
            status.write(Status {
                source_tower: p.source_tower,
                target: p.target,
                kind: StatusKind::Slow {
                    duration: p.slow_duration,
                },
            });
        }
        ProjKind::Poison => {
            dmg.write(Damage {
                source_tower: p.source_tower,
                target: p.target,
                amount: p.damage,
                magic: false,
                element: p.element,
                armor_pierce: p.armor_pierce,
            });
            status.write(Status {
                source_tower: p.source_tower,
                target: p.target,
                kind: StatusKind::Poison {
                    dmg: p.dot_damage,
                    duration: p.poison_duration,
                },
            });
        }
        ProjKind::Curse => {
            dmg.write(Damage {
                source_tower: p.source_tower,
                target: p.target,
                amount: p.damage,
                magic: true,
                element: p.element,
                armor_pierce: p.armor_pierce,
            });
            status.write(Status {
                source_tower: p.source_tower,
                target: p.target,
                kind: StatusKind::Curse {
                    reduce: p.armor_reduce,
                    duration: p.curse_duration,
                },
            });
        }
        ProjKind::Knockback => {
            dmg.write(Damage {
                source_tower: p.source_tower,
                target: p.target,
                amount: p.damage,
                magic: false,
                element: p.element,
                armor_pierce: p.armor_pierce,
            });
            status.write(Status {
                source_tower: p.source_tower,
                target: p.target,
                kind: StatusKind::Knockback {
                    dist: p.knock_dist,
                    stun: p.stun_duration,
                },
            });
        }
        ProjKind::Normal => {
            let _ = target_pos;
            dmg.write(Damage {
                source_tower: p.source_tower,
                target: p.target,
                amount: p.damage,
                magic: p.magic,
                element: p.element,
                armor_pierce: p.armor_pierce,
            });
        }
    }
}

pub fn update_shot_fx(
    mut commands: Commands,
    time: Res<Time>,
    run: Res<RunState>,
    mut fx: Query<(Entity, &mut ShotFx, &mut Sprite, &mut Transform)>,
) {
    let dt = time.delta_secs() * run.game_speed;
    for (entity, mut shot, mut sprite, mut tf) in &mut fx {
        shot.life -= dt;
        if shot.life <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        let t = (shot.life / shot.max_life).clamp(0.0, 1.0);
        sprite.color.set_alpha(shot.alpha * t);
        if shot.shrink_x {
            tf.scale.x = t.max(0.05);
        }
        tf.scale.y = 0.55 + 0.45 * t;
    }
}

// ============================ Summons ============================

fn summon_tint(kind: crate::data::EnemyKind) -> Color {
    use crate::data::EnemyKind::*;
    match kind {
        Healer | Regenerating => Color::srgb(0.5, 1.0, 0.75),
        Flying | Fast | Charger => Color::srgb(0.55, 0.9, 1.0),
        Armored | Shielded | Tank => Color::srgb(0.7, 0.82, 1.0),
        Silencer | Invisible => Color::srgb(0.72, 0.55, 1.0),
        Climber | Moss => Color::srgb(0.55, 0.95, 0.65),
        Boss => Color::srgb(0.95, 0.78, 1.0),
        Splitter | Swarmer => Color::srgb(0.6, 0.85, 1.0),
        Normal => Color::srgb(0.55, 0.8, 1.0),
    }
}

/// Spawn an allied unit (blue-tinted animated creature) that fights enemies.
pub fn spawn_ally(
    commands: &mut Commands,
    creatures: &crate::creatures::Creatures,
    kind: crate::data::EnemyKind,
    pos: Vec2,
    hp: f32,
    damage: f32,
    speed: f32,
    lifetime: f32,
    owner: Entity,
) {
    let px = kind.def().size * 4.5;
    let bar_w = (px * 0.62).max(16.0);
    let bar_y = px * 0.5 + 1.5;
    let (mut sprite, anim) = creatures.sprite(kind, px);
    sprite.color = summon_tint(kind);
    commands
        .spawn((
            Summon {
                hp,
                max_hp: hp,
                damage,
                speed,
                target: None,
                attack_timer: 0.0,
                owner,
                kind,
                lifetime,
                buff: 0.0,
            },
            sprite,
            anim,
            Transform::from_translation(pos.extend(5.5)),
            LevelEntity,
        ))
        .with_children(|p| {
            p.spawn((
                Sprite {
                    color: Color::srgb(0.05, 0.08, 0.14),
                    custom_size: Some(Vec2::new(bar_w, 3.0)),
                    ..default()
                },
                Transform::from_xyz(0.0, bar_y, 0.1),
            ));
            p.spawn((
                Sprite {
                    color: Color::srgb(0.42, 0.8, 1.0),
                    custom_size: Some(Vec2::new(bar_w, 3.0)),
                    ..default()
                },
                Anchor::CENTER_LEFT,
                Transform::from_xyz(-bar_w / 2.0, bar_y, 0.2),
                SummonHpBarFg,
            ));
        });
}

pub fn update_summon_hp_bars(
    summons: Query<(&Summon, &Children)>,
    mut bars: Query<&mut Transform, With<SummonHpBarFg>>,
) {
    for (summon, children) in &summons {
        let frac = if summon.max_hp > 0.0 {
            (summon.hp / summon.max_hp).clamp(0.0, 1.0)
        } else {
            0.0
        };
        for child in children.iter() {
            if let Ok(mut tf) = bars.get_mut(child) {
                tf.scale.x = frac;
            }
        }
    }
}

/// Allies seek the nearest enemy, march to melee range, and attack it.
pub fn update_summons(
    mut commands: Commands,
    time: Res<Time>,
    run: Res<RunState>,
    snap: Res<Snapshot>,
    mut summons: Query<(Entity, &mut Summon, &mut Transform)>,
    mut dmg: MessageWriter<Damage>,
    mut vfx: MessageWriter<crate::vfx::VfxEvent>,
) {
    let dt = time.delta_secs() * run.game_speed;

    for (entity, mut s, mut tf) in &mut summons {
        if s.lifetime.is_finite() {
            s.lifetime -= dt;
        }
        if s.attack_timer > 0.0 {
            s.attack_timer -= dt;
        }
        let pos = tf.translation.truncate();

        let need_target = match s.target {
            Some(t) => !snap.enemies.iter().any(|e| e.entity == t),
            None => true,
        };
        if need_target {
            let mut best: Option<Entity> = None;
            let mut bd = f32::INFINITY;
            for e in &snap.enemies {
                let d = pos.distance(e.pos);
                if d < bd {
                    bd = d;
                    best = Some(e.entity);
                }
            }
            s.target = best;
        }

        if let Some(target) = s.target {
            if let Some(te) = snap.enemies.iter().find(|e| e.entity == target) {
                let delta = te.pos - pos;
                let dist = delta.length();
                if dist > TILE_SIZE * 0.6 {
                    let step = delta / dist * s.speed * dt;
                    tf.translation.x += step.x;
                    tf.translation.y += step.y;
                } else if s.attack_timer <= 0.0 {
                    s.attack_timer = 0.6;
                    dmg.write(Damage {
                        source_tower: Some(s.owner),
                        target,
                        amount: s.damage * (1.0 + s.buff),
                        magic: false,
                        element: Element::Physical,
                        armor_pierce: 0.0,
                    });
                }
            }
        }

        if s.hp <= 0.0 || s.lifetime <= 0.0 {
            vfx.write(crate::vfx::VfxEvent::Death {
                pos,
                color: summon_tint(s.kind),
                big: false,
            });
            commands.entity(entity).despawn();
        }
    }
}

/// Enemies fight back: an enemy adjacent to an allied unit stops advancing and
/// deals melee damage to it (this is the "monsters attack each other" part).
pub fn enemy_vs_ally(
    time: Res<Time>,
    run: Res<RunState>,
    mut commands: Commands,
    mut enemies: Query<(&mut Enemy, &Transform)>,
    mut summons: Query<(Entity, &mut Summon, &Transform)>,
    mut heroes: Query<(Entity, &mut Tower)>,
    mut vfx: MessageWriter<crate::vfx::VfxEvent>,
) {
    let dt = time.delta_secs() * run.game_speed;
    let engage = TILE_SIZE * 0.7;
    let allies: Vec<(Entity, Vec2)> = summons
        .iter()
        .map(|(e, _, t)| (e, t.translation.truncate()))
        .collect();
    // The hero (a movable Tower) joins the melee: enemies that reach it stop and
    // fight it, just like they do summon allies. A slightly larger engage radius lets
    // it "hold the line" on the path.
    let hero: Option<(Entity, Vec2)> = heroes
        .iter()
        .find(|(_, t)| t.hero)
        .map(|(e, t)| (e, t.center()));
    let hero_engage = TILE_SIZE * 0.85;

    let mut hits: Vec<(Entity, f32)> = Vec::new();
    let mut hero_dmg = 0.0;
    for (mut enemy, etf) in &mut enemies {
        enemy.blocked = false;
        let pos = etf.translation.truncate();
        let mut best: Option<Entity> = None;
        let mut bd = engage;
        for (ae, apos) in &allies {
            let d = pos.distance(*apos);
            if d <= bd {
                bd = d;
                best = Some(*ae);
            }
        }
        if let Some(ae) = best {
            enemy.blocked = true;
            hits.push((ae, enemy.melee * dt));
        }
        if let Some((_, hpos)) = hero {
            if pos.distance(hpos) <= hero_engage {
                enemy.blocked = true;
                hero_dmg += enemy.melee * dt;
            }
        }
    }
    for (ae, d) in hits {
        if let Ok((_, mut s, _)) = summons.get_mut(ae) {
            s.hp -= d;
        }
    }
    if hero_dmg > 0.0 {
        if let Some((he, hpos)) = hero {
            if let Ok((_, mut t)) = heroes.get_mut(he) {
                t.hp -= hero_dmg;
                if t.hp <= 0.0 {
                    vfx.write(crate::vfx::VfxEvent::Death {
                        pos: hpos,
                        color: t.color,
                        big: true,
                    });
                    commands.entity(he).despawn();
                }
            }
        }
    }
}

/// Tower-raider enemies attack defensive towers directly. MOSS additionally has
/// a one-shot boss skill that destroys the first tower it reaches.
pub fn draw_tower_raider_threats(
    mut gizmos: Gizmos,
    enemies: Query<(&Enemy, &Transform)>,
    towers: Query<&Tower>,
) {
    let tower_positions: Vec<Vec2> = towers.iter().map(|t| t.center()).collect();
    if tower_positions.is_empty() {
        return;
    }

    for (enemy, etf) in &enemies {
        if (!enemy.tower_raider && !enemy.moss_destroy) || enemy.hp <= 0.0 {
            continue;
        }
        let pos = etf.translation.truncate();
        let sense = if enemy.moss_destroy {
            MOSS_TOWER_SENSE
        } else {
            TOWER_RAIDER_SENSE
        };
        let Some((target, dist)) = tower_positions
            .iter()
            .filter_map(|tpos| {
                let dist = pos.distance(*tpos);
                (dist <= sense).then_some((*tpos, dist))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
        else {
            continue;
        };

        let urgency = (1.0 - (dist / sense).clamp(0.0, 1.0)).max(0.2);
        let color = if enemy.moss_destroy {
            Color::srgb(0.35, 1.0, 0.35)
        } else {
            Color::srgb(1.0, 0.35, 0.18)
        };
        gizmos.line_2d(pos, target, color.with_alpha(0.25 + urgency * 0.45));
        gizmos.circle_2d(
            target,
            TOWER_RAIDER_ENGAGE,
            color.with_alpha(0.2 + urgency * 0.35),
        );
        if dist <= TOWER_RAIDER_ENGAGE {
            gizmos.circle_2d(target, TILE_SIZE * 0.55, color.with_alpha(0.75));
        }
    }
}

fn equipment_rarity_alpha(item: Equipment) -> f32 {
    match item.def().rarity {
        Rarity::Common => 0.46,
        Rarity::Uncommon => 0.54,
        Rarity::Rare => 0.62,
        Rarity::Epic => 0.72,
        Rarity::Legendary => 0.82,
        Rarity::Mythic => 0.94,
    }
}

fn equipment_rarity_scale(item: Equipment) -> f32 {
    match item.def().rarity {
        Rarity::Common => 0.92,
        Rarity::Uncommon => 0.98,
        Rarity::Rare => 1.05,
        Rarity::Epic => 1.12,
        Rarity::Legendary => 1.22,
        Rarity::Mythic => 1.36,
    }
}

fn equipment_visual_color(item: Equipment) -> Color {
    item.def()
        .element
        .map(|element| element.color())
        .unwrap_or_else(|| item.def().rarity.color())
        .mix(&Color::WHITE, 0.12)
}

fn local_point(origin: Vec2, rot: f32, scale: f32, point: Vec2) -> Vec2 {
    let (sin, cos) = rot.sin_cos();
    origin + Vec2::new(point.x * cos - point.y * sin, point.x * sin + point.y * cos) * scale
}

fn draw_local_line(
    gizmos: &mut Gizmos,
    origin: Vec2,
    rot: f32,
    scale: f32,
    a: Vec2,
    b: Vec2,
    color: Color,
) {
    gizmos.line_2d(
        local_point(origin, rot, scale, a),
        local_point(origin, rot, scale, b),
        color,
    );
}

fn draw_local_polyline(
    gizmos: &mut Gizmos,
    origin: Vec2,
    rot: f32,
    scale: f32,
    points: &[Vec2],
    closed: bool,
    color: Color,
) {
    for pair in points.windows(2) {
        draw_local_line(gizmos, origin, rot, scale, pair[0], pair[1], color);
    }
    if closed && points.len() > 2 {
        draw_local_line(
            gizmos,
            origin,
            rot,
            scale,
            *points.last().unwrap(),
            points[0],
            color,
        );
    }
}

fn draw_regular_polygon(
    gizmos: &mut Gizmos,
    origin: Vec2,
    rot: f32,
    radius: f32,
    sides: usize,
    color: Color,
) {
    let mut prev = local_point(origin, rot, radius, Vec2::new(1.0, 0.0));
    for i in 1..=sides {
        let a = std::f32::consts::TAU * i as f32 / sides as f32;
        let next = local_point(origin, rot, radius, Vec2::new(a.cos(), a.sin()));
        gizmos.line_2d(prev, next, color);
        prev = next;
    }
}

fn draw_star(gizmos: &mut Gizmos, origin: Vec2, rot: f32, size: f32, points: usize, color: Color) {
    for i in 0..points {
        let a = rot + std::f32::consts::TAU * i as f32 / points as f32;
        let dir = Vec2::new(a.cos(), a.sin());
        let side = Vec2::new(-dir.y, dir.x);
        gizmos.line_2d(origin + dir * size, origin - dir * size * 0.55, color);
        gizmos.line_2d(
            origin + dir * size * 0.38,
            origin + side * size * 0.34,
            color.mix(&Color::WHITE, 0.08).with_alpha(0.38),
        );
    }
}

fn draw_equipment_glyph(
    gizmos: &mut Gizmos,
    item: Equipment,
    origin: Vec2,
    size: f32,
    phase: f32,
    pulse: f32,
) {
    let alpha = equipment_rarity_alpha(item);
    let col = equipment_visual_color(item).with_alpha(alpha);
    let hot = col
        .mix(&Color::WHITE, 0.34)
        .with_alpha((alpha + 0.1).min(1.0));
    let dim = col.with_alpha(alpha * 0.45);
    let rot = phase;

    match item.visual() {
        EquipmentVisual::Crosshair => {
            gizmos.circle_2d(origin, size * (0.72 + 0.08 * pulse), col);
            gizmos.circle_2d(origin, size * 0.22, hot);
            for a in [
                0.0,
                std::f32::consts::FRAC_PI_2,
                std::f32::consts::PI,
                std::f32::consts::PI * 1.5,
            ] {
                let dir = Vec2::new((rot + a).cos(), (rot + a).sin());
                gizmos.line_2d(origin + dir * size * 0.48, origin + dir * size * 1.04, hot);
            }
        }
        EquipmentVisual::WardSigil => {
            let shield = [
                Vec2::new(0.0, 1.0),
                Vec2::new(0.82, 0.36),
                Vec2::new(0.55, -0.72),
                Vec2::new(0.0, -1.05),
                Vec2::new(-0.55, -0.72),
                Vec2::new(-0.82, 0.36),
            ];
            draw_local_polyline(gizmos, origin, rot, size, &shield, true, col);
            draw_local_line(
                gizmos,
                origin,
                rot,
                size,
                Vec2::new(0.0, 0.74),
                Vec2::ZERO,
                hot,
            );
            draw_local_line(
                gizmos,
                origin,
                rot,
                size,
                Vec2::ZERO,
                Vec2::new(0.0, -0.68),
                hot,
            );
        }
        EquipmentVisual::Feather => {
            draw_local_line(
                gizmos,
                origin,
                rot,
                size,
                Vec2::new(-0.15, -1.0),
                Vec2::new(0.28, 1.0),
                hot,
            );
            for y in [-0.55, -0.15, 0.25, 0.62] {
                draw_local_line(
                    gizmos,
                    origin,
                    rot,
                    size,
                    Vec2::new(0.05, y),
                    Vec2::new(-0.66, y + 0.2),
                    col,
                );
                draw_local_line(
                    gizmos,
                    origin,
                    rot,
                    size,
                    Vec2::new(0.08, y + 0.05),
                    Vec2::new(0.66, y + 0.28),
                    dim,
                );
            }
        }
        EquipmentVisual::FuseSpark => {
            let fuse = [
                Vec2::new(-0.9, -0.55),
                Vec2::new(-0.35, -0.1),
                Vec2::new(0.2, 0.04),
                Vec2::new(0.68, 0.42),
            ];
            draw_local_polyline(gizmos, origin, rot, size, &fuse, false, col);
            draw_star(
                gizmos,
                local_point(origin, rot, size, Vec2::new(0.7, 0.43)),
                rot + pulse,
                size * 0.36,
                6,
                hot,
            );
        }
        EquipmentVisual::Prism => {
            draw_regular_polygon(gizmos, origin, rot + 0.28, size * 0.76, 3, col);
            for y in [-0.36, 0.0, 0.36] {
                draw_local_line(
                    gizmos,
                    origin,
                    rot,
                    size,
                    Vec2::new(0.1, y),
                    Vec2::new(1.08, y + 0.18),
                    hot.with_alpha((alpha + 0.1).min(1.0) * 0.82),
                );
            }
        }
        EquipmentVisual::FrostLens => {
            gizmos.circle_2d(origin, size * 0.68, col);
            for k in 0..6 {
                let a = rot + std::f32::consts::TAU * k as f32 / 6.0;
                let dir = Vec2::new(a.cos(), a.sin());
                let side = Vec2::new(-dir.y, dir.x);
                gizmos.line_2d(origin - dir * size * 0.16, origin + dir * size * 0.9, hot);
                gizmos.line_2d(
                    origin + dir * size * 0.48,
                    origin + dir * size * 0.65 + side * size * 0.18,
                    dim,
                );
                gizmos.line_2d(
                    origin + dir * size * 0.48,
                    origin + dir * size * 0.65 - side * size * 0.18,
                    dim,
                );
            }
        }
        EquipmentVisual::EmberCore => {
            gizmos.circle_2d(origin, size * (0.5 + pulse * 0.12), dim);
            let flame = [
                Vec2::new(0.0, 1.04),
                Vec2::new(0.42, 0.25),
                Vec2::new(0.16, -0.78),
                Vec2::new(0.0, -0.96),
                Vec2::new(-0.22, -0.62),
                Vec2::new(-0.44, 0.08),
            ];
            draw_local_polyline(gizmos, origin, rot, size, &flame, true, hot);
            draw_local_line(
                gizmos,
                origin,
                rot,
                size,
                Vec2::new(0.0, -0.4),
                Vec2::new(0.0, 0.55),
                col,
            );
        }
        EquipmentVisual::VenomDrop => {
            let drop = [
                Vec2::new(0.0, 1.05),
                Vec2::new(0.58, 0.2),
                Vec2::new(0.34, -0.72),
                Vec2::new(0.0, -0.98),
                Vec2::new(-0.34, -0.72),
                Vec2::new(-0.58, 0.2),
            ];
            draw_local_polyline(gizmos, origin, rot, size, &drop, true, col);
            gizmos.circle_2d(
                local_point(origin, rot, size, Vec2::new(0.36, 0.54)),
                size * 0.14,
                hot,
            );
            gizmos.circle_2d(
                local_point(origin, rot, size, Vec2::new(-0.28, -0.18)),
                size * 0.1,
                dim,
            );
        }
        EquipmentVisual::ThunderCoil => {
            gizmos.circle_2d(origin, size * 0.76, dim);
            let bolt = [
                Vec2::new(-0.2, 1.02),
                Vec2::new(0.28, 0.16),
                Vec2::new(-0.06, 0.16),
                Vec2::new(0.16, -1.0),
            ];
            draw_local_polyline(gizmos, origin, rot, size, &bolt, false, hot);
            for k in 0..3 {
                let a = rot + pulse + std::f32::consts::TAU * k as f32 / 3.0;
                let p = origin + Vec2::new(a.cos(), a.sin()) * size * 0.84;
                gizmos.circle_2d(p, size * 0.08, col);
            }
        }
        EquipmentVisual::ShadowSeal => {
            gizmos.circle_2d(origin, size * 0.72, col);
            gizmos.circle_2d(
                origin + Vec2::new(rot.cos(), rot.sin()) * size * 0.22,
                size * 0.52,
                dim,
            );
            draw_local_line(
                gizmos,
                origin,
                rot,
                size,
                Vec2::new(-0.7, -0.5),
                Vec2::new(0.72, 0.52),
                hot,
            );
        }
        EquipmentVisual::BulwarkPlate => {
            for x in [-0.62, 0.0, 0.62] {
                let plate = [
                    Vec2::new(x - 0.24, 0.66),
                    Vec2::new(x + 0.24, 0.66),
                    Vec2::new(x + 0.2, -0.62),
                    Vec2::new(x - 0.2, -0.62),
                ];
                draw_local_polyline(gizmos, origin, rot, size, &plate, true, col);
            }
            gizmos.circle_2d(origin, size * (0.95 + pulse * 0.08), dim);
        }
        EquipmentVisual::ClockworkGear => {
            gizmos.circle_2d(origin, size * 0.58, col);
            gizmos.circle_2d(origin, size * 0.24, hot);
            for k in 0..8 {
                let a = rot + std::f32::consts::TAU * k as f32 / 8.0;
                let dir = Vec2::new(a.cos(), a.sin());
                gizmos.line_2d(origin + dir * size * 0.58, origin + dir * size * 0.94, col);
            }
        }
        EquipmentVisual::SaltCrystal => {
            draw_star(gizmos, origin, rot, size * 0.86, 4, hot);
            draw_star(gizmos, origin, rot + 0.78, size * 0.52, 4, dim);
            gizmos.circle_2d(origin, size * 0.16, col);
        }
        EquipmentVisual::DeepScale => {
            for k in 0..3 {
                let y = -0.42 + k as f32 * 0.38;
                let scale = [
                    Vec2::new(-0.62, y),
                    Vec2::new(-0.22, y + 0.38),
                    Vec2::new(0.22, y + 0.38),
                    Vec2::new(0.62, y),
                ];
                draw_local_polyline(
                    gizmos,
                    origin,
                    rot,
                    size,
                    &scale,
                    false,
                    if k == 1 { hot } else { col },
                );
            }
            gizmos.circle_2d(origin, size * 0.9, dim);
        }
        EquipmentVisual::ForbiddenTome => {
            let book = [
                Vec2::new(-0.72, 0.84),
                Vec2::new(0.72, 0.7),
                Vec2::new(0.72, -0.84),
                Vec2::new(-0.72, -0.7),
            ];
            draw_local_polyline(gizmos, origin, rot, size, &book, true, col);
            draw_local_line(
                gizmos,
                origin,
                rot,
                size,
                Vec2::new(0.0, 0.76),
                Vec2::new(0.0, -0.74),
                dim,
            );
            draw_regular_polygon(gizmos, origin, rot + pulse * 0.25, size * 0.32, 3, hot);
        }
        EquipmentVisual::StarBarrel => {
            draw_local_line(
                gizmos,
                origin,
                rot,
                size,
                Vec2::new(-0.88, -0.24),
                Vec2::new(0.92, -0.24),
                col,
            );
            draw_local_line(
                gizmos,
                origin,
                rot,
                size,
                Vec2::new(-0.72, 0.24),
                Vec2::new(0.72, 0.24),
                col,
            );
            draw_local_line(
                gizmos,
                origin,
                rot,
                size,
                Vec2::new(0.72, 0.24),
                Vec2::new(0.92, -0.24),
                col,
            );
            draw_star(
                gizmos,
                local_point(origin, rot, size, Vec2::new(0.92, 0.44)),
                rot,
                size * 0.32,
                5,
                hot,
            );
        }
        EquipmentVisual::VoidCapacitor => {
            let left = local_point(origin, rot, size, Vec2::new(-0.55, 0.0));
            let right = local_point(origin, rot, size, Vec2::new(0.55, 0.0));
            gizmos.circle_2d(left, size * 0.26, col);
            gizmos.circle_2d(right, size * 0.26, col);
            gizmos.line_2d(left, right, hot);
            gizmos.circle_2d(origin, size * (0.84 + pulse * 0.12), dim);
        }
        EquipmentVisual::SaintedGear => {
            gizmos.circle_2d(origin, size * 0.74, col);
            for k in 0..8 {
                let a = rot + std::f32::consts::TAU * k as f32 / 8.0;
                let dir = Vec2::new(a.cos(), a.sin());
                gizmos.line_2d(origin + dir * size * 0.62, origin + dir * size * 0.88, dim);
            }
            draw_local_line(
                gizmos,
                origin,
                rot,
                size,
                Vec2::new(0.0, 0.72),
                Vec2::new(0.0, -0.72),
                hot,
            );
            draw_local_line(
                gizmos,
                origin,
                rot,
                size,
                Vec2::new(-0.5, 0.05),
                Vec2::new(0.5, 0.05),
                hot,
            );
        }
        EquipmentVisual::KrakenHeart => {
            let heart = [
                Vec2::new(0.0, -0.86),
                Vec2::new(0.72, -0.16),
                Vec2::new(0.5, 0.6),
                Vec2::new(0.0, 0.38),
                Vec2::new(-0.5, 0.6),
                Vec2::new(-0.72, -0.16),
            ];
            draw_local_polyline(
                gizmos,
                origin,
                rot,
                size * (1.0 + pulse * 0.08),
                &heart,
                true,
                hot,
            );
            for x in [-0.5, 0.0, 0.5] {
                draw_local_line(
                    gizmos,
                    origin,
                    rot + pulse * 0.15,
                    size,
                    Vec2::new(x, -0.45),
                    Vec2::new(x * 1.25, -1.08),
                    dim,
                );
            }
        }
        EquipmentVisual::AzathothEye => {
            let eye = [
                Vec2::new(-1.0, 0.0),
                Vec2::new(-0.35, 0.46),
                Vec2::new(0.35, 0.46),
                Vec2::new(1.0, 0.0),
                Vec2::new(0.35, -0.46),
                Vec2::new(-0.35, -0.46),
            ];
            draw_local_polyline(gizmos, origin, rot, size, &eye, true, hot);
            gizmos.circle_2d(origin, size * (0.28 + pulse * 0.06), col);
            gizmos.circle_2d(origin, size * 0.1, Color::WHITE.with_alpha(0.88));
        }
    }
}

pub fn draw_equipment_resonance(time: Res<Time>, mut gizmos: Gizmos, towers: Query<&Tower>) {
    let t = time.elapsed_secs();
    let pulse = (t * 2.8).sin() * 0.5 + 0.5;
    for tower in &towers {
        // Max-level (Lv3) crown aura: a slow golden double-ring + a rotating spark
        // ring so fully-upgraded towers stand out. Drawn regardless of equipment.
        if tower.level >= 3 {
            let c = tower.center();
            let r = TILE_SIZE * (0.5 + tower.footprint as f32 * 0.3);
            let gold = Color::srgb(1.0, 0.84, 0.34);
            gizmos.circle_2d(c, r + pulse * 4.0, gold.with_alpha(0.4));
            gizmos.circle_2d(c, r * 0.86, gold.with_alpha(0.18 + 0.14 * pulse));
            let spin = t * 1.1;
            for k in 0..6 {
                let a = spin + std::f32::consts::TAU * k as f32 / 6.0;
                let p = c + Vec2::new(a.cos(), a.sin()) * (r + 3.0);
                gizmos.circle_2d(p, 2.4 + pulse, gold.with_alpha(0.7));
            }
        }

        // Collect the equipped relics (法球): each one becomes an orbiting orb.
        let equipped: Vec<crate::equipment::Equipment> =
            tower.equipment.iter().flatten().copied().collect();
        if equipped.is_empty() {
            continue;
        }
        let pos = tower.center();
        let base_radius = TILE_SIZE * (0.42 + tower.footprint as f32 * 0.26);
        let set_bonus = equipment_set_bonus(&tower.equipment);

        // --- Set-resonance halo (only when a real set bonus is active) ---
        if set_bonus.active() {
            let color = if let Some(element) = set_bonus.resonance_element {
                element.color()
            } else if set_bonus.grade_tier >= 2 {
                Color::srgb(1.0, 0.78, 0.34)
            } else {
                Color::srgb(0.72, 0.86, 1.0)
            };
            let strength = if set_bonus.resonance_count >= 3 || set_bonus.grade_tier >= 3 {
                1.0
            } else {
                0.72
            };
            gizmos.circle_2d(
                pos,
                base_radius + pulse * 5.0,
                color.with_alpha(0.22 + 0.24 * strength),
            );
            gizmos.circle_2d(
                pos,
                base_radius * 0.72,
                Color::WHITE.with_alpha(0.08 + 0.12 * strength),
            );
            if set_bonus.grade_tier > 0 {
                gizmos.circle_2d(
                    pos,
                    base_radius * (0.48 + 0.08 * set_bonus.grade_tier as f32),
                    Color::srgb(1.0, 0.86, 0.42).with_alpha(0.12 + 0.08 * pulse),
                );
            }
        }

        // --- Slot glyphs: each equipped item gets its own readable symbol. ---
        // These sit closer to the tower body than the orbiting orbs, so the player
        // can identify the equipped pieces without opening the bottom loadout dock.
        let slot_radius = base_radius * 0.78;
        for (slot, item) in tower.equipment.iter().enumerate() {
            let Some(item) = item else {
                continue;
            };
            let slot_angle =
                -std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU * slot as f32 / 3.0;
            let wobble = (t * 1.3 + slot as f32 * 1.7).sin() * 0.08;
            let dir = Vec2::from_angle(slot_angle + wobble);
            let anchor = pos + dir * slot_radius;
            let col = equipment_visual_color(*item)
                .with_alpha(0.28 + equipment_rarity_alpha(*item) * 0.32);
            gizmos.line_2d(pos + dir * base_radius * 0.28, anchor, col);
            draw_equipment_glyph(
                &mut gizmos,
                *item,
                anchor,
                TILE_SIZE * 0.145 * equipment_rarity_scale(*item),
                t * (0.42 + slot as f32 * 0.08),
                pulse,
            );
        }

        // --- Orbiting 法球 orbs: one glowing orb per equipped relic ---
        // A full resonance (all three same element) spins faster and brighter.
        let n = equipped.len();
        let orbit_r = base_radius * 1.16;
        let resonant = set_bonus.resonance_count >= 3;
        let spin = t * if resonant { 2.2 } else { 1.35 };
        for (i, eq) in equipped.iter().enumerate() {
            let ang = spin + std::f32::consts::TAU * i as f32 / n as f32;
            let op = pos + Vec2::new(ang.cos(), ang.sin()) * orbit_r;
            let col = equipment_visual_color(*eq);
            let orb_r = (4.0 + 1.6 * pulse + if resonant { 1.2 } else { 0.0 })
                * equipment_rarity_scale(*eq);
            // Faint outer glow, a couple of body rings, then a white-hot pip —
            // gizmos only draw outlines, so concentric circles fake a filled orb.
            gizmos.circle_2d(op, orb_r * 1.9, col.with_alpha(0.16));
            gizmos.circle_2d(op, orb_r * 1.25, col.with_alpha(0.4));
            gizmos.circle_2d(op, orb_r * 0.8, col.with_alpha(0.65));
            gizmos.circle_2d(op, orb_r * 0.34, Color::WHITE.with_alpha(0.75));
        }
    }
}

pub fn enemy_vs_tower(
    time: Res<Time>,
    mut run: ResMut<RunState>,
    mut commands: Commands,
    mut enemies: Query<(&mut Enemy, &Transform)>,
    mut towers: Query<(Entity, &mut Tower)>,
    mut inv: ResMut<EquipmentInventory>,
    mut vfx: MessageWriter<crate::vfx::VfxEvent>,
) {
    let dt = time.delta_secs() * run.game_speed;
    for (_, mut tower) in &mut towers {
        tower.siege_vfx_timer = (tower.siege_vfx_timer - dt).max(0.0);
        if tower.max_hp > 0.0 && tower.hp / tower.max_hp > 0.45 {
            tower.low_hp_warned = false;
        }
    }

    let tower_infos: Vec<(Entity, Vec2)> = towers.iter().map(|(e, t)| (e, t.center())).collect();
    if tower_infos.is_empty() {
        return;
    }

    let mut hits: HashMap<Entity, f32> = HashMap::new();
    let mut crushes: Vec<(Entity, Vec2)> = Vec::new();
    let mut destroyed_towers = HashSet::new();

    for (mut enemy, etf) in &mut enemies {
        if !enemy.tower_raider && !enemy.moss_destroy {
            continue;
        }
        let pos = etf.translation.truncate();
        let mut best: Option<(Entity, Vec2)> = None;
        let mut bd = TOWER_RAIDER_ENGAGE;
        for (te, tpos) in &tower_infos {
            let d = pos.distance(*tpos);
            if d <= bd {
                bd = d;
                best = Some((*te, *tpos));
            }
        }
        let Some((target, tpos)) = best else {
            continue;
        };

        if enemy.moss_destroy && !enemy.moss_destroyed {
            enemy.moss_destroyed = true;
            crushes.push((target, tpos));
            continue;
        }

        if enemy.tower_dps > 0.0 {
            enemy.blocked = true;
            *hits.entry(target).or_default() += enemy.tower_dps * dt;
        }
    }

    for (target, pos) in crushes {
        if !destroyed_towers.insert(target) {
            continue;
        }
        if let Ok((_, tower)) = towers.get_mut(target) {
            return_equipment_to_inventory(&mut inv, &tower);
        }
        commands.entity(target).despawn();
        run.show(crate::i18n::t("MOSS吞噬了第一座防御塔！"));
        vfx.write(crate::vfx::VfxEvent::Explosion {
            pos,
            radius: TILE_SIZE * 1.3,
            color: Color::srgb(0.25, 0.9, 0.35),
        });
    }

    for (target, raw) in hits {
        if destroyed_towers.contains(&target) {
            continue;
        }
        let Ok((ent, mut tower)) = towers.get_mut(target) else {
            continue;
        };
        let set_bonus = equipment_set_bonus(&tower.equipment);
        let effective_armor = (tower.armor + set_bonus.armor_add).max(0.0);
        let actual = raw * (100.0 / (100.0 + effective_armor));
        tower.hp -= actual;
        let hp_frac = if tower.max_hp > 0.0 {
            (tower.hp / tower.max_hp).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let pos = tower.center();
        if tower.siege_vfx_timer <= 0.0 {
            tower.siege_vfx_timer = if hp_frac <= 0.3 { 0.24 } else { 0.38 };
            let feedback_pos =
                pos + Vec2::new(0.0, TILE_SIZE * (0.18 + tower.footprint as f32 * 0.12));
            vfx.write(crate::vfx::VfxEvent::Hit {
                pos: feedback_pos,
                color: Color::srgb(1.0, 0.34, 0.16),
                element: crate::data::Element::Physical,
            });
            vfx.write(crate::vfx::VfxEvent::TaggedNumber {
                pos: feedback_pos,
                amount: actual.max(1.0),
                color: Color::srgb(1.0, 0.62, 0.24),
                label: "攻塔",
            });
        }
        if tower.hp > 0.0 && hp_frac <= 0.3 && !tower.low_hp_warned {
            tower.low_hp_warned = true;
            run.show_for(
                crate::i18n::tf(
                    "{}防御塔受损严重，按 R 修理！",
                    &[&crate::i18n::t(tower.kind.def().name)],
                ),
                2.6,
            );
            vfx.write(crate::vfx::VfxEvent::Text {
                pos: pos + Vec2::new(0.0, TILE_SIZE * 0.78),
                text: crate::i18n::t("防御塔濒危"),
                color: Color::srgb(1.0, 0.22, 0.14),
                size: 15.0,
                life: 1.0,
            });
        }
        if tower.hp <= 0.0 && destroyed_towers.insert(ent) {
            return_equipment_to_inventory(&mut inv, &tower);
            commands.entity(ent).despawn();
            run.show(crate::i18n::t("防御塔被怪物摧毁！"));
            vfx.write(crate::vfx::VfxEvent::Death {
                pos,
                color: tower.element.color(),
                big: tower.footprint > 1,
            });
        }
    }
}

/// Necromancer towers raise enemies that die within range into allied units.
pub fn necromancer_raise(
    mut died: MessageReader<EnemyDied>,
    mut commands: Commands,
    creatures: Res<crate::creatures::Creatures>,
    mut towers: Query<(Entity, &mut Tower, &Transform)>,
    allies: Query<(), With<Summon>>,
    mut vfx: MessageWriter<crate::vfx::VfxEvent>,
) {
    let mut ally_count = allies.iter().count();
    const ALLY_CAP: usize = 24;
    for ev in died.read() {
        if ally_count >= ALLY_CAP {
            break;
        }
        for (entity, mut t, _) in &mut towers {
            if t.behavior != Behavior::Necromancer || t.cooldown_timer > 0.0 {
                continue;
            }
            if t.center().distance(ev.pos) <= t.range {
                t.cooldown_timer = t.cooldown;
                let center = t.center();
                spawn_layered_beam(
                    &mut commands,
                    center,
                    ev.pos,
                    t.element.color().mix(&Color::WHITE, 0.10),
                    4.0,
                    0.32,
                    10.0,
                    true,
                );
                vfx.write(crate::vfx::VfxEvent::ElementPulse {
                    pos: ev.pos,
                    color: t.element.color(),
                    strong: false,
                });
                // Level unlocks "+1 revive": raise `level` undead per kill, each
                // stronger at higher levels.
                let raises = t.level.max(1);
                let hp = (ev.max_hp * (0.4 + 0.12 * t.level as f32)).max(20.0);
                let dmg = 16.0 + 10.0 * t.level as f32;
                for i in 0..raises {
                    if ally_count >= ALLY_CAP {
                        break;
                    }
                    // Fan the revived units out a little so they don't stack.
                    let spread = ((i as f32) - (raises as f32 - 1.0) / 2.0) * 16.0;
                    spawn_ally(
                        &mut commands,
                        &creatures,
                        ev.kind,
                        ev.pos + Vec2::new(spread, 0.0),
                        hp,
                        dmg,
                        90.0,
                        14.0, // raised allies fade after 14s
                        entity,
                    );
                    ally_count += 1;
                }
                break;
            }
        }
    }
}

/// Burning ground patches: damage enemies standing in them, fade out, despawn.
pub fn update_fire_grounds(
    time: Res<Time>,
    run: Res<RunState>,
    mut commands: Commands,
    mut vfx: MessageWriter<crate::vfx::VfxEvent>,
    mut grounds: Query<
        (
            Entity,
            &mut crate::components::FireGround,
            &mut Transform,
            &mut Sprite,
        ),
        Without<Enemy>,
    >,
    mut enemies: Query<(&mut Enemy, &Transform), Without<crate::components::FireGround>>,
    detectors: Query<&Tower>,
) {
    let dt = time.delta_secs() * run.game_speed;
    // Detector coverage so undetected invisible enemies don't burn in fire walls.
    let detect: Vec<(Vec2, f32)> = detectors
        .iter()
        .filter(|t| t.detector)
        .map(|t| (t.center(), t.range))
        .collect();
    for (e, mut g, mut tf, mut sprite) in &mut grounds {
        g.life -= dt;
        if g.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        let frac = g.life / g.max_life;
        // Two out-of-phase sines give a turbulent flame flicker rather than a
        // steady throb; the wall also tapers and brightens to white as it peaks.
        let flick = ((g.life * 23.0).sin() * 0.5 + 0.5) * ((g.life * 8.3).cos() * 0.5 + 0.5);
        let core = g.dps <= 0.0; // the visual-only inner sliver
        let base = if core {
            Color::srgb(1.0, 0.93, 0.66)
        } else {
            g.element.color()
        };
        let white = if core { 0.0 } else { 0.18 + 0.4 * flick };
        let alpha = if core {
            (0.25 + 0.5 * frac) * (0.6 + 0.4 * flick)
        } else {
            (0.14 + 0.5 * frac) * (0.7 + 0.3 * flick)
        };
        sprite.color = base.mix(&Color::WHITE, white).with_alpha(alpha);
        // Flames lick upward (scale.y) and shimmer sideways (scale.x); a fading
        // wall also shrinks so it dies down instead of vanishing abruptly.
        let settle = (frac * 3.0).min(1.0);
        tf.scale.y = (0.82 + flick * 0.34) * settle;
        tf.scale.x = (1.0 + (g.life * 6.0).sin() * 0.035) * (0.9 + 0.1 * settle);

        let pos = tf.translation.truncate();

        // Spit embers off the wall so it reads as live fire, not a flat decal.
        g.ember_timer -= dt;
        if g.ember_timer <= 0.0 {
            g.ember_timer = 0.22;
            let axis = Vec2::from_angle(g.angle);
            // Walk the burst point along the wall using cheap pseudo-noise.
            let along = (g.life * 4.7).sin() * (g.life * 1.9).cos();
            vfx.write(crate::vfx::VfxEvent::ElementPulse {
                pos: pos + axis * along * g.half_len * 0.85,
                color: g.element.color().mix(&Color::srgb(1.0, 0.62, 0.16), 0.45),
                strong: false,
            });
        }

        if core {
            continue; // the inner core is decoration only — no burn application
        }
        for (mut enemy, etf) in &mut enemies {
            let epos = etf.translation.truncate();
            // Undetected invisible enemies don't take fire-wall burn.
            if enemy.invisible && !detect.iter().any(|(p, r)| p.distance(epos) <= *r) {
                continue;
            }
            if point_in_fire_wall(epos, pos, g.angle, g.half_len, g.half_width) {
                // Reuse the burn-DoT slots, but carry this patch's current element.
                let replacing = enemy.fire_timer <= 0.0 || g.dps >= enemy.fire_damage;
                enemy.fire_timer = enemy.fire_timer.max(0.4);
                enemy.fire_damage = enemy.fire_damage.max(g.dps);
                if replacing {
                    enemy.fire_element = g.element;
                    enemy.fire_source_tower = g.source_tower;
                }
            }
        }
    }
}

// ============================ Apply events ============================

#[derive(Message)]
pub struct HealCarrot {
    pub amount: i32,
}

pub fn apply_heal(mut events: MessageReader<HealCarrot>, mut run: ResMut<RunState>) {
    for h in events.read() {
        let cap = run.start_lives.max(1);
        if run.lives < cap {
            run.lives = (run.lives + h.amount).min(cap);
        }
    }
}

pub fn apply_buffs(mut events: MessageReader<BuffTower>, mut towers: Query<&mut Tower>) {
    for b in events.read() {
        if let Ok(mut t) = towers.get_mut(b.target) {
            t.base_damage = (t.base_damage * 1.02).floor();
        }
    }
}

/// Recompute each tower's adjacency synergy and equipment set resonance.
/// Effective `damage` is derived from `base_damage` so talents, upgrades,
/// equipment, and adjacency bonuses compose cleanly.
pub fn compute_synergy(mut towers: Query<(Entity, &mut Tower)>) {
    let infos: Vec<(Entity, Vec2, Category, f32)> = towers
        .iter()
        .map(|(e, t)| (e, t.center(), t.kind.def().category, t.footprint as f32))
        .collect();
    for (e, mut t) in &mut towers {
        let cat = t.kind.def().category;
        let c = t.center();
        let fa = t.footprint as f32;
        let count = infos
            .iter()
            .filter(|(oe, opos, ocat, ofp)| {
                *oe != e
                    && *ocat == cat
                    && c.distance(*opos) <= (fa * 0.5 + ofp * 0.5 + 1.2) * TILE_SIZE
            })
            .count();
        let bonus = (0.12 * count as f32).min(0.6);
        let set_bonus = equipment_set_bonus(&t.equipment);
        t.synergy = bonus;
        // Hero doctrine aura stacks additively with adjacency synergy.
        t.damage =
            (t.base_damage * (1.0 + bonus + t.aura_damage) * set_bonus.damage_mult).floor();
    }
}

pub fn apply_status(
    mut events: MessageReader<Status>,
    mut enemies: Query<(&mut Enemy, &mut Transform)>,
    board: Res<Board>,
) {
    for s in events.read() {
        let Ok((mut e, mut tf)) = enemies.get_mut(s.target) else {
            continue;
        };
        match s.kind {
            StatusKind::Slow { duration } => e.slow_timer = e.slow_timer.max(duration),
            StatusKind::Freeze { duration } => {
                e.frozen = true;
                e.stun_timer = e.stun_timer.max(duration);
            }
            StatusKind::Poison { dmg, duration } => {
                let replacing = e.poison_timer <= 0.0 || dmg >= e.poison_damage;
                e.poison_timer = e.poison_timer.max(duration);
                e.poison_damage = e.poison_damage.max(dmg);
                if replacing {
                    e.poison_source_tower = s.source_tower;
                }
            }
            StatusKind::Fire {
                dmg,
                duration,
                element,
            } => {
                let replacing = e.fire_timer <= 0.0 || dmg >= e.fire_damage;
                e.fire_timer = e.fire_timer.max(duration);
                e.fire_damage = e.fire_damage.max(dmg);
                if replacing {
                    e.fire_element = element;
                    e.fire_source_tower = s.source_tower;
                }
            }
            StatusKind::Curse { reduce, duration } => {
                if e.curse_timer <= 0.0 {
                    e.armor_reduce = reduce;
                }
                e.curse_timer = e.curse_timer.max(duration);
            }
            StatusKind::Knockback { dist, stun } => {
                // Knock back along the path toward the previous waypoint.
                let prev = board.path_world[e.path_index.min(board.path_world.len() - 1)];
                let pos = tf.translation.truncate();
                let d = pos - prev;
                if d.length() > 0.0 {
                    let k = d.normalize() * dist;
                    tf.translation.x += k.x;
                    tf.translation.y += k.y;
                }
                e.frozen = true;
                e.stun_timer = e.stun_timer.max(stun);
            }
        }
    }
}

pub fn apply_damage(
    mut events: MessageReader<Damage>,
    mut enemies: Query<(&mut Enemy, &Transform)>,
    mut towers: Query<&mut Tower>,
    mut vfx: MessageWriter<crate::vfx::VfxEvent>,
) {
    for d in events.read() {
        let Ok((mut e, tf)) = enemies.get_mut(d.target) else {
            continue;
        };
        let mut actual = if !d.magic {
            let armor = (e.armor - e.armor_reduce - d.armor_pierce).max(0.0);
            d.amount * (100.0 / (100.0 + armor))
        } else {
            let mr = (e.magic_resist - e.armor_reduce).max(0.0);
            d.amount * (100.0 / (100.0 + mr))
        };
        let element_resist = e.element_resist.get(d.element);
        let element_mult = (1.0 - element_resist).clamp(0.25, 1.75);
        actual *= element_mult;
        // Shield absorbs damage before hp.
        let hp_before = e.hp.max(0.0);
        let mut absorbed = 0.0;
        if e.shield > 0.0 {
            absorbed = actual.min(e.shield);
            e.shield -= absorbed;
            actual -= absorbed;
        }
        let hp_damage = actual.min(hp_before).max(0.0);
        e.hp -= actual;
        let dealt = absorbed + hp_damage;

        if dealt > 0.0 {
            e.last_hit_tower = d.source_tower;
            if let Some(source_tower) = d.source_tower {
                if let Ok(mut tower) = towers.get_mut(source_tower) {
                    tower.damage_done += dealt;
                }
            }
        }

        // Juice: pop + spark + damage number for meaningful hits (skip tiny DoT ticks).
        if dealt >= 1.0 {
            let pos = tf.translation.truncate();
            e.hit_flash = 0.12;
            let matchup = if element_resist <= -0.15 {
                Some(("易伤", Color::srgb(1.0, 0.78, 0.24)))
            } else if element_resist >= 0.20 {
                Some(("抗性", Color::srgb(0.62, 0.72, 0.82)))
            } else {
                None
            };
            if let Some((label, color)) = matchup {
                vfx.write(crate::vfx::VfxEvent::TaggedNumber {
                    pos,
                    amount: dealt,
                    color,
                    label,
                });
                vfx.write(crate::vfx::VfxEvent::ElementPulse {
                    pos,
                    color: if element_resist <= -0.15 {
                        d.element.color()
                    } else {
                        Color::srgb(0.48, 0.58, 0.70)
                    },
                    strong: element_resist <= -0.15,
                });
            } else {
                vfx.write(crate::vfx::VfxEvent::Number { pos, amount: dealt });
            }
            vfx.write(crate::vfx::VfxEvent::Hit {
                pos,
                color: d.element.color(),
                element: d.element,
            });
        }
    }
}
