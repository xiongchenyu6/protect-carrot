//! The unique **hero tower** (英雄塔): a single, movable, race + weapon defined unit.
//!
//! Unlike ordinary towers (grid-snapped, static), the hero is summoned once per run,
//! walks to a tapped destination, and fights along the way. It is implemented as a
//! regular [`Tower`] (so it reuses attack/render/HP/damage) carrying the `hero`
//! flag, a free-floating `hero_pos`, and an optional `move_target`.

use crate::data::{BOARD_H, Behavior, Element, TowerKind};
use crate::hero_gear::{self, HeroGear, HeroGearInventory, HeroGearSlot, HeroWeaponKind};
use crate::tower::{TargetPriority, Tower};
use bevy::prelude::*;

/// Hero race — a multiplicative modifier layered over the weapon base stats.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Race {
    Human,
    Elf,
    Orc,
}

impl Race {
    pub const ALL: [Race; 3] = [Race::Human, Race::Elf, Race::Orc];

    pub fn name(self) -> &'static str {
        match self {
            Race::Human => "人类",
            Race::Elf => "精灵",
            Race::Orc => "兽人",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Race::Human => "全能：生命+15% 伤害+10% 攻速+5%",
            Race::Elf => "敏捷：射程+25% 攻速+25% 移速+15% 生命-10%",
            Race::Orc => "狂暴：伤害+25% 生命+35% 射程-10% 攻速-5% 移速-10%",
        }
    }

    pub fn color(self) -> Color {
        match self {
            Race::Human => Color::srgb(0.55, 0.78, 1.0),
            Race::Elf => Color::srgb(0.55, 1.0, 0.7),
            Race::Orc => Color::srgb(1.0, 0.55, 0.45),
        }
    }

    /// (damage, range, cooldown, hp, speed) multipliers.
    fn mods(self) -> RaceMods {
        match self {
            Race::Human => RaceMods {
                damage: 1.1,
                range: 1.0,
                cooldown: 0.95,
                hp: 1.15,
                speed: 1.0,
            },
            Race::Elf => RaceMods {
                damage: 1.0,
                range: 1.25,
                cooldown: 0.8,
                hp: 0.9,
                speed: 1.15,
            },
            Race::Orc => RaceMods {
                damage: 1.25,
                range: 0.9,
                cooldown: 1.05,
                hp: 1.35,
                speed: 0.9,
            },
        }
    }
}

struct RaceMods {
    damage: f32,
    range: f32,
    cooldown: f32,
    hp: f32,
    speed: f32,
}

/// Hero weapon — base combat profile and attack behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HeroWeapon {
    BannerSword,
    StarfireStaff,
    ShadowBow,
    OathShield,
    StormOrb,
    SentryCrossbow,
    NightDagger,
    SummonStaff,
    ForgeHammer,
}

impl HeroWeapon {
    pub const ALL: [HeroWeapon; 9] = [
        HeroWeapon::BannerSword,
        HeroWeapon::StarfireStaff,
        HeroWeapon::ShadowBow,
        HeroWeapon::OathShield,
        HeroWeapon::StormOrb,
        HeroWeapon::SentryCrossbow,
        HeroWeapon::NightDagger,
        HeroWeapon::SummonStaff,
        HeroWeapon::ForgeHammer,
    ];

    pub fn name(self) -> &'static str {
        match self {
            HeroWeapon::BannerSword => "战旗长剑",
            HeroWeapon::StarfireStaff => "星火法杖",
            HeroWeapon::ShadowBow => "猎影长弓",
            HeroWeapon::OathShield => "誓约盾锤",
            HeroWeapon::StormOrb => "雷暴法器",
            HeroWeapon::SentryCrossbow => "哨戒弩",
            HeroWeapon::NightDagger => "夜刃匕首",
            HeroWeapon::SummonStaff => "召唤法杖",
            HeroWeapon::ForgeHammer => "工匠战锤",
        }
    }

    pub fn blurb(self) -> &'static str {
        // Each blurb leads with the weapon's DOCTRINE — its signature passive and the
        // playstyle it pushes (单刷守关 / 打钱 / 塔联动), so the picker communicates routes.
        match self {
            HeroWeapon::BannerSword => "【不灭战魂】持续回血，可单刷守关不靠塔",
            HeroWeapon::StarfireStaff => "【湮灭领域】范围歼灭，并增幅周围法系塔",
            HeroWeapon::ShadowBow => "【赏金猎手】残血追猎并获得额外金币，发育打钱最快",
            HeroWeapon::OathShield => "【统御军阵】光环为周围塔加攻、自身扛线",
            HeroWeapon::StormOrb => "【风暴领域】身边形成减速力场，群体控场核心",
            HeroWeapon::SentryCrossbow => "【戍卫结界】穿透哨箭减速敌线，并大幅提升周围塔射程",
            HeroWeapon::NightDagger => "【背击刺杀】绕后爆发，毒影飞溅并缠住怪群，专精猎杀BOSS",
            HeroWeapon::SummonStaff => "【异界契约】召唤移动神话怪物，强化所有召唤物",
            HeroWeapon::ForgeHammer => "【临时工事】挥锤守线，并能组装临时守卫",
        }
    }

    /// Every class reserves its third skill for the level-30 ultimate.
    pub fn ult_slot(self) -> usize {
        2
    }

    /// Sprite file (under `sprites/hero_talents/`) for the ultimate talent slot.
    pub fn ultimate_sprite_name(self) -> &'static str {
        match self {
            HeroWeapon::BannerSword => "ult_warrior",
            HeroWeapon::StarfireStaff => "ult_mage",
            HeroWeapon::ShadowBow => "ult_ranger",
            HeroWeapon::OathShield => "ult_guardian",
            HeroWeapon::StormOrb => "ult_stormcaller",
            HeroWeapon::SentryCrossbow => "ult_warden",
            HeroWeapon::NightDagger => "ult_assassin",
            HeroWeapon::SummonStaff => "ult_summoner",
            HeroWeapon::ForgeHammer => "ult_engineer",
        }
    }

    /// The weapon's level-30 ultimate name (shown on the ult talent slot).
    pub fn ultimate_name(self) -> &'static str {
        match self {
            HeroWeapon::BannerSword => "裂地连斩",
            HeroWeapon::StarfireStaff => "群星坠落",
            HeroWeapon::ShadowBow => "万箭风暴",
            HeroWeapon::OathShield => "圣域庇护",
            HeroWeapon::StormOrb => "雷霆风暴",
            HeroWeapon::SentryCrossbow => "永恒哨域",
            HeroWeapon::NightDagger => "绝命刺杀",
            HeroWeapon::SummonStaff => "旧日眷属",
            HeroWeapon::ForgeHammer => "守卫工坊",
        }
    }

    /// The weapon's level-30 ultimate description.
    pub fn ultimate_desc(self) -> &'static str {
        match self {
            HeroWeapon::BannerSword => "30级解锁：蓄力挥剑，连续震裂周围地面，击退并震晕敌群",
            HeroWeapon::StarfireStaff => "30级解锁：展开星阵，召下五颗陨星爆炸轰击敌群并附加灼烧",
            HeroWeapon::ShadowBow => "30级解锁：向空中射出箭群，多轮箭雨覆盖敌军阵地",
            HeroWeapon::OathShield => {
                "30级解锁：举盾展开圣域，持续修复英雄与防御塔并震退侵入的敌人"
            }
            HeroWeapon::StormOrb => "30级解锁：引导雷云，多轮落雷轰击并麻痹目标",
            HeroWeapon::SentryCrossbow => "30级解锁：部署三处哨戒阵位，持续发射远程哨箭封锁防线",
            HeroWeapon::NightDagger => {
                "30级解锁：锁定重伤目标，双刃交叉处决，按目标已损生命追加伤害"
            }
            HeroWeapon::SummonStaff => {
                "30级解锁：开启旧日之门，召唤远古眷属与两名护卫独立作战；共享12名存活上限"
            }
            HeroWeapon::ForgeHammer => {
                "30级解锁：展开守卫工坊，组装四座重装守卫并持续修复周围英雄与防御塔"
            }
        }
    }

    /// One-line role tag (攻击距离 · 定位) shown in the weapon tooltip so the player can
    /// tell at a glance how the weapon is meant to be played.
    pub fn role(self) -> &'static str {
        match self {
            HeroWeapon::BannerSword => "近战 · 单刷守关",
            HeroWeapon::StarfireStaff => "远程 · 范围歼灭",
            HeroWeapon::ShadowBow => "远程 · 打钱发育",
            HeroWeapon::OathShield => "近战 · 扛线增伤",
            HeroWeapon::StormOrb => "辅助 · 群体减速",
            HeroWeapon::SentryCrossbow => "辅助 · 射程增幅 · 反隐形",
            HeroWeapon::NightDagger => "近战 · 背击打BOSS",
            HeroWeapon::SummonStaff => "远程 · 神话召唤",
            HeroWeapon::ForgeHammer => "近战 · 临时守卫 · 攻速超频",
        }
    }

    /// The weapon's signature passive — see [`Doctrine`]. This is the main thing that
    /// makes weapons play differently (solo / economy / tower-synergy routes).
    pub fn doctrine(self) -> Doctrine {
        match self {
            // Solo bruiser: heavy self-regen → hold a lane with no towers.
            HeroWeapon::BannerSword => Doctrine {
                name: "不灭战魂",
                desc: "每秒回复生命，越战越勇，可脱离防御塔单独守关",
                regen_pct: 0.05,
                ..Doctrine::ZERO
            },
            // Solo nuker that also amps nearby magic towers.
            HeroWeapon::StarfireStaff => Doctrine {
                name: "湮灭领域",
                desc: "范围歼灭敌群，并为周围防御塔提供奥术增幅(+攻击)",
                aura_damage: 0.10,
                ..Doctrine::ZERO
            },
            // Economy: bounty gold on every kill (anywhere) while alive.
            HeroWeapon::ShadowBow => Doctrine {
                name: "赏金猎手",
                desc: "全场击杀额外获得16%金币；箭矢对35%生命以下目标造成三倍伤害",
                gold_bonus: 0.16,
                ..Doctrine::ZERO
            },
            // Frontline commander: damage aura + a little self-regen.
            HeroWeapon::OathShield => Doctrine {
                name: "统御军阵",
                desc: "光环提升周围防御塔伤害(+15%)，自身扛线回血",
                aura_damage: 0.15,
                regen_pct: 0.02,
                ..Doctrine::ZERO
            },
            // Battlefield control: a persistent slow FIELD around the hero — the only
            // weapon that debuffs enemies directly (群体减速核心), plus a small dmg aura.
            HeroWeapon::StormOrb => Doctrine {
                name: "风暴领域",
                desc: "在身边形成减速力场，范围内敌人持续被减速，并小幅增伤周围塔",
                enemy_slow: 0.25,
                aura_damage: 0.08,
                ..Doctrine::ZERO
            },
            // Sentinel: extends the RANGE of nearby towers — lets short-range towers
            // cover far more path (远程辅助核心), distinct from the haste/damage buffers.
            HeroWeapon::SentryCrossbow => Doctrine {
                name: "戍卫结界",
                desc: "提升周围防御塔射程(+22%)，让防线覆盖更远的路径",
                aura_range: 0.22,
                ..Doctrine::ZERO
            },
            // Duelist economy hybrid: small bounty + sustain.
            HeroWeapon::NightDagger => Doctrine {
                name: "背击刺杀",
                desc: "背击对BOSS伤害x2.6并飞溅减速毒影，击杀额外15%金币",
                gold_bonus: 0.15,
                regen_pct: 0.02,
                ..Doctrine::ZERO
            },
            // Summon support: the weapon calls mythic allies and spectral copies.
            HeroWeapon::SummonStaff => Doctrine {
                name: "异界契约",
                desc: "强化所有召唤物(+65%伤害/回血/延寿)，神话眷属与幽魂协同守线",
                regen_pct: 0.01,
                aura_haste: 0.16,
                summon_power: 0.65,
                ..Doctrine::ZERO
            },
            // Builder route: attack-speed aura plus a passive that constructs
            // temporary guards, distinct from the summon staff's mobile mythic allies.
            HeroWeapon::ForgeHammer => Doctrine {
                name: "临时工事",
                desc: "全面提升周围防御塔攻速(+30%)；战斗中自动组装临时守卫",
                aura_haste: 0.30,
                ..Doctrine::ZERO
            },
        }
    }

    pub fn sprite_name(self) -> &'static str {
        match self {
            HeroWeapon::BannerSword => "warrior",
            HeroWeapon::StarfireStaff => "mage",
            HeroWeapon::ShadowBow => "ranger",
            HeroWeapon::OathShield => "guardian",
            HeroWeapon::StormOrb => "stormcaller",
            HeroWeapon::SentryCrossbow => "warden",
            HeroWeapon::NightDagger => "assassin",
            HeroWeapon::SummonStaff => "summoner",
            HeroWeapon::ForgeHammer => "engineer",
        }
    }

    pub fn skill_name(self) -> &'static str {
        self.talent_name(0)
    }

    pub fn skill_desc(self) -> &'static str {
        match self {
            HeroWeapon::BannerSword => "自动施展破阵冲锋与旋锋横扫；30级解锁裂地连斩",
            HeroWeapon::StarfireStaff => "自动施展星火长径与寒星禁锢；30级解锁群星坠落",
            HeroWeapon::ShadowBow => "自动施展淬毒齐射与穿云狙击；30级解锁万箭风暴",
            HeroWeapon::OathShield => "自动施展盾击震退与圣光修复；30级解锁圣域庇护",
            HeroWeapon::StormOrb => "自动施展雷链审判与风眼涡流；30级解锁雷霆风暴",
            HeroWeapon::SentryCrossbow => "自动施展棱光折射与缚影哨箭；30级解锁永恒哨域",
            HeroWeapon::NightDagger => "自动施展暗影伏击与毒刃散射；30级解锁绝命刺杀",
            HeroWeapon::SummonStaff => "自动召唤神话眷属与亡魂友军；30级召唤旧日眷属",
            HeroWeapon::ForgeHammer => "自动组装守卫并发射追踪火箭；30级解锁守卫工坊",
        }
    }

    pub fn skill_sprite_name(self) -> &'static str {
        match self {
            HeroWeapon::BannerSword => "warrior_banner",
            HeroWeapon::StarfireStaff => "mage_storm",
            HeroWeapon::ShadowBow => "ranger_volley",
            HeroWeapon::OathShield => "guardian_shield",
            HeroWeapon::StormOrb => "stormcaller_tempest",
            HeroWeapon::SentryCrossbow => "warden_totem",
            HeroWeapon::NightDagger => "assassin_mark",
            HeroWeapon::SummonStaff => "summoner_calling",
            HeroWeapon::ForgeHammer => "engineer_overclock",
        }
    }

    pub fn skill_color(self) -> Color {
        match self {
            HeroWeapon::BannerSword => Color::srgb(1.0, 0.42, 0.22),
            HeroWeapon::StarfireStaff => Color::srgb(0.55, 0.42, 1.0),
            HeroWeapon::ShadowBow => Color::srgb(0.35, 0.92, 0.55),
            HeroWeapon::OathShield => Color::srgb(0.35, 0.72, 1.0),
            HeroWeapon::StormOrb => Color::srgb(1.0, 0.92, 0.28),
            HeroWeapon::SentryCrossbow => Color::srgb(0.42, 0.86, 0.62),
            HeroWeapon::NightDagger => Color::srgb(0.76, 0.38, 0.95),
            HeroWeapon::SummonStaff => Color::srgb(0.50, 1.0, 0.78),
            HeroWeapon::ForgeHammer => Color::srgb(1.0, 0.63, 0.32),
        }
    }

    pub fn talent_name(self, index: usize) -> &'static str {
        if index == self.ult_slot() {
            return self.ultimate_name();
        }
        match (self, index) {
            (HeroWeapon::BannerSword, 0) => "破阵冲锋",
            (HeroWeapon::BannerSword, 1) => "旋锋横扫",
            (HeroWeapon::StarfireStaff, 0) => "星火长径",
            (HeroWeapon::StarfireStaff, 1) => "寒星禁锢",
            (HeroWeapon::ShadowBow, 0) => "淬毒齐射",
            (HeroWeapon::ShadowBow, 1) => "穿云狙击",
            (HeroWeapon::OathShield, 0) => "盾击震退",
            (HeroWeapon::OathShield, 1) => "圣光修复",
            (HeroWeapon::StormOrb, 0) => "雷链审判",
            (HeroWeapon::StormOrb, 1) => "风眼涡流",
            (HeroWeapon::SentryCrossbow, 0) => "棱光折射",
            (HeroWeapon::SentryCrossbow, 1) => "缚影哨箭",
            (HeroWeapon::NightDagger, 0) => "暗影伏击",
            (HeroWeapon::NightDagger, 1) => "毒刃散射",
            (HeroWeapon::SummonStaff, 0) => "神话契约",
            (HeroWeapon::SummonStaff, 1) => "亡魂召回",
            (HeroWeapon::ForgeHammer, 0) => "守卫组装",
            (HeroWeapon::ForgeHammer, 1) => "追踪火箭",
            _ => "未知技能",
        }
    }

    pub fn talent_desc(self, index: usize) -> &'static str {
        if index == self.ult_slot() {
            return self.ultimate_desc();
        }
        match (self, index) {
            (HeroWeapon::BannerSword, 0) => {
                "蓄势冲向前锋，沿途剑锋击退并震晕敌人；升阶强化本次冲锋"
            }
            (HeroWeapon::BannerSword, 1) => {
                "转身挥出弧形剑锋，横扫近身敌群；升阶强化横扫伤害与范围"
            }
            (HeroWeapon::StarfireStaff, 0) => {
                "吟唱后贯穿目标方向，铺出持续灼烧的火焰长径；升阶强化火径"
            }
            (HeroWeapon::StarfireStaff, 1) => {
                "在敌群脚下凝结冰阵，释放时冻结范围内敌人；升阶强化冰牢"
            }
            (HeroWeapon::ShadowBow, 0) => "搭起多支毒箭，齐射数名敌人并附加剧毒；升阶强化本次齐射",
            (HeroWeapon::ShadowBow, 1) => {
                "拉满弓弦射出高穿甲箭，贯穿直线上的所有敌人；升阶强化狙击"
            }
            (HeroWeapon::OathShield, 0) => "架盾蓄力后猛击前方，击退并震晕近敌；升阶强化盾击",
            (HeroWeapon::OathShield, 1) => {
                "举起圣锤引导治疗，修复受损防御塔并治疗英雄与萝卜；升阶强化修复"
            }
            (HeroWeapon::StormOrb, 0) => "聚集电荷后释放连锁闪电，在相邻敌人间跳跃；升阶强化雷链",
            (HeroWeapon::StormOrb, 1) => "凝聚风眼，持续牵引并禁锢范围内敌群；升阶强化涡流",
            (HeroWeapon::SentryCrossbow, 0) => {
                "瞄准防线锚点，发射多路折射哨箭并减速敌人；升阶强化折射"
            }
            (HeroWeapon::SentryCrossbow, 1) => {
                "装填束缚箭，命中后冻结目标并施加破甲诅咒；升阶强化束缚"
            }
            (HeroWeapon::NightDagger, 0) => {
                "隐步蓄势，绕到首领优先目标身后突刺并留下死印；升阶强化伏击"
            }
            (HeroWeapon::NightDagger, 1) => "翻转双刃，向敌群掷出多柄淬毒飞刀；升阶强化飞刀",
            (HeroWeapon::SummonStaff, 0) => {
                "吟唱开启契约法阵，召出移动作战的神话眷属；升阶强化本次眷属，共享12名上限"
            }
            (HeroWeapon::SummonStaff, 1) => {
                "引导亡魂之门，消耗已击败非首领的魂魄召回友军；升阶强化本次亡魂，共享12名上限"
            }
            (HeroWeapon::ForgeHammer, 0) => {
                "挥锤展开零件，组装在指定防区巡逻近战的机械守卫；升阶强化本次守卫"
            }
            (HeroWeapon::ForgeHammer, 1) => {
                "架起发射器，发射追踪目标并爆炸燃烧的火箭；升阶强化本次火箭"
            }
            _ => "",
        }
    }

    pub fn talent_sprite_name(self, index: usize) -> &'static str {
        if index == self.ult_slot() {
            return self.ultimate_sprite_name();
        }
        match (self, index) {
            (HeroWeapon::BannerSword, 0) => "warrior_banner",
            (HeroWeapon::BannerSword, 1) => "warrior_cleave",
            (HeroWeapon::StarfireStaff, 0) => "mage_overload",
            (HeroWeapon::StarfireStaff, 1) => "mage_froststar",
            (HeroWeapon::ShadowBow, 0) => "ranger_venom",
            (HeroWeapon::ShadowBow, 1) => "ranger_mark",
            (HeroWeapon::OathShield, 0) => "guardian_counter",
            (HeroWeapon::OathShield, 1) => "guardian_repair",
            (HeroWeapon::StormOrb, 0) => "stormcaller_chain",
            (HeroWeapon::StormOrb, 1) => "stormcaller_eye",
            (HeroWeapon::SentryCrossbow, 0) => "warden_watch",
            (HeroWeapon::SentryCrossbow, 1) => "warden_vines",
            (HeroWeapon::NightDagger, 0) => "assassin_step",
            (HeroWeapon::NightDagger, 1) => "assassin_venom",
            (HeroWeapon::SummonStaff, 0) => "summoner_pact",
            (HeroWeapon::SummonStaff, 1) => "summoner_ritual",
            (HeroWeapon::ForgeHammer, 0) => "engineer_mount",
            (HeroWeapon::ForgeHammer, 1) => "engineer_pulse",
            _ => "warrior_cleave",
        }
    }

    /// (damage, range, cooldown_s, hp, behavior, element, aoe_radius).
    fn base(self) -> WeaponBase {
        match self {
            HeroWeapon::BannerSword => WeaponBase {
                damage: 82.0, // melee cleave: ~2-tile reach, hits a GROUP
                range: 80.0,
                cooldown: 0.6,
                hp: 640.0,
                behavior: Behavior::Aoe,
                element: Element::Physical,
                aoe_radius: 64.0,
            },
            HeroWeapon::StarfireStaff => WeaponBase {
                damage: 53.0,
                range: 165.0,
                cooldown: 0.9,
                hp: 420.0,
                behavior: Behavior::Aoe,
                element: Element::Arcane,
                aoe_radius: 84.0,
            },
            HeroWeapon::ShadowBow => WeaponBase {
                damage: 50.0,
                range: 210.0, // longest reach
                cooldown: 0.58,
                hp: 400.0,
                behavior: Behavior::Single,
                element: Element::Physical,
                aoe_radius: 0.0,
            },
            HeroWeapon::OathShield => WeaponBase {
                damage: 56.0, // melee tank: single-target, ~1.5 tiles
                range: 62.0,
                cooldown: 0.78,
                hp: 760.0,
                behavior: Behavior::Single,
                element: Element::Physical,
                aoe_radius: 0.0,
            },
            HeroWeapon::StormOrb => WeaponBase {
                damage: 64.0,
                range: 195.0,
                cooldown: 0.60,
                hp: 520.0,
                behavior: Behavior::Chain,
                element: Element::Storm,
                aoe_radius: 0.0,
            },
            HeroWeapon::SentryCrossbow => WeaponBase {
                damage: 40.0,
                range: 210.0,
                cooldown: 0.86,
                hp: 520.0,
                behavior: Behavior::Slow,
                element: Element::Frost,
                aoe_radius: 0.0,
            },
            HeroWeapon::NightDagger => WeaponBase {
                damage: 72.0, // melee rogue: single-target poison, ~1 tile, fast
                range: 64.0,
                cooldown: 0.46,
                hp: 420.0,
                behavior: Behavior::Poison,
                element: Element::Toxic,
                aoe_radius: 0.0,
            },
            HeroWeapon::SummonStaff => WeaponBase {
                damage: 36.0,
                range: 160.0,
                cooldown: 0.92,
                hp: 500.0,
                behavior: Behavior::Curse,
                element: Element::Arcane,
                aoe_radius: 0.0,
            },
            HeroWeapon::ForgeHammer => WeaponBase {
                damage: 68.0,
                range: 58.0,
                cooldown: 0.58,
                hp: 610.0,
                behavior: Behavior::Single,
                element: Element::Physical,
                aoe_radius: 0.0,
            },
        }
    }
}

struct WeaponBase {
    damage: f32,
    range: f32,
    cooldown: f32,
    hp: f32,
    behavior: Behavior,
    element: Element,
    aoe_radius: f32,
}

/// A weapon's signature passive identity, applied every frame by [`hero_doctrine`].
/// Different fields drive different macro playstyles: `regen_pct` → solo survival,
/// `gold_bonus` → economy, `aura_*`/`tower_heal` → tower synergy (联动).
#[derive(Clone, Copy)]
pub struct Doctrine {
    pub name: &'static str,
    pub desc: &'static str,
    /// Hero HP regenerated per second, as a fraction of max HP.
    pub regen_pct: f32,
    /// +damage fraction granted to towers within the hero's aura.
    pub aura_damage: f32,
    /// +attack-speed fraction granted to towers within the hero's aura.
    pub aura_haste: f32,
    /// +range fraction granted to towers within the hero's aura (Sentry Crossbow).
    pub aura_range: f32,
    /// If >0, refreshes a slow on enemies within the aura (Storm Orb CC field):
    /// the value is the slow_timer seconds re-applied each frame.
    pub enemy_slow: f32,
    /// HP/sec (fraction of the tower's max HP) repaired to towers in the aura.
    pub tower_heal: f32,
    /// +gold fraction on every enemy kill while the hero is alive.
    pub gold_bonus: f32,
    /// +damage fraction granted to all hero summons, which are also healed and
    /// have their decay slowed.
    pub summon_power: f32,
}

impl Doctrine {
    pub const ZERO: Doctrine = Doctrine {
        name: "",
        desc: "",
        regen_pct: 0.0,
        aura_damage: 0.0,
        aura_haste: 0.0,
        aura_range: 0.0,
        enemy_slow: 0.0,
        tower_heal: 0.0,
        gold_bonus: 0.0,
        summon_power: 0.0,
    };
}

/// Each frame, project the living hero's weapon doctrine onto the battlefield:
/// regenerate the hero, buff/heal towers within its aura (联动), and set the global
/// gold bounty (打钱). This is the main source of per-weapon playstyle divergence.
pub fn hero_doctrine(
    time: Res<Time>,
    mut run: ResMut<crate::game::RunState>,
    loadout: Res<HeroLoadout>,
    mut towers: Query<(Entity, &mut Tower)>,
    mut summons: Query<&mut crate::tower::Summon>,
    mut enemies: Query<(&mut crate::components::Enemy, &Transform)>,
) {
    let dt = time.delta_secs() * run.game_speed;
    let doc = loadout.weapon.doctrine();
    let gear = hero_gear::gear_stats(&loadout.gear);
    let affinity = hero_gear::weapon_affinity_stats(&loadout.gear, loadout.weapon);
    let scale = 1.0 + loadout.level.saturating_sub(1) as f32 * 0.03;

    // Find the living hero (entity, position, aura radius) before mutating.
    let hero = towers
        .iter()
        .find(|(_, t)| t.hero && t.hp > 0.0)
        .map(|(e, t)| (e, t.center(), t.buff_range));

    run.hero_gold_bonus = match hero {
        Some(_) => doc.gold_bonus + gear.gold_bonus_add + affinity.gold_bonus_add,
        None => 0.0,
    };

    // Summon Staff 异界契约: empower every hero summon with damage, regen, and
    // slowed decay. Reset when no summon-staff hero is alive.
    let summon_power = match hero {
        Some(_) => {
            doc.summon_power * scale
                + loadout.run_mods.summon_power_add
                + gear.summon_power_add
                + affinity.summon_power_add
        }
        None => 0.0,
    };
    for mut s in &mut summons {
        s.buff = summon_power;
        if summon_power > 0.0 {
            if s.hp > 0.0 && s.hp < s.max_hp {
                s.hp = (s.hp + s.max_hp * 0.05 * dt).min(s.max_hp);
            }
            // Slow the crumble timer of temporary minions (skeletons are infinite).
            if run.wave_in_progress && s.lifetime.is_finite() {
                s.lifetime += dt * 0.5;
            }
        }
    }

    let Some((hero_e, hero_pos, radius)) = hero else {
        for (_, mut t) in &mut towers {
            t.aura_damage = 0.0;
            t.aura_haste = 0.0;
            t.aura_range = 0.0;
        }
        return;
    };

    for (e, mut t) in &mut towers {
        if e == hero_e {
            if doc.regen_pct > 0.0 && t.hp > 0.0 {
                t.hp = (t.hp + t.max_hp * doc.regen_pct * scale * dt).min(t.max_hp);
            }
            continue;
        }
        if radius > 0.0 && t.center().distance(hero_pos) <= radius {
            t.aura_damage = doc.aura_damage * scale
                + loadout.run_mods.aura_damage_add
                + gear.aura_damage_add
                + affinity.aura_damage_add;
            t.aura_haste = doc.aura_haste * scale + gear.tower_haste_add + affinity.tower_haste_add;
            t.aura_range = doc.aura_range; // range bonus doesn't scale with level
            if doc.tower_heal > 0.0 && t.hp > 0.0 && t.hp < t.max_hp {
                t.hp = (t.hp + t.max_hp * doc.tower_heal * scale * dt).min(t.max_hp);
            }
        } else {
            t.aura_damage = 0.0;
            t.aura_haste = 0.0;
            t.aura_range = 0.0;
        }
    }

    // Storm Orb 风暴领域: a persistent slow field. Re-apply the slow each frame to
    // enemies inside the hero's aura so they stay slowed while in range.
    if doc.enemy_slow > 0.0 && radius > 0.0 {
        for (mut enemy, tf) in &mut enemies {
            if tf.translation.truncate().distance(hero_pos) <= radius {
                enemy.slow_timer = enemy.slow_timer.max(doc.enemy_slow);
            }
        }
    }
}

#[derive(Clone, Copy)]
struct HeroSave {
    race: Race,
    weapon: HeroWeapon,
    level: u8,
    xp: i32,
    points: u8,
    talents: [[u8; HeroLoadout::TALENT_SLOTS]; HeroWeapon::ALL.len()],
    gear: [Option<HeroGear>; HeroGearSlot::COUNT],
}

/// Per-level roguelite modifiers. These are intentionally not serialized with the
/// hero save; a new map starts with a clean build.
#[derive(Clone, Copy)]
pub struct HeroRunMods {
    pub damage_mult: f32,
    pub range_mult: f32,
    pub cooldown_mult: f32,
    pub skill_interval_mult: f32,
    pub skill_power_mult: f32,
    pub hp_mult: f32,
    pub move_mult: f32,
    pub armor_add: f32,
    pub aura_damage_add: f32,
    pub summon_power_add: f32,
}

impl Default for HeroRunMods {
    fn default() -> Self {
        Self {
            damage_mult: 1.0,
            range_mult: 1.0,
            cooldown_mult: 1.0,
            skill_interval_mult: 1.0,
            skill_power_mult: 1.0,
            hp_mult: 1.0,
            move_mult: 1.0,
            armor_add: 0.0,
            aura_damage_add: 0.0,
            summon_power_add: 0.0,
        }
    }
}

/// The player's chosen hero, persisted across sessions, plus run state.
#[derive(Resource)]
pub struct HeroLoadout {
    pub race: Race,
    pub weapon: HeroWeapon,
    pub level: u8,
    pub xp: i32,
    pub talent_points: u8,
    pub weapon_talents: [[u8; Self::TALENT_SLOTS]; HeroWeapon::ALL.len()],
    pub gear: [Option<HeroGear>; HeroGearSlot::COUNT],
    /// Independent remaining game seconds before each class skill can cast.
    pub skill_cooldowns: [f32; Self::TALENT_SLOTS],
    /// Temporary roguelite build modifiers for the active level only.
    pub run_mods: HeroRunMods,
    /// Whether the hero is currently alive in the run.
    pub alive: bool,
    /// Waves remaining before the hero can be re-summoned after dying (0 = ready).
    pub respawn_waves: i32,
}

impl Default for HeroLoadout {
    fn default() -> Self {
        let saved = load_hero();
        Self {
            race: saved.race,
            weapon: saved.weapon,
            level: saved.level.clamp(1, Self::MAX_LEVEL),
            xp: saved.xp.max(0),
            talent_points: saved.points,
            weapon_talents: saved.talents,
            gear: saved.gear,
            skill_cooldowns: [0.0; Self::TALENT_SLOTS],
            run_mods: HeroRunMods::default(),
            alive: false,
            respawn_waves: 0,
        }
    }
}

impl HeroLoadout {
    pub const MAX_LEVEL: u8 = 30;
    pub const TALENT_SLOTS: usize = 3;
    pub const TALENT_MAX_RANK: u8 = 5;

    /// Pick a weapon directly (hero selection screen), persisting the choice.
    pub fn set_weapon(&mut self, weapon: HeroWeapon) {
        if self.weapon != weapon {
            self.skill_cooldowns = [0.0; Self::TALENT_SLOTS];
        }
        self.weapon = weapon;
        save_hero(self);
    }

    /// Pick a race directly (hero selection screen), persisting the choice.
    pub fn set_race(&mut self, race: Race) {
        self.race = race;
        save_hero(self);
    }

    pub fn xp_to_next(&self) -> i32 {
        xp_to_next(self.level)
    }

    pub fn weapon_index(&self) -> usize {
        HeroWeapon::ALL
            .iter()
            .position(|weapon| *weapon == self.weapon)
            .unwrap_or(0)
    }

    pub fn talent_rank(&self, index: usize) -> u8 {
        if index == self.weapon.ult_slot() {
            return 0;
        }
        self.weapon_talents
            .get(self.weapon_index())
            .and_then(|row| row.get(index))
            .copied()
            .unwrap_or(0)
    }

    pub fn spent_in_current_weapon(&self) -> u8 {
        self.weapon_talents[self.weapon_index()][..self.weapon.ult_slot()]
            .iter()
            .sum()
    }

    pub fn weapon_kind(&self) -> HeroWeaponKind {
        HeroWeaponKind::for_weapon(self.weapon)
    }

    pub fn gear_count(&self) -> usize {
        hero_gear::gear_count(&self.gear)
    }

    pub fn gear_summary(&self) -> String {
        hero_gear::summary_for_weapon(&self.gear, Some(self.weapon))
    }

    pub fn equip_gear(&mut self, item: HeroGear) -> Option<HeroGear> {
        let replaced = hero_gear::equip(&mut self.gear, item);
        save_hero(self);
        replaced
    }

    pub fn unequip_gear_slot(&mut self, slot: HeroGearSlot) -> Option<HeroGear> {
        let removed = hero_gear::unequip_slot(&mut self.gear, slot);
        save_hero(self);
        removed
    }

    pub fn gain_xp(&mut self, amount: i32) -> u8 {
        if amount <= 0 || self.level >= Self::MAX_LEVEL {
            return 0;
        }
        self.xp += amount;
        let mut gained = 0;
        while self.level < Self::MAX_LEVEL && self.xp >= xp_to_next(self.level) {
            self.xp -= xp_to_next(self.level);
            self.level += 1;
            self.talent_points = self.talent_points.saturating_add(1);
            gained += 1;
        }
        if self.level >= Self::MAX_LEVEL {
            self.xp = 0;
        }
        save_hero(self);
        gained
    }

    pub fn add_talent(&mut self, index: usize) -> Result<(), &'static str> {
        if index >= Self::TALENT_SLOTS {
            return Err("未知技能");
        }
        if index == self.weapon.ult_slot() {
            return Err("终极技能将在30级自动解锁，无需投点");
        }
        if self.talent_points == 0 {
            return Err("没有可用天赋点");
        }
        let weapon_index = self.weapon_index();
        if self.weapon_talents[weapon_index][index] >= Self::TALENT_MAX_RANK {
            return Err("该技能已满阶");
        }
        self.weapon_talents[weapon_index][index] += 1;
        self.talent_points -= 1;
        save_hero(self);
        Ok(())
    }

    pub fn respec_current_weapon(&mut self) -> u8 {
        let weapon_index = self.weapon_index();
        let refunded = self.spent_in_current_weapon();
        self.weapon_talents[weapon_index] = [0; Self::TALENT_SLOTS];
        self.talent_points = self.talent_points.saturating_add(refunded);
        save_hero(self);
        refunded
    }

    pub fn skill_unlocked(&self, index: usize) -> bool {
        index < Self::TALENT_SLOTS
            && (index != self.weapon.ult_slot() || self.level >= Self::MAX_LEVEL)
    }

    /// Cooldowns belong to casts; training never changes baseline combat stats.
    pub fn skill_interval(&self, index: usize) -> f32 {
        let intervals = match self.weapon {
            HeroWeapon::BannerSword => [18.0, 12.0, 38.0],
            HeroWeapon::StarfireStaff => [24.0, 18.0, 42.0],
            HeroWeapon::ShadowBow => [18.0, 14.0, 38.0],
            HeroWeapon::OathShield => [14.0, 24.0, 42.0],
            HeroWeapon::StormOrb => [18.0, 20.0, 40.0],
            HeroWeapon::SentryCrossbow => [20.0, 16.0, 42.0],
            HeroWeapon::NightDagger => [18.0, 14.0, 38.0],
            HeroWeapon::SummonStaff => [24.0, 18.0, 45.0],
            HeroWeapon::ForgeHammer => [24.0, 16.0, 42.0],
        };
        let gear = hero_gear::gear_stats(&self.gear);
        let affinity = hero_gear::weapon_affinity_stats(&self.gear, self.weapon);
        ((intervals.get(index).copied().unwrap_or(intervals[0])
            - gear.passive_interval_reduction as f32
            - affinity.passive_interval_reduction as f32)
            * self.run_mods.skill_interval_mult)
            .max(crate::data::HERO_PASSIVE_MIN_INTERVAL)
    }

    /// Primary skill interval used by the existing attribute comparison report.
    pub fn passive_interval(&self) -> f32 {
        self.skill_interval(0)
    }

    pub fn skill_damage_mult(&self) -> f32 {
        let level = 1.0 + (self.level.saturating_sub(1) as f32 * 0.045);
        let gear = hero_gear::gear_stats(&self.gear);
        let affinity = hero_gear::weapon_affinity_stats(&self.gear, self.weapon);
        level * gear.skill_mult * affinity.skill_mult * self.run_mods.skill_power_mult
    }
}

/// Movement speed (world px/sec) for this race+weapon.
pub fn hero_move_speed(loadout: &HeroLoadout) -> f32 {
    let weapon_speed = match loadout.weapon {
        HeroWeapon::OathShield => 0.92,
        HeroWeapon::SentryCrossbow | HeroWeapon::ForgeHammer => 0.98,
        HeroWeapon::NightDagger => 1.12,
        HeroWeapon::SummonStaff => 0.96,
        _ => 1.0,
    };
    let gear = hero_gear::gear_stats(&loadout.gear);
    let affinity = hero_gear::weapon_affinity_stats(&loadout.gear, loadout.weapon);
    110.0
        * loadout.race.mods().speed
        * weapon_speed
        * gear.move_mult
        * affinity.move_mult
        * loadout.run_mods.move_mult
}

pub fn validate_hero_gear_inventory(
    mut loadout: ResMut<HeroLoadout>,
    inventory: Res<HeroGearInventory>,
) {
    if !loadout.is_changed() && !inventory.is_changed() {
        return;
    }
    let mut changed = false;
    for slot in &mut loadout.gear {
        if slot.is_some_and(|item| !inventory.owns(item)) {
            *slot = None;
            changed = true;
        }
    }
    if changed {
        save_hero(&loadout);
    }
}

/// Build a [`Tower`] configured as the hero at `pos`.
pub fn make_hero_tower(loadout: &HeroLoadout, pos: Vec2) -> Tower {
    // Start from an ordinary def so every Tower field has a sane value, then
    // overwrite the combat stats with the race×weapon profile.
    let mut t = Tower::from_def(TowerKind::Arrow.def(), 0, 0);
    t.hero = true;
    t.hero_weapon = Some(loadout.weapon);
    t.hero_pos = pos;
    t.move_target = None;
    t.footprint = 1;
    apply_loadout_to_tower(loadout, &mut t);
    t.hp = t.max_hp;
    t
}

pub fn apply_loadout_to_tower(loadout: &HeroLoadout, t: &mut Tower) {
    let base = loadout.weapon.base();
    if t.hero {
        t.hero_weapon = Some(loadout.weapon);
    }
    let m = loadout.race.mods();
    let hp_frac = if t.max_hp > 0.0 {
        (t.hp / t.max_hp).clamp(0.05, 1.0)
    } else {
        1.0
    };
    let level_mult = 1.0 + loadout.level.saturating_sub(1) as f32 * 0.04;
    let gear = hero_gear::gear_stats(&loadout.gear);
    let affinity = hero_gear::weapon_affinity_stats(&loadout.gear, loadout.weapon);
    let mut damage_mult = level_mult;
    let mut range_mult = 1.0;
    let mut cooldown_mult = 1.0;
    let mut hp_mult = 1.0;
    let mut armor_bonus = 0.0;
    let mut armor_pierce_bonus = 0.0;

    t.behavior = base.behavior;
    // Sentry Crossbow is the 哨兵 (sentinel): built-in 反隐形 — reveals invisible enemies in
    // range so the player never needs a separate detection tower with this hero.
    t.detector = loadout.weapon == HeroWeapon::SentryCrossbow;
    t.chain_count = 0;
    t.chain_range = 0.0;
    t.slow_duration = 0.0;
    t.knock_dist = 0.0;
    t.stun_duration = 0.0;
    t.freeze_duration = 0.0;
    t.armor_reduce = 0.0;
    t.curse_duration = 0.0;
    t.buff_range = 0.0;
    t.dot_damage = 0.0;
    t.poison_duration = 0.0;
    t.fire_duration = 0.0;
    t.target_priority = match loadout.weapon {
        HeroWeapon::BannerSword | HeroWeapon::ShadowBow | HeroWeapon::NightDagger => {
            TargetPriority::Weakest
        }
        HeroWeapon::OathShield | HeroWeapon::StormOrb | HeroWeapon::ForgeHammer => {
            TargetPriority::Front
        }
        HeroWeapon::SentryCrossbow => TargetPriority::Threat,
        HeroWeapon::StarfireStaff | HeroWeapon::SummonStaff => TargetPriority::Strongest,
    };

    match loadout.weapon {
        HeroWeapon::BannerSword | HeroWeapon::ShadowBow => {}
        HeroWeapon::StarfireStaff => {
            t.dot_damage = 12.0;
            t.fire_duration = 2.2;
            t.armor_reduce = 7.0;
            t.curse_duration = 1.6;
            t.freeze_duration = 0.65;
            t.buff_range = 115.0;
        }
        HeroWeapon::OathShield => {
            t.buff_range = 110.0;
        }
        HeroWeapon::StormOrb => {
            t.behavior = Behavior::Chain;
            t.chain_count = 4;
            t.chain_range = 120.0;
            t.slow_duration = 0.6;
            t.buff_range = 125.0;
        }
        HeroWeapon::SentryCrossbow => {
            t.behavior = Behavior::Slow;
            t.slow_duration = 0.9;
            t.buff_range = 130.0;
        }
        HeroWeapon::NightDagger => {
            t.behavior = Behavior::Poison;
            t.dot_damage = 18.0;
            t.poison_duration = 3.0;
            t.armor_reduce = 6.0;
            t.curse_duration = 1.5;
        }
        HeroWeapon::SummonStaff => {
            t.behavior = Behavior::Curse;
            t.armor_reduce = 8.0;
            t.curse_duration = 1.8;
            t.buff_range = 145.0;
        }
        HeroWeapon::ForgeHammer => {
            t.buff_range = 135.0;
        }
    }

    t.element = base.element;
    t.magic = base.element != Element::Physical;
    t.color = loadout.race.color();
    t.aoe_radius = base.aoe_radius;
    damage_mult *= gear.damage_mult;
    range_mult *= gear.range_mult;
    cooldown_mult *= gear.cooldown_mult;
    hp_mult *= gear.hp_mult;
    armor_bonus += gear.armor_add;
    armor_pierce_bonus += gear.armor_pierce;
    damage_mult *= affinity.damage_mult;
    range_mult *= affinity.range_mult;
    cooldown_mult *= affinity.cooldown_mult;
    hp_mult *= affinity.hp_mult;
    armor_bonus += affinity.armor_add;
    armor_pierce_bonus += affinity.armor_pierce;
    damage_mult *= loadout.run_mods.damage_mult;
    range_mult *= loadout.run_mods.range_mult;
    cooldown_mult *= loadout.run_mods.cooldown_mult;
    hp_mult *= loadout.run_mods.hp_mult;
    armor_bonus += loadout.run_mods.armor_add;
    // Power compensation: the hero is now free and auto-present from the start of a
    // level (no 200g summon), so its raw combat stats are scaled up to stay relevant.
    const HERO_DMG: f32 = 1.5;
    const HERO_HP: f32 = 1.6;
    t.base_damage = (base.damage * m.damage * damage_mult * HERO_DMG).floor();
    t.damage = t.base_damage;
    t.range = base.range * m.range * range_mult;
    t.cooldown = (base.cooldown * m.cooldown * cooldown_mult.max(0.35)).max(0.05);
    t.cooldown_timer = 0.0;
    t.max_hp = (base.hp * m.hp * hp_mult * HERO_HP).floor();
    t.hp = (t.max_hp * hp_frac).clamp(1.0, t.max_hp);
    t.armor = 6.0 + armor_bonus;
    t.armor_pierce = armor_pierce_bonus;
    t.hp = (t.max_hp * hp_frac).clamp(1.0, t.max_hp);
}

/// Default world spawn point for the hero: lower-middle of the board (the player
/// can move it immediately by tapping).
pub fn hero_spawn_pos() -> Vec2 {
    Vec2::new(0.0, -BOARD_H * 0.22)
}

pub fn xp_to_next(level: u8) -> i32 {
    if level >= HeroLoadout::MAX_LEVEL {
        0
    } else {
        90 + level as i32 * 55
    }
}

// ---- persistence (race,weapon indices) ----

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(inline_js = r#"
export function load_hero() {
  try { return globalThis.localStorage?.getItem('protect_carrot_hero') || ''; }
  catch (_) { return ''; }
}
export function save_hero(value) {
  try { globalThis.localStorage?.setItem('protect_carrot_hero', value); }
  catch (_) {}
}
"#)]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = load_hero)]
    fn load_hero_js() -> String;
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = save_hero)]
    fn save_hero_js(value: &str);
}

fn default_save() -> HeroSave {
    HeroSave {
        race: Race::Human,
        weapon: HeroWeapon::BannerSword,
        level: 1,
        xp: 0,
        points: 0,
        talents: [[0; HeroLoadout::TALENT_SLOTS]; HeroWeapon::ALL.len()],
        gear: hero_gear::empty_gear(),
    }
}

fn parse_hero(raw: &str) -> HeroSave {
    let Some(rest) = raw.trim().strip_prefix("v4,") else {
        return default_save();
    };
    let Some((numbers, gear)) = rest.split_once('|') else {
        return default_save();
    };
    let Ok(nums) = numbers
        .split(',')
        .map(str::parse::<i32>)
        .collect::<Result<Vec<_>, _>>()
    else {
        return default_save();
    };
    if nums.len() != 5 + HeroWeapon::ALL.len() * HeroLoadout::TALENT_SLOTS {
        return default_save();
    }
    let mut save = default_save();
    save.race = Race::ALL
        .get(nums[0].max(0) as usize)
        .copied()
        .unwrap_or(save.race);
    save.weapon = HeroWeapon::ALL
        .get(nums[1].max(0) as usize)
        .copied()
        .unwrap_or(save.weapon);
    save.level = nums[2].clamp(1, HeroLoadout::MAX_LEVEL as i32) as u8;
    save.xp = nums[3].max(0);
    save.points = nums[4].clamp(0, 99) as u8;
    for (weapon, row) in save.talents.iter_mut().enumerate() {
        for (slot, rank) in row.iter_mut().enumerate() {
            *rank = if slot == HeroWeapon::ALL[weapon].ult_slot() {
                0
            } else {
                nums[5 + weapon * HeroLoadout::TALENT_SLOTS + slot]
                    .clamp(0, HeroLoadout::TALENT_MAX_RANK as i32) as u8
            };
        }
    }
    save.gear = hero_gear::decode(gear);
    save
}

fn encode_hero(loadout: &HeroLoadout) -> String {
    let ri = Race::ALL
        .iter()
        .position(|r| *r == loadout.race)
        .unwrap_or(0);
    let wi = HeroWeapon::ALL
        .iter()
        .position(|weapon| *weapon == loadout.weapon)
        .unwrap_or(0);
    let mut parts = vec![
        "v4".to_string(),
        ri.to_string(),
        wi.to_string(),
        loadout.level.to_string(),
        loadout.xp.max(0).to_string(),
        loadout.talent_points.to_string(),
    ];
    for weapon in 0..HeroWeapon::ALL.len() {
        for talent in 0..HeroLoadout::TALENT_SLOTS {
            parts.push(loadout.weapon_talents[weapon][talent].to_string());
        }
    }
    format!(
        "{numbers}|{gear}",
        numbers = parts.join(","),
        gear = hero_gear::encode(&loadout.gear)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_loadout() -> HeroLoadout {
        HeroLoadout {
            race: Race::Human,
            weapon: HeroWeapon::BannerSword,
            level: 1,
            xp: 0,
            talent_points: 0,
            weapon_talents: [[0; HeroLoadout::TALENT_SLOTS]; HeroWeapon::ALL.len()],
            gear: hero_gear::empty_gear(),
            skill_cooldowns: [0.0; HeroLoadout::TALENT_SLOTS],
            run_mods: HeroRunMods::default(),
            alive: true,
            respawn_waves: 0,
        }
    }

    #[test]
    fn boots_gear_changes_hero_combat_and_movement() {
        let base = test_loadout();
        let base_tower = make_hero_tower(&base, Vec2::ZERO);
        let base_speed = hero_move_speed(&base);

        let mut geared = test_loadout();
        hero_gear::equip(&mut geared.gear, HeroGear::WayfarerBoots);
        let geared_tower = make_hero_tower(&geared, Vec2::ZERO);
        let geared_speed = hero_move_speed(&geared);

        assert!(geared_tower.range > base_tower.range);
        assert!(geared_tower.cooldown < base_tower.cooldown);
        assert!(geared_speed > base_speed);
    }

    #[test]
    fn weapon_affinity_changes_active_weapon_combat_stats() {
        let mut matched = test_loadout();
        matched.weapon = HeroWeapon::SummonStaff;
        hero_gear::equip(&mut matched.gear, HeroGear::SummonerGreaves);

        let mut unmatched = test_loadout();
        unmatched.weapon = HeroWeapon::BannerSword;
        hero_gear::equip(&mut unmatched.gear, HeroGear::SummonerGreaves);

        assert!(matched.skill_damage_mult() > unmatched.skill_damage_mult());

        let matched_affinity =
            hero_gear::weapon_affinity_stats(&matched.gear, matched.weapon).summon_power_add;
        let unmatched_affinity =
            hero_gear::weapon_affinity_stats(&unmatched.gear, unmatched.weapon).summon_power_add;
        assert!(matched_affinity > unmatched_affinity);
    }

    #[test]
    fn starfire_staff_carries_arcane_scorch_payload() {
        let mut loadout = test_loadout();
        loadout.weapon = HeroWeapon::StarfireStaff;
        let tower = make_hero_tower(&loadout, Vec2::ZERO);

        assert!(matches!(tower.behavior, crate::data::Behavior::Aoe));
        assert!(tower.magic);
        assert!(tower.dot_damage > 0.0);
        assert!(tower.fire_duration > 0.0);
        assert!(tower.armor_reduce > 0.0);
        assert!(tower.curse_duration > 0.0);
    }

    #[test]
    fn weapons_spawn_with_role_specific_targeting() {
        let mut loadout = test_loadout();
        loadout.weapon = HeroWeapon::NightDagger;
        assert_eq!(
            make_hero_tower(&loadout, Vec2::ZERO).target_priority,
            TargetPriority::Weakest
        );

        loadout.weapon = HeroWeapon::OathShield;
        assert_eq!(
            make_hero_tower(&loadout, Vec2::ZERO).target_priority,
            TargetPriority::Front
        );

        loadout.weapon = HeroWeapon::SentryCrossbow;
        assert_eq!(
            make_hero_tower(&loadout, Vec2::ZERO).target_priority,
            TargetPriority::Threat
        );
    }

    #[test]
    fn skill_training_never_changes_baseline_combat_stats() {
        for weapon in HeroWeapon::ALL {
            for level in [1, HeroLoadout::MAX_LEVEL] {
                let mut loadout = test_loadout();
                loadout.weapon = weapon;
                loadout.level = level;
                let before = make_hero_tower(&loadout, Vec2::ZERO);
                let speed = hero_move_speed(&loadout);
                let power = loadout.skill_damage_mult();
                let intervals = std::array::from_fn::<_, 3, _>(|i| loadout.skill_interval(i));
                let weapon_index = loadout.weapon_index();
                loadout.weapon_talents[weapon_index] = [5, 5, 0];
                let after = make_hero_tower(&loadout, Vec2::ZERO);
                assert_eq!(before.damage, after.damage);
                assert_eq!(before.range, after.range);
                assert_eq!(before.cooldown, after.cooldown);
                assert_eq!(before.max_hp, after.max_hp);
                assert_eq!(before.armor, after.armor);
                assert_eq!(before.armor_pierce, after.armor_pierce);
                assert_eq!(before.aoe_radius, after.aoe_radius);
                assert_eq!(before.chain_count, after.chain_count);
                assert_eq!(before.dot_damage, after.dot_damage);
                assert_eq!(before.buff_range, after.buff_range);
                assert_eq!(speed, hero_move_speed(&loadout));
                assert_eq!(power, loadout.skill_damage_mult());
                assert_eq!(
                    intervals,
                    std::array::from_fn(|i| loadout.skill_interval(i))
                );
            }
        }
    }

    #[test]
    fn every_class_has_two_base_casts_and_one_level_thirty_ultimate() {
        for weapon in HeroWeapon::ALL {
            let mut loadout = test_loadout();
            loadout.weapon = weapon;
            assert!(loadout.skill_unlocked(0));
            assert!(loadout.skill_unlocked(1));
            assert!(!loadout.skill_unlocked(2));
            assert!(!loadout.skill_unlocked(3));
            assert_ne!(weapon.talent_name(0), weapon.talent_name(1));
            assert_ne!(weapon.talent_name(1), weapon.talent_name(2));
            loadout.level = HeroLoadout::MAX_LEVEL;
            assert!(loadout.skill_unlocked(2));
            for slot in 0..HeroLoadout::TALENT_SLOTS {
                assert_ne!(
                    crate::i18n::tr(crate::i18n::Lang::En, weapon.talent_name(slot)),
                    weapon.talent_name(slot)
                );
                assert_ne!(
                    crate::i18n::tr(crate::i18n::Lang::En, weapon.talent_desc(slot)),
                    weapon.talent_desc(slot)
                );
            }
        }
    }

    #[test]
    fn skill_hex_modifiers_are_separate_from_training() {
        let mut loadout = test_loadout();
        let cooldowns = std::array::from_fn::<_, 3, _>(|i| loadout.skill_interval(i));
        let power = loadout.skill_damage_mult();
        loadout.run_mods.skill_interval_mult = 0.9;
        loadout.run_mods.skill_power_mult = 1.15;
        for (slot, original) in cooldowns.into_iter().enumerate() {
            assert!((loadout.skill_interval(slot) - original * 0.9).abs() < 0.001);
        }
        assert!((loadout.skill_damage_mult() - power * 1.15).abs() < 0.001);
    }

    #[test]
    fn current_save_roundtrips_without_ultimate_training() {
        let mut loadout = test_loadout();
        loadout.weapon = HeroWeapon::SummonStaff;
        loadout.level = 30;
        loadout.talent_points = 7;
        loadout.weapon_talents[7] = [3, 4, 0];
        hero_gear::equip(&mut loadout.gear, HeroGear::SummonerGreaves);
        let saved = parse_hero(&encode_hero(&loadout));
        assert!(saved.weapon == loadout.weapon);
        assert_eq!(saved.level, 30);
        assert_eq!(saved.points, 7);
        assert_eq!(saved.talents, loadout.weapon_talents);
        assert!(saved.gear == loadout.gear);
    }

    #[test]
    fn obsolete_or_malformed_skill_saves_are_not_reinterpreted() {
        for raw in [
            "v3,2,7,30,0,4|",
            "v2,2,7,30,0,4",
            "2,7",
            "v4,0,7,30,oops,2|",
        ] {
            let saved = parse_hero(raw);
            assert_eq!(saved.level, 1);
            assert_eq!(saved.talents, [[0; 3]; HeroWeapon::ALL.len()]);
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn load_hero() -> HeroSave {
    parse_hero(&load_hero_js())
}

#[cfg(target_arch = "wasm32")]
fn save_hero(loadout: &HeroLoadout) {
    save_hero_js(&encode_hero(loadout));
}

#[cfg(not(target_arch = "wasm32"))]
fn load_hero() -> HeroSave {
    parse_hero(&std::fs::read_to_string("tmp/hero.txt").unwrap_or_default())
}

#[cfg(not(target_arch = "wasm32"))]
fn save_hero(loadout: &HeroLoadout) {
    let _ = std::fs::create_dir_all("tmp");
    let _ = std::fs::write("tmp/hero.txt", encode_hero(loadout));
}
