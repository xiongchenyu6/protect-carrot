//! Per-run roguelite build layer: after each cleared wave the player chooses one
//! of three talents drawn from race, current weapon, and common pools.

use crate::audio::Sound;
use crate::data::TowerKind;
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
    Tower,
}

impl TalentPool {
    pub fn label(self) -> &'static str {
        match self {
            TalentPool::Race => "种族",
            TalentPool::Weapon => "武器",
            TalentPool::Common => "公共",
            TalentPool::Tower => "防线",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
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
    ArrowVolley,
    CannonShockwave,
    ArcaneSurge,
    Frostbound,
    ScoutNetwork,
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
            RogueliteTalent::ArrowVolley
            | RogueliteTalent::CannonShockwave
            | RogueliteTalent::ArcaneSurge
            | RogueliteTalent::Frostbound
            | RogueliteTalent::ScoutNetwork => TalentPool::Tower,
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
            RogueliteTalent::ArrowVolley => "箭雨齐射".to_string(),
            RogueliteTalent::CannonShockwave => "震地炮膛".to_string(),
            RogueliteTalent::ArcaneSurge => "奥术过载".to_string(),
            RogueliteTalent::Frostbound => "永冻之痕".to_string(),
            RogueliteTalent::ScoutNetwork => "哨戒网络".to_string(),
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
            TalentPool::Tower => crate::i18n::tf(
                "{}池 · 塔系专精",
                &[&crate::i18n::t(TalentPool::Tower.label())],
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
                    "{}伤害与技能威力 +15%；{}",
                    &[
                        &crate::i18n::t(loadout.weapon_kind().name()),
                        &crate::i18n::t(extra),
                    ],
                )
            }
            RogueliteTalent::WeaponTempo => crate::i18n::tf(
                "{}攻速 +10%、移速 +8%、技能冷却 -10%",
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
            RogueliteTalent::ArrowVolley => {
                crate::i18n::t("箭塔伤害 +25%、射程 +12%；现有箭塔立刻齐射")
            }
            RogueliteTalent::CannonShockwave => {
                crate::i18n::t("炮塔爆炸范围 +25%、攻速 +10%；现有炮塔震地蓄能")
            }
            RogueliteTalent::ArcaneSurge => {
                crate::i18n::t("魔法塔伤害 +25%、攻速 +10%；奥术回路立即过载")
            }
            RogueliteTalent::Frostbound => {
                crate::i18n::t("冰塔伤害 +20%、减速持续 +40%；冻结力场即时扩散")
            }
            RogueliteTalent::ScoutNetwork => {
                crate::i18n::t("侦测塔射程 +25%；全场侦测网络即时上线")
            }
        }
    }

    /// Color and one-shot sound used for both the selected card and every tower
    /// it changed. This makes the result readable before the next enemy arrives.
    pub fn feedback(self) -> (Color, Sound) {
        match self {
            RogueliteTalent::ArrowVolley => (Color::srgb(1.0, 0.36, 0.20), Sound::Combo),
            RogueliteTalent::CannonShockwave => (Color::srgb(1.0, 0.58, 0.16), Sound::Explosion),
            RogueliteTalent::ArcaneSurge => (Color::srgb(0.72, 0.42, 1.0), Sound::Laser),
            RogueliteTalent::Frostbound => (Color::srgb(0.34, 0.84, 1.0), Sound::Freeze),
            RogueliteTalent::ScoutNetwork => (Color::srgb(0.58, 0.70, 1.0), Sound::Chain),
            RogueliteTalent::HumanLogistics | RogueliteTalent::CarrotDividend => {
                (Color::srgb(1.0, 0.80, 0.25), Sound::Gold)
            }
            RogueliteTalent::WeaponMastery
            | RogueliteTalent::WeaponTempo
            | RogueliteTalent::WeaponSignature => (Color::srgb(0.96, 0.42, 0.62), Sound::Combo),
            RogueliteTalent::ElfMoonstep | RogueliteTalent::ElfForestSight => {
                (Color::srgb(0.42, 0.92, 0.72), Sound::Summon)
            }
            RogueliteTalent::OrcBloodrage | RogueliteTalent::OrcWarDrum => {
                (Color::srgb(1.0, 0.27, 0.20), Sound::Boss)
            }
            _ => (Color::srgb(0.86, 0.95, 0.48), Sound::Upgrade),
        }
    }

    /// Returns whether a deployed tower should visibly acknowledge this card.
    pub fn affects_tower(self, tower: &Tower) -> bool {
        if tower.hero {
            return matches!(self.pool(), TalentPool::Race | TalentPool::Weapon);
        }
        match self {
            RogueliteTalent::ArrowVolley => tower.kind == TowerKind::Arrow,
            RogueliteTalent::CannonShockwave => tower.kind == TowerKind::Cannon,
            RogueliteTalent::ArcaneSurge => tower.kind == TowerKind::Magic,
            RogueliteTalent::Frostbound => tower.kind == TowerKind::Ice,
            RogueliteTalent::ScoutNetwork => tower.kind == TowerKind::Detection,
            // 与 apply() 保持一致：只有真正改了塔数值的卡才让塔应答，
            // 纯英雄卡/纯金币卡不点亮塔，免得玩家误以为塔也被强化了。
            RogueliteTalent::HumanFormation
            | RogueliteTalent::HumanLogistics
            | RogueliteTalent::ElfForestSight
            | RogueliteTalent::TowerOverclock
            | RogueliteTalent::GemResonance => true,
            _ => false,
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
                loadout.run_mods.skill_power_mult *= 1.15;
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
                loadout.run_mods.skill_interval_mult *= 0.90;
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
            RogueliteTalent::ArrowVolley => {
                talents.rogue_arrow_damage_mult *= 1.25;
                talents.rogue_arrow_range_mult *= 1.12;
                apply_tower_kind_mods(towers, TowerKind::Arrow, 1.25, 1.12, 1.0, 1.0, 1.0);
            }
            RogueliteTalent::CannonShockwave => {
                talents.rogue_cannon_radius_mult *= 1.25;
                talents.rogue_cannon_firerate_mult *= 0.90;
                apply_tower_kind_mods(towers, TowerKind::Cannon, 1.0, 1.0, 0.90, 1.25, 1.0);
            }
            RogueliteTalent::ArcaneSurge => {
                talents.rogue_magic_damage_mult *= 1.25;
                talents.rogue_magic_firerate_mult *= 0.90;
                apply_tower_kind_mods(towers, TowerKind::Magic, 1.25, 1.0, 0.90, 1.0, 1.0);
            }
            RogueliteTalent::Frostbound => {
                talents.rogue_ice_damage_mult *= 1.20;
                talents.rogue_ice_slow_mult *= 1.40;
                apply_tower_kind_mods(towers, TowerKind::Ice, 1.20, 1.0, 1.0, 1.0, 1.40);
            }
            RogueliteTalent::ScoutNetwork => {
                talents.rogue_detection_range_mult *= 1.25;
                apply_tower_kind_mods(towers, TowerKind::Detection, 1.0, 1.25, 1.0, 1.0, 1.0);
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

/// 塔系专精卡：只作用于指定族系的已部署塔；之后新建的同族塔由
/// [`apply_tower_special_mods`] 按 `Talents` 里累计的乘区补上。
fn apply_tower_kind_mods(
    towers: &mut Query<(Entity, &mut Tower)>,
    kind: TowerKind,
    damage: f32,
    range: f32,
    cooldown: f32,
    radius: f32,
    slow: f32,
) {
    for (_, mut tower) in towers.iter_mut() {
        if !tower.hero && tower.kind == kind {
            scale_tower(&mut tower, damage, range, cooldown, radius, slow);
        }
    }
}

/// 新建塔继承本局已拿到的塔系专精加成（在 `spawn_tower` 里调用）。
pub fn apply_tower_special_mods(tower: &mut Tower, talents: &Talents) {
    if tower.hero {
        return;
    }
    let (damage, range, cooldown, radius, slow) = match tower.kind {
        TowerKind::Arrow => (
            talents.rogue_arrow_damage_mult,
            talents.rogue_arrow_range_mult,
            1.0,
            1.0,
            1.0,
        ),
        TowerKind::Cannon => (
            1.0,
            1.0,
            talents.rogue_cannon_firerate_mult,
            talents.rogue_cannon_radius_mult,
            1.0,
        ),
        TowerKind::Magic => (
            talents.rogue_magic_damage_mult,
            1.0,
            talents.rogue_magic_firerate_mult,
            1.0,
            1.0,
        ),
        TowerKind::Ice => (
            talents.rogue_ice_damage_mult,
            1.0,
            1.0,
            1.0,
            talents.rogue_ice_slow_mult,
        ),
        TowerKind::Detection => (1.0, talents.rogue_detection_range_mult, 1.0, 1.0, 1.0),
        _ => return,
    };
    scale_tower(tower, damage, range, cooldown, radius, slow);
}

fn scale_tower(tower: &mut Tower, damage: f32, range: f32, cooldown: f32, radius: f32, slow: f32) {
    tower.base_damage = (tower.base_damage * damage).floor().max(1.0);
    tower.damage = tower.base_damage;
    tower.range = (tower.range * range).floor().max(1.0);
    tower.cooldown = (tower.cooldown * cooldown).max(0.03);
    tower.aoe_radius *= radius;
    tower.slow_duration *= slow;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tower(kind: TowerKind) -> Tower {
        Tower::from_def(kind.def(), 0, 0)
    }

    #[test]
    fn new_tower_inherits_its_family_card_bonus() {
        let talents = Talents {
            rogue_arrow_damage_mult: 1.25,
            rogue_arrow_range_mult: 1.12,
            ..Default::default()
        };
        let base = tower(TowerKind::Arrow);
        let mut boosted = tower(TowerKind::Arrow);
        apply_tower_special_mods(&mut boosted, &talents);
        assert!(boosted.base_damage > base.base_damage);
        assert!(boosted.range > base.range);
    }

    #[test]
    fn family_card_bonus_does_not_leak_to_other_towers() {
        let talents = Talents {
            rogue_arrow_damage_mult: 1.25,
            ..Default::default()
        };
        let base = tower(TowerKind::Cannon);
        let mut cannon = tower(TowerKind::Cannon);
        apply_tower_special_mods(&mut cannon, &talents);
        assert_eq!(cannon.base_damage, base.base_damage);
    }

    #[test]
    fn hero_only_card_does_not_light_up_towers() {
        assert!(!RogueliteTalent::OrcBloodrage.affects_tower(&tower(TowerKind::Arrow)));
        assert!(RogueliteTalent::TowerOverclock.affects_tower(&tower(TowerKind::Arrow)));
    }

    #[test]
    fn family_card_feedback_targets_only_its_family() {
        assert!(RogueliteTalent::ArrowVolley.affects_tower(&tower(TowerKind::Arrow)));
        assert!(!RogueliteTalent::ArrowVolley.affects_tower(&tower(TowerKind::Cannon)));
    }
}
