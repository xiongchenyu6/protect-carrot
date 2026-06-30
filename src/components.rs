//! ECS components shared across systems. In Bevy, components are plain data structs
//! attached to entities; systems query for the combinations they need.

use crate::data::{Element, ElementProfile, EnemyKind};
use crate::monster::EliteAffix;
use bevy::prelude::*;
use moonshine_kind::prelude::Instance;

/// A walking enemy. All status-effect state lives here (durations in **seconds**).
#[derive(Component)]
#[allow(dead_code)]
pub struct Enemy {
    pub kind: EnemyKind,
    pub species_id: usize,
    pub hp: f32,
    pub max_hp: f32,
    /// Movement speed in pixels/second (before slow/freeze).
    pub base_speed: f32,
    pub reward: i32,
    /// Index of the path waypoint we are currently walking *from*.
    pub path_index: usize,
    pub armor: f32,
    pub magic_resist: f32,
    pub element_resist: ElementProfile,
    pub flying: bool,
    pub invisible: bool,
    /// 技能等级倍率（普通 1.0 / 中级 1.5 / 高级 2.0）。冲锋爆发、攻塔索敌范围
    /// 等运行期行为按此放大。
    pub skill_mult: f32,
    /// 隐形单位的“探测半径折扣”：探测塔需把有效射程乘以此值才能照出它
    /// （普通 1.0、中级 ~0.67、高级 0.5，越高级越难被发现）。非隐形单位为 1.0。
    pub stealth: f32,
    /// Fraction of max hp regained per second (0 if none).
    pub regen: f32,
    pub boss: bool,
    pub size: f32,

    // --- status effects ---
    pub slow_timer: f32,
    pub stun_timer: f32,
    pub frozen: bool,
    pub poison_timer: f32,
    pub poison_damage: f32, // per second
    pub fire_timer: f32,
    pub fire_damage: f32, // per second
    pub fire_element: Element,
    pub poison_source_tower: Option<Entity>,
    pub fire_source_tower: Option<Entity>,
    pub curse_timer: f32,
    /// Amount of armor/resist temporarily removed by a curse (restored on expiry).
    pub armor_reduce: f32,

    // --- new-mechanic state ---
    /// Shield pool; absorbs damage before hp.
    pub shield: f32,
    pub max_shield: f32,
    /// Small enemies spawned on death.
    pub splits: i32,
    /// HP/sec healed to nearby allies.
    pub heal_aura: f32,
    /// Periodic speed-burst enemy + its phase timer.
    pub charger: bool,
    pub charge_timer: f32,
    /// Brief "hit" timer driving a scale-pop when damaged (juice).
    pub hit_flash: f32,
    /// Last tower credited with meaningful damage, used for kill attribution.
    pub last_hit_tower: Option<Entity>,
    /// True while engaged in melee with an allied unit (stops advancing).
    pub blocked: bool,
    /// Melee damage/second this enemy deals to allied units it's fighting.
    pub melee: f32,
    /// Elite variant (gold aura, buffed stats, high reward).
    pub elite: bool,
    /// Special elite mutation changing behavior beyond raw stats.
    pub elite_affix: EliteAffix,
    /// Seconds accumulated toward this boss species' next special cast.
    pub boss_skill_timer: f32,
    /// Boss has entered its low-health pressure phase.
    pub enraged: bool,
    /// Temporary phasing/invisibility duration from boss skills.
    pub phase_timer: f32,
    /// Leaves the path to damage nearby towers.
    pub tower_raider: bool,
    pub tower_dps: f32,
    /// Disables tower attacks in this radius.
    pub silence_aura: f32,
    /// Ranged tower harassment: shoots towers from the path instead of walking
    /// off-route into melee.
    pub ranged_tower: bool,
    pub ranged_range: f32,
    pub ranged_damage: f32,
    pub ranged_cooldown: f32,
    pub ranged_timer: f32,
    /// Active self-detonation. It triggers only while alive after approaching a
    /// tower/hero; being killed by the player does not detonate it.
    pub explosive: bool,
    pub explode_damage: f32,
    pub explode_radius: f32,
    pub explode_sense: f32,
    pub explode_trigger: f32,
    /// MOSS boss one-shot tower destruction skill.
    pub moss_destroy: bool,
    pub moss_destroyed: bool,
    /// 孵化：存活满固定时间会变强。普通/中级周期性强化（incubate_stacks 计数），
    /// 高级则直接孵化为本关 boss。incubate_timer 累计存活时间。
    pub incubate: bool,
    pub incubate_timer: f32,
    pub incubate_stacks: i32,
    /// Normalized movement direction, updated as the enemy walks. Used to detect
    /// the assassin's 背击 (backstab): a hit landed from behind this facing.
    pub facing: Vec2,
}

/// A short-lived visual spark/debris bit that drifts and fades.
#[derive(Component)]
pub struct Particle {
    pub vel: Vec2,
    pub life: f32,
    pub max_life: f32,
}

/// A floating combat-text number that rises and fades.
#[derive(Component)]
pub struct FloatText {
    pub life: f32,
    pub max_life: f32,
}

/// An expanding, fading shockwave ring (explosions).
#[derive(Component)]
pub struct Shockwave {
    pub life: f32,
    pub max_life: f32,
    pub radius: f32,
}

/// A persistent elemental wall on the ground: enemies inside take damage/sec.
#[derive(Component)]
pub struct FireGround {
    pub half_len: f32,
    pub half_width: f32,
    pub angle: f32,
    pub dps: f32,
    pub element: Element,
    pub source_tower: Option<Entity>,
    pub life: f32,
    pub max_life: f32,
    /// Countdown to the next ember burst (flame embers spit off the wall).
    pub ember_timer: f32,
}

impl Enemy {
    /// Current move speed accounting for slow/freeze.
    pub fn current_speed(&self) -> f32 {
        let speed = if self.frozen {
            0.0
        } else if self.slow_timer > 0.0 {
            self.base_speed * 0.5
        } else {
            self.base_speed
        };
        if self.boss && self.enraged {
            speed * 1.12
        } else {
            speed
        }
    }
}

/// The enemy spawn portal (the abyss) at the path start. Grows with the wave count.
#[derive(Component)]
pub struct SpawnPortal;

/// The carrot at the path end (the thing we defend).
#[derive(Component)]
pub struct Carrot {
    /// Short hit pulse after enemies breach the seal or abilities spend life.
    pub pulse_timer: f32,
    /// Last observed run life count; used to detect non-enemy life changes too.
    pub last_lives: i32,
}

/// Foreground seal-life bar drawn next to the carrot.
#[derive(Component)]
pub struct CarrotSealBar {
    pub width: f32,
}

/// Marker for anything tied to the current level, so a level reload can despawn it all.
#[derive(Component)]
pub struct LevelEntity;

/// The foreground (green) quad of an enemy's HP bar; its x-scale tracks hp fraction.
#[derive(Component)]
pub struct HpBarFg;

/// Blue overlay bar for an enemy's shield pool.
#[derive(Component)]
pub struct ShieldBarFg;

/// Blue foreground bar for allied summoned units.
#[derive(Component)]
pub struct SummonHpBarFg;

/// A world-space tower HP bar segment linked to a tower entity. It is not a child
/// because tower sprites rotate toward targets.
#[derive(Component)]
pub struct TowerHpBar {
    pub owner: Instance<crate::tower::Tower>,
    pub width: f32,
    pub offset_y: f32,
    pub foreground: bool,
}
