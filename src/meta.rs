//! Run-wide meta systems: global tower talents (bought with gold) and the
//! player's active abilities (meteor / freeze / gold rush) on cooldowns.

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

// ============================ Abilities ============================

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ability {
    Meteor,
    Freeze,
    GoldRush,
}

/// Active-ability cooldowns measured in **waves (回合)** — a cast goes on cooldown
/// for N waves and ticks down each time a new wave begins. Plus a one-shot cast
/// request from the UI.
#[derive(Resource, Default)]
pub struct Abilities {
    pub meteor_cd: i32,
    pub freeze_cd: i32,
    pub gold_cd: i32,
    /// Last `run.wave` we observed, to detect wave advances / level resets.
    pub last_wave: i32,
    pub pending: Option<Ability>,
}

impl Abilities {
    // Cooldowns in WAVES (回合), not seconds.
    pub const METEOR_MAX: i32 = 2;
    pub const FREEZE_MAX: i32 = 3;
    pub const GOLD_MAX: i32 = 3;
    pub const METEOR_COST: i32 = 50;
    pub const FREEZE_COST: i32 = 40;

    pub fn cd(&self, a: Ability) -> i32 {
        match a {
            Ability::Meteor => self.meteor_cd,
            Ability::Freeze => self.freeze_cd,
            Ability::GoldRush => self.gold_cd,
        }
    }
}

/// Tick ability cooldowns by **waves**: each new wave subtracts 1; a level reset
/// (wave count drops) clears all cooldowns.
pub fn tick_cooldowns(run: Res<RunState>, mut ab: ResMut<Abilities>) {
    if run.wave > ab.last_wave {
        let d = run.wave - ab.last_wave;
        ab.meteor_cd = (ab.meteor_cd - d).max(0);
        ab.freeze_cd = (ab.freeze_cd - d).max(0);
        ab.gold_cd = (ab.gold_cd - d).max(0);
        ab.last_wave = run.wave;
    } else if run.wave < ab.last_wave {
        ab.meteor_cd = 0;
        ab.freeze_cd = 0;
        ab.gold_cd = 0;
        ab.last_wave = run.wave;
    }
}

/// Keyboard casts: Q = meteor, W = freeze, E = gold rush.
pub fn ability_keys(keys: Res<ButtonInput<KeyCode>>, mut ab: ResMut<Abilities>) {
    if keys.just_pressed(KeyCode::KeyQ) {
        ab.pending = Some(Ability::Meteor);
    } else if keys.just_pressed(KeyCode::KeyW) {
        ab.pending = Some(Ability::Freeze);
    } else if keys.just_pressed(KeyCode::KeyE) {
        ab.pending = Some(Ability::GoldRush);
    }
}

/// Resolve a pending ability cast if its cooldown is ready.
pub fn cast_abilities(
    mut ab: ResMut<Abilities>,
    mut run: ResMut<RunState>,
    mut dmg: MessageWriter<Damage>,
    mut enemies: Query<(Entity, &mut Enemy, &Transform)>,
    heroes: Query<&crate::tower::Tower>,
    mut sfx: MessageWriter<crate::audio::SfxEvent>,
    mut vfx: MessageWriter<crate::vfx::VfxEvent>,
) {
    use crate::audio::{SfxEvent, Sound};
    let Some(which) = ab.pending.take() else {
        return;
    };
    let cd = ab.cd(which);
    if cd > 0 {
        run.show(crate::i18n::tf(
            "技能冷却中，还需 {} 回合",
            &[&cd.to_string()],
        ));
        return;
    }
    // When the hero is on the field it acts as the spell anchor — abilities
    // strike / emanate from where the hero stands (王者-style「先走位再放大」).
    let hero_pos = heroes.iter().find(|t| t.hero).map(|t| t.hero_pos);
    match which {
        Ability::Meteor => {
            // Anchored on the hero when alive; otherwise the highest-hp enemy.
            let center = match hero_pos {
                Some(p) => Some(p),
                None => enemies
                    .iter()
                    .max_by(|a, b| a.1.hp.total_cmp(&b.1.hp))
                    .map(|(_, _, tf)| tf.translation.truncate()),
            };
            let Some(center) = center else {
                run.show(crate::i18n::t("没有可击中的目标"));
                return;
            };
            if run.gold < Abilities::METEOR_COST {
                run.show(crate::i18n::t("金币不足（陨石需50）"));
                return;
            }
            run.gold -= Abilities::METEOR_COST;
            // The hero amplifies the strike, rewarding good positioning.
            let anchored = hero_pos.is_some();
            let radius = if anchored { 115.0 } else { 90.0 };
            let amount = if anchored { 520.0 } else { 400.0 };
            let mut hit_count = 0;
            for (e, _, tf) in &enemies {
                if tf.translation.truncate().distance(center) <= radius {
                    hit_count += 1;
                    dmg.write(Damage {
                        source_tower: None,
                        target: e,
                        amount,
                        magic: false,
                        element: Element::Fire,
                        armor_pierce: 999.0,
                    });
                }
            }
            vfx.write(crate::vfx::VfxEvent::MeteorStorm { center, radius });
            vfx.write(crate::vfx::VfxEvent::Text {
                pos: center + Vec2::new(0.0, 28.0),
                text: crate::i18n::tf("陨石命中 {}", &[&hit_count.to_string()]),
                color: Color::srgb(1.0, 0.72, 0.28),
                size: 18.0,
                life: 0.95,
            });
            ab.meteor_cd = Abilities::METEOR_MAX;
            sfx.write(SfxEvent(Sound::Meteor));
            run.show(crate::i18n::t(if anchored {
                "英雄引导陨石！ -50金"
            } else {
                "陨石轰炸！ -50金"
            }));
        }
        Ability::Freeze => {
            if !enemies.iter().any(|(_, e, _)| e.hp > 0.0) {
                run.show(crate::i18n::t("没有可冰封的敌人"));
                return;
            }
            if run.gold < Abilities::FREEZE_COST {
                run.show(crate::i18n::t("金币不足（冰封需40）"));
                return;
            }
            run.gold -= Abilities::FREEZE_COST;
            let mut frozen = 0;
            for (_, mut e, _) in &mut enemies {
                if e.hp > 0.0 {
                    frozen += 1;
                    e.frozen = true;
                    e.stun_timer = e.stun_timer.max(2.5);
                }
            }
            vfx.write(crate::vfx::VfxEvent::FrostNova {
                center: hero_pos.unwrap_or(Vec2::ZERO),
            });
            vfx.write(crate::vfx::VfxEvent::Text {
                pos: Vec2::new(0.0, BOARD_H * 0.32),
                text: crate::i18n::tf("全场冰封 {}", &[&frozen.to_string()]),
                color: Color::srgb(0.65, 0.92, 1.0),
                size: 20.0,
                life: 1.0,
            });
            ab.freeze_cd = Abilities::FREEZE_MAX;
            sfx.write(SfxEvent(Sound::Freeze));
            run.show(crate::i18n::t("全场冰封！ -40金"));
        }
        Ability::GoldRush => {
            // Sacrifice 1 life for gold (needs more than 1 life remaining).
            if run.lives <= 1 {
                run.show(crate::i18n::t("生命不足，无法献祭"));
                return;
            }
            run.wave_perfect = false;
            run.lives -= 1;
            run.gold += 120;
            vfx.write(crate::vfx::VfxEvent::GoldExplosion {
                center: hero_pos.unwrap_or(Vec2::ZERO),
            });
            vfx.write(crate::vfx::VfxEvent::Text {
                pos: Vec2::new(0.0, BOARD_H * 0.28),
                text: crate::i18n::t("+120 金"),
                color: Color::srgb(1.0, 0.88, 0.32),
                size: 21.0,
                life: 1.05,
            });
            ab.gold_cd = Abilities::GOLD_MAX;
            sfx.write(SfxEvent(Sound::Gold));
            run.show(crate::i18n::t("献祭1生命 +120金"));
        }
    }
}
