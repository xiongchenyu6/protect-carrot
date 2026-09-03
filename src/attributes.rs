//! Player-facing combat attribute aggregation.
//!
//! Combat modifiers enter through several independent systems (race, weapon
//! talents, hero gear, per-run cards, tower research, gems, and local auras).
//! This module derives one report from the same runtime data used by combat so
//! the HUD never has to reproduce those formulas with hand-maintained labels.

use crate::equipment::equipment_set_bonus;
use crate::hero::{HeroLoadout, HeroRunMods, hero_move_speed, make_hero_tower};
use crate::hero_gear::{self, HeroGearSlot};
use crate::meta::Talents;
use crate::tower::Tower;
use bevy::prelude::Vec2;

#[derive(Clone, Copy, Debug)]
pub struct HeroCombatValues {
    pub damage: f32,
    pub attack_speed: f32,
    pub range: f32,
    pub max_hp: f32,
    pub move_speed: f32,
    pub armor: f32,
    pub armor_pierce: f32,
    pub skill_power: f32,
    pub skill_cooldown: i32,
}

#[derive(Clone, Copy, Debug)]
pub struct AttributeContribution {
    pub damage_mult: f32,
    pub attack_speed_mult: f32,
    pub range_mult: f32,
    pub hp_mult: f32,
    pub move_mult: f32,
    pub skill_mult: f32,
    pub armor_add: f32,
    pub armor_pierce_add: f32,
    pub skill_cooldown_reduction: i32,
}

impl Default for AttributeContribution {
    fn default() -> Self {
        Self {
            damage_mult: 1.0,
            attack_speed_mult: 1.0,
            range_mult: 1.0,
            hp_mult: 1.0,
            move_mult: 1.0,
            skill_mult: 1.0,
            armor_add: 0.0,
            armor_pierce_add: 0.0,
            skill_cooldown_reduction: 0,
        }
    }
}

impl AttributeContribution {
    pub fn active(self) -> bool {
        [
            self.damage_mult,
            self.attack_speed_mult,
            self.range_mult,
            self.hp_mult,
            self.move_mult,
            self.skill_mult,
        ]
        .into_iter()
        .any(|value| (value - 1.0).abs() > 0.001)
            || self.armor_add.abs() > 0.01
            || self.armor_pierce_add.abs() > 0.01
            || self.skill_cooldown_reduction != 0
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct HeroConditionalBonuses {
    pub battlefield_damage_mult: f32,
    pub tower_aura_damage: f32,
    pub tower_aura_attack_speed: f32,
    pub tower_aura_range: f32,
    pub summon_power: f32,
    pub kill_gold: f32,
    pub regen_per_second: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct HeroAttributeReport {
    /// The selected race + weapon at level 1, with no talents, gear, or run cards.
    pub baseline: HeroCombatValues,
    pub current: HeroCombatValues,
    pub total: AttributeContribution,
    pub level: AttributeContribution,
    pub weapon_talents: AttributeContribution,
    pub gear: AttributeContribution,
    pub run_cards: AttributeContribution,
    pub conditional: HeroConditionalBonuses,
}

#[derive(Clone, Copy, Debug)]
pub struct TowerGlobalAttributeReport {
    pub permanent: AttributeContribution,
    pub run_cards: AttributeContribution,
    pub total: AttributeContribution,
}

#[derive(Clone, Copy, Debug)]
pub struct SelectedTowerAttributeReport {
    pub damage: f32,
    pub attack_speed: f32,
    pub range: f32,
    pub armor: f32,
    pub max_hp: f32,
    pub local_damage_mult: f32,
    pub local_attack_speed_mult: f32,
    pub local_range_mult: f32,
    pub synergy_add: f32,
    pub battle_focus_add: f32,
    pub aura_damage_add: f32,
    pub aura_attack_speed_add: f32,
}

pub fn hero_attribute_report(
    loadout: &HeroLoadout,
    battlefield_hero: Option<&Tower>,
) -> HeroAttributeReport {
    let mut initial = runtime_copy(loadout);
    initial.level = 1;
    initial.weapon_talents = [[0; HeroLoadout::TALENT_SLOTS]; crate::hero::HeroWeapon::ALL.len()];
    initial.gear = [None; HeroGearSlot::COUNT];
    initial.run_mods = HeroRunMods::default();

    let mut leveled = runtime_copy(&initial);
    leveled.level = loadout.level;

    let mut talented = runtime_copy(&leveled);
    talented.weapon_talents = loadout.weapon_talents;

    let mut geared = runtime_copy(&talented);
    geared.gear = loadout.gear;

    let baseline = hero_values(&initial);
    let level_values = hero_values(&leveled);
    let talent_values = hero_values(&talented);
    let gear_values = hero_values(&geared);
    let current = hero_values(loadout);

    let doc = loadout.weapon.doctrine();
    let gear_stats = hero_gear::gear_stats(&loadout.gear);
    let affinity = hero_gear::weapon_affinity_stats(&loadout.gear, loadout.weapon);
    let doctrine_scale = 1.0 + loadout.level.saturating_sub(1) as f32 * 0.03;
    let stable_damage = current.damage.max(1.0);
    let battlefield_damage = battlefield_hero
        .map(|tower| tower.damage)
        .unwrap_or(stable_damage);

    HeroAttributeReport {
        baseline,
        current,
        total: contribution_between(baseline, current),
        level: contribution_between(baseline, level_values),
        weapon_talents: contribution_between(level_values, talent_values),
        gear: contribution_between(talent_values, gear_values),
        run_cards: contribution_between(gear_values, current),
        conditional: HeroConditionalBonuses {
            battlefield_damage_mult: (battlefield_damage / stable_damage).max(0.0),
            tower_aura_damage: doc.aura_damage * doctrine_scale
                + loadout.run_mods.aura_damage_add
                + gear_stats.aura_damage_add
                + affinity.aura_damage_add,
            tower_aura_attack_speed: doc.aura_haste * doctrine_scale
                + gear_stats.tower_haste_add
                + affinity.tower_haste_add,
            tower_aura_range: doc.aura_range,
            summon_power: doc.summon_power * doctrine_scale
                + loadout.run_mods.summon_power_add
                + gear_stats.summon_power_add
                + affinity.summon_power_add,
            kill_gold: doc.gold_bonus + gear_stats.gold_bonus_add + affinity.gold_bonus_add,
            regen_per_second: doc.regen_pct * doctrine_scale,
        },
    }
}

pub fn tower_global_attribute_report(talents: &Talents) -> TowerGlobalAttributeReport {
    let permanent = tower_contribution(
        talents.damage_mult,
        talents.range_mult,
        talents.firerate_mult,
    );
    let run_cards = tower_contribution(
        talents.rogue_damage_mult,
        talents.rogue_range_mult,
        talents.rogue_firerate_mult,
    );
    let total = tower_contribution(
        talents.damage_mult * talents.rogue_damage_mult,
        talents.range_mult * talents.rogue_range_mult,
        talents.firerate_mult * talents.rogue_firerate_mult,
    );
    TowerGlobalAttributeReport {
        permanent,
        run_cards,
        total,
    }
}

pub fn selected_tower_attribute_report(tower: &Tower) -> SelectedTowerAttributeReport {
    let set = equipment_set_bonus(&tower.equipment);
    let attack_speed_mult = (1.0 + tower.aura_haste) * set.attack_speed_mult;
    let range_mult = (1.0 + tower.aura_range) * set.range_mult;
    let local_damage_mult =
        (1.0 + tower.synergy + tower.battle_focus + tower.aura_damage) * set.damage_mult;
    SelectedTowerAttributeReport {
        damage: tower.base_damage * local_damage_mult,
        attack_speed: attack_speed_mult / tower.cooldown.max(0.03),
        range: tower.range * range_mult,
        armor: tower.armor + set.armor_add,
        max_hp: tower.max_hp,
        local_damage_mult,
        local_attack_speed_mult: attack_speed_mult,
        local_range_mult: range_mult,
        synergy_add: tower.synergy,
        battle_focus_add: tower.battle_focus,
        aura_damage_add: tower.aura_damage,
        aura_attack_speed_add: tower.aura_haste,
    }
}

fn tower_contribution(
    damage_mult: f32,
    range_mult: f32,
    cooldown_mult: f32,
) -> AttributeContribution {
    AttributeContribution {
        damage_mult,
        attack_speed_mult: 1.0 / cooldown_mult.max(0.01),
        range_mult,
        ..Default::default()
    }
}

fn hero_values(loadout: &HeroLoadout) -> HeroCombatValues {
    let tower = make_hero_tower(loadout, Vec2::ZERO);
    HeroCombatValues {
        damage: tower.base_damage,
        attack_speed: 1.0 / tower.cooldown.max(0.01),
        range: tower.range,
        max_hp: tower.max_hp,
        move_speed: hero_move_speed(loadout),
        armor: tower.armor,
        armor_pierce: tower.armor_pierce,
        skill_power: loadout.skill_damage_mult(),
        skill_cooldown: loadout.skill_cooldown_max(),
    }
}

fn contribution_between(
    before: HeroCombatValues,
    after: HeroCombatValues,
) -> AttributeContribution {
    AttributeContribution {
        damage_mult: safe_ratio(after.damage, before.damage),
        attack_speed_mult: safe_ratio(after.attack_speed, before.attack_speed),
        range_mult: safe_ratio(after.range, before.range),
        hp_mult: safe_ratio(after.max_hp, before.max_hp),
        move_mult: safe_ratio(after.move_speed, before.move_speed),
        skill_mult: safe_ratio(after.skill_power, before.skill_power),
        armor_add: after.armor - before.armor,
        armor_pierce_add: after.armor_pierce - before.armor_pierce,
        skill_cooldown_reduction: before.skill_cooldown - after.skill_cooldown,
    }
}

fn safe_ratio(after: f32, before: f32) -> f32 {
    if before.abs() <= f32::EPSILON {
        1.0
    } else {
        after / before
    }
}

fn runtime_copy(source: &HeroLoadout) -> HeroLoadout {
    HeroLoadout {
        race: source.race,
        weapon: source.weapon,
        level: source.level,
        xp: source.xp,
        talent_points: source.talent_points,
        weapon_talents: source.weapon_talents,
        gear: source.gear,
        skill_cd: source.skill_cd,
        run_mods: source.run_mods,
        alive: source.alive,
        respawn_waves: source.respawn_waves,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hero::{HeroWeapon, Race};
    use crate::hero_gear::{HeroGear, empty_gear};

    fn loadout() -> HeroLoadout {
        HeroLoadout {
            race: Race::Human,
            weapon: HeroWeapon::BannerSword,
            level: 1,
            xp: 0,
            talent_points: 0,
            weapon_talents: [[0; HeroLoadout::TALENT_SLOTS]; HeroWeapon::ALL.len()],
            gear: empty_gear(),
            skill_cd: 0,
            run_mods: HeroRunMods::default(),
            alive: true,
            respawn_waves: 0,
        }
    }

    #[test]
    fn hero_report_separates_level_gear_and_run_cards() {
        let mut hero = loadout();
        hero.level = 10;
        hero.weapon_talents[0][0] = 2;
        hero.gear[HeroGearSlot::Armor.idx()] = Some(HeroGear::VowPlate);
        hero.run_mods.damage_mult = 1.25;
        hero.run_mods.cooldown_mult = 0.80;

        let report = hero_attribute_report(&hero, None);
        assert!(report.level.damage_mult > 1.30);
        assert!(report.weapon_talents.damage_mult > 1.20);
        assert!(report.gear.active());
        assert!(report.run_cards.damage_mult > 1.20);
        assert!(report.run_cards.attack_speed_mult > 1.20);
        assert!(report.total.damage_mult > report.run_cards.damage_mult);
    }

    #[test]
    fn tower_attack_speed_inverts_cooldown_multipliers() {
        let talents = Talents {
            damage_mult: 1.15,
            range_mult: 1.12,
            firerate_mult: 0.90,
            rogue_damage_mult: 1.08,
            rogue_range_mult: 1.05,
            rogue_firerate_mult: 0.90,
            dmg_lvl: 1,
            rng_lvl: 1,
            spd_lvl: 1,
        };
        let report = tower_global_attribute_report(&talents);
        assert!((report.total.damage_mult - 1.242).abs() < 0.001);
        assert!((report.total.range_mult - 1.176).abs() < 0.001);
        assert!((report.total.attack_speed_mult - 1.234_567_9).abs() < 0.001);
    }
}
