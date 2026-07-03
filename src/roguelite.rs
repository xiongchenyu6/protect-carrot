//! Per-run roguelite build layer: after each cleared wave the player chooses one
//! of three talents drawn from race, current weapon, and common pools.

use crate::game::{Rng, RunState};
use crate::hero::{HeroLoadout, HeroWeapon, Race};
use crate::meta::Talents;
use crate::tower::Tower;
use bevy::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum TalentPool {
    Race,
    Weapon,
    Common,
}

impl TalentPool {
    pub fn label(self) -> &'static str {
        match self {
            TalentPool::Race => "种族",
            TalentPool::Weapon => "武器",
            TalentPool::Common => "公共",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RogueliteTalent {
    HumanFormation,
    HumanLogistics,
    ElfMoonstep,
    ElfForestSight,
    OrcBloodrage,
    OrcWarDrum,
    WeaponMastery,
    WeaponTempo,
    WeaponSignature,
    TowerOverclock,
    GemResonance,
    CarrotDividend,
}

impl RogueliteTalent {
    pub fn pool(self) -> TalentPool {
        match self {
            RogueliteTalent::HumanFormation
            | RogueliteTalent::HumanLogistics
            | RogueliteTalent::ElfMoonstep
            | RogueliteTalent::ElfForestSight
            | RogueliteTalent::OrcBloodrage
            | RogueliteTalent::OrcWarDrum => TalentPool::Race,
            RogueliteTalent::WeaponMastery
            | RogueliteTalent::WeaponTempo
            | RogueliteTalent::WeaponSignature => TalentPool::Weapon,
            RogueliteTalent::TowerOverclock
            | RogueliteTalent::GemResonance
            | RogueliteTalent::CarrotDividend => TalentPool::Common,
        }
    }

    pub fn name(self, loadout: &HeroLoadout) -> String {
        match self {
            RogueliteTalent::HumanFormation => "协同阵线".to_string(),
            RogueliteTalent::HumanLogistics => "王国补给".to_string(),
            RogueliteTalent::ElfMoonstep => "月影步法".to_string(),
            RogueliteTalent::ElfForestSight => "林地感知".to_string(),
            RogueliteTalent::OrcBloodrage => "血怒突袭".to_string(),
            RogueliteTalent::OrcWarDrum => "战鼓号令".to_string(),
            RogueliteTalent::WeaponMastery => {
                crate::i18n::tf("{}精通", &[&crate::i18n::t(loadout.weapon_kind().name())])
            }
            RogueliteTalent::WeaponTempo => {
                crate::i18n::tf("{}节奏", &[&crate::i18n::t(loadout.weapon_kind().name())])
            }
            RogueliteTalent::WeaponSignature => {
                crate::i18n::tf("{}秘技", &[&crate::i18n::t(loadout.weapon_kind().name())])
            }
            RogueliteTalent::TowerOverclock => "防线超频".to_string(),
            RogueliteTalent::GemResonance => "宝石共振".to_string(),
            RogueliteTalent::CarrotDividend => "萝卜分红".to_string(),
        }
    }

    pub fn source(self, loadout: &HeroLoadout) -> String {
        match self.pool() {
            TalentPool::Race => crate::i18n::tf(
                "{}池 · {}",
                &[
                    &crate::i18n::t(TalentPool::Race.label()),
                    &crate::i18n::t(loadout.race.name()),
                ],
            ),
            TalentPool::Weapon => crate::i18n::tf(
                "{}池 · {}",
                &[
                    &crate::i18n::t(TalentPool::Weapon.label()),
                    &crate::i18n::t(loadout.weapon_kind().name()),
                ],
            ),
            TalentPool::Common => crate::i18n::tf(
                "{}池 · 全局构筑",
                &[&crate::i18n::t(TalentPool::Common.label())],
            ),
        }
    }

    pub fn desc(self, loadout: &HeroLoadout, wave: i32) -> String {
        match self {
            RogueliteTalent::HumanFormation => crate::i18n::t("英雄伤害 +10%；所有防御塔伤害 +5%"),
            RogueliteTalent::HumanLogistics => {
                crate::i18n::t("立即获得补给金币；所有防御塔攻速 +5%")
            }
            RogueliteTalent::ElfMoonstep => crate::i18n::t("英雄射程 +12%、移速 +15%、攻速 +5%"),
            RogueliteTalent::ElfForestSight => crate::i18n::t("英雄射程 +10%；所有防御塔射程 +4%"),
            RogueliteTalent::OrcBloodrage => crate::i18n::t("英雄伤害 +18%、生命 +10%、攻速 +4%"),
            RogueliteTalent::OrcWarDrum => {
                crate::i18n::t("英雄光环强化：周围塔伤害 +6%、召唤物强度 +10%")
            }
            RogueliteTalent::WeaponMastery => {
                let extra = if weapon_is_melee(loadout.weapon) {
                    "近战额外获得生命 +8%"
                } else {
                    "远程额外获得射程 +8%"
                };
                crate::i18n::tf(
                    "{}伤害 +15%；{}",
                    &[
                        &crate::i18n::t(loadout.weapon_kind().name()),
                        &crate::i18n::t(extra),
                    ],
                )
            }
            RogueliteTalent::WeaponTempo => crate::i18n::tf(
                "{}攻速 +10%、移速 +8%",
                &[&crate::i18n::t(loadout.weapon_kind().name())],
            ),
            RogueliteTalent::WeaponSignature => signature_desc(loadout.weapon),
            RogueliteTalent::TowerOverclock => {
                crate::i18n::t("所有防御塔攻速 +10%，已建塔和新建塔都生效")
            }
            RogueliteTalent::GemResonance => {
                crate::i18n::t("所有防御塔伤害 +8%、射程 +5%，强化宝石构筑路线")
            }
            RogueliteTalent::CarrotDividend => crate::i18n::tf(
                "立即获得 {} 金；保留给下一波部署窗口",
                &[&(70 + wave * 6).to_string()],
            ),
        }
    }

    fn apply(
        self,
        wave: i32,
        loadout: &mut HeroLoadout,
        talents: &mut Talents,
        run: &mut RunState,
        towers: &mut Query<(Entity, &mut Tower)>,
    ) {
        match self {
            RogueliteTalent::HumanFormation => {
                loadout.run_mods.damage_mult *= 1.10;
                apply_tower_damage(talents, towers, 1.05);
            }
            RogueliteTalent::HumanLogistics => {
                run.gold += 55 + wave * 5;
                apply_tower_cooldown(talents, towers, 0.95);
            }
            RogueliteTalent::ElfMoonstep => {
                loadout.run_mods.range_mult *= 1.12;
                loadout.run_mods.move_mult *= 1.15;
                loadout.run_mods.cooldown_mult *= 0.95;
            }
            RogueliteTalent::ElfForestSight => {
                loadout.run_mods.range_mult *= 1.10;
                apply_tower_range(talents, towers, 1.04);
            }
            RogueliteTalent::OrcBloodrage => {
                loadout.run_mods.damage_mult *= 1.18;
                loadout.run_mods.hp_mult *= 1.10;
                loadout.run_mods.cooldown_mult *= 0.96;
            }
            RogueliteTalent::OrcWarDrum => {
                loadout.run_mods.aura_damage_add += 0.06;
                loadout.run_mods.summon_power_add += 0.10;
            }
            RogueliteTalent::WeaponMastery => {
                loadout.run_mods.damage_mult *= 1.15;
                if weapon_is_melee(loadout.weapon) {
                    loadout.run_mods.hp_mult *= 1.08;
                    loadout.run_mods.armor_add += 3.0;
                } else {
                    loadout.run_mods.range_mult *= 1.08;
                }
            }
            RogueliteTalent::WeaponTempo => {
                loadout.run_mods.cooldown_mult *= 0.90;
                loadout.run_mods.move_mult *= 1.08;
            }
            RogueliteTalent::WeaponSignature => apply_signature(loadout),
            RogueliteTalent::TowerOverclock => apply_tower_cooldown(talents, towers, 0.90),
            RogueliteTalent::GemResonance => {
                apply_tower_damage(talents, towers, 1.08);
                apply_tower_range(talents, towers, 1.05);
            }
            RogueliteTalent::CarrotDividend => {
                run.gold += 70 + wave * 6;
            }
        }
        reapply_hero_towers(loadout, towers);
    }
}

#[derive(Clone)]
pub struct RogueliteDraft {
    pub wave: i32,
    pub choices: [RogueliteTalent; 3],
}

#[derive(Resource, Default)]
pub struct RogueliteRun {
    pub draft: Option<RogueliteDraft>,
    pub picked: Vec<RogueliteTalent>,
}

impl RogueliteRun {
    pub fn reset(&mut self) {
        self.draft = None;
        self.picked.clear();
    }

    pub fn is_waiting(&self) -> bool {
        self.draft.is_some()
    }

    pub fn offer_wave_draft(&mut self, loadout: &HeroLoadout, wave: i32, rng: &mut Rng) -> bool {
        if wave <= 0 || self.draft.is_some() {
            return false;
        }
        self.draft = Some(RogueliteDraft {
            wave,
            choices: draft_choices(loadout, rng),
        });
        true
    }

    pub fn pick(
        &mut self,
        index: usize,
        loadout: &mut HeroLoadout,
        talents: &mut Talents,
        run: &mut RunState,
        towers: &mut Query<(Entity, &mut Tower)>,
    ) -> Option<RogueliteTalent> {
        let draft = self.draft.take()?;
        let picked = *draft.choices.get(index)?;
        picked.apply(draft.wave, loadout, talents, run, towers);
        self.picked.push(picked);
        Some(picked)
    }
}

pub fn reset_run(
    mut roguelite: ResMut<RogueliteRun>,
    mut loadout: ResMut<HeroLoadout>,
    mut talents: ResMut<Talents>,
) {
    roguelite.reset();
    loadout.run_mods = Default::default();
    talents.rogue_damage_mult = 1.0;
    talents.rogue_range_mult = 1.0;
    talents.rogue_firerate_mult = 1.0;
}

fn draft_choices(loadout: &HeroLoadout, rng: &mut Rng) -> [RogueliteTalent; 3] {
    let race_pool: &[RogueliteTalent] = match loadout.race {
        Race::Human => &[
            RogueliteTalent::HumanFormation,
            RogueliteTalent::HumanLogistics,
        ],
        Race::Elf => &[
            RogueliteTalent::ElfMoonstep,
            RogueliteTalent::ElfForestSight,
        ],
        Race::Orc => &[RogueliteTalent::OrcBloodrage, RogueliteTalent::OrcWarDrum],
    };
    let weapon_pool = [
        RogueliteTalent::WeaponMastery,
        RogueliteTalent::WeaponTempo,
        RogueliteTalent::WeaponSignature,
    ];
    let common_pool = [
        RogueliteTalent::TowerOverclock,
        RogueliteTalent::GemResonance,
        RogueliteTalent::CarrotDividend,
    ];
    [
        race_pool[rng.range(race_pool.len())],
        weapon_pool[rng.range(weapon_pool.len())],
        common_pool[rng.range(common_pool.len())],
    ]
}

fn weapon_is_melee(weapon: HeroWeapon) -> bool {
    matches!(
        weapon,
        HeroWeapon::BannerSword
            | HeroWeapon::OathShield
            | HeroWeapon::NightDagger
            | HeroWeapon::ForgeHammer
    )
}

fn signature_desc(weapon: HeroWeapon) -> String {
    match weapon {
        HeroWeapon::BannerSword
        | HeroWeapon::OathShield
        | HeroWeapon::NightDagger
        | HeroWeapon::ForgeHammer => crate::i18n::t("近战秘技：生命 +12%、护甲 +4、伤害 +8%"),
        HeroWeapon::StarfireStaff
        | HeroWeapon::ShadowBow
        | HeroWeapon::StormOrb
        | HeroWeapon::SentryCrossbow => crate::i18n::t("远程秘技：射程 +10%、伤害 +8%、攻速 +4%"),
        HeroWeapon::SummonStaff => crate::i18n::t("召唤秘技：神话眷属强度 +15%、光环伤害 +5%"),
    }
}

fn apply_signature(loadout: &mut HeroLoadout) {
    match loadout.weapon {
        HeroWeapon::BannerSword
        | HeroWeapon::OathShield
        | HeroWeapon::NightDagger
        | HeroWeapon::ForgeHammer => {
            loadout.run_mods.hp_mult *= 1.12;
            loadout.run_mods.armor_add += 4.0;
            loadout.run_mods.damage_mult *= 1.08;
        }
        HeroWeapon::StarfireStaff
        | HeroWeapon::ShadowBow
        | HeroWeapon::StormOrb
        | HeroWeapon::SentryCrossbow => {
            loadout.run_mods.range_mult *= 1.10;
            loadout.run_mods.damage_mult *= 1.08;
            loadout.run_mods.cooldown_mult *= 0.96;
        }
        HeroWeapon::SummonStaff => {
            loadout.run_mods.summon_power_add += 0.15;
            loadout.run_mods.aura_damage_add += 0.05;
        }
    }
}

fn apply_tower_damage(talents: &mut Talents, towers: &mut Query<(Entity, &mut Tower)>, mult: f32) {
    talents.rogue_damage_mult *= mult;
    for (_, mut tower) in towers.iter_mut() {
        if !tower.hero {
            tower.base_damage = (tower.base_damage * mult).floor().max(1.0);
            tower.damage = tower.base_damage;
        }
    }
}

fn apply_tower_range(talents: &mut Talents, towers: &mut Query<(Entity, &mut Tower)>, mult: f32) {
    talents.rogue_range_mult *= mult;
    for (_, mut tower) in towers.iter_mut() {
        if !tower.hero {
            tower.range = (tower.range * mult).floor().max(1.0);
        }
    }
}

fn apply_tower_cooldown(
    talents: &mut Talents,
    towers: &mut Query<(Entity, &mut Tower)>,
    mult: f32,
) {
    talents.rogue_firerate_mult *= mult;
    for (_, mut tower) in towers.iter_mut() {
        if !tower.hero {
            tower.cooldown = (tower.cooldown * mult).max(0.03);
        }
    }
}

fn reapply_hero_towers(loadout: &HeroLoadout, towers: &mut Query<(Entity, &mut Tower)>) {
    for (_, mut tower) in towers.iter_mut() {
        if tower.hero {
            crate::hero::apply_loadout_to_tower(loadout, &mut tower);
        }
    }
}
