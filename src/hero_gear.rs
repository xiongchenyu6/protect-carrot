//! Hero-only equipment and paperdoll metadata.
//!
//! Tower relics stay in `equipment.rs`. Hero gear is deliberately separate so the
//! hero no longer inherits tower socket rules, tower resonance, or tower item
//! stat tuning.

use bevy::prelude::*;

use crate::equipment::Rarity;
use crate::hero::HeroWeapon;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum HeroGearSlot {
    Armor,
    Charm,
    Relic,
    Boots,
}

impl HeroGearSlot {
    pub const ALL: [HeroGearSlot; 4] = [
        HeroGearSlot::Armor,
        HeroGearSlot::Charm,
        HeroGearSlot::Relic,
        HeroGearSlot::Boots,
    ];
    pub const COUNT: usize = Self::ALL.len();

    pub fn idx(self) -> usize {
        match self {
            HeroGearSlot::Armor => 0,
            HeroGearSlot::Charm => 1,
            HeroGearSlot::Relic => 2,
            HeroGearSlot::Boots => 3,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            HeroGearSlot::Armor => "战衣",
            HeroGearSlot::Charm => "护符",
            HeroGearSlot::Relic => "圣物",
            HeroGearSlot::Boots => "靴履",
        }
    }

    pub fn paperdoll_slot(self) -> u32 {
        match self {
            HeroGearSlot::Armor => 20,
            HeroGearSlot::Charm => 21,
            HeroGearSlot::Relic => 22,
            HeroGearSlot::Boots => 23,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum HeroWeaponKind {
    Sword,
    Staff,
    Bow,
    Shield,
    StormOrb,
    SentryBow,
    Dagger,
    SummonStaff,
    Hammer,
}

impl HeroWeaponKind {
    pub fn for_weapon(weapon: HeroWeapon) -> Self {
        match weapon {
            HeroWeapon::BannerSword => HeroWeaponKind::Sword,
            HeroWeapon::StarfireStaff => HeroWeaponKind::Staff,
            HeroWeapon::ShadowBow => HeroWeaponKind::Bow,
            HeroWeapon::OathShield => HeroWeaponKind::Shield,
            HeroWeapon::StormOrb => HeroWeaponKind::StormOrb,
            HeroWeapon::SentryCrossbow => HeroWeaponKind::SentryBow,
            HeroWeapon::NightDagger => HeroWeaponKind::Dagger,
            HeroWeapon::SummonStaff => HeroWeaponKind::SummonStaff,
            HeroWeapon::ForgeHammer => HeroWeaponKind::Hammer,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            HeroWeaponKind::Sword => "战旗长剑",
            HeroWeaponKind::Staff => "星火法杖",
            HeroWeaponKind::Bow => "猎影长弓",
            HeroWeaponKind::Shield => "誓约盾锤",
            HeroWeaponKind::StormOrb => "雷暴法器",
            HeroWeaponKind::SentryBow => "哨戒弩",
            HeroWeaponKind::Dagger => "夜刃匕首",
            HeroWeaponKind::SummonStaff => "召唤法杖",
            HeroWeaponKind::Hammer => "工匠战锤",
        }
    }

    pub fn paperdoll_fragment(self) -> u32 {
        match self {
            HeroWeaponKind::Sword => 100,
            HeroWeaponKind::Staff => 101,
            HeroWeaponKind::Bow => 102,
            HeroWeaponKind::Shield => 103,
            HeroWeaponKind::StormOrb => 104,
            HeroWeaponKind::SentryBow => 105,
            HeroWeaponKind::Dagger => 106,
            HeroWeaponKind::SummonStaff => 107,
            HeroWeaponKind::Hammer => 108,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum HeroGear {
    VowPlate,
    StarweaveRobe,
    WindrunnerCloak,
    WildhideHarness,
    MoonthreadVest,
    NullMantle,
    ThunderCharm,
    SaintBell,
    NightMask,
    BloodBanner,
    EmberPrayer,
    ClockworkBadge,
    ForgeGauntlet,
    SentryScope,
    CarrotHalo,
    RiftIdol,
    BrassCompass,
    DragonheartCrown,
    WayfarerBoots,
    BloodstepGreaves,
    StarpathSandals,
    EngineerTreads,
    SummonerGreaves,
    CarrotWings,
    WarflagTabard,
    MeteorCodex,
    BountyQuiver,
    CitadelSeal,
    TempestCore,
    WatchtowerGreaves,
    AssassinWraps,
    MythcallerTotem,
    GolemBlueprint,
}

impl HeroGear {
    pub const ALL: [HeroGear; 33] = [
        HeroGear::VowPlate,
        HeroGear::StarweaveRobe,
        HeroGear::WindrunnerCloak,
        HeroGear::ThunderCharm,
        HeroGear::SaintBell,
        HeroGear::NightMask,
        HeroGear::ForgeGauntlet,
        HeroGear::SentryScope,
        HeroGear::CarrotHalo,
        HeroGear::WildhideHarness,
        HeroGear::MoonthreadVest,
        HeroGear::NullMantle,
        HeroGear::BloodBanner,
        HeroGear::EmberPrayer,
        HeroGear::ClockworkBadge,
        HeroGear::RiftIdol,
        HeroGear::BrassCompass,
        HeroGear::DragonheartCrown,
        HeroGear::WayfarerBoots,
        HeroGear::BloodstepGreaves,
        HeroGear::StarpathSandals,
        HeroGear::EngineerTreads,
        HeroGear::SummonerGreaves,
        HeroGear::CarrotWings,
        HeroGear::WarflagTabard,
        HeroGear::MeteorCodex,
        HeroGear::BountyQuiver,
        HeroGear::CitadelSeal,
        HeroGear::TempestCore,
        HeroGear::WatchtowerGreaves,
        HeroGear::AssassinWraps,
        HeroGear::MythcallerTotem,
        HeroGear::GolemBlueprint,
    ];

    pub fn idx(self) -> usize {
        match self {
            HeroGear::VowPlate => 0,
            HeroGear::StarweaveRobe => 1,
            HeroGear::WindrunnerCloak => 2,
            HeroGear::ThunderCharm => 3,
            HeroGear::SaintBell => 4,
            HeroGear::NightMask => 5,
            HeroGear::ForgeGauntlet => 6,
            HeroGear::SentryScope => 7,
            HeroGear::CarrotHalo => 8,
            HeroGear::WildhideHarness => 9,
            HeroGear::MoonthreadVest => 10,
            HeroGear::NullMantle => 11,
            HeroGear::BloodBanner => 12,
            HeroGear::EmberPrayer => 13,
            HeroGear::ClockworkBadge => 14,
            HeroGear::RiftIdol => 15,
            HeroGear::BrassCompass => 16,
            HeroGear::DragonheartCrown => 17,
            HeroGear::WayfarerBoots => 18,
            HeroGear::BloodstepGreaves => 19,
            HeroGear::StarpathSandals => 20,
            HeroGear::EngineerTreads => 21,
            HeroGear::SummonerGreaves => 22,
            HeroGear::CarrotWings => 23,
            HeroGear::WarflagTabard => 24,
            HeroGear::MeteorCodex => 25,
            HeroGear::BountyQuiver => 26,
            HeroGear::CitadelSeal => 27,
            HeroGear::TempestCore => 28,
            HeroGear::WatchtowerGreaves => 29,
            HeroGear::AssassinWraps => 30,
            HeroGear::MythcallerTotem => 31,
            HeroGear::GolemBlueprint => 32,
        }
    }

    pub fn from_idx(idx: usize) -> Option<Self> {
        Self::ALL.get(idx).copied()
    }

    pub fn def(self) -> &'static HeroGearDef {
        &HERO_GEAR_DEFS[self.idx()]
    }

    pub fn affinity_weapons_label(self) -> &'static str {
        match self {
            HeroGear::VowPlate => "战旗长剑 / 誓约盾锤",
            HeroGear::StarweaveRobe => "星火法杖 / 雷暴法器 / 召唤法杖",
            HeroGear::WindrunnerCloak => "猎影长弓 / 哨戒弩 / 夜刃匕首",
            HeroGear::WildhideHarness => "战旗长剑 / 夜刃匕首 / 工匠战锤",
            HeroGear::MoonthreadVest => "星火法杖 / 猎影长弓 / 哨戒弩 / 雷暴法器",
            HeroGear::NullMantle => "召唤法杖 / 雷暴法器 / 夜刃匕首",
            HeroGear::ThunderCharm => "雷暴法器 / 星火法杖",
            HeroGear::SaintBell => "召唤法杖 / 誓约盾锤",
            HeroGear::NightMask => "夜刃匕首 / 猎影长弓",
            HeroGear::BloodBanner => "战旗长剑 / 誓约盾锤 / 猎影长弓",
            HeroGear::EmberPrayer => "星火法杖 / 雷暴法器",
            HeroGear::ClockworkBadge => "工匠战锤 / 哨戒弩",
            HeroGear::ForgeGauntlet => "工匠战锤 / 誓约盾锤 / 战旗长剑",
            HeroGear::SentryScope => "哨戒弩 / 猎影长弓 / 星火法杖",
            HeroGear::CarrotHalo => "全部武器",
            HeroGear::RiftIdol => "召唤法杖 / 雷暴法器",
            HeroGear::BrassCompass => "猎影长弓 / 哨戒弩 / 工匠战锤",
            HeroGear::DragonheartCrown => "战旗长剑 / 誓约盾锤 / 召唤法杖",
            HeroGear::WayfarerBoots => "全部武器",
            HeroGear::BloodstepGreaves => "夜刃匕首 / 战旗长剑 / 工匠战锤",
            HeroGear::StarpathSandals => "星火法杖 / 雷暴法器 / 召唤法杖",
            HeroGear::EngineerTreads => "工匠战锤 / 哨戒弩 / 誓约盾锤",
            HeroGear::SummonerGreaves => "召唤法杖",
            HeroGear::CarrotWings => "全部武器",
            HeroGear::WarflagTabard => "战旗长剑",
            HeroGear::MeteorCodex => "星火法杖",
            HeroGear::BountyQuiver => "猎影长弓 / 哨戒弩",
            HeroGear::CitadelSeal => "誓约盾锤 / 战旗长剑",
            HeroGear::TempestCore => "雷暴法器 / 星火法杖",
            HeroGear::WatchtowerGreaves => "哨戒弩 / 誓约盾锤",
            HeroGear::AssassinWraps => "夜刃匕首 / 猎影长弓",
            HeroGear::MythcallerTotem => "召唤法杖",
            HeroGear::GolemBlueprint => "工匠战锤",
        }
    }

    pub fn has_weapon_affinity(self, weapon: HeroWeapon) -> bool {
        match self {
            HeroGear::VowPlate => {
                matches!(weapon, HeroWeapon::BannerSword | HeroWeapon::OathShield)
            }
            HeroGear::StarweaveRobe => matches!(
                weapon,
                HeroWeapon::StarfireStaff | HeroWeapon::StormOrb | HeroWeapon::SummonStaff
            ),
            HeroGear::WindrunnerCloak => matches!(
                weapon,
                HeroWeapon::ShadowBow | HeroWeapon::SentryCrossbow | HeroWeapon::NightDagger
            ),
            HeroGear::WildhideHarness => matches!(
                weapon,
                HeroWeapon::BannerSword | HeroWeapon::NightDagger | HeroWeapon::ForgeHammer
            ),
            HeroGear::MoonthreadVest => matches!(
                weapon,
                HeroWeapon::StarfireStaff
                    | HeroWeapon::ShadowBow
                    | HeroWeapon::SentryCrossbow
                    | HeroWeapon::StormOrb
            ),
            HeroGear::NullMantle => matches!(
                weapon,
                HeroWeapon::SummonStaff | HeroWeapon::StormOrb | HeroWeapon::NightDagger
            ),
            HeroGear::ThunderCharm => {
                matches!(weapon, HeroWeapon::StormOrb | HeroWeapon::StarfireStaff)
            }
            HeroGear::SaintBell => {
                matches!(weapon, HeroWeapon::SummonStaff | HeroWeapon::OathShield)
            }
            HeroGear::NightMask => {
                matches!(weapon, HeroWeapon::NightDagger | HeroWeapon::ShadowBow)
            }
            HeroGear::BloodBanner => matches!(
                weapon,
                HeroWeapon::BannerSword | HeroWeapon::OathShield | HeroWeapon::ShadowBow
            ),
            HeroGear::EmberPrayer => {
                matches!(weapon, HeroWeapon::StarfireStaff | HeroWeapon::StormOrb)
            }
            HeroGear::ClockworkBadge => {
                matches!(weapon, HeroWeapon::ForgeHammer | HeroWeapon::SentryCrossbow)
            }
            HeroGear::ForgeGauntlet => matches!(
                weapon,
                HeroWeapon::ForgeHammer | HeroWeapon::OathShield | HeroWeapon::BannerSword
            ),
            HeroGear::SentryScope => matches!(
                weapon,
                HeroWeapon::SentryCrossbow | HeroWeapon::ShadowBow | HeroWeapon::StarfireStaff
            ),
            HeroGear::CarrotHalo | HeroGear::WayfarerBoots | HeroGear::CarrotWings => true,
            HeroGear::RiftIdol => matches!(weapon, HeroWeapon::SummonStaff | HeroWeapon::StormOrb),
            HeroGear::BrassCompass => matches!(
                weapon,
                HeroWeapon::ShadowBow | HeroWeapon::SentryCrossbow | HeroWeapon::ForgeHammer
            ),
            HeroGear::DragonheartCrown => matches!(
                weapon,
                HeroWeapon::BannerSword | HeroWeapon::OathShield | HeroWeapon::SummonStaff
            ),
            HeroGear::BloodstepGreaves => matches!(
                weapon,
                HeroWeapon::NightDagger | HeroWeapon::BannerSword | HeroWeapon::ForgeHammer
            ),
            HeroGear::StarpathSandals => matches!(
                weapon,
                HeroWeapon::StarfireStaff | HeroWeapon::StormOrb | HeroWeapon::SummonStaff
            ),
            HeroGear::EngineerTreads => matches!(
                weapon,
                HeroWeapon::ForgeHammer | HeroWeapon::SentryCrossbow | HeroWeapon::OathShield
            ),
            HeroGear::SummonerGreaves => weapon == HeroWeapon::SummonStaff,
            HeroGear::WarflagTabard => weapon == HeroWeapon::BannerSword,
            HeroGear::MeteorCodex => weapon == HeroWeapon::StarfireStaff,
            HeroGear::BountyQuiver => {
                matches!(weapon, HeroWeapon::ShadowBow | HeroWeapon::SentryCrossbow)
            }
            HeroGear::CitadelSeal => {
                matches!(weapon, HeroWeapon::OathShield | HeroWeapon::BannerSword)
            }
            HeroGear::TempestCore => {
                matches!(weapon, HeroWeapon::StormOrb | HeroWeapon::StarfireStaff)
            }
            HeroGear::WatchtowerGreaves => {
                matches!(weapon, HeroWeapon::SentryCrossbow | HeroWeapon::OathShield)
            }
            HeroGear::AssassinWraps => {
                matches!(weapon, HeroWeapon::NightDagger | HeroWeapon::ShadowBow)
            }
            HeroGear::MythcallerTotem => weapon == HeroWeapon::SummonStaff,
            HeroGear::GolemBlueprint => weapon == HeroWeapon::ForgeHammer,
        }
    }
}

#[derive(Resource, Clone, Debug)]
pub struct HeroGearInventory {
    counts: [u32; HeroGear::ALL.len()],
}

impl Default for HeroGearInventory {
    fn default() -> Self {
        Self {
            counts: load_inventory_counts(),
        }
    }
}

impl HeroGearInventory {
    pub fn count(&self, item: HeroGear) -> u32 {
        self.counts[item.idx()]
    }

    pub fn owns(&self, item: HeroGear) -> bool {
        self.count(item) > 0
    }

    pub fn add(&mut self, item: HeroGear) {
        let idx = item.idx();
        self.counts[idx] = self.counts[idx].saturating_add(1);
        save_inventory_counts(&self.counts);
    }

    /// Grant temporary ownership without persisting it. Used by deterministic
    /// harness scenes that need to validate equipped paperdoll visuals.
    pub fn ensure_runtime_owned(&mut self, item: HeroGear) {
        let idx = item.idx();
        self.counts[idx] = self.counts[idx].max(1);
    }

    /// Set a count without persisting it. Used by deterministic capture harnesses
    /// that need a known hero-bag surface.
    pub fn set_runtime_count(&mut self, item: HeroGear, count: u32) {
        self.counts[item.idx()] = count;
    }

    pub fn total(&self) -> u32 {
        self.counts.iter().copied().sum()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct HeroGearDef {
    pub item: HeroGear,
    pub slot: HeroGearSlot,
    pub name: &'static str,
    pub short: &'static str,
    pub desc: &'static str,
    pub rarity: Rarity,
    pub damage_mult: f32,
    pub range_mult: f32,
    pub cooldown_mult: f32,
    pub hp_mult: f32,
    pub armor_add: f32,
    pub armor_pierce: f32,
    pub move_mult: f32,
    pub skill_mult: f32,
    pub skill_cooldown_reduction: i32,
    pub summon_power_add: f32,
    pub aura_damage_add: f32,
    pub tower_haste_add: f32,
    pub gold_bonus_add: f32,
    pub paperdoll_fragment: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct HeroGearStats {
    pub damage_mult: f32,
    pub range_mult: f32,
    pub cooldown_mult: f32,
    pub hp_mult: f32,
    pub armor_add: f32,
    pub armor_pierce: f32,
    pub move_mult: f32,
    pub skill_mult: f32,
    pub skill_cooldown_reduction: i32,
    pub summon_power_add: f32,
    pub aura_damage_add: f32,
    pub tower_haste_add: f32,
    pub gold_bonus_add: f32,
}

impl Default for HeroGearStats {
    fn default() -> Self {
        Self {
            damage_mult: 1.0,
            range_mult: 1.0,
            cooldown_mult: 1.0,
            hp_mult: 1.0,
            armor_add: 0.0,
            armor_pierce: 0.0,
            move_mult: 1.0,
            skill_mult: 1.0,
            skill_cooldown_reduction: 0,
            summon_power_add: 0.0,
            aura_damage_add: 0.0,
            tower_haste_add: 0.0,
            gold_bonus_add: 0.0,
        }
    }
}

impl HeroGearStats {
    pub fn combine(&mut self, other: HeroGearStats) {
        self.damage_mult *= other.damage_mult;
        self.range_mult *= other.range_mult;
        self.cooldown_mult *= other.cooldown_mult;
        self.hp_mult *= other.hp_mult;
        self.armor_add += other.armor_add;
        self.armor_pierce += other.armor_pierce;
        self.move_mult *= other.move_mult;
        self.skill_mult *= other.skill_mult;
        self.skill_cooldown_reduction += other.skill_cooldown_reduction;
        self.summon_power_add += other.summon_power_add;
        self.aura_damage_add += other.aura_damage_add;
        self.tower_haste_add += other.tower_haste_add;
        self.gold_bonus_add += other.gold_bonus_add;
    }

    pub fn add_item(&mut self, item: HeroGear) {
        let def = item.def();
        self.damage_mult *= def.damage_mult;
        self.range_mult *= def.range_mult;
        self.cooldown_mult *= def.cooldown_mult;
        self.hp_mult *= def.hp_mult;
        self.armor_add += def.armor_add;
        self.armor_pierce += def.armor_pierce;
        self.move_mult *= def.move_mult;
        self.skill_mult *= def.skill_mult;
        self.skill_cooldown_reduction += def.skill_cooldown_reduction;
        self.summon_power_add += def.summon_power_add;
        self.aura_damage_add += def.aura_damage_add;
        self.tower_haste_add += def.tower_haste_add;
        self.gold_bonus_add += def.gold_bonus_add;
    }

    pub fn add_weapon_affinity(&mut self, item: HeroGear, weapon: HeroWeapon) {
        if !item.has_weapon_affinity(weapon) {
            return;
        }

        self.damage_mult *= 1.04;
        self.skill_mult *= 1.04;

        match item {
            HeroGear::VowPlate => {
                self.hp_mult *= 1.06;
                self.armor_add += 4.0;
            }
            HeroGear::StarweaveRobe => {
                self.range_mult *= 1.04;
                self.skill_mult *= 1.05;
            }
            HeroGear::WindrunnerCloak => {
                self.move_mult *= 1.04;
                self.cooldown_mult *= 0.97;
            }
            HeroGear::WildhideHarness => {
                self.hp_mult *= 1.04;
                self.move_mult *= 1.03;
            }
            HeroGear::MoonthreadVest => {
                self.range_mult *= 1.03;
                self.skill_cooldown_reduction += 1;
            }
            HeroGear::NullMantle => {
                self.armor_pierce += 5.0;
                self.aura_damage_add += 0.02;
            }
            HeroGear::ThunderCharm => {
                self.cooldown_mult *= 0.96;
                self.skill_mult *= 1.06;
            }
            HeroGear::SaintBell => {
                self.summon_power_add += 0.08;
                self.hp_mult *= 1.04;
            }
            HeroGear::NightMask => {
                self.armor_pierce += 8.0;
                self.move_mult *= 1.04;
            }
            HeroGear::BloodBanner => {
                self.aura_damage_add += 0.03;
                self.gold_bonus_add += 0.04;
            }
            HeroGear::EmberPrayer => {
                self.skill_mult *= 1.08;
                self.cooldown_mult *= 0.98;
            }
            HeroGear::ClockworkBadge => {
                self.tower_haste_add += 0.04;
                self.skill_cooldown_reduction += 1;
            }
            HeroGear::ForgeGauntlet => {
                self.damage_mult *= 1.06;
                self.armor_pierce += 6.0;
            }
            HeroGear::SentryScope => {
                self.range_mult *= 1.07;
                self.armor_pierce += 6.0;
            }
            HeroGear::CarrotHalo => {
                self.aura_damage_add += 0.02;
                self.tower_haste_add += 0.02;
            }
            HeroGear::RiftIdol => {
                self.summon_power_add += 0.12;
                self.skill_mult *= 1.04;
            }
            HeroGear::BrassCompass => {
                self.gold_bonus_add += 0.03;
                self.move_mult *= 1.04;
            }
            HeroGear::DragonheartCrown => {
                self.hp_mult *= 1.05;
                self.damage_mult *= 1.05;
            }
            HeroGear::WayfarerBoots => {
                self.move_mult *= 1.03;
            }
            HeroGear::BloodstepGreaves => {
                self.armor_pierce += 8.0;
                self.cooldown_mult *= 0.97;
            }
            HeroGear::StarpathSandals => {
                self.skill_mult *= 1.07;
                self.range_mult *= 1.03;
            }
            HeroGear::EngineerTreads => {
                self.tower_haste_add += 0.05;
                self.hp_mult *= 1.04;
            }
            HeroGear::SummonerGreaves => {
                self.summon_power_add += 0.18;
                self.hp_mult *= 1.05;
            }
            HeroGear::CarrotWings => {
                self.move_mult *= 1.04;
                self.gold_bonus_add += 0.04;
            }
            HeroGear::WarflagTabard => {
                self.hp_mult *= 1.05;
                self.aura_damage_add += 0.04;
                self.gold_bonus_add += 0.03;
            }
            HeroGear::MeteorCodex => {
                self.range_mult *= 1.03;
                self.skill_mult *= 1.12;
                self.skill_cooldown_reduction += 1;
            }
            HeroGear::BountyQuiver => {
                self.armor_pierce += 14.0;
                self.damage_mult *= 1.12;
                self.cooldown_mult *= 0.92;
                self.gold_bonus_add += 0.06;
                self.move_mult *= 1.03;
            }
            HeroGear::CitadelSeal => {
                self.hp_mult *= 1.08;
                self.armor_add += 6.0;
                self.tower_haste_add += 0.03;
            }
            HeroGear::TempestCore => {
                self.cooldown_mult *= 0.95;
                self.skill_mult *= 1.08;
                self.aura_damage_add += 0.03;
            }
            HeroGear::WatchtowerGreaves => {
                self.range_mult *= 1.05;
                self.tower_haste_add += 0.04;
                self.armor_add += 2.0;
            }
            HeroGear::AssassinWraps => {
                self.damage_mult *= 1.10;
                self.cooldown_mult *= 0.92;
                self.move_mult *= 1.05;
                self.armor_pierce += 10.0;
            }
            HeroGear::MythcallerTotem => {
                self.summon_power_add += 0.26;
                self.skill_mult *= 1.08;
                self.skill_cooldown_reduction += 1;
            }
            HeroGear::GolemBlueprint => {
                self.hp_mult *= 1.04;
                self.tower_haste_add += 0.07;
                self.skill_cooldown_reduction += 1;
            }
        }
    }

    pub fn add_weapon_resonance(&mut self, weapon: HeroWeapon, affinity_count: usize) {
        if affinity_count < 2 {
            return;
        }

        self.damage_mult *= 1.04;
        self.skill_mult *= 1.04;
        self.cooldown_mult *= 0.98;

        if affinity_count >= 3 {
            match weapon {
                HeroWeapon::BannerSword => {
                    self.damage_mult *= 1.03;
                    self.hp_mult *= 1.08;
                    self.armor_add += 5.0;
                }
                HeroWeapon::OathShield => {
                    self.hp_mult *= 1.08;
                    self.armor_add += 7.0;
                    self.aura_damage_add += 0.02;
                    self.tower_haste_add += 0.03;
                }
                HeroWeapon::ForgeHammer => {
                    self.hp_mult *= 1.05;
                    self.armor_add += 3.0;
                    self.tower_haste_add += 0.06;
                    self.skill_cooldown_reduction += 1;
                }
                HeroWeapon::StarfireStaff => {
                    self.range_mult *= 1.04;
                    self.skill_mult *= 1.09;
                    self.cooldown_mult *= 0.98;
                }
                HeroWeapon::StormOrb => {
                    self.range_mult *= 1.04;
                    self.skill_mult *= 1.05;
                    self.cooldown_mult *= 0.97;
                    self.aura_damage_add += 0.02;
                }
                HeroWeapon::ShadowBow => {
                    self.move_mult *= 1.04;
                    self.armor_pierce += 6.0;
                    self.cooldown_mult *= 0.98;
                    self.gold_bonus_add += 0.03;
                }
                HeroWeapon::SentryCrossbow => {
                    self.range_mult *= 1.07;
                    self.armor_pierce += 8.0;
                    self.tower_haste_add += 0.03;
                }
                HeroWeapon::NightDagger => {
                    self.damage_mult *= 1.04;
                    self.move_mult *= 1.05;
                    self.armor_pierce += 10.0;
                }
                HeroWeapon::SummonStaff => {
                    self.summon_power_add += 0.14;
                    self.hp_mult *= 1.04;
                    self.skill_mult *= 1.04;
                }
            }
        }

        if affinity_count >= 4 {
            self.damage_mult *= 1.05;
            self.cooldown_mult *= 0.97;
            self.aura_damage_add += 0.03;
            self.gold_bonus_add += 0.02;
            match weapon {
                HeroWeapon::BannerSword | HeroWeapon::OathShield => {
                    self.hp_mult *= 1.04;
                    self.armor_add += 4.0;
                }
                HeroWeapon::ForgeHammer => {
                    self.tower_haste_add += 0.04;
                }
                HeroWeapon::StarfireStaff | HeroWeapon::StormOrb => {
                    self.skill_mult *= 1.05;
                }
                HeroWeapon::ShadowBow | HeroWeapon::SentryCrossbow | HeroWeapon::NightDagger => {
                    self.armor_pierce += 8.0;
                }
                HeroWeapon::SummonStaff => {
                    self.summon_power_add += 0.10;
                }
            }
        }
    }
}

const fn gear_base(
    item: HeroGear,
    slot: HeroGearSlot,
    name: &'static str,
    short: &'static str,
    desc: &'static str,
    rarity: Rarity,
    paperdoll_fragment: u32,
) -> HeroGearDef {
    HeroGearDef {
        item,
        slot,
        name,
        short,
        desc,
        rarity,
        damage_mult: 1.0,
        range_mult: 1.0,
        cooldown_mult: 1.0,
        hp_mult: 1.0,
        armor_add: 0.0,
        armor_pierce: 0.0,
        move_mult: 1.0,
        skill_mult: 1.0,
        skill_cooldown_reduction: 0,
        summon_power_add: 0.0,
        aura_damage_add: 0.0,
        tower_haste_add: 0.0,
        gold_bonus_add: 0.0,
        paperdoll_fragment,
    }
}

pub static HERO_GEAR_DEFS: &[HeroGearDef] = &[
    HeroGearDef {
        item: HeroGear::VowPlate,
        slot: HeroGearSlot::Armor,
        name: "誓约板甲",
        short: "板甲",
        desc: "英雄专属战衣，提升生命与护甲，适合近战扛线武器。",
        rarity: Rarity::Common,
        damage_mult: 1.02,
        range_mult: 1.0,
        cooldown_mult: 1.0,
        hp_mult: 1.20,
        armor_add: 8.0,
        armor_pierce: 0.0,
        move_mult: 0.96,
        skill_mult: 1.0,
        skill_cooldown_reduction: 0,
        summon_power_add: 0.0,
        aura_damage_add: 0.0,
        tower_haste_add: 0.0,
        gold_bonus_add: 0.0,
        paperdoll_fragment: 200,
    },
    HeroGearDef {
        item: HeroGear::StarweaveRobe,
        slot: HeroGearSlot::Armor,
        name: "星织法袍",
        short: "星袍",
        desc: "给法术与圣光武器准备的轻甲，提升技能伤害和射程。",
        rarity: Rarity::Uncommon,
        damage_mult: 1.10,
        range_mult: 1.06,
        cooldown_mult: 0.98,
        hp_mult: 1.06,
        armor_add: 2.0,
        armor_pierce: 0.0,
        move_mult: 1.0,
        skill_mult: 1.08,
        skill_cooldown_reduction: 0,
        summon_power_add: 0.0,
        aura_damage_add: 0.0,
        tower_haste_add: 0.0,
        gold_bonus_add: 0.0,
        paperdoll_fragment: 201,
    },
    HeroGearDef {
        item: HeroGear::WindrunnerCloak,
        slot: HeroGearSlot::Armor,
        name: "逐风斗篷",
        short: "风篷",
        desc: "游走型英雄的轻装，提高移动速度、射程和攻速。",
        rarity: Rarity::Rare,
        damage_mult: 1.05,
        range_mult: 1.10,
        cooldown_mult: 0.92,
        hp_mult: 0.98,
        armor_add: 1.0,
        armor_pierce: 0.0,
        move_mult: 1.12,
        skill_mult: 1.0,
        skill_cooldown_reduction: 0,
        summon_power_add: 0.0,
        aura_damage_add: 0.0,
        tower_haste_add: 0.0,
        gold_bonus_add: 0.0,
        paperdoll_fragment: 202,
    },
    HeroGearDef {
        item: HeroGear::ThunderCharm,
        slot: HeroGearSlot::Charm,
        name: "雷纹护符",
        short: "雷符",
        desc: "强化连锁、法球和高频攻击，牺牲少量稳定性换取爆发。",
        rarity: Rarity::Rare,
        damage_mult: 1.13,
        range_mult: 1.0,
        cooldown_mult: 0.94,
        hp_mult: 1.0,
        armor_add: 0.0,
        armor_pierce: 4.0,
        move_mult: 1.0,
        skill_mult: 1.0,
        skill_cooldown_reduction: 0,
        summon_power_add: 0.0,
        aura_damage_add: 0.0,
        tower_haste_add: 0.0,
        gold_bonus_add: 0.0,
        paperdoll_fragment: 210,
    },
    HeroGearDef {
        item: HeroGear::SaintBell,
        slot: HeroGearSlot::Charm,
        name: "契约铃符",
        short: "契符",
        desc: "提高召唤、裂界削弱和团队支援稳定性。",
        rarity: Rarity::Epic,
        damage_mult: 1.08,
        range_mult: 1.04,
        cooldown_mult: 0.96,
        hp_mult: 1.12,
        armor_add: 4.0,
        armor_pierce: 0.0,
        move_mult: 1.0,
        skill_mult: 1.0,
        skill_cooldown_reduction: 0,
        summon_power_add: 0.10,
        aura_damage_add: 0.0,
        tower_haste_add: 0.0,
        gold_bonus_add: 0.0,
        paperdoll_fragment: 211,
    },
    HeroGearDef {
        item: HeroGear::NightMask,
        slot: HeroGearSlot::Charm,
        name: "夜袭面具",
        short: "夜面",
        desc: "夜刃匕首与猎影长弓的输出护符，提升穿甲、速度和背击收益。",
        rarity: Rarity::Epic,
        damage_mult: 1.15,
        range_mult: 1.02,
        cooldown_mult: 0.90,
        hp_mult: 0.96,
        armor_add: 0.0,
        armor_pierce: 10.0,
        move_mult: 1.08,
        skill_mult: 1.12,
        skill_cooldown_reduction: 0,
        summon_power_add: 0.0,
        aura_damage_add: 0.0,
        tower_haste_add: 0.0,
        gold_bonus_add: 0.0,
        paperdoll_fragment: 212,
    },
    HeroGearDef {
        item: HeroGear::ForgeGauntlet,
        slot: HeroGearSlot::Relic,
        name: "熔炉手甲",
        short: "手甲",
        desc: "工匠战锤与誓约盾锤的重型圣物，提高近战打击和塔联动节奏。",
        rarity: Rarity::Legendary,
        damage_mult: 1.18,
        range_mult: 1.0,
        cooldown_mult: 0.88,
        hp_mult: 1.10,
        armor_add: 6.0,
        armor_pierce: 6.0,
        move_mult: 0.98,
        skill_mult: 1.06,
        skill_cooldown_reduction: 0,
        summon_power_add: 0.0,
        aura_damage_add: 0.0,
        tower_haste_add: 0.04,
        gold_bonus_add: 0.0,
        paperdoll_fragment: 220,
    },
    HeroGearDef {
        item: HeroGear::SentryScope,
        slot: HeroGearSlot::Relic,
        name: "哨戒目镜",
        short: "目镜",
        desc: "强化侦察、反隐和远程压制，适合哨戒弩与猎影长弓。",
        rarity: Rarity::Legendary,
        damage_mult: 1.12,
        range_mult: 1.16,
        cooldown_mult: 0.94,
        hp_mult: 1.0,
        armor_add: 2.0,
        armor_pierce: 12.0,
        move_mult: 1.04,
        skill_mult: 1.0,
        skill_cooldown_reduction: 0,
        summon_power_add: 0.0,
        aura_damage_add: 0.0,
        tower_haste_add: 0.0,
        gold_bonus_add: 0.0,
        paperdoll_fragment: 221,
    },
    HeroGearDef {
        item: HeroGear::CarrotHalo,
        slot: HeroGearSlot::Relic,
        name: "萝卜光环",
        short: "光环",
        desc: "终局英雄圣物，兼顾生存、输出、支援和纸娃娃辨识度。",
        rarity: Rarity::Mythic,
        damage_mult: 1.20,
        range_mult: 1.08,
        cooldown_mult: 0.90,
        hp_mult: 1.20,
        armor_add: 10.0,
        armor_pierce: 10.0,
        move_mult: 1.05,
        skill_mult: 1.12,
        skill_cooldown_reduction: 1,
        summon_power_add: 0.10,
        aura_damage_add: 0.04,
        tower_haste_add: 0.03,
        gold_bonus_add: 0.08,
        paperdoll_fragment: 222,
    },
    HeroGearDef {
        damage_mult: 1.08,
        hp_mult: 1.16,
        armor_add: 5.0,
        move_mult: 1.04,
        skill_mult: 1.04,
        ..gear_base(
            HeroGear::WildhideHarness,
            HeroGearSlot::Armor,
            "荒兽皮甲",
            "兽皮",
            "兽人与近战武器的轻重混合战衣，保持机动同时提高贴线生存。",
            Rarity::Uncommon,
            200,
        )
    },
    HeroGearDef {
        range_mult: 1.12,
        cooldown_mult: 0.96,
        move_mult: 1.06,
        skill_cooldown_reduction: 1,
        ..gear_base(
            HeroGear::MoonthreadVest,
            HeroGearSlot::Armor,
            "月线轻甲",
            "月甲",
            "精灵游走装束，缩短主动技能循环，适合弓、弩、法器频繁换位。",
            Rarity::Rare,
            202,
        )
    },
    HeroGearDef {
        hp_mult: 1.12,
        armor_add: 7.0,
        armor_pierce: 8.0,
        aura_damage_add: 0.03,
        ..gear_base(
            HeroGear::NullMantle,
            HeroGearSlot::Armor,
            "虚无披肩",
            "虚披",
            "把英雄站位变成压制点，兼顾护甲、穿甲和小范围联动增伤。",
            Rarity::Epic,
            201,
        )
    },
    HeroGearDef {
        damage_mult: 1.07,
        hp_mult: 1.08,
        aura_damage_add: 0.05,
        gold_bonus_add: 0.05,
        ..gear_base(
            HeroGear::BloodBanner,
            HeroGearSlot::Charm,
            "血旗吊坠",
            "血旗",
            "强化英雄周围防线的击杀节奏，适合站在关键路口带塔打钱。",
            Rarity::Rare,
            211,
        )
    },
    HeroGearDef {
        damage_mult: 1.10,
        skill_mult: 1.18,
        cooldown_mult: 0.97,
        ..gear_base(
            HeroGear::EmberPrayer,
            HeroGearSlot::Charm,
            "余烬祷文",
            "烬文",
            "主动技能爆发护符，法杖、雷暴和圣光武器能明显放大清场窗口。",
            Rarity::Epic,
            210,
        )
    },
    HeroGearDef {
        hp_mult: 1.05,
        tower_haste_add: 0.06,
        gold_bonus_add: 0.06,
        skill_cooldown_reduction: 1,
        ..gear_base(
            HeroGear::ClockworkBadge,
            HeroGearSlot::Charm,
            "发条工牌",
            "工牌",
            "工匠与塔联动核心，缩短主动技能循环并提高附近塔攻速。",
            Rarity::Legendary,
            212,
        )
    },
    HeroGearDef {
        hp_mult: 1.10,
        summon_power_add: 0.18,
        skill_mult: 1.10,
        skill_cooldown_reduction: 1,
        ..gear_base(
            HeroGear::RiftIdol,
            HeroGearSlot::Relic,
            "裂隙神像",
            "裂像",
            "召唤法杖和召唤塔的专用追求，显著提高召唤物强度与主动频率。",
            Rarity::Legendary,
            222,
        )
    },
    HeroGearDef {
        range_mult: 1.08,
        move_mult: 1.08,
        gold_bonus_add: 0.08,
        tower_haste_add: 0.02,
        ..gear_base(
            HeroGear::BrassCompass,
            HeroGearSlot::Relic,
            "黄铜罗盘",
            "罗盘",
            "经济型圣物，鼓励英雄游走补线，击杀收益和附近塔节奏同步提高。",
            Rarity::Legendary,
            221,
        )
    },
    HeroGearDef {
        damage_mult: 1.16,
        hp_mult: 1.18,
        armor_add: 8.0,
        skill_mult: 1.16,
        aura_damage_add: 0.06,
        summon_power_add: 0.08,
        ..gear_base(
            HeroGear::DragonheartCrown,
            HeroGearSlot::Relic,
            "龙心王冠",
            "龙冠",
            "终局构筑圣物，兼顾英雄爆发、站场和周围防线联动。",
            Rarity::Mythic,
            222,
        )
    },
    HeroGearDef {
        range_mult: 1.03,
        move_mult: 1.10,
        cooldown_mult: 0.99,
        ..gear_base(
            HeroGear::WayfarerBoots,
            HeroGearSlot::Boots,
            "巡路短靴",
            "巡靴",
            "通用机动靴履，帮助英雄更快补线，适合所有武器的开局过渡。",
            Rarity::Common,
            230,
        )
    },
    HeroGearDef {
        damage_mult: 1.08,
        armor_pierce: 6.0,
        move_mult: 1.12,
        skill_mult: 1.04,
        ..gear_base(
            HeroGear::BloodstepGreaves,
            HeroGearSlot::Boots,
            "血步胫甲",
            "血靴",
            "近战与背击路线的突进靴，提升穿甲、移动和主动爆发。",
            Rarity::Rare,
            231,
        )
    },
    HeroGearDef {
        range_mult: 1.08,
        cooldown_mult: 0.97,
        skill_mult: 1.12,
        skill_cooldown_reduction: 1,
        ..gear_base(
            HeroGear::StarpathSandals,
            HeroGearSlot::Boots,
            "星路便鞋",
            "星履",
            "远程施法与控场武器的循环靴，扩展射程并缩短主动技能节奏。",
            Rarity::Epic,
            232,
        )
    },
    HeroGearDef {
        hp_mult: 1.08,
        move_mult: 1.05,
        skill_cooldown_reduction: 1,
        tower_haste_add: 0.05,
        ..gear_base(
            HeroGear::EngineerTreads,
            HeroGearSlot::Boots,
            "工匠履带",
            "履带",
            "临时守卫和塔联动路线的靴履，站到塔群旁时能进一步抬高攻速。",
            Rarity::Legendary,
            233,
        )
    },
    HeroGearDef {
        hp_mult: 1.10,
        move_mult: 1.04,
        skill_mult: 1.08,
        summon_power_add: 0.15,
        ..gear_base(
            HeroGear::SummonerGreaves,
            HeroGearSlot::Boots,
            "唤灵胫靴",
            "唤靴",
            "召唤法杖和召唤塔的靴履分支，让神话怪物与塔召唤物更耐打。",
            Rarity::Legendary,
            234,
        )
    },
    HeroGearDef {
        damage_mult: 1.10,
        range_mult: 1.10,
        cooldown_mult: 0.94,
        move_mult: 1.16,
        skill_mult: 1.10,
        tower_haste_add: 0.03,
        gold_bonus_add: 0.05,
        ..gear_base(
            HeroGear::CarrotWings,
            HeroGearSlot::Boots,
            "萝卜翼靴",
            "翼靴",
            "终局靴履，兼顾游走、攻速、金币收益和防线节奏。",
            Rarity::Mythic,
            235,
        )
    },
    HeroGearDef {
        damage_mult: 1.09,
        hp_mult: 1.12,
        armor_add: 4.0,
        aura_damage_add: 0.05,
        gold_bonus_add: 0.02,
        ..gear_base(
            HeroGear::WarflagTabard,
            HeroGearSlot::Armor,
            "战旗罩袍",
            "旗袍",
            "战旗长剑签名战衣，把英雄站位变成前线旗点，兼顾自保、塔增伤和击杀收益。",
            Rarity::Rare,
            200,
        )
    },
    HeroGearDef {
        damage_mult: 1.08,
        range_mult: 1.06,
        skill_mult: 1.20,
        skill_cooldown_reduction: 1,
        ..gear_base(
            HeroGear::MeteorCodex,
            HeroGearSlot::Relic,
            "陨星法典",
            "星典",
            "星火法杖签名圣物，把主动技能推向大范围爆发，适合围绕清屏窗口构筑。",
            Rarity::Epic,
            221,
        )
    },
    HeroGearDef {
        damage_mult: 1.20,
        range_mult: 1.06,
        cooldown_mult: 0.88,
        hp_mult: 1.12,
        armor_pierce: 12.0,
        move_mult: 1.04,
        skill_mult: 1.15,
        gold_bonus_add: 0.10,
        ..gear_base(
            HeroGear::BountyQuiver,
            HeroGearSlot::Charm,
            "赏金箭袋",
            "金袋",
            "猎影长弓签名护符，鼓励游走补刀，用额外金币把优势滚进防御塔阵。",
            Rarity::Rare,
            212,
        )
    },
    HeroGearDef {
        hp_mult: 1.16,
        armor_add: 8.0,
        aura_damage_add: 0.04,
        tower_haste_add: 0.02,
        ..gear_base(
            HeroGear::CitadelSeal,
            HeroGearSlot::Charm,
            "城塞徽印",
            "塞印",
            "誓约盾锤签名护符，把英雄塑造成塔群前排，提升防线整体节奏。",
            Rarity::Epic,
            211,
        )
    },
    HeroGearDef {
        damage_mult: 1.08,
        range_mult: 1.04,
        cooldown_mult: 0.94,
        skill_mult: 1.16,
        aura_damage_add: 0.03,
        ..gear_base(
            HeroGear::TempestCore,
            HeroGearSlot::Relic,
            "风暴核心",
            "暴核",
            "雷暴法器签名圣物，强化连锁节奏和雷云技能，把怪线拖进塔火力。",
            Rarity::Legendary,
            222,
        )
    },
    HeroGearDef {
        range_mult: 1.08,
        hp_mult: 1.04,
        armor_add: 2.0,
        move_mult: 1.06,
        tower_haste_add: 0.04,
        ..gear_base(
            HeroGear::WatchtowerGreaves,
            HeroGearSlot::Boots,
            "望塔胫甲",
            "望靴",
            "哨戒弩签名靴履，让英雄站位更像移动哨塔，扩展射程并带动塔群攻速。",
            Rarity::Rare,
            233,
        )
    },
    HeroGearDef {
        damage_mult: 1.18,
        cooldown_mult: 0.86,
        hp_mult: 1.12,
        armor_pierce: 12.0,
        move_mult: 1.08,
        skill_mult: 1.10,
        ..gear_base(
            HeroGear::AssassinWraps,
            HeroGearSlot::Armor,
            "刺客裹衣",
            "刺衣",
            "夜刃匕首签名战衣，牺牲坦度换取绕后、穿甲和首领爆发。",
            Rarity::Epic,
            202,
        )
    },
    HeroGearDef {
        hp_mult: 1.12,
        skill_mult: 1.12,
        skill_cooldown_reduction: 1,
        summon_power_add: 0.28,
        tower_haste_add: 0.02,
        ..gear_base(
            HeroGear::MythcallerTotem,
            HeroGearSlot::Charm,
            "唤神图腾",
            "神图",
            "召唤法杖签名护符，专注神话眷属和召唤塔协同，强化召唤物存在感。",
            Rarity::Legendary,
            211,
        )
    },
    HeroGearDef {
        damage_mult: 1.06,
        hp_mult: 1.10,
        armor_add: 4.0,
        skill_cooldown_reduction: 1,
        tower_haste_add: 0.08,
        summon_power_add: 0.06,
        ..gear_base(
            HeroGear::GolemBlueprint,
            HeroGearSlot::Relic,
            "魔像蓝图",
            "蓝图",
            "工匠战锤签名圣物，强化临时守卫和塔超频，是工坊路线核心。",
            Rarity::Legendary,
            220,
        )
    },
];

pub fn roll_clear_reward(
    rng: &mut crate::game::Rng,
    stars: u8,
    difficulty_bonus: i32,
    level_index: usize,
    weapon: HeroWeapon,
) -> Option<HeroGear> {
    let stars = stars.clamp(1, 3);
    let chance = (0.20
        + stars as f32 * 0.10
        + difficulty_bonus.max(0) as f32 * 0.06
        + level_index as f32 * 0.008)
        .min(0.82);
    if rng.frac() >= chance {
        return None;
    }
    let rarity = roll_reward_rarity(rng.frac(), stars, difficulty_bonus, level_index);
    Some(pick_by_rarity_for_weapon(rng, rarity, weapon))
}

fn roll_reward_rarity(p: f32, stars: u8, difficulty_bonus: i32, level_index: usize) -> Rarity {
    let depth_bonus = if level_index >= 16 {
        2
    } else if level_index >= 8 {
        1
    } else {
        0
    };
    let tier = stars as i32 - 1 + difficulty_bonus.max(0) + depth_bonus;
    match tier {
        0 => {
            if p < 0.20 {
                Rarity::Uncommon
            } else {
                Rarity::Common
            }
        }
        1 => {
            if p < 0.18 {
                Rarity::Rare
            } else if p < 0.48 {
                Rarity::Uncommon
            } else {
                Rarity::Common
            }
        }
        2 => {
            if p < 0.12 {
                Rarity::Epic
            } else if p < 0.42 {
                Rarity::Rare
            } else if p < 0.74 {
                Rarity::Uncommon
            } else {
                Rarity::Common
            }
        }
        3 => {
            if p < 0.08 {
                Rarity::Legendary
            } else if p < 0.28 {
                Rarity::Epic
            } else if p < 0.62 {
                Rarity::Rare
            } else {
                Rarity::Uncommon
            }
        }
        _ => {
            if p < 0.04 {
                Rarity::Mythic
            } else if p < 0.18 {
                Rarity::Legendary
            } else if p < 0.42 {
                Rarity::Epic
            } else if p < 0.74 {
                Rarity::Rare
            } else {
                Rarity::Uncommon
            }
        }
    }
}

fn pick_by_rarity_for_weapon(
    rng: &mut crate::game::Rng,
    rarity: Rarity,
    weapon: HeroWeapon,
) -> HeroGear {
    let mut candidates = HeroGear::ALL
        .into_iter()
        .filter(|item| item.def().rarity == rarity)
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        candidates = HeroGear::ALL
            .into_iter()
            .filter(|item| rarity_tier(item.def().rarity) <= rarity_tier(rarity))
            .collect();
    }

    let total_weight: usize = candidates
        .iter()
        .map(|item| reward_affinity_weight(*item, weapon))
        .sum();
    let mut roll = rng.range(total_weight.max(1));
    for item in candidates {
        let weight = reward_affinity_weight(item, weapon);
        if roll < weight {
            return item;
        }
        roll -= weight;
    }
    HeroGear::WayfarerBoots
}

fn reward_affinity_weight(item: HeroGear, weapon: HeroWeapon) -> usize {
    if matches!(
        item,
        HeroGear::CarrotHalo | HeroGear::WayfarerBoots | HeroGear::CarrotWings
    ) {
        3
    } else if item.has_weapon_affinity(weapon) {
        6
    } else {
        1
    }
}

fn rarity_tier(rarity: Rarity) -> i32 {
    match rarity {
        Rarity::Common => 0,
        Rarity::Uncommon => 1,
        Rarity::Rare => 2,
        Rarity::Epic => 3,
        Rarity::Legendary => 4,
        Rarity::Mythic => 5,
    }
}

pub fn empty_gear() -> [Option<HeroGear>; HeroGearSlot::COUNT] {
    [None; HeroGearSlot::COUNT]
}

pub fn gear_stats(slots: &[Option<HeroGear>; HeroGearSlot::COUNT]) -> HeroGearStats {
    let mut stats = HeroGearStats::default();
    for item in slots.iter().flatten() {
        stats.add_item(*item);
    }
    stats
}

pub fn weapon_affinity_stats(
    slots: &[Option<HeroGear>; HeroGearSlot::COUNT],
    weapon: HeroWeapon,
) -> HeroGearStats {
    let mut stats = HeroGearStats::default();
    for item in slots.iter().flatten() {
        stats.add_weapon_affinity(*item, weapon);
    }
    stats.add_weapon_resonance(weapon, weapon_affinity_count(slots, weapon));
    stats
}

pub fn active_stats_for_weapon(
    slots: &[Option<HeroGear>; HeroGearSlot::COUNT],
    weapon: HeroWeapon,
) -> HeroGearStats {
    let mut stats = gear_stats(slots);
    stats.combine(weapon_affinity_stats(slots, weapon));
    stats
}

pub fn weapon_affinity_count(
    slots: &[Option<HeroGear>; HeroGearSlot::COUNT],
    weapon: HeroWeapon,
) -> usize {
    slots
        .iter()
        .flatten()
        .filter(|item| item.has_weapon_affinity(weapon))
        .count()
}

pub fn gear_count(slots: &[Option<HeroGear>; HeroGearSlot::COUNT]) -> usize {
    slots.iter().filter(|item| item.is_some()).count()
}

pub fn equip(
    slots: &mut [Option<HeroGear>; HeroGearSlot::COUNT],
    item: HeroGear,
) -> Option<HeroGear> {
    let idx = item.def().slot.idx();
    let replaced = slots[idx];
    slots[idx] = Some(item);
    replaced
}

pub fn unequip_slot(
    slots: &mut [Option<HeroGear>; HeroGearSlot::COUNT],
    slot: HeroGearSlot,
) -> Option<HeroGear> {
    slots[slot.idx()].take()
}

pub fn encode(slots: &[Option<HeroGear>; HeroGearSlot::COUNT]) -> String {
    slots
        .iter()
        .map(|item| {
            item.map(|gear| gear.idx().to_string())
                .unwrap_or_else(|| "-".to_string())
        })
        .collect::<Vec<_>>()
        .join("/")
}

pub fn decode(raw: &str) -> [Option<HeroGear>; HeroGearSlot::COUNT] {
    let mut slots = empty_gear();
    for (slot, value) in slots.iter_mut().zip(raw.split('/')) {
        if value == "-" || value.is_empty() {
            continue;
        }
        *slot = value.parse::<usize>().ok().and_then(HeroGear::from_idx);
    }
    slots
}

pub fn summary(slots: &[Option<HeroGear>; HeroGearSlot::COUNT]) -> String {
    summary_for_weapon(slots, None)
}

pub fn summary_for_weapon(
    slots: &[Option<HeroGear>; HeroGearSlot::COUNT],
    weapon: Option<HeroWeapon>,
) -> String {
    if gear_count(slots) == 0 {
        return crate::i18n::t("英雄装备：无");
    }
    let stats = weapon
        .map(|weapon| active_stats_for_weapon(slots, weapon))
        .unwrap_or_else(|| gear_stats(slots));
    let resonance = weapon
        .and_then(|weapon| weapon_resonance_summary(slots, weapon))
        .unwrap_or_default();
    let names = HeroGearSlot::ALL
        .iter()
        .enumerate()
        .map(|(idx, slot)| {
            slots[idx]
                .map(|item| crate::i18n::t(item.def().short))
                .unwrap_or_else(|| crate::i18n::t(slot.name()))
        })
        .collect::<Vec<_>>()
        .join("/");
    crate::i18n::tf(
        "英雄装备：{}{}  伤害×{}  HP×{}  攻速×{}  技能×{}  召唤+{}%  光环+{}%",
        &[
            &names,
            &resonance,
            &format!("{:.2}", stats.damage_mult),
            &format!("{:.2}", stats.hp_mult),
            &format!("{:.2}", 1.0 / stats.cooldown_mult.max(0.01)),
            &format!("{:.2}", stats.skill_mult),
            &format!("{:.0}", stats.summon_power_add * 100.0),
            &format!(
                "{:.0}",
                (stats.aura_damage_add + stats.tower_haste_add) * 100.0
            ),
        ],
    )
}

pub fn weapon_resonance_summary(
    slots: &[Option<HeroGear>; HeroGearSlot::COUNT],
    weapon: HeroWeapon,
) -> Option<String> {
    let count = weapon_affinity_count(slots, weapon);
    if count < 2 {
        return None;
    }
    let tier = match count {
        2 => crate::i18n::t("初鸣"),
        3 => crate::i18n::t("强鸣"),
        _ => crate::i18n::t("满鸣"),
    };
    let (route, _) = weapon_resonance_route(weapon);
    Some(crate::i18n::tf(
        "  共鸣{}件·{}·{}",
        &[&count.to_string(), &tier, &crate::i18n::t(route)],
    ))
}

pub fn weapon_resonance_detail(
    slots: &[Option<HeroGear>; HeroGearSlot::COUNT],
    weapon: HeroWeapon,
) -> Option<String> {
    let count = weapon_affinity_count(slots, weapon);
    if count < 2 {
        return None;
    }
    let tier = match count {
        2 => crate::i18n::t("初鸣"),
        3 => crate::i18n::t("强鸣"),
        _ => crate::i18n::t("满鸣"),
    };
    let (route, desc) = weapon_resonance_route(weapon);
    Some(crate::i18n::tf(
        "武器共鸣：{}件·{}·{}\n{}",
        &[
            &count.to_string(),
            &tier,
            &crate::i18n::t(route),
            &crate::i18n::t(desc),
        ],
    ))
}

pub fn weapon_resonance_route(weapon: HeroWeapon) -> (&'static str, &'static str) {
    match weapon {
        HeroWeapon::BannerSword => (
            "战旗血誓",
            "适配装备越多，英雄越像独立守线核心：提高伤害、生命和护甲。",
        ),
        HeroWeapon::StarfireStaff => (
            "星火核爆",
            "把装备共鸣转化为射程和主动技能爆发，适合清理密集波次。",
        ),
        HeroWeapon::ShadowBow => (
            "猎影赏金",
            "提高游走、穿甲和击杀金币，让英雄负责补刀和发育滚雪球。",
        ),
        HeroWeapon::OathShield => (
            "誓约壁垒",
            "强化生命、护甲与塔群光环，适合站在关键路口带塔抗线。",
        ),
        HeroWeapon::StormOrb => (
            "雷暴矩阵",
            "提高技能循环、范围和光环压制，把敌线拖在防御塔火力里。",
        ),
        HeroWeapon::SentryCrossbow => (
            "哨戒阵列",
            "提高射程、穿甲和塔攻速联动，专注反隐与远距离防线覆盖。",
        ),
        HeroWeapon::NightDagger => (
            "夜刃背刺",
            "提高移动、穿甲和爆发，鼓励绕后处理首领和高护甲怪。",
        ),
        HeroWeapon::SummonStaff => (
            "异界眷属",
            "把装备共鸣集中到召唤强度，让神话召唤物和召唤塔一起变强。",
        ),
        HeroWeapon::ForgeHammer => (
            "守卫工坊",
            "提高塔攻速联动与主动循环，让工匠围绕临时守卫和塔群作战。",
        ),
    }
}

fn starter_counts() -> [u32; HeroGear::ALL.len()] {
    let mut counts = [0; HeroGear::ALL.len()];
    counts[HeroGear::VowPlate.idx()] = 1;
    counts[HeroGear::ThunderCharm.idx()] = 1;
    counts[HeroGear::ForgeGauntlet.idx()] = 1;
    counts[HeroGear::WayfarerBoots.idx()] = 1;
    counts
}

fn encode_counts(counts: &[u32; HeroGear::ALL.len()]) -> String {
    counts
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

fn decode_counts(raw: &str) -> [u32; HeroGear::ALL.len()] {
    let mut counts = starter_counts();
    if raw.trim().is_empty() {
        return counts;
    }
    for (slot, value) in counts.iter_mut().zip(raw.split(',')) {
        *slot = value.trim().parse::<u32>().unwrap_or(0).min(999);
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hero_gear_catalog_matches_enum_and_has_four_slots() {
        assert_eq!(HERO_GEAR_DEFS.len(), HeroGear::ALL.len());
        assert_eq!(HeroGear::ALL.len(), 33);
        assert_eq!(HeroGearSlot::COUNT, 4);
        assert_eq!(HeroGearSlot::Boots.idx(), 3);
        assert!(
            HeroGear::ALL
                .iter()
                .any(|item| item.def().slot == HeroGearSlot::Boots)
        );
    }

    #[test]
    fn signature_gear_expands_every_weapon_route() {
        let signatures = [
            (HeroWeapon::BannerSword, HeroGear::WarflagTabard),
            (HeroWeapon::StarfireStaff, HeroGear::MeteorCodex),
            (HeroWeapon::ShadowBow, HeroGear::BountyQuiver),
            (HeroWeapon::OathShield, HeroGear::CitadelSeal),
            (HeroWeapon::StormOrb, HeroGear::TempestCore),
            (HeroWeapon::SentryCrossbow, HeroGear::WatchtowerGreaves),
            (HeroWeapon::NightDagger, HeroGear::AssassinWraps),
            (HeroWeapon::SummonStaff, HeroGear::MythcallerTotem),
            (HeroWeapon::ForgeHammer, HeroGear::GolemBlueprint),
        ];

        for (weapon, item) in signatures {
            assert!(
                item.has_weapon_affinity(weapon),
                "{:?} should be a signature item for {}",
                item,
                weapon.name()
            );
            assert!(
                HeroGear::ALL
                    .iter()
                    .filter(|candidate| candidate.has_weapon_affinity(weapon))
                    .count()
                    >= 5,
                "{} should have enough gear choices to support builds",
                weapon.name()
            );
        }
    }

    #[test]
    fn boots_slot_equips_and_replaces_independently() {
        let mut slots = empty_gear();
        assert_eq!(equip(&mut slots, HeroGear::WayfarerBoots), None);
        assert_eq!(
            slots[HeroGearSlot::Boots.idx()],
            Some(HeroGear::WayfarerBoots)
        );
        assert_eq!(gear_count(&slots), 1);

        assert_eq!(
            equip(&mut slots, HeroGear::BloodstepGreaves),
            Some(HeroGear::WayfarerBoots)
        );
        assert_eq!(
            slots[HeroGearSlot::Boots.idx()],
            Some(HeroGear::BloodstepGreaves)
        );
        assert_eq!(gear_count(&slots), 1);
    }

    #[test]
    fn four_slot_encoding_keeps_legacy_three_slot_saves_compatible() {
        let mut slots = empty_gear();
        equip(&mut slots, HeroGear::VowPlate);
        equip(&mut slots, HeroGear::ThunderCharm);
        equip(&mut slots, HeroGear::ForgeGauntlet);
        equip(&mut slots, HeroGear::WayfarerBoots);

        let encoded = encode(&slots);
        assert_eq!(encoded.split('/').count(), HeroGearSlot::COUNT);
        assert_eq!(decode(&encoded), slots);

        let legacy = decode("0/3/6");
        assert_eq!(legacy[HeroGearSlot::Armor.idx()], Some(HeroGear::VowPlate));
        assert_eq!(
            legacy[HeroGearSlot::Charm.idx()],
            Some(HeroGear::ThunderCharm)
        );
        assert_eq!(
            legacy[HeroGearSlot::Relic.idx()],
            Some(HeroGear::ForgeGauntlet)
        );
        assert_eq!(legacy[HeroGearSlot::Boots.idx()], None);
    }

    #[test]
    fn boots_contribute_distinct_build_stats() {
        let mut slots = empty_gear();
        equip(&mut slots, HeroGear::EngineerTreads);
        let engineer = gear_stats(&slots);
        assert!(engineer.move_mult > 1.0);
        assert!(engineer.tower_haste_add > 0.0);
        assert!(engineer.skill_cooldown_reduction > 0);

        equip(&mut slots, HeroGear::SummonerGreaves);
        let summoner = gear_stats(&slots);
        assert!(summoner.summon_power_add > 0.0);
        assert!(summoner.hp_mult > 1.0);
    }

    #[test]
    fn weapon_affinity_stats_are_weapon_specific() {
        let mut slots = empty_gear();
        equip(&mut slots, HeroGear::SummonerGreaves);
        let summon = weapon_affinity_stats(&slots, HeroWeapon::SummonStaff);
        let sword = weapon_affinity_stats(&slots, HeroWeapon::BannerSword);

        assert!(HeroGear::SummonerGreaves.has_weapon_affinity(HeroWeapon::SummonStaff));
        assert!(!HeroGear::SummonerGreaves.has_weapon_affinity(HeroWeapon::BannerSword));
        assert!(summon.summon_power_add > sword.summon_power_add);
        assert!(summon.hp_mult > sword.hp_mult);

        equip(&mut slots, HeroGear::SentryScope);
        let sentry = weapon_affinity_stats(&slots, HeroWeapon::SentryCrossbow);
        let shield = weapon_affinity_stats(&slots, HeroWeapon::OathShield);
        assert!(sentry.range_mult > shield.range_mult);
        assert!(sentry.armor_pierce > shield.armor_pierce);
    }

    #[test]
    fn multi_piece_affinity_triggers_weapon_resonance() {
        let mut slots = empty_gear();
        equip(&mut slots, HeroGear::StarweaveRobe);
        equip(&mut slots, HeroGear::SaintBell);

        let two_piece = weapon_affinity_stats(&slots, HeroWeapon::SummonStaff);
        assert_eq!(weapon_affinity_count(&slots, HeroWeapon::SummonStaff), 2);
        assert!(two_piece.damage_mult > 1.0);
        assert!(
            weapon_resonance_summary(&slots, HeroWeapon::SummonStaff)
                .unwrap()
                .contains("初鸣")
        );
        assert!(
            weapon_resonance_detail(&slots, HeroWeapon::SummonStaff)
                .unwrap()
                .contains("异界眷属")
        );

        equip(&mut slots, HeroGear::RiftIdol);
        let three_piece = weapon_affinity_stats(&slots, HeroWeapon::SummonStaff);
        assert_eq!(weapon_affinity_count(&slots, HeroWeapon::SummonStaff), 3);
        assert!(three_piece.summon_power_add > two_piece.summon_power_add);

        equip(&mut slots, HeroGear::SummonerGreaves);
        assert_eq!(weapon_affinity_count(&slots, HeroWeapon::SummonStaff), 4);
        assert!(
            weapon_resonance_summary(&slots, HeroWeapon::SummonStaff)
                .unwrap()
                .contains("满鸣")
        );
    }

    #[test]
    fn summary_uses_active_weapon_stats_and_route() {
        let mut slots = empty_gear();
        equip(&mut slots, HeroGear::StarweaveRobe);
        equip(&mut slots, HeroGear::SaintBell);
        equip(&mut slots, HeroGear::RiftIdol);
        equip(&mut slots, HeroGear::SummonerGreaves);

        let base = gear_stats(&slots);
        let active = active_stats_for_weapon(&slots, HeroWeapon::SummonStaff);
        assert!(active.summon_power_add > base.summon_power_add);
        assert!(active.skill_mult > base.skill_mult);

        let summary = summary_for_weapon(&slots, Some(HeroWeapon::SummonStaff));
        assert!(summary.contains("满鸣"));
        assert!(summary.contains("异界眷属"));
        assert!(summary.contains("召唤+"));
    }

    #[test]
    fn resonance_routes_create_distinct_weapon_builds() {
        let mut shadow_slots = empty_gear();
        equip(&mut shadow_slots, HeroGear::WindrunnerCloak);
        equip(&mut shadow_slots, HeroGear::NightMask);
        equip(&mut shadow_slots, HeroGear::BrassCompass);

        let mut hammer_slots = empty_gear();
        equip(&mut hammer_slots, HeroGear::WildhideHarness);
        equip(&mut hammer_slots, HeroGear::ClockworkBadge);
        equip(&mut hammer_slots, HeroGear::ForgeGauntlet);

        let shadow = weapon_affinity_stats(&shadow_slots, HeroWeapon::ShadowBow);
        let hammer = weapon_affinity_stats(&hammer_slots, HeroWeapon::ForgeHammer);
        assert!(shadow.gold_bonus_add > hammer.gold_bonus_add);
        assert!(shadow.armor_pierce > hammer.armor_pierce);
        assert!(hammer.tower_haste_add > shadow.tower_haste_add);
        assert!(hammer.skill_cooldown_reduction > shadow.skill_cooldown_reduction);

        assert!(
            weapon_resonance_detail(&shadow_slots, HeroWeapon::ShadowBow)
                .unwrap()
                .contains("猎影赏金")
        );
        assert!(
            weapon_resonance_detail(&hammer_slots, HeroWeapon::ForgeHammer)
                .unwrap()
                .contains("守卫工坊")
        );
    }

    #[test]
    fn signature_items_create_route_specific_stats() {
        let mut summon_slots = empty_gear();
        equip(&mut summon_slots, HeroGear::StarweaveRobe);
        equip(&mut summon_slots, HeroGear::MythcallerTotem);
        equip(&mut summon_slots, HeroGear::RiftIdol);
        equip(&mut summon_slots, HeroGear::SummonerGreaves);
        let summon = active_stats_for_weapon(&summon_slots, HeroWeapon::SummonStaff);
        assert!(summon.summon_power_add >= 0.85);
        assert!(summon.skill_cooldown_reduction >= 2);

        let mut forge_slots = empty_gear();
        equip(&mut forge_slots, HeroGear::WildhideHarness);
        equip(&mut forge_slots, HeroGear::ClockworkBadge);
        equip(&mut forge_slots, HeroGear::GolemBlueprint);
        equip(&mut forge_slots, HeroGear::EngineerTreads);
        let forge = active_stats_for_weapon(&forge_slots, HeroWeapon::ForgeHammer);
        assert!(forge.tower_haste_add >= 0.35);
        assert!(forge.skill_cooldown_reduction >= 4);

        let mut bounty_slots = empty_gear();
        equip(&mut bounty_slots, HeroGear::WindrunnerCloak);
        equip(&mut bounty_slots, HeroGear::BountyQuiver);
        equip(&mut bounty_slots, HeroGear::BrassCompass);
        equip(&mut bounty_slots, HeroGear::CarrotWings);
        let bounty = active_stats_for_weapon(&bounty_slots, HeroWeapon::ShadowBow);
        assert!(bounty.gold_bonus_add >= 0.40);
        assert!(bounty.move_mult > forge.move_mult);
    }

    #[test]
    fn clear_reward_weights_favor_current_weapon_affinity() {
        let summon_specific =
            reward_affinity_weight(HeroGear::SummonerGreaves, HeroWeapon::SummonStaff);
        let unrelated = reward_affinity_weight(HeroGear::SummonerGreaves, HeroWeapon::BannerSword);
        let universal = reward_affinity_weight(HeroGear::CarrotWings, HeroWeapon::BannerSword);

        assert!(summon_specific > universal);
        assert!(universal > unrelated);
        assert_eq!(
            reward_affinity_weight(HeroGear::SentryScope, HeroWeapon::SentryCrossbow),
            summon_specific
        );
    }
}

#[cfg(target_arch = "wasm32")]
fn load_inventory_counts() -> [u32; HeroGear::ALL.len()] {
    decode_counts(&load_hero_gear_js())
}

#[cfg(target_arch = "wasm32")]
fn save_inventory_counts(counts: &[u32; HeroGear::ALL.len()]) {
    save_hero_gear_js(&encode_counts(counts));
}

#[cfg(not(target_arch = "wasm32"))]
fn load_inventory_counts() -> [u32; HeroGear::ALL.len()] {
    decode_counts(&std::fs::read_to_string("tmp/hero_gear.txt").unwrap_or_default())
}

#[cfg(not(target_arch = "wasm32"))]
fn save_inventory_counts(counts: &[u32; HeroGear::ALL.len()]) {
    let _ = std::fs::create_dir_all("tmp");
    let _ = std::fs::write("tmp/hero_gear.txt", encode_counts(counts));
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(inline_js = r#"
export function load_hero_gear() {
  try { return window.localStorage.getItem('protect_carrot_hero_gear') || ''; } catch (e) { return ''; }
}
export function save_hero_gear(value) {
  try { window.localStorage.setItem('protect_carrot_hero_gear', value); } catch (e) {}
}
"#)]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = load_hero_gear)]
    fn load_hero_gear_js() -> String;
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = save_hero_gear)]
    fn save_hero_gear_js(value: &str);
}
