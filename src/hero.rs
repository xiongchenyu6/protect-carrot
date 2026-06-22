//! The unique **hero tower** (英雄塔): a single, movable, race + class defined unit.
//!
//! Unlike ordinary towers (grid-snapped, static), the hero is summoned once per run,
//! walks to a tapped destination, and fights along the way. It is implemented as a
//! regular [`Tower`] (so it reuses attack/render/HP/damage) carrying the `hero`
//! flag, a free-floating `hero_pos`, and an optional `move_target`.

use crate::data::{Behavior, Element, TowerKind, BOARD_H};
use crate::tower::Tower;
use bevy::prelude::*;

/// Hero race — a multiplicative modifier layered over the class base stats.
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

/// Hero class — base combat profile and attack behavior.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Class {
    Warrior,
    Mage,
    Ranger,
    Guardian,
    Stormcaller,
    Warden,
    Assassin,
    Priest,
    Engineer,
}

impl Class {
    pub const ALL: [Class; 9] = [
        Class::Warrior,
        Class::Mage,
        Class::Ranger,
        Class::Guardian,
        Class::Stormcaller,
        Class::Warden,
        Class::Assassin,
        Class::Priest,
        Class::Engineer,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Class::Warrior => "战士",
            Class::Mage => "法师",
            Class::Ranger => "游侠",
            Class::Guardian => "守护者",
            Class::Stormcaller => "风暴使",
            Class::Warden => "哨卫",
            Class::Assassin => "刺客",
            Class::Priest => "祭司",
            Class::Engineer => "工匠",
        }
    }

    pub fn blurb(self) -> &'static str {
        // Each blurb leads with the class's DOCTRINE — its signature passive and the
        // playstyle it pushes (单刷守关 / 打钱 / 塔联动), so the picker communicates routes.
        match self {
            Class::Warrior => "【不灭战魂】持续回血，可单刷守关不靠塔",
            Class::Mage => "【湮灭领域】范围歼灭，并增幅周围法系塔",
            Class::Ranger => "【赏金猎手】击杀额外金币，发育打钱最快",
            Class::Guardian => "【统御军阵】光环为周围塔加攻、自身扛线",
            Class::Stormcaller => "【风暴领域】身边形成减速力场，群体控场核心",
            Class::Warden => "【戍卫结界】大幅提升周围塔射程的远程辅助",
            Class::Assassin => "【背击刺杀】绕后背击爆发，专精操作打BOSS",
            Class::Priest => "【圣疗领域】持续治疗萝卜与周围塔的守护者",
            Class::Engineer => "【超频力场】最强塔联动，全面拉高周围塔攻速",
        }
    }

    /// The talent slot repurposed as this class's level-30 ULTIMATE. It is the class's
    /// weakest old talent (so converting it removes the least): it can no longer be
    /// invested (its old per-rank effect therefore stays at 0 = removed), and instead
    /// auto-activates as the ultimate once the hero hits level 30.
    pub fn ult_slot(self) -> usize {
        match self {
            Class::Warrior => 3,    // was 血性反击
            Class::Mage => 4,       // was 元素亲和
            Class::Ranger => 3,     // was 连珠追猎
            Class::Guardian => 3,   // was 壁垒修复 (slot was unused in stats anyway)
            Class::Stormcaller => 4, // was 充能矩阵
            Class::Warden => 4,     // was 补给信标 (slot was unused in stats anyway)
            Class::Assassin => 4,   // was 破甲诅咒
            Class::Priest => 1,     // was 祷言祝福
            Class::Engineer => 1,   // was 过载线圈 → 神之塔
        }
    }

    /// Sprite file (under `sprites/hero_talents/`) for the ultimate talent slot.
    pub fn ultimate_sprite_name(self) -> &'static str {
        match self {
            Class::Warrior => "ult_warrior",
            Class::Mage => "ult_mage",
            Class::Ranger => "ult_ranger",
            Class::Guardian => "ult_guardian",
            Class::Stormcaller => "ult_stormcaller",
            Class::Warden => "ult_warden",
            Class::Assassin => "ult_assassin",
            Class::Priest => "ult_priest",
            Class::Engineer => "ult_engineer",
        }
    }

    /// The class's level-30 ultimate name (shown on the ult talent slot).
    pub fn ultimate_name(self) -> &'static str {
        match self {
            Class::Warrior => "不死战神",
            Class::Mage => "群星坠落",
            Class::Ranger => "万箭风暴",
            Class::Guardian => "不灭壁垒",
            Class::Stormcaller => "诸神黄昏",
            Class::Warden => "永恒哨域",
            Class::Assassin => "绝命刺杀",
            Class::Priest => "圣光复苏",
            Class::Engineer => "神之塔",
        }
    }

    /// The class's level-30 ultimate description.
    pub fn ultimate_desc(self) -> &'static str {
        match self {
            Class::Warrior => "30级解锁：伤害+50%、生命+60%、护甲大幅提升，真正的单刷战神",
            Class::Mage => "30级解锁：伤害+60%、爆炸范围剧增，一发清屏",
            Class::Ranger => "30级解锁：攻速翻倍并获得巨额穿甲，万箭覆盖全场",
            Class::Guardian => "30级解锁：生命翻倍、护甲暴涨，不可撼动的壁垒",
            Class::Stormcaller => "30级解锁：雷链+5次跳跃、伤害+40%，连锁吞噬全场",
            Class::Warden => "30级解锁：自身射程+50%，戍卫之眼笼罩战场",
            Class::Assassin => "30级解锁：伤害+80%、巨额穿甲，背击必致命，专斩BOSS",
            Class::Priest => "30级解锁：治疗与召唤强化登峰，召唤大军永生不灭",
            Class::Engineer => "30级解锁：召唤一座【神之塔】，集合所有塔之力镇守全场",
        }
    }

    /// One-line role tag (攻击距离 · 定位) shown in the class tooltip so the player can
    /// tell at a glance how the class is meant to be played.
    pub fn role(self) -> &'static str {
        match self {
            Class::Warrior => "近战 · 单刷守关",
            Class::Mage => "远程 · 范围歼灭",
            Class::Ranger => "远程 · 打钱发育",
            Class::Guardian => "近战 · 扛线增伤",
            Class::Stormcaller => "辅助 · 群体减速",
            Class::Warden => "辅助 · 射程增幅 · 反隐形",
            Class::Assassin => "近战 · 背击打BOSS",
            Class::Priest => "辅助 · 召唤治疗",
            Class::Engineer => "辅助 · 攻速超频",
        }
    }

    /// The class's signature passive — see [`Doctrine`]. This is the main thing that
    /// makes classes play differently (solo / economy / tower-synergy routes).
    pub fn doctrine(self) -> Doctrine {
        match self {
            // Solo bruiser: heavy self-regen → hold a lane with no towers.
            Class::Warrior => Doctrine {
                name: "不灭战魂",
                desc: "每秒回复生命，越战越勇，可脱离防御塔单独守关",
                regen_pct: 0.05,
                ..Doctrine::ZERO
            },
            // Solo nuker that also amps nearby magic towers.
            Class::Mage => Doctrine {
                name: "湮灭领域",
                desc: "范围歼灭敌群，并为周围防御塔提供奥术增幅(+攻击)",
                aura_damage: 0.12,
                ..Doctrine::ZERO
            },
            // Economy: bounty gold on every kill (anywhere) while alive.
            Class::Ranger => Doctrine {
                name: "赏金猎手",
                desc: "全场击杀额外获得30%金币，发育与打钱效率最高",
                gold_bonus: 0.30,
                ..Doctrine::ZERO
            },
            // Frontline commander: damage aura + a little self-regen.
            Class::Guardian => Doctrine {
                name: "统御军阵",
                desc: "光环提升周围防御塔伤害(+15%)，自身扛线回血",
                aura_damage: 0.15,
                regen_pct: 0.02,
                ..Doctrine::ZERO
            },
            // Battlefield control: a persistent slow FIELD around the hero — the only
            // class that debuffs enemies directly (群体减速核心), plus a small dmg aura.
            Class::Stormcaller => Doctrine {
                name: "风暴领域",
                desc: "在身边形成减速力场，范围内敌人持续被减速，并小幅增伤周围塔",
                enemy_slow: 0.25,
                aura_damage: 0.08,
                ..Doctrine::ZERO
            },
            // Sentinel: extends the RANGE of nearby towers — lets short-range towers
            // cover far more path (远程辅助核心), distinct from the haste/damage buffers.
            Class::Warden => Doctrine {
                name: "戍卫结界",
                desc: "大幅提升周围防御塔射程(+30%)，让防线覆盖更远的路径",
                aura_range: 0.30,
                ..Doctrine::ZERO
            },
            // Duelist economy hybrid: small bounty + sustain.
            Class::Assassin => Doctrine {
                name: "背击刺杀",
                desc: "从敌人背后攻击触发背击(对BOSS伤害x2.6)，击杀额外15%金币",
                gold_bonus: 0.15,
                regen_pct: 0.02,
                ..Doctrine::ZERO
            },
            // Summon support: repairs towers, and massively empowers summons —
            // makes 召唤塔 / 复活塔 the core build (faster spawns, stronger minions).
            Class::Priest => Doctrine {
                name: "圣疗领域",
                desc: "修复周围塔并治疗萝卜；强化召唤物(+60%伤害/回血/延寿)，并加速召唤塔与复活塔",
                tower_heal: 0.03,
                regen_pct: 0.02,
                aura_haste: 0.20,
                summon_power: 0.6,
                ..Doctrine::ZERO
            },
            // Best raw tower-synergy: big attack-speed aura.
            Class::Engineer => Doctrine {
                name: "超频力场",
                desc: "塔联动最强：全面提升周围防御塔攻速(+35%)",
                aura_haste: 0.35,
                ..Doctrine::ZERO
            },
        }
    }

    pub fn sprite_name(self) -> &'static str {
        match self {
            Class::Warrior => "warrior",
            Class::Mage => "mage",
            Class::Ranger => "ranger",
            Class::Guardian => "guardian",
            Class::Stormcaller => "stormcaller",
            Class::Warden => "warden",
            Class::Assassin => "assassin",
            Class::Priest => "priest",
            Class::Engineer => "engineer",
        }
    }

    pub fn skill_name(self) -> &'static str {
        match self {
            Class::Warrior => "战旗冲锋",
            Class::Mage => "星火风暴",
            Class::Ranger => "猎影齐射",
            Class::Guardian => "守护壁垒",
            Class::Stormcaller => "雷云审判",
            Class::Warden => "哨戒结界",
            Class::Assassin => "死印爆发",
            Class::Priest => "圣辉祷言",
            Class::Engineer => "过载装置",
        }
    }

    pub fn skill_desc(self) -> &'static str {
        match self {
            Class::Warrior => "震击英雄周围敌人，造成物理伤害并短暂眩晕，同时恢复英雄生命",
            Class::Mage => "锁定高生命敌人，引爆奥术风暴，对范围内敌人造成魔法伤害和冰冻",
            Class::Ranger => "标记最靠前的敌群，连续穿透射击并附加剧毒减速",
            Class::Guardian => "修复附近防御塔，鼓舞塔攻势，并冻结贴近防线的敌人",
            Class::Stormcaller => "召来雷云多段轰击前线敌群，造成雷风伤害和减速",
            Class::Warden => "展开哨戒结界，强化附近塔并缠绕、削弱敌群",
            Class::Assassin => "给最靠前敌人打上死印，造成暗影爆发、剧毒和破甲诅咒",
            Class::Priest => "治疗英雄和周围防御塔，祝福塔群，同时虚弱敌人",
            Class::Engineer => "超频附近防御塔，修复结构，并用电磁脉冲迟滞敌人",
        }
    }

    pub fn skill_sprite_name(self) -> &'static str {
        match self {
            Class::Warrior => "warrior_banner",
            Class::Mage => "mage_storm",
            Class::Ranger => "ranger_volley",
            Class::Guardian => "guardian_shield",
            Class::Stormcaller => "stormcaller_tempest",
            Class::Warden => "warden_totem",
            Class::Assassin => "assassin_mark",
            Class::Priest => "priest_benediction",
            Class::Engineer => "engineer_overclock",
        }
    }

    pub fn skill_color(self) -> Color {
        match self {
            Class::Warrior => Color::srgb(1.0, 0.42, 0.22),
            Class::Mage => Color::srgb(0.55, 0.42, 1.0),
            Class::Ranger => Color::srgb(0.35, 0.92, 0.55),
            Class::Guardian => Color::srgb(0.35, 0.72, 1.0),
            Class::Stormcaller => Color::srgb(1.0, 0.92, 0.28),
            Class::Warden => Color::srgb(0.42, 0.86, 0.62),
            Class::Assassin => Color::srgb(0.76, 0.38, 0.95),
            Class::Priest => Color::srgb(1.0, 0.92, 0.62),
            Class::Engineer => Color::srgb(1.0, 0.63, 0.32),
        }
    }

    pub fn talent_name(self, index: usize) -> &'static str {
        if index == self.ult_slot() {
            return self.ultimate_name();
        }
        match (self, index) {
            (Class::Warrior, 0) => "破阵重击",
            (Class::Warrior, 1) => "钢铁壁垒",
            (Class::Warrior, 2) => "战旗统御",
            (Class::Mage, 0) => "奥术过载",
            (Class::Mage, 1) => "扩散符文",
            (Class::Mage, 2) => "时序回响",
            (Class::Ranger, 0) => "鹰眼射术",
            (Class::Ranger, 1) => "疾行游猎",
            (Class::Ranger, 2) => "淬毒陷击",
            (Class::Warrior, 3) => "血性反击",
            (Class::Warrior, 4) => "震地压制",
            (Class::Warrior, 5) => "霸者姿态",
            (Class::Mage, 3) => "寒星禁锢",
            (Class::Mage, 4) => "元素亲和",
            (Class::Mage, 5) => "群星法阵",
            (Class::Ranger, 3) => "连珠追猎",
            (Class::Ranger, 4) => "弱点标记",
            (Class::Ranger, 5) => "风行大师",
            (Class::Guardian, 0) => "坚盾训练",
            (Class::Guardian, 1) => "守护光环",
            (Class::Guardian, 2) => "反击阵线",
            (Class::Guardian, 3) => "壁垒修复",
            (Class::Guardian, 4) => "挑衅压制",
            (Class::Guardian, 5) => "不动堡垒",
            (Class::Stormcaller, 0) => "雷击导体",
            (Class::Stormcaller, 1) => "暴风链",
            (Class::Stormcaller, 2) => "静电过载",
            (Class::Stormcaller, 3) => "风暴减速",
            (Class::Stormcaller, 4) => "充能矩阵",
            (Class::Stormcaller, 5) => "天怒风眼",
            (Class::Warden, 0) => "哨戒阵地",
            (Class::Warden, 1) => "战术协同",
            (Class::Warden, 2) => "藤蔓缠绕",
            (Class::Warden, 3) => "远望标尺",
            (Class::Warden, 4) => "补给信标",
            (Class::Warden, 5) => "森罗壁垒",
            (Class::Assassin, 0) => "暗刃训练",
            (Class::Assassin, 1) => "毒影灌注",
            (Class::Assassin, 2) => "死亡标记",
            (Class::Assassin, 3) => "闪袭步法",
            (Class::Assassin, 4) => "破甲诅咒",
            (Class::Assassin, 5) => "终结手法",
            (Class::Priest, 0) => "圣辉灌注",
            (Class::Priest, 1) => "祷言祝福",
            (Class::Priest, 2) => "赦罪护盾",
            (Class::Priest, 3) => "虚弱咒",
            (Class::Priest, 4) => "恢复仪式",
            (Class::Priest, 5) => "神圣共鸣",
            (Class::Engineer, 0) => "精密齿轮",
            (Class::Engineer, 1) => "过载线圈",
            (Class::Engineer, 2) => "扩容炮架",
            (Class::Engineer, 3) => "自动修复",
            (Class::Engineer, 4) => "电磁脉冲",
            (Class::Engineer, 5) => "主控核心",
            _ => "未知天赋",
        }
    }

    pub fn talent_desc(self, index: usize) -> &'static str {
        if index == self.ult_slot() {
            return self.ultimate_desc();
        }
        match (self, index) {
            (Class::Warrior, 0) => "提高攻击，并把普攻改为小范围顺劈",
            (Class::Warrior, 1) => "提高生命和护甲，让英雄能顶住攻城怪",
            (Class::Warrior, 2) => "提高攻速和移动速度，并缩短职业技能冷却",
            (Class::Mage, 0) => "提高奥术伤害，职业技能伤害同步提高",
            (Class::Mage, 1) => "提高射程和爆炸范围，增强控场覆盖",
            (Class::Mage, 2) => "提高施法频率，并延长职业技能冰冻",
            (Class::Ranger, 0) => "提高远程伤害和射程",
            (Class::Ranger, 1) => "提高攻速和移动速度，便于游走补线",
            (Class::Ranger, 2) => "强化毒箭和猎影齐射的减速/毒伤",
            (Class::Warrior, 3) => "受伤后更能维持输出，并提高生命",
            (Class::Warrior, 4) => "扩大顺劈范围，技能眩晕更稳定",
            (Class::Warrior, 5) => "提高全属性成长，降低技能冷却",
            (Class::Mage, 3) => "延长冰冻并增强控制技能",
            (Class::Mage, 4) => "提高元素伤害并略微提高生存",
            (Class::Mage, 5) => "扩大技能法阵，提升终局爆发",
            (Class::Ranger, 3) => "提高连射频率和齐射目标数",
            (Class::Ranger, 4) => "提高穿甲，并让技能更擅长点杀",
            (Class::Ranger, 5) => "进一步提升移速、射程和技能冷却",
            (Class::Guardian, 0) => "提高生命、护甲和前线承伤能力",
            (Class::Guardian, 1) => "强化主动技能对附近塔的鼓舞",
            (Class::Guardian, 2) => "提高反击伤害并获得击退普通攻击",
            (Class::Guardian, 3) => "主动技能修复更多塔生命",
            (Class::Guardian, 4) => "让攻击和技能更擅长迟滞敌人",
            (Class::Guardian, 5) => "大幅提升坦度并缩短壁垒冷却",
            (Class::Stormcaller, 0) => "提高雷风伤害",
            (Class::Stormcaller, 1) => "增加连锁次数和跳跃距离",
            (Class::Stormcaller, 2) => "提高攻速并缩短技能冷却",
            (Class::Stormcaller, 3) => "增强普攻和技能减速",
            (Class::Stormcaller, 4) => "主动技能额外超频附近塔",
            (Class::Stormcaller, 5) => "提高雷云半径和终局伤害",
            (Class::Warden, 0) => "提高阵地伤害和耐久",
            (Class::Warden, 1) => "主动技能鼓舞更多防御塔",
            (Class::Warden, 2) => "增强缠绕减速和毒性压制",
            (Class::Warden, 3) => "提高射程和索敌稳定性",
            (Class::Warden, 4) => "主动技能额外修复防御塔",
            (Class::Warden, 5) => "提高阵地范围和技能冷却效率",
            (Class::Assassin, 0) => "提高暗影直伤和穿甲",
            (Class::Assassin, 1) => "强化毒伤和持续时间",
            (Class::Assassin, 2) => "技能死印命中更多目标",
            (Class::Assassin, 3) => "提高攻速、移速和脱战能力",
            (Class::Assassin, 4) => "增强诅咒破甲效果",
            (Class::Assassin, 5) => "提高对高生命目标的终结爆发",
            (Class::Priest, 0) => "提高圣辉伤害和治疗量",
            (Class::Priest, 1) => "主动技能祝福更多塔并提高增益",
            (Class::Priest, 2) => "提高英雄和塔的护甲恢复",
            (Class::Priest, 3) => "增强敌人虚弱和诅咒持续",
            (Class::Priest, 4) => "扩大恢复范围并提高生命",
            (Class::Priest, 5) => "提高圣辉共鸣，缩短祷言冷却",
            (Class::Engineer, 0) => "提高机械伤害和基础攻速",
            (Class::Engineer, 1) => "主动技能更强力地超频防御塔",
            (Class::Engineer, 2) => "提高射程和设备覆盖范围",
            (Class::Engineer, 3) => "主动技能修复更多塔结构",
            (Class::Engineer, 4) => "增强电磁脉冲减速和伤害",
            (Class::Engineer, 5) => "提高装备收益和超频冷却效率",
            _ => "",
        }
    }

    pub fn talent_sprite_name(self, index: usize) -> &'static str {
        if index == self.ult_slot() {
            return self.ultimate_sprite_name();
        }
        match (self, index) {
            (Class::Warrior, 0) => "warrior_cleave",
            (Class::Warrior, 1) => "warrior_guard",
            (Class::Warrior, 2) => "warrior_banner",
            (Class::Mage, 0) => "mage_overload",
            (Class::Mage, 1) => "mage_rune",
            (Class::Mage, 2) => "mage_echo",
            (Class::Ranger, 0) => "ranger_eye",
            (Class::Ranger, 1) => "ranger_stride",
            (Class::Ranger, 2) => "ranger_venom",
            (Class::Warrior, 3) => "warrior_counter",
            (Class::Warrior, 4) => "warrior_quake",
            (Class::Warrior, 5) => "warrior_warlord",
            (Class::Mage, 3) => "mage_froststar",
            (Class::Mage, 4) => "mage_attune",
            (Class::Mage, 5) => "mage_constellation",
            (Class::Ranger, 3) => "ranger_barrage",
            (Class::Ranger, 4) => "ranger_mark",
            (Class::Ranger, 5) => "ranger_mastery",
            (Class::Guardian, 0) => "guardian_shield",
            (Class::Guardian, 1) => "guardian_aura",
            (Class::Guardian, 2) => "guardian_counter",
            (Class::Guardian, 3) => "guardian_repair",
            (Class::Guardian, 4) => "guardian_taunt",
            (Class::Guardian, 5) => "guardian_bastion",
            (Class::Stormcaller, 0) => "stormcaller_conduit",
            (Class::Stormcaller, 1) => "stormcaller_chain",
            (Class::Stormcaller, 2) => "stormcaller_static",
            (Class::Stormcaller, 3) => "stormcaller_slow",
            (Class::Stormcaller, 4) => "stormcaller_matrix",
            (Class::Stormcaller, 5) => "stormcaller_eye",
            (Class::Warden, 0) => "warden_watch",
            (Class::Warden, 1) => "warden_coordination",
            (Class::Warden, 2) => "warden_vines",
            (Class::Warden, 3) => "warden_sight",
            (Class::Warden, 4) => "warden_supply",
            (Class::Warden, 5) => "warden_grove",
            (Class::Assassin, 0) => "assassin_blade",
            (Class::Assassin, 1) => "assassin_venom",
            (Class::Assassin, 2) => "assassin_mark",
            (Class::Assassin, 3) => "assassin_step",
            (Class::Assassin, 4) => "assassin_curse",
            (Class::Assassin, 5) => "assassin_execute",
            (Class::Priest, 0) => "priest_light",
            (Class::Priest, 1) => "priest_blessing",
            (Class::Priest, 2) => "priest_shield",
            (Class::Priest, 3) => "priest_weaken",
            (Class::Priest, 4) => "priest_ritual",
            (Class::Priest, 5) => "priest_resonance",
            (Class::Engineer, 0) => "engineer_gears",
            (Class::Engineer, 1) => "engineer_overclock",
            (Class::Engineer, 2) => "engineer_mount",
            (Class::Engineer, 3) => "engineer_repair",
            (Class::Engineer, 4) => "engineer_pulse",
            (Class::Engineer, 5) => "engineer_core",
            _ => "warrior_cleave",
        }
    }

    /// (damage, range, cooldown_s, hp, behavior, element, aoe_radius).
    fn base(self) -> ClassBase {
        match self {
            Class::Warrior => ClassBase {
                damage: 82.0, // melee cleave: ~2-tile reach, hits a GROUP
                range: 80.0,
                cooldown: 0.6,
                hp: 640.0,
                behavior: Behavior::Aoe,
                element: Element::Physical,
                aoe_radius: 64.0,
            },
            Class::Mage => ClassBase {
                damage: 46.0,
                range: 175.0,
                cooldown: 1.1,
                hp: 340.0,
                behavior: Behavior::Aoe,
                element: Element::Arcane,
                aoe_radius: 72.0,
            },
            Class::Ranger => ClassBase {
                damage: 52.0,
                range: 250.0, // longest reach
                cooldown: 0.5,
                hp: 360.0,
                behavior: Behavior::Single,
                element: Element::Physical,
                aoe_radius: 0.0,
            },
            Class::Guardian => ClassBase {
                damage: 56.0, // melee tank: single-target, ~1.5 tiles
                range: 62.0,
                cooldown: 0.78,
                hp: 760.0,
                behavior: Behavior::Single,
                element: Element::Physical,
                aoe_radius: 0.0,
            },
            Class::Stormcaller => ClassBase {
                damage: 38.0,
                range: 195.0,
                cooldown: 0.82,
                hp: 420.0,
                behavior: Behavior::Chain,
                element: Element::Storm,
                aoe_radius: 0.0,
            },
            Class::Warden => ClassBase {
                damage: 40.0,
                range: 210.0,
                cooldown: 0.86,
                hp: 520.0,
                behavior: Behavior::Slow,
                element: Element::Frost,
                aoe_radius: 0.0,
            },
            Class::Assassin => ClassBase {
                damage: 70.0, // melee rogue: single-target poison, ~1 tile, fast
                range: 48.0,
                cooldown: 0.46,
                hp: 330.0,
                behavior: Behavior::Poison,
                element: Element::Toxic,
                aoe_radius: 0.0,
            },
            Class::Priest => ClassBase {
                damage: 36.0,
                range: 160.0,
                cooldown: 0.92,
                hp: 500.0,
                behavior: Behavior::Curse,
                element: Element::Arcane,
                aoe_radius: 0.0,
            },
            Class::Engineer => ClassBase {
                damage: 48.0,
                range: 175.0,
                cooldown: 0.64,
                hp: 460.0,
                behavior: Behavior::Single,
                element: Element::Physical,
                aoe_radius: 0.0,
            },
        }
    }
}

struct ClassBase {
    damage: f32,
    range: f32,
    cooldown: f32,
    hp: f32,
    behavior: Behavior,
    element: Element,
    aoe_radius: f32,
}

/// A class's signature passive identity, applied every frame by [`hero_doctrine`].
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
    /// +range fraction granted to towers within the hero's aura (Warden).
    pub aura_range: f32,
    /// If >0, refreshes a slow on enemies within the aura (Stormcaller CC field):
    /// the value is the slow_timer seconds re-applied each frame.
    pub enemy_slow: f32,
    /// HP/sec (fraction of the tower's max HP) repaired to towers in the aura.
    pub tower_heal: f32,
    /// +gold fraction on every enemy kill while the hero is alive.
    pub gold_bonus: f32,
    /// +damage fraction granted to ALL summons (召唤物), who are also healed and
    /// have their decay slowed. Drives the Priest's summon-synergy route (召唤塔/复活塔).
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

/// Each frame, project the living hero's class doctrine onto the battlefield:
/// regenerate the hero, buff/heal towers within its aura (联动), and set the global
/// gold bounty (打钱). This is the main source of per-class playstyle divergence.
pub fn hero_doctrine(
    time: Res<Time>,
    mut run: ResMut<crate::game::RunState>,
    loadout: Res<HeroLoadout>,
    mut towers: Query<(Entity, &mut Tower)>,
    mut summons: Query<&mut crate::tower::Summon>,
    mut enemies: Query<(&mut crate::components::Enemy, &Transform)>,
) {
    let dt = time.delta_secs() * run.game_speed;
    let doc = loadout.class.doctrine();
    let scale = 1.0 + loadout.level.saturating_sub(1) as f32 * 0.03;

    // Find the living hero (entity, position, aura radius) before mutating.
    let hero = towers
        .iter()
        .find(|(_, t)| t.hero && t.hp > 0.0)
        .map(|(e, t)| (e, t.center(), t.buff_range));

    run.hero_gold_bonus = match hero {
        Some(_) => doc.gold_bonus * scale,
        None => 0.0,
    };

    // Priest 圣疗领域: empower every summon (召唤物联动) — bonus damage, regen, and
    // slowed decay so召唤塔/复活塔 builds scale. Reset to 0 when no Priest is alive.
    let summon_power = match hero {
        Some(_) => doc.summon_power * scale,
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
            t.aura_damage = doc.aura_damage * scale;
            t.aura_haste = doc.aura_haste * scale;
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

    // Stormcaller 风暴领域: a persistent slow field. Re-apply the slow each frame to
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
    class: Class,
    level: u8,
    xp: i32,
    points: u8,
    talents: [[u8; HeroLoadout::TALENT_SLOTS]; Class::ALL.len()],
}

/// The player's chosen hero, persisted across sessions, plus run state.
#[derive(Resource)]
pub struct HeroLoadout {
    pub race: Race,
    pub class: Class,
    pub level: u8,
    pub xp: i32,
    pub talent_points: u8,
    pub class_talents: [[u8; Self::TALENT_SLOTS]; Class::ALL.len()],
    pub skill_cd: i32,
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
            class: saved.class,
            level: saved.level.clamp(1, Self::MAX_LEVEL),
            xp: saved.xp.max(0),
            talent_points: saved.points,
            class_talents: saved.talents,
            skill_cd: 0,
            alive: false,
            respawn_waves: 0,
        }
    }
}

impl HeroLoadout {
    pub const MAX_LEVEL: u8 = 30;
    pub const TALENT_SLOTS: usize = 6;
    pub const TALENT_MAX_RANK: u8 = 5;

    /// Pick a class directly (hero selection screen), persisting the choice.
    pub fn set_class(&mut self, class: Class) {
        self.class = class;
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

    pub fn class_index(&self) -> usize {
        Class::ALL
            .iter()
            .position(|class| *class == self.class)
            .unwrap_or(0)
    }

    pub fn talent_rank(&self, index: usize) -> u8 {
        self.class_talents
            .get(self.class_index())
            .and_then(|row| row.get(index))
            .copied()
            .unwrap_or(0)
    }

    pub fn spent_in_current_class(&self) -> u8 {
        self.class_talents[self.class_index()].iter().sum()
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
        if index == self.class.ult_slot() {
            return Err("终极天赋将在30级自动解锁，无需投点");
        }
        if self.talent_points == 0 {
            return Err("没有可用天赋点");
        }
        let class_index = self.class_index();
        if self.class_talents[class_index][index] >= Self::TALENT_MAX_RANK {
            return Err("该天赋已满级");
        }
        self.class_talents[class_index][index] += 1;
        self.talent_points -= 1;
        save_hero(self);
        Ok(())
    }

    pub fn respec_current_class(&mut self) -> u8 {
        let class_index = self.class_index();
        let refunded: u8 = self.class_talents[class_index].iter().sum();
        self.class_talents[class_index] = [0; Self::TALENT_SLOTS];
        self.talent_points = self.talent_points.saturating_add(refunded);
        save_hero(self);
        refunded
    }

    pub fn tick_wave_cooldowns(&mut self) {
        self.skill_cd = (self.skill_cd - 1).max(0);
    }

    pub fn skill_cooldown_max(&self) -> i32 {
        let base = match self.class {
            Class::Warrior => 3,
            Class::Mage => 4,
            Class::Ranger => 3,
            Class::Guardian => 4,
            Class::Stormcaller => 4,
            Class::Warden => 4,
            Class::Assassin => 3,
            Class::Priest => 4,
            Class::Engineer => 4,
        };
        (base - ((self.talent_rank(2) + self.talent_rank(5)) / 3) as i32).max(1)
    }

    pub fn skill_damage_mult(&self) -> f32 {
        let level = 1.0 + (self.level.saturating_sub(1) as f32 * 0.045);
        let talent = match self.class {
            Class::Warrior => {
                1.0 + self.talent_rank(0) as f32 * 0.13 + self.talent_rank(5) as f32 * 0.05
            }
            Class::Mage => {
                1.0 + self.talent_rank(0) as f32 * 0.16 + self.talent_rank(5) as f32 * 0.07
            }
            Class::Ranger => {
                1.0 + self.talent_rank(0) as f32 * 0.11 + self.talent_rank(4) as f32 * 0.06
            }
            Class::Guardian => {
                1.0 + self.talent_rank(2) as f32 * 0.08 + self.talent_rank(5) as f32 * 0.04
            }
            Class::Stormcaller => {
                1.0 + self.talent_rank(0) as f32 * 0.12 + self.talent_rank(5) as f32 * 0.08
            }
            Class::Warden => {
                1.0 + self.talent_rank(0) as f32 * 0.08 + self.talent_rank(5) as f32 * 0.05
            }
            Class::Assassin => {
                1.0 + self.talent_rank(0) as f32 * 0.12 + self.talent_rank(5) as f32 * 0.10
            }
            Class::Priest => {
                1.0 + self.talent_rank(0) as f32 * 0.08 + self.talent_rank(5) as f32 * 0.05
            }
            Class::Engineer => {
                1.0 + self.talent_rank(0) as f32 * 0.10 + self.talent_rank(5) as f32 * 0.06
            }
        };
        level * talent
    }
}

/// Movement speed (world px/sec) for this race+class.
pub fn hero_move_speed(loadout: &HeroLoadout) -> f32 {
    let talent_speed = match loadout.class {
        Class::Warrior => 1.0 + loadout.talent_rank(2) as f32 * 0.04,
        Class::Mage => 1.0,
        Class::Ranger => {
            1.0 + loadout.talent_rank(1) as f32 * 0.07 + loadout.talent_rank(5) as f32 * 0.03
        }
        Class::Guardian => 0.92 + loadout.talent_rank(5) as f32 * 0.025,
        Class::Stormcaller => 1.0 + loadout.talent_rank(2) as f32 * 0.035,
        Class::Warden => 0.98 + loadout.talent_rank(3) as f32 * 0.025,
        Class::Assassin => {
            1.12 + loadout.talent_rank(3) as f32 * 0.06 + loadout.talent_rank(5) as f32 * 0.02
        }
        Class::Priest => 0.96 + loadout.talent_rank(4) as f32 * 0.025,
        Class::Engineer => 0.98 + loadout.talent_rank(2) as f32 * 0.02,
    };
    110.0 * loadout.race.mods().speed * talent_speed
}

/// Build a [`Tower`] configured as the hero at `pos`.
pub fn make_hero_tower(loadout: &HeroLoadout, pos: Vec2) -> Tower {
    // Start from an ordinary def so every Tower field has a sane value, then
    // overwrite the combat stats with the race×class profile.
    let mut t = Tower::from_def(TowerKind::Arrow.def(), 0, 0);
    t.hero = true;
    t.hero_pos = pos;
    t.move_target = None;
    t.footprint = 1;
    apply_loadout_to_tower(loadout, &mut t);
    t.hp = t.max_hp;
    t
}

pub fn apply_loadout_to_tower(loadout: &HeroLoadout, t: &mut Tower) {
    let base = loadout.class.base();
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
    let equipped_count = t.equipment_count() as f32;
    let mut damage_mult = level_mult;
    let mut range_mult = 1.0;
    let mut cooldown_mult = 1.0;
    let mut hp_mult = 1.0;
    let mut armor_bonus = 0.0;
    let mut armor_pierce_bonus = 0.0;
    let mut aoe_bonus = 0.0;

    t.behavior = base.behavior;
    // Warden is the 哨兵 (sentinel): built-in 反隐形 — reveals invisible enemies in
    // range so the player never needs a separate detection tower with this hero.
    t.detector = loadout.class == Class::Warden;
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

    match loadout.class {
        Class::Warrior => {
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
        Class::Mage => {
            damage_mult *= 1.0 + a * 0.13 + e * 0.05 + f * 0.06;
            range_mult *= 1.0 + b * 0.06 + e * 0.02;
            cooldown_mult *= 1.0 - c * 0.06;
            hp_mult *= 1.0 + e * 0.03;
            aoe_bonus += b * 10.0 + f * 8.0;
            t.freeze_duration = 0.65 + d * 0.16;
            // Aura radius so the 湮灭领域 doctrine can amp nearby magic towers.
            t.buff_range = 115.0 + b * 16.0 + f * 8.0;
        }
        Class::Ranger => {
            damage_mult *= 1.0 + a * 0.10 + d * 0.04 + f * 0.04;
            range_mult *= 1.0 + a * 0.04 + f * 0.03;
            cooldown_mult *= 1.0 - b * 0.07 - d * 0.035;
            hp_mult *= 1.0 + f * 0.03;
            armor_pierce_bonus += e * 6.0;
            if c > 0.0 {
                t.behavior = Behavior::Poison;
                t.dot_damage = 14.0 + c * 9.0;
                t.poison_duration = 2.4 + c * 0.45;
            }
        }
        Class::Guardian => {
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
        Class::Stormcaller => {
            damage_mult *= 1.0 + a * 0.12 + f * 0.07;
            range_mult *= 1.0 + b * 0.02 + f * 0.02;
            cooldown_mult *= 1.0 - c * 0.05 - f * 0.02;
            hp_mult *= 1.0 + e * 0.03;
            t.behavior = Behavior::Chain;
            t.chain_count = 2 + b as i32;
            t.chain_range = 80.0 + b * 14.0 + f * 8.0;
            t.slow_duration = 0.6 + d * 0.18;
            t.buff_range = 125.0 + e * 16.0;
        }
        Class::Warden => {
            damage_mult *= 1.0 + a * 0.08 + f * 0.05;
            range_mult *= 1.0 + d * 0.06 + f * 0.02;
            cooldown_mult *= 1.0 - b * 0.025 - f * 0.025;
            hp_mult *= 1.0 + a * 0.06 + f * 0.05;
            armor_bonus += f * 3.0;
            t.behavior = Behavior::Slow;
            t.slow_duration = 0.9 + c * 0.22 + f * 0.08;
            t.buff_range = 130.0 + b * 18.0 + f * 10.0;
        }
        Class::Assassin => {
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
        Class::Priest => {
            damage_mult *= 1.0 + a * 0.08 + f * 0.04;
            range_mult *= 1.0 + e * 0.02;
            cooldown_mult *= 1.0 - f * 0.03;
            hp_mult *= 1.0 + c * 0.08 + e * 0.05;
            armor_bonus += c * 3.0 + f * 2.0;
            t.behavior = Behavior::Curse;
            t.armor_reduce = 8.0 + d * 4.0 + f * 2.0;
            t.curse_duration = 1.8 + d * 0.25;
            t.heal_amount = 12.0 + a * 4.0;
            t.buff_range = 135.0 + b * 18.0 + e * 12.0;
        }
        Class::Engineer => {
            damage_mult *= 1.0 + a * 0.10 + f * 0.04 + equipped_count * f * 0.025;
            range_mult *= 1.0 + c * 0.05 + f * 0.02;
            cooldown_mult *= 1.0 - a * 0.03 - b * 0.02 - f * 0.03 - equipped_count * f * 0.01;
            hp_mult *= 1.0 + d * 0.06;
            armor_bonus += d * 3.0;
            armor_pierce_bonus += e * 4.0;
            t.buff_range = 135.0 + c * 16.0 + f * 10.0;
            if e > 0.0 {
                t.behavior = Behavior::Slow;
                t.slow_duration = 0.65 + e * 0.18;
            }
        }
    }

    // ===== Level-30 ULTIMATE =====
    // A dramatic capstone that auto-activates at max level. The Engineer's ultimate
    // (神之塔) is a summoned entity handled by `summon_god_tower`, so it adds nothing here.
    if loadout.level >= HeroLoadout::MAX_LEVEL {
        match loadout.class {
            Class::Warrior => {
                damage_mult *= 1.5;
                hp_mult *= 1.6;
                armor_bonus += 30.0;
            }
            Class::Mage => {
                damage_mult *= 1.6;
                aoe_bonus += 90.0;
            }
            Class::Ranger => {
                cooldown_mult *= 0.5;
                armor_pierce_bonus += 30.0;
            }
            Class::Guardian => {
                hp_mult *= 2.0;
                armor_bonus += 50.0;
            }
            Class::Stormcaller => {
                damage_mult *= 1.4;
                t.chain_count += 5;
                t.chain_range += 60.0;
            }
            Class::Warden => {
                range_mult *= 1.5;
            }
            Class::Assassin => {
                damage_mult *= 1.8;
                armor_pierce_bonus += 40.0;
            }
            Class::Priest => {
                hp_mult *= 1.3;
                t.heal_amount += 40.0;
            }
            Class::Engineer => {} // 神之塔 spawned by summon_god_tower
        }
    }

    t.element = base.element;
    t.magic = base.element != Element::Physical;
    t.color = loadout.race.color();
    t.aoe_radius = (base.aoe_radius + aoe_bonus).max(base.aoe_radius);
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
    crate::equipment::apply_equipment_stats(t);
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

// ---- persistence (race,class indices) ----

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
        class: Class::Warrior,
        level: 1,
        xp: 0,
        points: 0,
        talents: [[0; HeroLoadout::TALENT_SLOTS]; Class::ALL.len()],
    }
}

fn parse_hero(raw: &str) -> HeroSave {
    let raw = raw.trim();
    let mut save = default_save();
    if let Some(rest) = raw.strip_prefix("v2,") {
        let nums = rest
            .split(',')
            .filter_map(|s| s.trim().parse::<i32>().ok())
            .collect::<Vec<_>>();
        save.race = nums
            .first()
            .and_then(|i| Race::ALL.get((*i).max(0) as usize).copied())
            .unwrap_or(save.race);
        save.class = nums
            .get(1)
            .and_then(|i| Class::ALL.get((*i).max(0) as usize).copied())
            .unwrap_or(save.class);
        save.level = nums
            .get(2)
            .copied()
            .unwrap_or(1)
            .clamp(1, HeroLoadout::MAX_LEVEL as i32) as u8;
        save.xp = nums.get(3).copied().unwrap_or(0).max(0);
        save.points = nums.get(4).copied().unwrap_or(0).clamp(0, 99) as u8;
        if nums.len() <= 5 + 3 * 3 {
            let mut cursor = 5;
            for class in 0..3 {
                for talent in 0..3 {
                    save.talents[class][talent] = nums
                        .get(cursor)
                        .copied()
                        .unwrap_or(0)
                        .clamp(0, HeroLoadout::TALENT_MAX_RANK as i32)
                        as u8;
                    cursor += 1;
                }
            }
            return save;
        }
        let mut cursor = 5;
        for class in 0..Class::ALL.len() {
            for talent in 0..HeroLoadout::TALENT_SLOTS {
                save.talents[class][talent] =
                    nums.get(cursor)
                        .copied()
                        .unwrap_or(0)
                        .clamp(0, HeroLoadout::TALENT_MAX_RANK as i32) as u8;
                cursor += 1;
            }
        }
        return save;
    }

    // Legacy format: "race,class".
    let mut parts = raw.split(',');
    let r = parts.next().and_then(|s| s.trim().parse::<usize>().ok());
    let c = parts.next().and_then(|s| s.trim().parse::<usize>().ok());
    save.race = r
        .and_then(|i| Race::ALL.get(i).copied())
        .unwrap_or(Race::Human);
    save.class = c
        .and_then(|i| Class::ALL.get(i).copied())
        .unwrap_or(Class::Warrior);
    save
}

fn encode_hero(loadout: &HeroLoadout) -> String {
    let ri = Race::ALL
        .iter()
        .position(|r| *r == loadout.race)
        .unwrap_or(0);
    let ci = Class::ALL
        .iter()
        .position(|c| *c == loadout.class)
        .unwrap_or(0);
    let mut parts = vec![
        "v2".to_string(),
        ri.to_string(),
        ci.to_string(),
        loadout.level.to_string(),
        loadout.xp.max(0).to_string(),
        loadout.talent_points.to_string(),
    ];
    for class in 0..Class::ALL.len() {
        for talent in 0..HeroLoadout::TALENT_SLOTS {
            parts.push(loadout.class_talents[class][talent].to_string());
        }
    }
    parts.join(",")
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
