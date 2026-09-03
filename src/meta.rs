//! Run-wide meta systems: global tower talents (bought with gold).
//! （塔系全局主动技能已移除——主动技只保留英雄技能。）

use crate::components::Enemy;
use crate::data::{BOARD_H, Element};
use crate::game::RunState;
use crate::tower::Damage;
use bevy::prelude::*;

// ============================ Talents ============================

/// Cumulative global multipliers applied to every tower (existing + future).
#[derive(Resource)]
pub struct Talents {
    pub damage_mult: f32,
    pub range_mult: f32,
    pub firerate_mult: f32, // cooldown multiplier (<1 = faster)
    /// Per-level roguelite tower multipliers. These reset on each map load and
    /// are kept separate from the gold-bought global upgrades.
    pub rogue_damage_mult: f32,
    pub rogue_range_mult: f32,
    pub rogue_firerate_mult: f32,
    pub dmg_lvl: i32,
    pub rng_lvl: i32,
    pub spd_lvl: i32,
}

impl Default for Talents {
    fn default() -> Self {
        Talents {
            damage_mult: 1.0,
            range_mult: 1.0,
            firerate_mult: 1.0,
            rogue_damage_mult: 1.0,
            rogue_range_mult: 1.0,
            rogue_firerate_mult: 1.0,
            dmg_lvl: 0,
            rng_lvl: 0,
            spd_lvl: 0,
        }
    }
}

/// Escalating cost for the `lvl`-th purchase of a talent.
pub fn talent_cost(lvl: i32) -> i32 {
    (80.0 * 1.5f32.powi(lvl)).floor() as i32
}

