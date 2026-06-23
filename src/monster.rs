//! Commercial-scale monster catalog.
//!
//! `EnemyKind` remains the compact behavior/art archetype used by systems. This
//! catalog adds 100 runtime species on top: each species has its own name,
//! unlock timing, stat tuning, elemental profile, and bestiary identity.

use crate::data::{Element, ElementProfile, EnemyDef, EnemyKind, BOSS_WAVE_INTERVAL};
use bevy::prelude::Color;

/// A monster's quality tier (品级) for the bestiary, derived from its threat
/// (appearance depth + stat multipliers), with a dedicated Boss tier.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MonsterGrade {
    Common,
    Elite,
    Rare,
    Epic,
    Boss,
}

impl MonsterGrade {
    /// Chinese display name (translated at the display site via i18n::t).
    pub fn name(self) -> &'static str {
        match self {
            MonsterGrade::Common => "普通",
            MonsterGrade::Elite => "精英",
            MonsterGrade::Rare => "稀有",
            MonsterGrade::Epic => "史诗",
            MonsterGrade::Boss => "首领",
        }
    }

    pub fn color(self) -> Color {
        match self {
            MonsterGrade::Common => Color::srgb(0.72, 0.75, 0.67),
            MonsterGrade::Elite => Color::srgb(0.27, 0.83, 0.51),
            MonsterGrade::Rare => Color::srgb(0.29, 0.64, 1.0),
            MonsterGrade::Epic => Color::srgb(0.71, 0.43, 1.0),
            MonsterGrade::Boss => Color::srgb(1.0, 0.31, 0.43),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MonsterSpecies {
    pub id: usize,
    pub name: &'static str,
    pub kind: EnemyKind,
    /// Zero-based level index where this species can first appear.
    pub min_level: usize,
    pub min_wave: i32,
    pub hp_mult: f32,
    pub speed_mult: f32,
    pub reward_mult: f32,
    pub armor_add: f32,
    pub magic_resist_add: f32,
    /// Additive elemental resistance/vulnerability layered over `EnemyDef`.
    pub resist: ElementProfile,
    pub tags: &'static str,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum BossSkill {
    None,
    SerpentRush,
    AbyssalShield,
    YellowSilence,
    StormSurge,
    FurnaceBurn,
    BroodHeal,
    VoidPhase,
    StarforgedBulwark,
    MossCrush,
    DreamEclipse,
}

impl BossSkill {
    /// File stem of this boss's portrait under `assets/sprites/bosses/`, or None.
    pub fn portrait_name(self) -> Option<&'static str> {
        Some(match self {
            BossSkill::None => return None,
            BossSkill::SerpentRush => "serpent",
            BossSkill::AbyssalShield => "abyssal",
            BossSkill::YellowSilence => "yellow",
            BossSkill::StormSurge => "storm",
            BossSkill::FurnaceBurn => "furnace",
            BossSkill::BroodHeal => "brood",
            BossSkill::VoidPhase => "void",
            BossSkill::StarforgedBulwark => "starforged",
            BossSkill::MossCrush => "moss",
            BossSkill::DreamEclipse => "dream",
        })
    }

    pub const ALL: [BossSkill; 10] = [
        BossSkill::SerpentRush,
        BossSkill::AbyssalShield,
        BossSkill::YellowSilence,
        BossSkill::StormSurge,
        BossSkill::FurnaceBurn,
        BossSkill::BroodHeal,
        BossSkill::VoidPhase,
        BossSkill::StarforgedBulwark,
        BossSkill::MossCrush,
        BossSkill::DreamEclipse,
    ];
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EliteAffix {
    None,
    Frenzy,
    Carapace,
    YellowSign,
    Brood,
    Bloodrite,
    Siege,
}

impl EliteAffix {
    pub const ALL: [EliteAffix; 6] = [
        EliteAffix::Frenzy,
        EliteAffix::Carapace,
        EliteAffix::YellowSign,
        EliteAffix::Brood,
        EliteAffix::Bloodrite,
        EliteAffix::Siege,
    ];

    pub fn name(self) -> &'static str {
        match self {
            EliteAffix::None => "",
            EliteAffix::Frenzy => "狂乱",
            EliteAffix::Carapace => "硬壳",
            EliteAffix::YellowSign => "黄印",
            EliteAffix::Brood => "孵化",
            EliteAffix::Bloodrite => "血祭",
            EliteAffix::Siege => "攻城",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            EliteAffix::None => "",
            EliteAffix::Frenzy => "高速突进，近战更凶",
            EliteAffix::Carapace => "额外护甲与护盾",
            EliteAffix::YellowSign => "携带小型静默场",
            EliteAffix::Brood => "死亡时孵化更多幼体",
            EliteAffix::Bloodrite => "缓慢回血并治疗附近眷族",
            EliteAffix::Siege => "离路攻击防御塔",
        }
    }

    pub fn available(self, wave: i32, level_index: usize) -> bool {
        match self {
            EliteAffix::None => false,
            EliteAffix::Frenzy | EliteAffix::Carapace => wave >= 4,
            EliteAffix::Brood => wave >= 6 && level_index >= 3,
            EliteAffix::YellowSign => wave >= 8 && level_index >= 5,
            EliteAffix::Bloodrite => wave >= 8 && level_index >= 6,
            EliteAffix::Siege => wave >= 9 && level_index >= 7,
        }
    }
}

impl BossSkill {
    pub fn name(self) -> &'static str {
        match self {
            BossSkill::None => "",
            BossSkill::SerpentRush => "蛇父疾袭",
            BossSkill::AbyssalShield => "蓝潮护幕",
            BossSkill::YellowSilence => "黄印静默",
            BossSkill::StormSurge => "雷暴跃迁",
            BossSkill::FurnaceBurn => "赤星焚炉",
            BossSkill::BroodHeal => "育母织巢",
            BossSkill::VoidPhase => "虚空相位",
            BossSkill::StarforgedBulwark => "星金壁垒",
            BossSkill::MossCrush => "菌毯塌陷",
            BossSkill::DreamEclipse => "梦蚀终眠",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            BossSkill::None => "",
            BossSkill::SerpentRush => "突进并获得少量护盾",
            BossSkill::AbyssalShield => "为周围怪物补护盾",
            BossSkill::YellowSilence => "延迟范围内防御塔开火",
            BossSkill::StormSurge => "跃迁并震荡防御塔",
            BossSkill::FurnaceBurn => "灼烧范围内防御塔",
            BossSkill::BroodHeal => "治疗盟友并孵化蛛群",
            BossSkill::VoidPhase => "短暂隐形并获得护盾",
            BossSkill::StarforgedBulwark => "为周围怪物建立壁垒",
            BossSkill::MossCrush => "震裂并压制附近防御塔",
            BossSkill::DreamEclipse => "大范围静默并唤醒梦魇",
        }
    }

    pub fn cooldown(self) -> f32 {
        match self {
            BossSkill::None => f32::INFINITY,
            BossSkill::SerpentRush => 5.5,
            BossSkill::AbyssalShield => 7.0,
            BossSkill::YellowSilence => 7.5,
            BossSkill::StormSurge => 6.0,
            BossSkill::FurnaceBurn => 6.5,
            BossSkill::BroodHeal => 8.0,
            BossSkill::VoidPhase => 7.0,
            BossSkill::StarforgedBulwark => 7.5,
            BossSkill::MossCrush => 6.5,
            BossSkill::DreamEclipse => 8.0,
        }
    }
}

pub fn species_skill(species: &MonsterSpecies) -> (&'static str, &'static str) {
    let boss = boss_skill(species.id);
    if boss != BossSkill::None {
        return (boss.name(), boss.description());
    }

    match species.kind {
        EnemyKind::Normal => ("稳步压迫", "基础单位，靠波次成长和元素抗性施压"),
        EnemyKind::Fast => ("疾行穿线", "移动速度更高，适合用冰霜、击退和眩晕打断"),
        EnemyKind::Tank => ("厚皮推进", "生命和护甲很高，会拖住火力窗口"),
        EnemyKind::Flying => ("空袭", "飞行单位，部分防御塔无法锁定"),
        EnemyKind::Invisible => ("潜伏", "隐形接近，未被侦测时普通防御塔无法锁定"),
        EnemyKind::Regenerating => ("再生", "持续回复生命，爆发伤害优先级更高"),
        EnemyKind::Armored => ("重甲", "物理减伤强，秘法和破甲更有效"),
        EnemyKind::Swarmer => ("群聚", "个体脆弱但速度和刷新压力高，低收益消耗火力"),
        EnemyKind::Boss => ("首领威压", "高生命、高抗性，并按物种释放首领技能"),
        EnemyKind::Shielded => ("护盾", "先消耗护盾条，溢出伤害才会伤及生命"),
        EnemyKind::Splitter => ("分裂", "死亡后孵化小型怪物，范围伤害能降低残局压力"),
        EnemyKind::Healer => ("治疗光环", "持续治疗附近怪物，建议优先集火"),
        EnemyKind::Charger => ("冲锋破阵", "周期性突进，并会寻找附近防御塔撕咬"),
        EnemyKind::Climber => ("攻塔攀附", "离开路线攻击防御塔，威胁塔阵边缘"),
        EnemyKind::Silencer => ("静默场", "范围内防御塔开火被压制，需要拉开站位"),
        EnemyKind::Moss => ("吞塔", "首领级攻塔单位，会摧毁或重创防御塔"),
    }
}

/// One entry in the monster-skill codex (技能图鉴): the skill, what it does, and
/// how its three tiers (普通/中级/高级) differ. The tier of a given monster's
/// skills is derived from its grade (see [`SkillTier::from_grade`]).
pub struct SkillCodexEntry {
    pub icon: &'static str,
    pub name: &'static str, // zh
    pub desc: &'static str, // zh
    pub tiers: &'static str, // zh — how 普通/中级/高级 differ
}

/// The full catalogue of monster skills, for the bestiary's skill codex screen.
pub fn skill_codex() -> &'static [SkillCodexEntry] {
    &[
        SkillCodexEntry {
            icon: "🪓",
            name: "分裂",
            desc: "死亡后分裂成更小的同类，每一代体型与属性减半",
            tiers: "普通：分裂 1 代 · 中级：2 代 · 高级：4 代",
        },
        SkillCodexEntry {
            icon: "❤️",
            name: "再生",
            desc: "持续回复自身生命，需要爆发集火压制",
            tiers: "普通 ×1 · 中级 ×1.5 · 高级 ×2（回复速度）",
        },
        SkillCodexEntry {
            icon: "🛡️",
            name: "护盾",
            desc: "先消耗护盾条，溢出伤害才会伤及生命",
            tiers: "普通 ×1 · 中级 ×1.5 · 高级 ×2（护盾值）",
        },
        SkillCodexEntry {
            icon: "✨",
            name: "治疗",
            desc: "治疗光环持续为附近怪物回血，建议优先集火",
            tiers: "普通 ×1 · 中级 ×1.5 · 高级 ×2（治疗量）",
        },
        SkillCodexEntry {
            icon: "🔇",
            name: "静默",
            desc: "范围内防御塔被压制，无法开火",
            tiers: "普通 ×1 · 中级 ×1.5 · 高级 ×2（静默范围）",
        },
        SkillCodexEntry {
            icon: "⚔️",
            name: "攻塔",
            desc: "离开路线撕咬、摧毁防御塔",
            tiers: "普通 ×1 · 中级 ×1.5 · 高级 ×2（拆塔伤害）",
        },
        SkillCodexEntry {
            icon: "🪨",
            name: "硬化",
            desc: "护甲与魔抗强化，物理减伤明显，需破甲或秘法",
            tiers: "普通 ×1 · 中级 ×1.5 · 高级 ×2（护甲/魔抗）",
        },
        SkillCodexEntry {
            icon: "🪽",
            name: "飞行",
            desc: "无视地形，走最短直线扑向萝卜；部分塔无法锁定",
            tiers: "普通：常速 · 中级/高级：飞行更快，更难拦截",
        },
        SkillCodexEntry {
            icon: "👻",
            name: "隐形",
            desc: "潜行接近，未被侦测时无法被锁定",
            tiers: "三级均需侦测塔或反隐英雄揭示",
        },
        SkillCodexEntry {
            icon: "💨",
            name: "冲锋",
            desc: "周期性突进提速，并寻找附近防御塔撕咬",
            tiers: "突进强度随品级提升",
        },
        SkillCodexEntry {
            icon: "🌀",
            name: "吞塔",
            desc: "首领级技能，会直接摧毁或重创防御塔",
            tiers: "首领专属，威压最强",
        },
    ]
}

impl MonsterSpecies {
    pub fn def(self) -> &'static EnemyDef {
        self.kind.def()
    }

    pub fn is_boss(self) -> bool {
        self.kind.def().boss
    }

    pub fn available(self, wave: i32, level_index: usize) -> bool {
        wave >= self.min_wave && level_index >= self.min_level
    }

    pub fn armor(self) -> f32 {
        (self.def().armor + self.armor_add).max(0.0)
    }

    pub fn magic_resist(self) -> f32 {
        (self.def().magic_resist + self.magic_resist_add).max(0.0)
    }

    pub fn resist_profile(self) -> ElementProfile {
        add_profile(self.def().resist, self.resist)
    }

    /// Quality tier (品级) for the bestiary. Bosses get the dedicated Boss tier;
    /// everyone else is bucketed by a threat score (appearance depth + stat mods),
    /// with thresholds calibrated to the species distribution (~quartiles).
    pub fn grade(self) -> MonsterGrade {
        if self.tags.contains("首领") || self.def().boss {
            return MonsterGrade::Boss;
        }
        let score = self.min_level as f32
            + (self.hp_mult - 1.0) * 4.0
            + (self.armor_add + self.magic_resist_add) / 15.0
            + (self.reward_mult - 1.0) * 1.5;
        if score >= 20.0 {
            MonsterGrade::Epic
        } else if score >= 15.0 {
            MonsterGrade::Rare
        } else if score >= 9.0 {
            MonsterGrade::Elite
        } else {
            MonsterGrade::Common
        }
    }

    pub fn traits(self) -> String {
        let def = self.def();
        // Every active skill carries a tier (普通/中级/高级) derived from grade, so
        // each ability tag is suffixed with its level, e.g. 再生·高级 / Regenerate·Advanced.
        let tier = SkillTier::from_grade(self.grade());
        let tier_label = crate::i18n::t(tier.label());
        let skill = |base: &str| format!("{}·{}", crate::i18n::t(base), tier_label);
        // Descriptive species-category tags (e.g. 犬群/火焰) are not skills — keep plain.
        let mut tags: Vec<String> = self
            .tags
            .split('/')
            .filter(|s| !s.is_empty())
            .map(crate::i18n::t)
            .collect();
        if def.boss {
            tags.push(crate::i18n::t("BOSS"));
        }
        if def.flying {
            tags.push(skill("飞行"));
        }
        if def.invisible {
            tags.push(skill("隐形"));
        }
        if def.regen > 0.0 {
            tags.push(skill("再生"));
        }
        if def.shield > 0.0 {
            tags.push(skill("护盾"));
        }
        if def.splits > 0 {
            tags.push(skill("分裂"));
        }
        if def.heal_aura > 0.0 {
            tags.push(skill("治疗"));
        }
        if def.charger {
            tags.push(skill("冲锋"));
        }
        if def.tower_raider {
            tags.push(skill("攻塔"));
        }
        if def.silence_aura > 0.0 {
            tags.push(skill("静默"));
        }
        if def.moss_destroy {
            tags.push(skill("吞塔"));
        }
        tags.sort_unstable();
        tags.dedup();
        if tags.is_empty() {
            crate::i18n::t("普通")
        } else {
            tags.join("/")
        }
    }
}

/// Tier of a monster's active skill (普通 / 中级 / 高级), derived from its grade.
/// Higher-grade monsters wield stronger versions of the same ability — e.g. a
/// 中级 splitter splits 2 generations, a 高级 splitter 4, each generation halving
/// the splinter's body size and stats.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SkillTier {
    Common,       // 普通
    Intermediate, // 中级
    Advanced,     // 高级
}

impl SkillTier {
    pub fn from_grade(grade: MonsterGrade) -> Self {
        match grade {
            MonsterGrade::Common => SkillTier::Common,
            MonsterGrade::Elite | MonsterGrade::Rare => SkillTier::Intermediate,
            MonsterGrade::Epic | MonsterGrade::Boss => SkillTier::Advanced,
        }
    }

    /// How many generations a splitter of this tier can split.
    pub fn split_generations(self) -> i32 {
        match self {
            SkillTier::Common => 1,
            SkillTier::Intermediate => 2,
            SkillTier::Advanced => 4,
        }
    }

    /// Magnitude multiplier applied to a monster's numeric ability stats (regen,
    /// shield, heal aura, silence radius, tower-raid dps …). 普通 leaves stats as
    /// authored; 中级/高级 make the same skill progressively stronger.
    pub fn power_mult(self) -> f32 {
        match self {
            SkillTier::Common => 1.0,
            SkillTier::Intermediate => 1.5,
            SkillTier::Advanced => 2.0,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SkillTier::Common => "普通",
            SkillTier::Intermediate => "中级",
            SkillTier::Advanced => "高级",
        }
    }
}

pub fn boss_skill(species_id: usize) -> BossSkill {
    match species_id {
        90 => BossSkill::SerpentRush,
        91 => BossSkill::AbyssalShield,
        92 => BossSkill::YellowSilence,
        93 => BossSkill::StormSurge,
        94 => BossSkill::FurnaceBurn,
        95 => BossSkill::BroodHeal,
        96 => BossSkill::VoidPhase,
        97 => BossSkill::StarforgedBulwark,
        98 => BossSkill::MossCrush,
        99 => BossSkill::DreamEclipse,
        _ => BossSkill::None,
    }
}

pub fn is_boss_wave(wave: i32, total_waves: i32) -> bool {
    wave > 0 && total_waves > 0 && (wave == total_waves || wave % BOSS_WAVE_INTERVAL == 0)
}

pub fn next_boss_wave(current_wave: i32, total_waves: i32) -> Option<i32> {
    ((current_wave + 1)..=total_waves).find(|wave| is_boss_wave(*wave, total_waves))
}

pub fn elite_affix_pool(wave: i32, level_index: usize) -> Vec<EliteAffix> {
    EliteAffix::ALL
        .into_iter()
        .filter(|affix| affix.available(wave, level_index))
        .collect()
}

pub fn pick_elite_affix(wave: i32, level_index: usize, roll: f32) -> EliteAffix {
    let pool = elite_affix_pool(wave, level_index);
    if pool.is_empty() {
        EliteAffix::Frenzy
    } else {
        let index =
            ((roll.clamp(0.0, 0.999_999) * pool.len() as f32).floor() as usize).min(pool.len() - 1);
        pool[index]
    }
}

const N: ElementProfile = ElementProfile::none();
const FLESH: ElementProfile = ElementProfile::new(0.0, 0.0, -0.12, 0.0, 0.0, 0.0, 0.08);
const CHITIN: ElementProfile = ElementProfile::new(0.10, -0.08, 0.0, 0.0, -0.10, 0.0, 0.0);
const FIRE: ElementProfile = ElementProfile::new(0.0, 0.0, 0.28, -0.20, 0.0, 0.0, 0.0);
const FROST: ElementProfile = ElementProfile::new(0.0, 0.0, -0.18, 0.28, 0.0, 0.0, 0.0);
const STORM: ElementProfile = ElementProfile::new(0.0, 0.0, 0.0, 0.0, 0.30, -0.10, 0.0);
const SHADOW: ElementProfile = ElementProfile::new(0.0, -0.10, 0.0, 0.0, 0.0, 0.32, 0.0);
const TOXIC: ElementProfile = ElementProfile::new(0.0, 0.0, -0.08, 0.0, 0.0, 0.0, 0.35);
const ARCANE: ElementProfile = ElementProfile::new(0.0, 0.30, 0.0, 0.0, -0.12, 0.0, 0.0);
const VOID: ElementProfile = ElementProfile::new(0.05, 0.18, 0.0, 0.0, 0.0, 0.28, -0.12);

const fn add_profile(a: ElementProfile, b: ElementProfile) -> ElementProfile {
    ElementProfile::new(
        a.physical + b.physical,
        a.arcane + b.arcane,
        a.fire + b.fire,
        a.frost + b.frost,
        a.storm + b.storm,
        a.shadow + b.shadow,
        a.toxic + b.toxic,
    )
}

macro_rules! sp {
    ($id:expr, $name:expr, $kind:ident, $level:expr, $wave:expr, $hp:expr, $speed:expr, $reward:expr, $armor:expr, $mr:expr, $resist:expr, $tags:expr) => {
        MonsterSpecies {
            id: $id,
            name: $name,
            kind: EnemyKind::$kind,
            min_level: $level,
            min_wave: $wave,
            hp_mult: $hp,
            speed_mult: $speed,
            reward_mult: $reward,
            armor_add: $armor,
            magic_resist_add: $mr,
            resist: $resist,
            tags: $tags,
        }
    };
}

pub static MONSTER_SPECIES: &[MonsterSpecies] = &[
    sp!(
        0,
        "黑骨蠕虫",
        Normal,
        0,
        1,
        1.00,
        1.00,
        1.00,
        0.0,
        0.0,
        FLESH,
        "骨虫/血肉"
    ),
    sp!(
        1,
        "寒骨疾虫",
        Fast,
        0,
        2,
        0.92,
        1.08,
        1.08,
        0.0,
        0.0,
        N,
        "骨虫/冰霜/疾行"
    ),
    sp!(
        2,
        "毒骨虫群",
        Swarmer,
        0,
        2,
        0.82,
        1.10,
        0.95,
        0.0,
        0.0,
        TOXIC,
        "骨虫/剧毒/群聚"
    ),
    sp!(
        3,
        "白骨浮蛭",
        Flying,
        1,
        4,
        1.00,
        1.00,
        1.12,
        0.0,
        0.0,
        STORM,
        "骨虫/空袭"
    ),
    sp!(
        4,
        "黑狱三头犬",
        Shielded,
        1,
        4,
        1.05,
        0.96,
        1.16,
        2.0,
        0.0,
        CHITIN,
        "犬群/护盾"
    ),
    sp!(
        5,
        "苔影三头犬",
        Invisible,
        2,
        5,
        0.95,
        1.04,
        1.18,
        0.0,
        4.0,
        SHADOW,
        "犬群/潜伏"
    ),
    sp!(
        6,
        "赤牙裂犬",
        Splitter,
        2,
        5,
        1.05,
        0.95,
        1.20,
        0.0,
        0.0,
        FLESH,
        "犬群/分裂/火焰"
    ),
    sp!(
        7,
        "白鬃再生犬",
        Regenerating,
        3,
        6,
        1.06,
        0.96,
        1.18,
        2.0,
        2.0,
        TOXIC,
        "犬群/再生"
    ),
    sp!(
        8,
        "黑泥愈合体",
        Healer,
        3,
        6,
        1.00,
        0.98,
        1.22,
        0.0,
        8.0,
        ARCANE,
        "软泥/治疗"
    ),
    sp!(
        9,
        "蓝晶泥甲",
        Armored,
        4,
        7,
        1.10,
        0.92,
        1.24,
        8.0,
        0.0,
        CHITIN,
        "软泥/重甲"
    ),
    sp!(
        10,
        "绿沼攀附体",
        Climber,
        4,
        8,
        1.05,
        1.02,
        1.28,
        4.0,
        0.0,
        FLESH,
        "软泥/攻塔"
    ),
    sp!(
        11,
        "赤泥噤声体",
        Silencer,
        5,
        10,
        1.00,
        0.98,
        1.30,
        0.0,
        8.0,
        SHADOW,
        "软泥/静默"
    ),
    sp!(
        12,
        "黑鼠疾影",
        Fast,
        5,
        5,
        0.88,
        1.18,
        1.16,
        0.0,
        0.0,
        FIRE,
        "鼠群/疾行"
    ),
    sp!(
        13,
        "棕鼠厚背",
        Tank,
        5,
        6,
        1.18,
        0.90,
        1.26,
        6.0,
        2.0,
        FROST,
        "鼠群/厚血"
    ),
    sp!(
        14,
        "绿鼠滑翔者",
        Flying,
        5,
        7,
        1.04,
        1.05,
        1.28,
        0.0,
        8.0,
        ARCANE,
        "鼠群/空袭"
    ),
    sp!(
        15,
        "白鼠护符",
        Shielded,
        6,
        7,
        1.14,
        0.95,
        1.30,
        7.0,
        2.0,
        CHITIN,
        "鼠群/护盾"
    ),
    sp!(
        16,
        "黑蝎幼群",
        Swarmer,
        6,
        5,
        0.76,
        1.22,
        1.05,
        0.0,
        0.0,
        TOXIC,
        "蝎群/群聚"
    ),
    sp!(
        17,
        "蓝影潜蝎",
        Invisible,
        6,
        8,
        1.00,
        1.08,
        1.32,
        0.0,
        6.0,
        VOID,
        "蝎群/潜伏"
    ),
    sp!(
        18,
        "绿针冲蝎",
        Charger,
        6,
        7,
        1.08,
        1.02,
        1.32,
        5.0,
        0.0,
        CHITIN,
        "蝎群/冲锋"
    ),
    sp!(
        19,
        "白针疗蝎",
        Healer,
        7,
        8,
        1.08,
        0.94,
        1.35,
        2.0,
        10.0,
        ARCANE,
        "蝎群/治疗"
    ),
    sp!(
        20,
        "蓝焰颅灵",
        Normal,
        7,
        6,
        1.12,
        0.96,
        1.20,
        0.0,
        3.0,
        FROST,
        "颅火/冰霜"
    ),
    sp!(
        21,
        "绿焰裂颅",
        Splitter,
        7,
        7,
        1.10,
        0.96,
        1.32,
        0.0,
        0.0,
        FIRE,
        "颅火/分裂/剧毒"
    ),
    sp!(
        22,
        "赤焰再生颅",
        Regenerating,
        7,
        8,
        1.18,
        0.95,
        1.34,
        3.0,
        4.0,
        TOXIC,
        "颅火/再生/火焰"
    ),
    sp!(
        23,
        "紫焰飞颅",
        Flying,
        8,
        8,
        1.08,
        1.08,
        1.35,
        2.0,
        0.0,
        SHADOW,
        "颅火/空袭/暗影"
    ),
    sp!(
        24,
        "蓝胶重甲",
        Armored,
        8,
        8,
        1.22,
        0.88,
        1.38,
        14.0,
        4.0,
        CHITIN,
        "史莱姆/重甲/冰霜"
    ),
    sp!(
        25,
        "绿胶护盾",
        Shielded,
        8,
        8,
        1.12,
        0.96,
        1.36,
        4.0,
        4.0,
        STORM,
        "史莱姆/护盾/剧毒"
    ),
    sp!(
        26,
        "黄胶噤声",
        Silencer,
        8,
        10,
        1.08,
        1.00,
        1.40,
        0.0,
        12.0,
        SHADOW,
        "史莱姆/静默/黄印"
    ),
    sp!(
        27,
        "赤胶拆塔体",
        Climber,
        8,
        9,
        1.16,
        1.02,
        1.42,
        7.0,
        0.0,
        CHITIN,
        "史莱姆/攻塔/火焰"
    ),
    sp!(
        28,
        "黑蛛急袭者",
        Fast,
        9,
        8,
        0.96,
        1.20,
        1.32,
        0.0,
        2.0,
        FIRE,
        "蛛群/疾行"
    ),
    sp!(
        29,
        "蓝甲巨蛛",
        Tank,
        9,
        8,
        1.28,
        0.86,
        1.45,
        8.0,
        8.0,
        FROST,
        "蛛群/厚血"
    ),
    sp!(
        30,
        "绿蛛跳跃者",
        Flying,
        9,
        9,
        1.00,
        1.10,
        1.40,
        0.0,
        4.0,
        TOXIC,
        "蛛群/空袭/剧毒"
    ),
    sp!(
        31,
        "白蛛镜盾",
        Shielded,
        9,
        9,
        1.20,
        0.92,
        1.46,
        10.0,
        10.0,
        ARCANE,
        "蛛群/护盾"
    ),
    sp!(
        32,
        "黑狼潜猎者",
        Invisible,
        9,
        9,
        1.06,
        1.08,
        1.44,
        0.0,
        12.0,
        VOID,
        "狼群/潜伏"
    ),
    sp!(
        33,
        "蓝鬃裂狼",
        Splitter,
        9,
        9,
        1.18,
        0.92,
        1.46,
        4.0,
        2.0,
        FLESH,
        "狼群/分裂"
    ),
    sp!(
        34,
        "苔鬃狼医",
        Healer,
        10,
        9,
        1.16,
        0.92,
        1.50,
        2.0,
        14.0,
        ARCANE,
        "狼群/治疗"
    ),
    sp!(
        35,
        "白狼冲锋者",
        Charger,
        10,
        9,
        1.12,
        1.08,
        1.48,
        4.0,
        4.0,
        STORM,
        "狼群/冲锋"
    ),
    sp!(
        36,
        "黑虫铁壳",
        Armored,
        10,
        9,
        1.26,
        0.88,
        1.52,
        18.0,
        0.0,
        TOXIC,
        "虫群/重甲"
    ),
    sp!(
        37,
        "褐虫禁鸣者",
        Silencer,
        10,
        10,
        1.14,
        0.96,
        1.55,
        2.0,
        16.0,
        SHADOW,
        "虫群/静默"
    ),
    sp!(
        38,
        "绿虫食塔者",
        Climber,
        10,
        10,
        1.22,
        1.03,
        1.56,
        10.0,
        0.0,
        CHITIN,
        "虫群/攻塔"
    ),
    sp!(
        39,
        "白虫潮",
        Swarmer,
        10,
        9,
        0.84,
        1.28,
        1.30,
        0.0,
        8.0,
        ARCANE,
        "虫群/群聚"
    ),
    sp!(
        40,
        "翠羽怨灵",
        Tank,
        11,
        10,
        1.36,
        0.84,
        1.60,
        10.0,
        6.0,
        FIRE,
        "亡灵/厚血"
    ),
    sp!(
        41,
        "蓝铠急先锋",
        Fast,
        11,
        10,
        1.04,
        1.18,
        1.48,
        0.0,
        6.0,
        FROST,
        "亡灵/疾行/重甲"
    ),
    sp!(
        42,
        "雾缚幽魂",
        Flying,
        11,
        10,
        1.12,
        1.10,
        1.55,
        0.0,
        14.0,
        SHADOW,
        "亡灵/空袭"
    ),
    sp!(
        43,
        "墓井食尸鬼",
        Regenerating,
        11,
        10,
        1.34,
        0.88,
        1.62,
        8.0,
        8.0,
        TOXIC,
        "亡灵/再生"
    ),
    sp!(
        44,
        "赤袍裂魂巫",
        Splitter,
        11,
        10,
        1.24,
        0.92,
        1.58,
        0.0,
        12.0,
        ARCANE,
        "亡灵/分裂/秘法"
    ),
    sp!(
        45,
        "绿肤禁咒兽",
        Silencer,
        11,
        11,
        1.18,
        0.98,
        1.62,
        4.0,
        18.0,
        VOID,
        "亡灵/静默"
    ),
    sp!(
        46,
        "白骨盾卫",
        Shielded,
        11,
        11,
        1.28,
        0.88,
        1.66,
        12.0,
        8.0,
        CHITIN,
        "亡灵/护盾"
    ),
    sp!(
        47,
        "紫袍越墙客",
        Climber,
        12,
        11,
        1.30,
        1.00,
        1.70,
        12.0,
        4.0,
        FLESH,
        "亡灵/攻塔"
    ),
    sp!(
        48,
        "金角温迪戈群",
        Swarmer,
        12,
        10,
        0.90,
        1.24,
        1.40,
        0.0,
        10.0,
        SHADOW,
        "亡灵/群聚"
    ),
    sp!(
        49,
        "墓土僵尸",
        Normal,
        12,
        11,
        1.30,
        0.98,
        1.58,
        4.0,
        6.0,
        ElementProfile::new(0.0, 0.0, 0.18, 0.18, -0.18, 0.0, 0.0),
        "亡灵"
    ),
    sp!(
        50,
        "绛羽重怨灵",
        Armored,
        12,
        11,
        1.38,
        0.84,
        1.76,
        24.0,
        10.0,
        ARCANE,
        "亡灵/重甲"
    ),
    sp!(
        51,
        "赤盾冲锋铠",
        Charger,
        12,
        11,
        1.22,
        1.12,
        1.68,
        6.0,
        6.0,
        STORM,
        "亡灵/冲锋"
    ),
    sp!(
        52,
        "银雾无形魂",
        Invisible,
        12,
        11,
        1.16,
        1.08,
        1.70,
        0.0,
        18.0,
        VOID,
        "亡灵/潜伏"
    ),
    sp!(
        53,
        "蓝袍尸医",
        Healer,
        12,
        11,
        1.22,
        0.90,
        1.78,
        4.0,
        20.0,
        TOXIC,
        "亡灵/治疗"
    ),
    sp!(
        54,
        "红冠爬塔巫",
        Climber,
        13,
        12,
        1.36,
        1.02,
        1.86,
        16.0,
        4.0,
        CHITIN,
        "亡灵/攻塔/秘法"
    ),
    sp!(
        55,
        "蓝肤缄默兽",
        Silencer,
        13,
        12,
        1.24,
        1.00,
        1.82,
        4.0,
        22.0,
        ARCANE,
        "亡灵/静默"
    ),
    sp!(
        56,
        "跃骨飞兵",
        Flying,
        13,
        12,
        1.18,
        1.12,
        1.76,
        0.0,
        10.0,
        TOXIC,
        "亡灵/空袭"
    ),
    sp!(
        57,
        "绛袍血裂者",
        Splitter,
        13,
        12,
        1.32,
        0.92,
        1.84,
        4.0,
        6.0,
        FIRE,
        "亡灵/分裂"
    ),
    sp!(
        58,
        "赤角盾温迪戈",
        Shielded,
        13,
        12,
        1.42,
        0.84,
        1.90,
        16.0,
        12.0,
        FROST,
        "亡灵/护盾"
    ),
    sp!(
        59,
        "赤肌奔尸",
        Fast,
        13,
        12,
        1.12,
        1.20,
        1.72,
        0.0,
        18.0,
        ARCANE,
        "亡灵/疾行"
    ),
    sp!(
        60,
        "黑羽复苏怨灵",
        Regenerating,
        14,
        12,
        1.46,
        0.88,
        1.92,
        8.0,
        10.0,
        TOXIC,
        "亡灵/再生"
    ),
    sp!(
        61,
        "符文咒甲卫",
        Armored,
        14,
        12,
        1.48,
        0.82,
        2.00,
        28.0,
        16.0,
        ARCANE,
        "亡灵/重甲"
    ),
    sp!(
        62,
        "灰雾飞魂",
        Flying,
        14,
        13,
        1.24,
        1.12,
        1.86,
        2.0,
        12.0,
        STORM,
        "亡灵/空袭"
    ),
    sp!(
        63,
        "蓝袍静默尸",
        Silencer,
        14,
        13,
        1.34,
        0.94,
        2.02,
        8.0,
        26.0,
        SHADOW,
        "亡灵/静默"
    ),
    sp!(
        64,
        "红袍幼巫群",
        Swarmer,
        14,
        12,
        0.96,
        1.34,
        1.62,
        0.0,
        14.0,
        VOID,
        "亡灵/群聚/秘法"
    ),
    sp!(
        65,
        "青肤拆塔兽",
        Climber,
        14,
        13,
        1.48,
        1.00,
        2.08,
        20.0,
        8.0,
        CHITIN,
        "亡灵/攻塔"
    ),
    sp!(
        66,
        "蓝骨巨兵",
        Tank,
        14,
        13,
        1.52,
        0.82,
        2.05,
        18.0,
        10.0,
        FIRE,
        "亡灵/厚血"
    ),
    sp!(
        67,
        "蓝袍血医",
        Healer,
        14,
        13,
        1.32,
        0.88,
        2.02,
        6.0,
        24.0,
        FROST,
        "亡灵/治疗"
    ),
    sp!(
        68,
        "赤角温迪戈",
        Normal,
        15,
        13,
        1.42,
        1.00,
        1.92,
        8.0,
        18.0,
        VOID,
        "亡灵"
    ),
    sp!(
        69,
        "绿肌突击尸",
        Charger,
        15,
        13,
        1.32,
        1.15,
        2.00,
        8.0,
        10.0,
        STORM,
        "亡灵/冲锋"
    ),
    sp!(
        70,
        "苔壳爬行兽",
        Armored,
        15,
        14,
        1.58,
        0.80,
        2.18,
        34.0,
        14.0,
        SHADOW,
        "深渊/重甲"
    ),
    sp!(
        71,
        "蓝晶章群母",
        Regenerating,
        15,
        14,
        1.54,
        0.86,
        2.12,
        12.0,
        16.0,
        FROST,
        "深渊/再生"
    ),
    sp!(
        72,
        "镜眼护核",
        Shielded,
        15,
        14,
        1.52,
        0.84,
        2.20,
        22.0,
        18.0,
        ARCANE,
        "深渊/护盾"
    ),
    sp!(
        73,
        "寒翼滑翔体",
        Flying,
        15,
        14,
        1.32,
        1.14,
        2.05,
        4.0,
        22.0,
        SHADOW,
        "深渊/空袭"
    ),
    sp!(
        74,
        "荧绿织卵蛛",
        Splitter,
        15,
        14,
        1.46,
        0.92,
        2.18,
        8.0,
        12.0,
        TOXIC,
        "深渊/分裂"
    ),
    sp!(
        75,
        "沉木吞噬者",
        Climber,
        16,
        14,
        1.62,
        0.98,
        2.30,
        26.0,
        10.0,
        CHITIN,
        "吞噬者/攻塔"
    ),
    sp!(
        76,
        "绯颚禁鸣者",
        Silencer,
        16,
        14,
        1.46,
        0.96,
        2.28,
        8.0,
        32.0,
        VOID,
        "吞噬者/静默"
    ),
    sp!(
        77,
        "血脊跳食者",
        Fast,
        16,
        14,
        1.26,
        1.28,
        2.08,
        4.0,
        12.0,
        FLESH,
        "吞噬者/疾行"
    ),
    sp!(
        78,
        "熔囊伏食兽",
        Normal,
        16,
        14,
        1.50,
        0.98,
        2.16,
        8.0,
        18.0,
        FIRE,
        "吞噬者/火焰"
    ),
    sp!(
        79,
        "白腭巢医",
        Healer,
        16,
        15,
        1.48,
        0.88,
        2.34,
        8.0,
        30.0,
        ARCANE,
        "吞噬者/治疗"
    ),
    sp!(
        80,
        "银舌潜食者",
        Invisible,
        16,
        15,
        1.34,
        1.12,
        2.20,
        2.0,
        34.0,
        ARCANE,
        "吞噬者/潜伏"
    ),
    sp!(
        81,
        "金脊破阵跳虫",
        Charger,
        16,
        15,
        1.42,
        1.16,
        2.28,
        12.0,
        14.0,
        STORM,
        "吞噬者/冲锋"
    ),
    sp!(
        82,
        "冰壳巨吞兽",
        Tank,
        17,
        15,
        1.74,
        0.78,
        2.42,
        24.0,
        18.0,
        FROST,
        "吞噬者/厚血/冰霜"
    ),
    sp!(
        83,
        "赤甲盾食者",
        Shielded,
        17,
        15,
        1.68,
        0.80,
        2.48,
        28.0,
        14.0,
        FIRE,
        "吞噬者/护盾"
    ),
    sp!(
        84,
        "蓝须幼食群",
        Swarmer,
        17,
        15,
        1.02,
        1.38,
        1.90,
        2.0,
        18.0,
        FROST,
        "吞噬者/群聚"
    ),
    sp!(
        85,
        "绿脊再造吞噬者",
        Regenerating,
        17,
        15,
        1.72,
        0.84,
        2.50,
        16.0,
        22.0,
        TOXIC,
        "吞噬者/再生"
    ),
    sp!(
        86,
        "蓝翼跳魇",
        Flying,
        17,
        16,
        1.46,
        1.16,
        2.38,
        6.0,
        28.0,
        VOID,
        "吞噬者/空袭"
    ),
    sp!(
        87,
        "铜甲啮塔爵",
        Climber,
        18,
        16,
        1.82,
        0.96,
        2.65,
        34.0,
        14.0,
        CHITIN,
        "吞噬者/攻塔/重甲"
    ),
    sp!(
        88,
        "绯颚静默王",
        Silencer,
        18,
        16,
        1.58,
        0.96,
        2.62,
        12.0,
        40.0,
        SHADOW,
        "吞噬者/静默"
    ),
    sp!(
        89,
        "霜壳重食兽",
        Armored,
        18,
        16,
        1.86,
        0.76,
        2.72,
        44.0,
        24.0,
        VOID,
        "吞噬者/重甲/冰霜"
    ),
    sp!(
        90,
        "赤骨蛇父",
        Boss,
        0,
        5,
        1.00,
        1.00,
        1.00,
        0.0,
        0.0,
        FLESH,
        "首领/蛇父"
    ),
    sp!(
        91,
        "蓝潮章王",
        Boss,
        4,
        5,
        1.12,
        0.96,
        1.12,
        8.0,
        10.0,
        FROST,
        "首领/深海/护盾"
    ),
    sp!(
        92,
        "黄印章主",
        Boss,
        6,
        10,
        1.18,
        0.98,
        1.18,
        6.0,
        18.0,
        SHADOW,
        "首领/邪教/静默"
    ),
    sp!(
        93,
        "雷须跃迁者",
        Boss,
        8,
        10,
        1.22,
        1.02,
        1.24,
        10.0,
        12.0,
        STORM,
        "首领/雷风"
    ),
    sp!(
        94,
        "赤星兽炉",
        Boss,
        10,
        10,
        1.28,
        0.94,
        1.30,
        14.0,
        14.0,
        FIRE,
        "首领/火焰"
    ),
    sp!(
        95,
        "育母织巢蛛",
        Boss,
        12,
        15,
        1.34,
        0.90,
        1.36,
        16.0,
        22.0,
        TOXIC,
        "首领/再生"
    ),
    sp!(
        96,
        "虚空虹眼",
        Boss,
        14,
        15,
        1.42,
        0.92,
        1.45,
        18.0,
        28.0,
        VOID,
        "首领/虚空"
    ),
    sp!(
        97,
        "星白巨章",
        Boss,
        16,
        15,
        1.55,
        0.86,
        1.56,
        34.0,
        24.0,
        ARCANE,
        "首领/重甲"
    ),
    sp!(
        98,
        "MOSS·吞噬跳虫王",
        Moss,
        4,
        10,
        1.24,
        1.00,
        1.40,
        10.0,
        10.0,
        TOXIC,
        "首领/吞塔/吞噬者"
    ),
    sp!(
        99,
        "封印下的古章",
        Moss,
        18,
        20,
        1.80,
        0.82,
        2.00,
        46.0,
        38.0,
        VOID,
        "终局首领/梦蚀"
    ),
];

pub fn default_species_id(kind: EnemyKind) -> usize {
    MONSTER_SPECIES
        .iter()
        .find(|s| s.kind == kind && !s.is_boss())
        .map(|s| s.id)
        .unwrap_or(0)
}

pub fn species_by_id(id: usize) -> Option<&'static MonsterSpecies> {
    MONSTER_SPECIES.iter().find(|s| s.id == id)
}

pub fn pick_regular(
    wave: i32,
    level_index: usize,
    rng: &mut crate::game::Rng,
) -> &'static MonsterSpecies {
    let pool: Vec<&MonsterSpecies> = MONSTER_SPECIES
        .iter()
        .filter(|s| !s.is_boss() && s.available(wave, level_index))
        .collect();
    if pool.is_empty() {
        &MONSTER_SPECIES[0]
    } else {
        pool[rng.range(pool.len())]
    }
}

pub fn pick_boss(
    wave: i32,
    total_waves: i32,
    level_index: usize,
    rng: &mut crate::game::Rng,
) -> &'static MonsterSpecies {
    if wave == total_waves && wave >= 20 && level_index >= 18 {
        return MONSTER_SPECIES
            .iter()
            .find(|s| s.id == 99)
            .unwrap_or(&MONSTER_SPECIES[99]);
    }

    let pool: Vec<&MonsterSpecies> = MONSTER_SPECIES
        .iter()
        .filter(|s| s.is_boss() && s.available(wave, level_index))
        .collect();
    if pool.is_empty() {
        MONSTER_SPECIES
            .iter()
            .find(|s| s.kind == EnemyKind::Boss)
            .unwrap_or(&MONSTER_SPECIES[90])
    } else {
        let total = total_waves.max(1) as f32;
        let level_progress = ((level_index + 1) as f32 / 20.0).clamp(0.0, 1.0);
        let wave_progress = (wave as f32 / total).clamp(0.0, 1.0);
        let progress = (level_progress * 0.65 + wave_progress * 0.35).clamp(0.0, 1.0);
        let target = ((progress * pool.len() as f32).ceil() as usize).saturating_sub(1);
        let start = target.saturating_sub(1);
        pool[start + rng.range(target - start + 1)]
    }
}

pub fn resistance_summary(profile: ElementProfile) -> Vec<String> {
    let mut out = Vec::new();
    for element in Element::ALL {
        let r = profile.get(element);
        if r.abs() >= 0.15 {
            if r > 0.0 {
                out.push(crate::i18n::tf(
                    "{}抗{}%",
                    &[&crate::i18n::t(element.name()), &((r * 100.0) as i32).to_string()],
                ));
            } else {
                out.push(crate::i18n::tf(
                    "{}弱{}%",
                    &[
                        &crate::i18n::t(element.name()),
                        &((-r * 100.0) as i32).to_string(),
                    ],
                ));
            }
        }
    }
    out
}
