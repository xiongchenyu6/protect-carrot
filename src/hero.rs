//! The unique **hero tower** (英雄塔): a single, movable, race + weapon defined unit.
//!
//! Unlike ordinary towers (grid-snapped, static), the hero is summoned once per run,
//! walks to a tapped destination, and fights along the way. It is implemented as a
//! regular [`Tower`] (so it reuses attack/render/HP/damage) carrying the `hero`
//! flag, a free-floating `hero_pos`, and an optional `move_target`.

use crate::data::{Behavior, Element, TowerKind, BOARD_H};
use crate::hero_gear::{self, HeroGear, HeroGearInventory, HeroGearSlot, HeroWeaponKind};
use crate::tower::Tower;
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
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
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
            HeroWeapon::NightDagger => "【背击刺杀】绕后背击爆发，专精操作打BOSS",
            HeroWeapon::SummonStaff => "【异界契约】召唤移动神话怪物，强化所有召唤物",
            HeroWeapon::ForgeHammer => "【临时工事】挥锤守线，并能组装临时守卫",
        }
    }

    /// The talent slot repurposed as this weapon's level-30 ULTIMATE. It is the weapon's
    /// weakest old talent (so converting it removes the least): it can no longer be
    /// invested (its old per-rank effect therefore stays at 0 = removed), and instead
    /// auto-activates as the ultimate once the hero hits level 30.
    pub fn ult_slot(self) -> usize {
        match self {
            HeroWeapon::BannerSword => 3,    // was 血性反击
            HeroWeapon::StarfireStaff => 4,  // was 元素亲和
            HeroWeapon::ShadowBow => 3,      // was 连珠追猎
            HeroWeapon::OathShield => 3,     // was 壁垒修复 (slot was unused in stats anyway)
            HeroWeapon::StormOrb => 4,       // was 充能矩阵
            HeroWeapon::SentryCrossbow => 4, // was 补给信标 (slot was unused in stats anyway)
            HeroWeapon::NightDagger => 4,    // was 破甲诅咒
            HeroWeapon::SummonStaff => 1,    // old support slot repurposed as the ultimate
            HeroWeapon::ForgeHammer => 1,    // old overclock slot repurposed as the ultimate
        }
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
            HeroWeapon::BannerSword => "不死战神",
            HeroWeapon::StarfireStaff => "群星坠落",
            HeroWeapon::ShadowBow => "万箭风暴",
            HeroWeapon::OathShield => "不灭壁垒",
            HeroWeapon::StormOrb => "诸神黄昏",
            HeroWeapon::SentryCrossbow => "永恒哨域",
            HeroWeapon::NightDagger => "绝命刺杀",
            HeroWeapon::SummonStaff => "旧日眷属",
            HeroWeapon::ForgeHammer => "守卫工坊",
        }
    }

    /// The weapon's level-30 ultimate description.
    pub fn ultimate_desc(self) -> &'static str {
        match self {
            HeroWeapon::BannerSword => "30级解锁：伤害+50%、生命+60%、护甲大幅提升，真正的单刷战神",
            HeroWeapon::StarfireStaff => "30级解锁：伤害+60%、爆炸范围剧增，一发清屏",
            HeroWeapon::ShadowBow => "30级解锁：攻速翻倍并获得巨额穿甲，万箭覆盖全场",
            HeroWeapon::OathShield => "30级解锁：生命翻倍、护甲暴涨，不可撼动的壁垒",
            HeroWeapon::StormOrb => "30级解锁：雷链+5次跳跃、伤害+40%，连锁吞噬全场",
            HeroWeapon::SentryCrossbow => "30级解锁：自身射程+50%，戍卫之眼笼罩战场",
            HeroWeapon::NightDagger => "30级解锁：伤害+80%、巨额穿甲，背击必致命，专斩BOSS",
            HeroWeapon::SummonStaff => {
                "30级解锁：神话眷属数量+1，召唤物伤害、生命和存在时间大幅提升"
            }
            HeroWeapon::ForgeHammer => "30级解锁：临时守卫更耐打，组装数量+1，并强化周围防御塔攻速",
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
                desc: "从敌人背后攻击触发背击(对BOSS伤害x2.6)，击杀额外15%金币",
                gold_bonus: 0.15,
                regen_pct: 0.02,
                ..Doctrine::ZERO
            },
            // Summon support: the weapon itself calls mobile mythic allies, and
            // its doctrine makes 召唤塔 / 复活塔 builds scale harder.
            HeroWeapon::SummonStaff => Doctrine {
                name: "异界契约",
                desc: "强化所有召唤物(+65%伤害/回血/延寿)，并加速附近召唤塔与死灵塔",
                regen_pct: 0.01,
                aura_haste: 0.16,
                summon_power: 0.65,
                ..Doctrine::ZERO
            },
            // Builder route: attack-speed aura plus an active skill that constructs
            // temporary guards, distinct from the summon staff's mobile mythic allies.
            HeroWeapon::ForgeHammer => Doctrine {
                name: "临时工事",
                desc: "全面提升周围防御塔攻速(+30%)；主动技能会组装临时守卫",
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
        match self {
            HeroWeapon::BannerSword => "战旗冲锋",
            HeroWeapon::StarfireStaff => "星火风暴",
            HeroWeapon::ShadowBow => "猎影齐射",
            HeroWeapon::OathShield => "守护壁垒",
            HeroWeapon::StormOrb => "雷云审判",
            HeroWeapon::SentryCrossbow => "哨戒结界",
            HeroWeapon::NightDagger => "死印爆发",
            HeroWeapon::SummonStaff => "神话召临",
            HeroWeapon::ForgeHammer => "组装守卫",
        }
    }

    pub fn skill_desc(self) -> &'static str {
        match self {
            HeroWeapon::BannerSword => "震击英雄周围敌人，造成物理伤害并短暂眩晕，同时恢复英雄生命",
            HeroWeapon::StarfireStaff => {
                "锁定高生命敌人，引爆奥术风暴，对范围内敌人造成魔法伤害和冰冻"
            }
            HeroWeapon::ShadowBow => "标记最靠前的敌群，连续穿透射击并附加剧毒减速",
            HeroWeapon::OathShield => "修复附近防御塔，鼓舞塔攻势，并冻结贴近防线的敌人",
            HeroWeapon::StormOrb => "召来雷云多段轰击前线敌群，造成雷风伤害和减速",
            HeroWeapon::SentryCrossbow => "展开哨戒结界，强化附近塔并缠绕、削弱敌群",
            HeroWeapon::NightDagger => "给最靠前敌人打上死印，造成暗影爆发、剧毒和破甲诅咒",
            HeroWeapon::SummonStaff => "召唤会移动、会攻击的神话眷属，并裂界削弱周围敌人",
            HeroWeapon::ForgeHammer => "组装临时机械守卫，超频附近防御塔，并用震荡锤击迟滞敌人",
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
            (HeroWeapon::BannerSword, 0) => "破阵重击",
            (HeroWeapon::BannerSword, 1) => "钢铁壁垒",
            (HeroWeapon::BannerSword, 2) => "战旗统御",
            (HeroWeapon::StarfireStaff, 0) => "奥术过载",
            (HeroWeapon::StarfireStaff, 1) => "扩散符文",
            (HeroWeapon::StarfireStaff, 2) => "时序回响",
            (HeroWeapon::ShadowBow, 0) => "鹰眼射术",
            (HeroWeapon::ShadowBow, 1) => "疾行游猎",
            (HeroWeapon::ShadowBow, 2) => "淬毒陷击",
            (HeroWeapon::BannerSword, 3) => "血性反击",
            (HeroWeapon::BannerSword, 4) => "震地压制",
            (HeroWeapon::BannerSword, 5) => "霸者姿态",
            (HeroWeapon::StarfireStaff, 3) => "寒星禁锢",
            (HeroWeapon::StarfireStaff, 4) => "元素亲和",
            (HeroWeapon::StarfireStaff, 5) => "群星法阵",
            (HeroWeapon::ShadowBow, 3) => "连珠追猎",
            (HeroWeapon::ShadowBow, 4) => "弱点标记",
            (HeroWeapon::ShadowBow, 5) => "风行大师",
            (HeroWeapon::OathShield, 0) => "坚盾训练",
            (HeroWeapon::OathShield, 1) => "守护光环",
            (HeroWeapon::OathShield, 2) => "反击阵线",
            (HeroWeapon::OathShield, 3) => "壁垒修复",
            (HeroWeapon::OathShield, 4) => "挑衅压制",
            (HeroWeapon::OathShield, 5) => "不动堡垒",
            (HeroWeapon::StormOrb, 0) => "雷击导体",
            (HeroWeapon::StormOrb, 1) => "暴风链",
            (HeroWeapon::StormOrb, 2) => "静电过载",
            (HeroWeapon::StormOrb, 3) => "风暴减速",
            (HeroWeapon::StormOrb, 4) => "充能矩阵",
            (HeroWeapon::StormOrb, 5) => "天怒风眼",
            (HeroWeapon::SentryCrossbow, 0) => "哨戒阵地",
            (HeroWeapon::SentryCrossbow, 1) => "战术协同",
            (HeroWeapon::SentryCrossbow, 2) => "藤蔓缠绕",
            (HeroWeapon::SentryCrossbow, 3) => "远望标尺",
            (HeroWeapon::SentryCrossbow, 4) => "补给信标",
            (HeroWeapon::SentryCrossbow, 5) => "森罗壁垒",
            (HeroWeapon::NightDagger, 0) => "暗刃训练",
            (HeroWeapon::NightDagger, 1) => "毒影灌注",
            (HeroWeapon::NightDagger, 2) => "死亡标记",
            (HeroWeapon::NightDagger, 3) => "闪袭步法",
            (HeroWeapon::NightDagger, 4) => "破甲诅咒",
            (HeroWeapon::NightDagger, 5) => "终结手法",
            (HeroWeapon::SummonStaff, 0) => "契约增幅",
            (HeroWeapon::SummonStaff, 1) => "旧日契印",
            (HeroWeapon::SummonStaff, 2) => "护主鳞甲",
            (HeroWeapon::SummonStaff, 3) => "裂界低语",
            (HeroWeapon::SummonStaff, 4) => "召唤仪式",
            (HeroWeapon::SummonStaff, 5) => "群星共鸣",
            (HeroWeapon::ForgeHammer, 0) => "精密齿轮",
            (HeroWeapon::ForgeHammer, 1) => "过载线圈",
            (HeroWeapon::ForgeHammer, 2) => "重装底盘",
            (HeroWeapon::ForgeHammer, 3) => "自动修复",
            (HeroWeapon::ForgeHammer, 4) => "震荡锤击",
            (HeroWeapon::ForgeHammer, 5) => "主控核心",
            _ => "未知天赋",
        }
    }

    pub fn talent_desc(self, index: usize) -> &'static str {
        if index == self.ult_slot() {
            return self.ultimate_desc();
        }
        match (self, index) {
            (HeroWeapon::BannerSword, 0) => "提高攻击，并把普攻改为小范围顺劈",
            (HeroWeapon::BannerSword, 1) => "提高生命和护甲，让英雄能顶住攻城怪",
            (HeroWeapon::BannerSword, 2) => "提高攻速和移动速度，并缩短武器技能冷却",
            (HeroWeapon::StarfireStaff, 0) => "提高奥术伤害，并强化火球命中的奥术灼痕",
            (HeroWeapon::StarfireStaff, 1) => "提高射程和爆炸范围，增强控场覆盖",
            (HeroWeapon::StarfireStaff, 2) => "提高施法频率，并延长武器技能冰冻",
            (HeroWeapon::ShadowBow, 0) => "提高远程伤害和射程",
            (HeroWeapon::ShadowBow, 1) => "提高攻速和移动速度，便于游走补线",
            (HeroWeapon::ShadowBow, 2) => "强化毒箭和猎影齐射的减速/毒伤",
            (HeroWeapon::BannerSword, 3) => "受伤后更能维持输出，并提高生命",
            (HeroWeapon::BannerSword, 4) => "扩大顺劈范围，技能眩晕更稳定",
            (HeroWeapon::BannerSword, 5) => "提高全属性成长，降低技能冷却",
            (HeroWeapon::StarfireStaff, 3) => "延长冰冻，并让奥术灼痕削弱敌人抗性",
            (HeroWeapon::StarfireStaff, 4) => "提高元素伤害并略微提高生存",
            (HeroWeapon::StarfireStaff, 5) => "扩大技能法阵，提升终局爆发和灼痕持续压制",
            (HeroWeapon::ShadowBow, 3) => "提高连射频率和齐射目标数",
            (HeroWeapon::ShadowBow, 4) => "提高穿甲，并让技能更擅长点杀",
            (HeroWeapon::ShadowBow, 5) => "进一步提升移速、射程和技能冷却",
            (HeroWeapon::OathShield, 0) => "提高生命、护甲和前线承伤能力",
            (HeroWeapon::OathShield, 1) => "强化主动技能对附近塔的鼓舞",
            (HeroWeapon::OathShield, 2) => "提高反击伤害并获得击退普通攻击",
            (HeroWeapon::OathShield, 3) => "主动技能修复更多塔生命",
            (HeroWeapon::OathShield, 4) => "让攻击和技能更擅长迟滞敌人",
            (HeroWeapon::OathShield, 5) => "大幅提升坦度并缩短壁垒冷却",
            (HeroWeapon::StormOrb, 0) => "提高雷风伤害",
            (HeroWeapon::StormOrb, 1) => "增加连锁次数和跳跃距离",
            (HeroWeapon::StormOrb, 2) => "提高攻速并缩短技能冷却",
            (HeroWeapon::StormOrb, 3) => "增强普攻和技能减速",
            (HeroWeapon::StormOrb, 4) => "主动技能额外超频附近塔",
            (HeroWeapon::StormOrb, 5) => "提高雷云半径和终局伤害",
            (HeroWeapon::SentryCrossbow, 0) => "提高阵地伤害和耐久",
            (HeroWeapon::SentryCrossbow, 1) => "主动技能鼓舞更多防御塔",
            (HeroWeapon::SentryCrossbow, 2) => "增强缠绕减速和毒性压制",
            (HeroWeapon::SentryCrossbow, 3) => "提高射程和索敌稳定性",
            (HeroWeapon::SentryCrossbow, 4) => "主动技能额外修复防御塔",
            (HeroWeapon::SentryCrossbow, 5) => "提高阵地范围和技能冷却效率",
            (HeroWeapon::NightDagger, 0) => "提高暗影直伤和穿甲",
            (HeroWeapon::NightDagger, 1) => "强化毒伤和持续时间",
            (HeroWeapon::NightDagger, 2) => "技能死印命中更多目标",
            (HeroWeapon::NightDagger, 3) => "提高攻速、移速和脱战能力",
            (HeroWeapon::NightDagger, 4) => "增强诅咒破甲效果",
            (HeroWeapon::NightDagger, 5) => "提高对高生命目标的终结爆发",
            (HeroWeapon::SummonStaff, 0) => "提高召唤物和神话眷属伤害",
            (HeroWeapon::SummonStaff, 1) => "主动技能额外召唤旧日眷属",
            (HeroWeapon::SummonStaff, 2) => "提高英雄生命，并强化召唤物耐久",
            (HeroWeapon::SummonStaff, 3) => "神话眷属降临时削弱更多敌人",
            (HeroWeapon::SummonStaff, 4) => "延长召唤物存在时间，并提高移动速度",
            (HeroWeapon::SummonStaff, 5) => "提高召唤共鸣，缩短神话召临冷却",
            (HeroWeapon::ForgeHammer, 0) => "提高机械伤害和基础攻速",
            (HeroWeapon::ForgeHammer, 1) => "主动技能更强力地超频防御塔",
            (HeroWeapon::ForgeHammer, 2) => "提高生命和设备覆盖范围",
            (HeroWeapon::ForgeHammer, 3) => "主动技能修复更多塔结构",
            (HeroWeapon::ForgeHammer, 4) => "增强锤击减速、穿甲和伤害",
            (HeroWeapon::ForgeHammer, 5) => "提高装备收益和超频冷却效率",
            _ => "",
        }
    }

    pub fn talent_sprite_name(self, index: usize) -> &'static str {
        if index == self.ult_slot() {
            return self.ultimate_sprite_name();
        }
        match (self, index) {
            (HeroWeapon::BannerSword, 0) => "warrior_cleave",
            (HeroWeapon::BannerSword, 1) => "warrior_guard",
            (HeroWeapon::BannerSword, 2) => "warrior_banner",
            (HeroWeapon::StarfireStaff, 0) => "mage_overload",
            (HeroWeapon::StarfireStaff, 1) => "mage_rune",
            (HeroWeapon::StarfireStaff, 2) => "mage_echo",
            (HeroWeapon::ShadowBow, 0) => "ranger_eye",
            (HeroWeapon::ShadowBow, 1) => "ranger_stride",
            (HeroWeapon::ShadowBow, 2) => "ranger_venom",
            (HeroWeapon::BannerSword, 3) => "warrior_counter",
            (HeroWeapon::BannerSword, 4) => "warrior_quake",
            (HeroWeapon::BannerSword, 5) => "warrior_warlord",
            (HeroWeapon::StarfireStaff, 3) => "mage_froststar",
            (HeroWeapon::StarfireStaff, 4) => "mage_attune",
            (HeroWeapon::StarfireStaff, 5) => "mage_constellation",
            (HeroWeapon::ShadowBow, 3) => "ranger_barrage",
            (HeroWeapon::ShadowBow, 4) => "ranger_mark",
            (HeroWeapon::ShadowBow, 5) => "ranger_mastery",
            (HeroWeapon::OathShield, 0) => "guardian_shield",
            (HeroWeapon::OathShield, 1) => "guardian_aura",
            (HeroWeapon::OathShield, 2) => "guardian_counter",
            (HeroWeapon::OathShield, 3) => "guardian_repair",
            (HeroWeapon::OathShield, 4) => "guardian_taunt",
            (HeroWeapon::OathShield, 5) => "guardian_bastion",
            (HeroWeapon::StormOrb, 0) => "stormcaller_conduit",
            (HeroWeapon::StormOrb, 1) => "stormcaller_chain",
            (HeroWeapon::StormOrb, 2) => "stormcaller_static",
            (HeroWeapon::StormOrb, 3) => "stormcaller_slow",
            (HeroWeapon::StormOrb, 4) => "stormcaller_matrix",
            (HeroWeapon::StormOrb, 5) => "stormcaller_eye",
            (HeroWeapon::SentryCrossbow, 0) => "warden_watch",
            (HeroWeapon::SentryCrossbow, 1) => "warden_coordination",
            (HeroWeapon::SentryCrossbow, 2) => "warden_vines",
            (HeroWeapon::SentryCrossbow, 3) => "warden_sight",
            (HeroWeapon::SentryCrossbow, 4) => "warden_supply",
            (HeroWeapon::SentryCrossbow, 5) => "warden_grove",
            (HeroWeapon::NightDagger, 0) => "assassin_blade",
            (HeroWeapon::NightDagger, 1) => "assassin_venom",
            (HeroWeapon::NightDagger, 2) => "assassin_mark",
            (HeroWeapon::NightDagger, 3) => "assassin_step",
            (HeroWeapon::NightDagger, 4) => "assassin_curse",
            (HeroWeapon::NightDagger, 5) => "assassin_execute",
            (HeroWeapon::SummonStaff, 0) => "summoner_pact",
            (HeroWeapon::SummonStaff, 1) => "summoner_sigil",
            (HeroWeapon::SummonStaff, 2) => "summoner_scales",
            (HeroWeapon::SummonStaff, 3) => "summoner_rift",
            (HeroWeapon::SummonStaff, 4) => "summoner_ritual",
            (HeroWeapon::SummonStaff, 5) => "summoner_resonance",
            (HeroWeapon::ForgeHammer, 0) => "engineer_gears",
            (HeroWeapon::ForgeHammer, 1) => "engineer_overclock",
            (HeroWeapon::ForgeHammer, 2) => "engineer_mount",
            (HeroWeapon::ForgeHammer, 3) => "engineer_repair",
            (HeroWeapon::ForgeHammer, 4) => "engineer_pulse",
            (HeroWeapon::ForgeHammer, 5) => "engineer_core",
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
    /// +damage fraction granted to ALL summons (召唤物), who are also healed and
    /// have their decay slowed. Drives the summon-staff route (召唤塔/复活塔).
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

    // Summon Staff 异界契约: empower every summon (召唤物联动) — bonus damage, regen, and
    // slowed decay so召唤塔/复活塔 builds scale. Reset when no summon-staff hero is alive.
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
            if s.lifetime.is_finite() {
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
    pub skill_cd: i32,
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
            skill_cd: 0,
            run_mods: HeroRunMods::default(),
            alive: false,
            respawn_waves: 0,
        }
    }
}

impl HeroLoadout {
    pub const MAX_LEVEL: u8 = 30;
    pub const TALENT_SLOTS: usize = 6;
    pub const TALENT_MAX_RANK: u8 = 5;

    /// Pick a weapon directly (hero selection screen), persisting the choice.
    pub fn set_weapon(&mut self, weapon: HeroWeapon) {
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
        self.weapon_talents
            .get(self.weapon_index())
            .and_then(|row| row.get(index))
            .copied()
            .unwrap_or(0)
    }

    pub fn spent_in_current_weapon(&self) -> u8 {
        self.weapon_talents[self.weapon_index()].iter().sum()
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
            return Err("未知天赋");
        }
        if index == self.weapon.ult_slot() {
            return Err("终极天赋将在30级自动解锁，无需投点");
        }
        if self.talent_points == 0 {
            return Err("没有可用天赋点");
        }
        let weapon_index = self.weapon_index();
        if self.weapon_talents[weapon_index][index] >= Self::TALENT_MAX_RANK {
            return Err("该天赋已满级");
        }
        self.weapon_talents[weapon_index][index] += 1;
        self.talent_points -= 1;
        save_hero(self);
        Ok(())
    }

    pub fn respec_current_weapon(&mut self) -> u8 {
        let weapon_index = self.weapon_index();
        let refunded: u8 = self.weapon_talents[weapon_index].iter().sum();
        self.weapon_talents[weapon_index] = [0; Self::TALENT_SLOTS];
        self.talent_points = self.talent_points.saturating_add(refunded);
        save_hero(self);
        refunded
    }

    pub fn tick_wave_cooldowns(&mut self) {
        self.skill_cd = (self.skill_cd - 1).max(0);
    }

    pub fn skill_cooldown_max(&self) -> i32 {
        let base = match self.weapon {
            HeroWeapon::BannerSword => 3,
            HeroWeapon::StarfireStaff => 4,
            HeroWeapon::ShadowBow => 3,
            HeroWeapon::OathShield => 4,
            HeroWeapon::StormOrb => 4,
            HeroWeapon::SentryCrossbow => 4,
            HeroWeapon::NightDagger => 3,
            HeroWeapon::SummonStaff => 4,
            HeroWeapon::ForgeHammer => 4,
        };
        let gear = hero_gear::gear_stats(&self.gear);
        let affinity = hero_gear::weapon_affinity_stats(&self.gear, self.weapon);
        (base
            - ((self.talent_rank(2) + self.talent_rank(5)) / 3) as i32
            - gear.skill_cooldown_reduction
            - affinity.skill_cooldown_reduction)
            .max(1)
    }

    pub fn skill_damage_mult(&self) -> f32 {
        let level = 1.0 + (self.level.saturating_sub(1) as f32 * 0.045);
        let gear = hero_gear::gear_stats(&self.gear);
        let affinity = hero_gear::weapon_affinity_stats(&self.gear, self.weapon);
        let talent = match self.weapon {
            HeroWeapon::BannerSword => {
                1.0 + self.talent_rank(0) as f32 * 0.13 + self.talent_rank(5) as f32 * 0.05
            }
            HeroWeapon::StarfireStaff => {
                1.0 + self.talent_rank(0) as f32 * 0.16 + self.talent_rank(5) as f32 * 0.07
            }
            HeroWeapon::ShadowBow => {
                1.0 + self.talent_rank(0) as f32 * 0.11 + self.talent_rank(4) as f32 * 0.06
            }
            HeroWeapon::OathShield => {
                1.0 + self.talent_rank(2) as f32 * 0.08 + self.talent_rank(5) as f32 * 0.04
            }
            HeroWeapon::StormOrb => {
                1.0 + self.talent_rank(0) as f32 * 0.12 + self.talent_rank(5) as f32 * 0.08
            }
            HeroWeapon::SentryCrossbow => {
                1.0 + self.talent_rank(0) as f32 * 0.08 + self.talent_rank(5) as f32 * 0.05
            }
            HeroWeapon::NightDagger => {
                1.0 + self.talent_rank(0) as f32 * 0.12 + self.talent_rank(5) as f32 * 0.10
            }
            HeroWeapon::SummonStaff => {
                1.0 + self.talent_rank(0) as f32 * 0.10 + self.talent_rank(5) as f32 * 0.06
            }
            HeroWeapon::ForgeHammer => {
                1.0 + self.talent_rank(0) as f32 * 0.10 + self.talent_rank(5) as f32 * 0.06
            }
        };
        level * talent * gear.skill_mult * affinity.skill_mult
    }
}

/// Movement speed (world px/sec) for this race+weapon.
pub fn hero_move_speed(loadout: &HeroLoadout) -> f32 {
    let talent_speed = match loadout.weapon {
        HeroWeapon::BannerSword => 1.0 + loadout.talent_rank(2) as f32 * 0.04,
        HeroWeapon::StarfireStaff => 1.0,
        HeroWeapon::ShadowBow => {
            1.0 + loadout.talent_rank(1) as f32 * 0.07 + loadout.talent_rank(5) as f32 * 0.03
        }
        HeroWeapon::OathShield => 0.92 + loadout.talent_rank(5) as f32 * 0.025,
        HeroWeapon::StormOrb => 1.0 + loadout.talent_rank(2) as f32 * 0.035,
        HeroWeapon::SentryCrossbow => 0.98 + loadout.talent_rank(3) as f32 * 0.025,
        HeroWeapon::NightDagger => {
            1.12 + loadout.talent_rank(3) as f32 * 0.06 + loadout.talent_rank(5) as f32 * 0.02
        }
        HeroWeapon::SummonStaff => 0.96 + loadout.talent_rank(4) as f32 * 0.025,
        HeroWeapon::ForgeHammer => 0.98 + loadout.talent_rank(2) as f32 * 0.02,
    };
    let gear = hero_gear::gear_stats(&loadout.gear);
    let affinity = hero_gear::weapon_affinity_stats(&loadout.gear, loadout.weapon);
    110.0
        * loadout.race.mods().speed
        * talent_speed
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
    let a = loadout.talent_rank(0) as f32;
    let b = loadout.talent_rank(1) as f32;
    let c = loadout.talent_rank(2) as f32;
    let d = loadout.talent_rank(3) as f32;
    let e = loadout.talent_rank(4) as f32;
    let f = loadout.talent_rank(5) as f32;
    let equipped_count = loadout.gear_count() as f32;
    let gear = hero_gear::gear_stats(&loadout.gear);
    let affinity = hero_gear::weapon_affinity_stats(&loadout.gear, loadout.weapon);
    let mut damage_mult = level_mult;
    let mut range_mult = 1.0;
    let mut cooldown_mult = 1.0;
    let mut hp_mult = 1.0;
    let mut armor_bonus = 0.0;
    let mut armor_pierce_bonus = 0.0;
    let mut aoe_bonus = 0.0;

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
    t.heal_amount = 0.0;
    t.buff_range = 0.0;
    t.dot_damage = 0.0;
    t.poison_duration = 0.0;
    t.fire_duration = 0.0;
    t.summon_hp = 0.0;
    t.summon_speed = 0.0;
    t.max_summons = 0;

    match loadout.weapon {
        HeroWeapon::BannerSword => {
            damage_mult *= 1.0 + a * 0.14 + f * 0.05;
            hp_mult *= 1.0 + b * 0.16 + d * 0.04 + f * 0.06;
            armor_bonus += b * 6.0 + d * 2.0 + f * 3.0;
            cooldown_mult *= 1.0 - c * 0.055 - f * 0.025;
            aoe_bonus += if a > 0.0 || e > 0.0 {
                42.0 + a * 8.0 + e * 10.0
            } else {
                0.0
            };
            if a > 0.0 || e > 0.0 {
                t.behavior = Behavior::Aoe;
            }
        }
        HeroWeapon::StarfireStaff => {
            damage_mult *= 1.0 + a * 0.10 + e * 0.035 + f * 0.045;
            range_mult *= 1.0 + b * 0.045 + e * 0.015;
            cooldown_mult *= 1.0 - c * 0.045;
            hp_mult *= 1.0 + e * 0.03;
            t.dot_damage = 12.0 + a * 4.5 + f * 5.5;
            t.fire_duration = 2.2 + c * 0.25 + f * 0.16;
            t.armor_reduce = 7.0 + d * 3.5 + f * 2.0;
            t.curse_duration = 1.6 + d * 0.28;
            aoe_bonus += b * 7.0 + f * 5.0;
            t.freeze_duration = 0.65 + d * 0.16;
            // Aura radius so the 湮灭领域 doctrine can amp nearby magic towers.
            t.buff_range = 115.0 + b * 16.0 + f * 8.0;
        }
        HeroWeapon::ShadowBow => {
            damage_mult *= 1.0 + a * 0.06 + d * 0.02 + f * 0.025;
            range_mult *= 1.0 + a * 0.024 + f * 0.016;
            cooldown_mult *= 1.0 - b * 0.04 - d * 0.02;
            hp_mult *= 1.0 + f * 0.03;
            armor_pierce_bonus += e * 4.0;
            if c > 0.0 {
                t.behavior = Behavior::Poison;
                t.dot_damage = 8.0 + c * 5.0;
                t.poison_duration = 2.4 + c * 0.45;
            }
        }
        HeroWeapon::OathShield => {
            damage_mult *= 1.0 + c * 0.08;
            range_mult *= 1.0 + b * 0.02;
            cooldown_mult *= 1.0 - c * 0.025 - f * 0.02;
            hp_mult *= 1.0 + a * 0.14 + f * 0.10;
            armor_bonus += a * 7.0 + c * 2.0 + f * 8.0;
            t.buff_range = 110.0 + b * 18.0;
            if e > 0.0 {
                t.behavior = Behavior::Knockback;
                t.knock_dist = 18.0 + e * 8.0;
                t.stun_duration = 0.16 + e * 0.04;
            }
        }
        HeroWeapon::StormOrb => {
            damage_mult *= 1.0 + a * 0.12 + f * 0.07;
            range_mult *= 1.0 + b * 0.02 + f * 0.02;
            cooldown_mult *= 1.0 - c * 0.05 - f * 0.02;
            hp_mult *= 1.0 + e * 0.03;
            t.behavior = Behavior::Chain;
            t.chain_count = 4 + b as i32;
            t.chain_range = 120.0 + b * 20.0 + f * 12.0;
            t.slow_duration = 0.6 + d * 0.18;
            t.buff_range = 125.0 + e * 16.0;
        }
        HeroWeapon::SentryCrossbow => {
            damage_mult *= 1.0 + a * 0.08 + f * 0.05;
            range_mult *= 1.0 + d * 0.045 + f * 0.015;
            cooldown_mult *= 1.0 - b * 0.025 - f * 0.025;
            hp_mult *= 1.0 + a * 0.06 + f * 0.05;
            armor_bonus += f * 3.0;
            t.behavior = Behavior::Slow;
            t.slow_duration = 0.9 + c * 0.22 + f * 0.08;
            t.buff_range = 130.0 + b * 18.0 + f * 10.0;
        }
        HeroWeapon::NightDagger => {
            damage_mult *= 1.0 + a * 0.12 + f * 0.08;
            range_mult *= 1.0 + e * 0.02;
            cooldown_mult *= 1.0 - d * 0.06 - f * 0.02;
            hp_mult *= 1.0 + d * 0.03;
            armor_pierce_bonus += a * 5.0 + e * 8.0;
            t.behavior = Behavior::Poison;
            t.dot_damage = 18.0 + b * 10.0 + f * 6.0;
            t.poison_duration = 3.0 + b * 0.45;
            t.armor_reduce = 6.0 + e * 4.0;
            t.curse_duration = 1.5 + e * 0.22;
        }
        HeroWeapon::SummonStaff => {
            damage_mult *= 1.0 + a * 0.10 + f * 0.05;
            range_mult *= 1.0 + e * 0.02;
            cooldown_mult *= 1.0 - e * 0.025 - f * 0.03;
            hp_mult *= 1.0 + c * 0.08 + e * 0.05;
            armor_bonus += c * 3.0 + f * 2.0;
            t.behavior = Behavior::Curse;
            t.armor_reduce = 8.0 + d * 4.0 + f * 2.0;
            t.curse_duration = 1.8 + d * 0.25;
            t.buff_range = 145.0 + e * 14.0 + f * 10.0;
        }
        HeroWeapon::ForgeHammer => {
            damage_mult *= 1.0 + a * 0.10 + f * 0.04 + equipped_count * f * 0.025;
            cooldown_mult *= 1.0 - a * 0.03 - b * 0.02 - f * 0.03 - equipped_count * f * 0.01;
            hp_mult *= 1.0 + c * 0.06 + d * 0.06;
            armor_bonus += c * 2.0 + d * 3.0;
            armor_pierce_bonus += e * 4.0;
            t.buff_range = 135.0 + c * 16.0 + f * 10.0;
            if e > 0.0 {
                t.slow_duration = 0.38 + e * 0.16;
            }
        }
    }

    // ===== Level-30 ULTIMATE =====
    // A dramatic capstone that auto-activates at max level.
    if loadout.level >= HeroLoadout::MAX_LEVEL {
        match loadout.weapon {
            HeroWeapon::BannerSword => {
                damage_mult *= 1.5;
                hp_mult *= 1.6;
                armor_bonus += 30.0;
            }
            HeroWeapon::StarfireStaff => {
                damage_mult *= 1.6;
                aoe_bonus += 90.0;
            }
            HeroWeapon::ShadowBow => {
                cooldown_mult *= 0.5;
                armor_pierce_bonus += 30.0;
            }
            HeroWeapon::OathShield => {
                hp_mult *= 2.0;
                armor_bonus += 50.0;
            }
            HeroWeapon::StormOrb => {
                damage_mult *= 1.4;
                t.chain_count += 5;
                t.chain_range += 60.0;
            }
            HeroWeapon::SentryCrossbow => {
                range_mult *= 1.5;
            }
            HeroWeapon::NightDagger => {
                damage_mult *= 1.8;
                armor_pierce_bonus += 40.0;
            }
            HeroWeapon::SummonStaff => {
                hp_mult *= 1.3;
                t.buff_range += 45.0;
                t.armor_reduce += 10.0;
            }
            HeroWeapon::ForgeHammer => {
                t.buff_range += 45.0;
                hp_mult *= 1.2;
            }
        }
    }

    t.element = base.element;
    t.magic = base.element != Element::Physical;
    t.color = loadout.race.color();
    t.aoe_radius = (base.aoe_radius + aoe_bonus).max(base.aoe_radius);
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
    let raw = raw.trim();
    let mut save = default_save();
    if let Some(rest) = raw.strip_prefix("v3,") {
        let (numbers, gear) = rest.split_once('|').unwrap_or((rest, ""));
        parse_hero_numbers(numbers, &mut save);
        save.gear = hero_gear::decode(gear);
        return save;
    }
    if let Some(rest) = raw.strip_prefix("v2,") {
        parse_hero_numbers(rest, &mut save);
        return save;
    }

    // Legacy format: "race,weapon".
    let mut parts = raw.split(',');
    let r = parts.next().and_then(|s| s.trim().parse::<usize>().ok());
    let c = parts.next().and_then(|s| s.trim().parse::<usize>().ok());
    save.race = r
        .and_then(|i| Race::ALL.get(i).copied())
        .unwrap_or(Race::Human);
    save.weapon = c
        .and_then(|i| HeroWeapon::ALL.get(i).copied())
        .unwrap_or(HeroWeapon::BannerSword);
    save
}

fn parse_hero_numbers(raw: &str, save: &mut HeroSave) {
    let nums = raw
        .split(',')
        .filter_map(|s| s.trim().parse::<i32>().ok())
        .collect::<Vec<_>>();
    save.race = nums
        .first()
        .and_then(|i| Race::ALL.get((*i).max(0) as usize).copied())
        .unwrap_or(save.race);
    save.weapon = nums
        .get(1)
        .and_then(|i| HeroWeapon::ALL.get((*i).max(0) as usize).copied())
        .unwrap_or(save.weapon);
    save.level = nums
        .get(2)
        .copied()
        .unwrap_or(1)
        .clamp(1, HeroLoadout::MAX_LEVEL as i32) as u8;
    save.xp = nums.get(3).copied().unwrap_or(0).max(0);
    save.points = nums.get(4).copied().unwrap_or(0).clamp(0, 99) as u8;
    if nums.len() <= 5 + 3 * 3 {
        let mut cursor = 5;
        for weapon in 0..3 {
            for talent in 0..3 {
                save.talents[weapon][talent] =
                    nums.get(cursor)
                        .copied()
                        .unwrap_or(0)
                        .clamp(0, HeroLoadout::TALENT_MAX_RANK as i32) as u8;
                cursor += 1;
            }
        }
        return;
    }
    let mut cursor = 5;
    for weapon in 0..HeroWeapon::ALL.len() {
        for talent in 0..HeroLoadout::TALENT_SLOTS {
            save.talents[weapon][talent] =
                nums.get(cursor)
                    .copied()
                    .unwrap_or(0)
                    .clamp(0, HeroLoadout::TALENT_MAX_RANK as i32) as u8;
            cursor += 1;
        }
    }
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
        "v3".to_string(),
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
            skill_cd: 0,
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
        geared.equip_gear(HeroGear::WayfarerBoots);
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
        matched.equip_gear(HeroGear::SummonerGreaves);

        let mut unmatched = test_loadout();
        unmatched.weapon = HeroWeapon::BannerSword;
        unmatched.equip_gear(HeroGear::SummonerGreaves);

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
