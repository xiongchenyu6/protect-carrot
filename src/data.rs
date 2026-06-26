//! Static game data ported from the original `保卫萝卜.html` constant tables
//! (`TOWER_TYPES`, `ENEMY_TYPES`, `LEVELS`, ...). Kept as plain `const` data so it
//! is easy to compare against the source and easy to tune.
//!
//! Learning note: in Bevy you typically keep *design data* like this as plain Rust
//! (consts / a `Resource` loaded from disk), separate from the *runtime ECS state*
//! (components on entities). This module is the "design data" half.

use bevy::prelude::*;

// ----- Grid / board geometry (from the top of the original <script>) -----
pub const TILE_SIZE: f32 = 40.0;
pub const COLS: i32 = 20;
pub const ROWS: i32 = 15;
pub const BOARD_W: f32 = COLS as f32 * TILE_SIZE; // 800
pub const BOARD_H: f32 = ROWS as f32 * TILE_SIZE; // 600
pub const TOWER_RAIDER_SENSE: f32 = TILE_SIZE * 3.5;
pub const MOSS_TOWER_SENSE: f32 = TILE_SIZE * 5.0;
pub const TOWER_RAIDER_ENGAGE: f32 = TILE_SIZE * 1.25;

/// Convert a hex `0xRRGGBB` into a Bevy `Color`. Used by the data tables below so
/// the literals read like the original CSS hex colors.
pub const fn hex(c: u32) -> Color {
    Color::srgb(
        ((c >> 16) & 0xff) as f32 / 255.0,
        ((c >> 8) & 0xff) as f32 / 255.0,
        (c & 0xff) as f32 / 255.0,
    )
}

// ============================ Elements ============================

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Element {
    Physical,
    Arcane,
    Fire,
    Frost,
    Storm,
    Shadow,
    Toxic,
}

impl Element {
    pub const ALL: [Element; 7] = [
        Element::Physical,
        Element::Arcane,
        Element::Fire,
        Element::Frost,
        Element::Storm,
        Element::Shadow,
        Element::Toxic,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Element::Physical => "物理",
            Element::Arcane => "秘法",
            Element::Fire => "火焰",
            Element::Frost => "冰霜",
            Element::Storm => "雷风",
            Element::Shadow => "暗影",
            Element::Toxic => "剧毒",
        }
    }

    pub fn color(self) -> Color {
        match self {
            Element::Physical => hex(0xe8dcc0),
            Element::Arcane => hex(0xb58cff),
            Element::Fire => hex(0xff7a2f),
            Element::Frost => hex(0x80d8ff),
            Element::Storm => hex(0xffeb6b),
            Element::Shadow => hex(0x7a6a99),
            Element::Toxic => hex(0x72d572),
        }
    }
}

/// Fractional elemental resistance. `0.25` means 25% less damage; `-0.20` means
/// 20% vulnerability. The final multiplier is clamped in combat resolution.
#[derive(Clone, Copy, Debug)]
pub struct ElementProfile {
    pub physical: f32,
    pub arcane: f32,
    pub fire: f32,
    pub frost: f32,
    pub storm: f32,
    pub shadow: f32,
    pub toxic: f32,
}

impl ElementProfile {
    pub const fn new(
        physical: f32,
        arcane: f32,
        fire: f32,
        frost: f32,
        storm: f32,
        shadow: f32,
        toxic: f32,
    ) -> Self {
        Self {
            physical,
            arcane,
            fire,
            frost,
            storm,
            shadow,
            toxic,
        }
    }

    pub const fn none() -> Self {
        Self::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
    }

    pub fn get(self, e: Element) -> f32 {
        match e {
            Element::Physical => self.physical,
            Element::Arcane => self.arcane,
            Element::Fire => self.fire,
            Element::Frost => self.frost,
            Element::Storm => self.storm,
            Element::Shadow => self.shadow,
            Element::Toxic => self.toxic,
        }
    }
}

/// Convert a grid cell (col,row; origin top-left like the HTML canvas) to a Bevy
/// world position (origin center, +y up). We render the board centered on (0,0).
pub fn cell_center(col: f32, row: f32) -> Vec2 {
    Vec2::new(
        col * TILE_SIZE + TILE_SIZE / 2.0 - BOARD_W / 2.0,
        BOARD_H / 2.0 - (row * TILE_SIZE + TILE_SIZE / 2.0),
    )
}

// ============================ Towers ============================

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TowerKind {
    // attack
    Arrow,
    Cannon,
    Magic,
    Sniper,
    Thunder,
    Laser,
    Missile,
    Fortress,
    // control
    Ice,
    Wind,
    FrostNova,
    Shadow,
    // support
    Holy,
    Detection,
    // special
    Poison,
    Fire,
    Summon,
    Prism,
    Necromancer,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Category {
    Attack,
    Control,
    Support,
    Special,
}

impl Category {
    pub const ALL: [Category; 4] = [
        Category::Attack,
        Category::Control,
        Category::Support,
        Category::Special,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Category::Attack => "攻击",
            Category::Control => "控制",
            Category::Support => "辅助",
            Category::Special => "特殊",
        }
    }
}

/// How a tower behaves when it acts on a target. Mirrors the `type` field of the
/// original `TOWER_TYPES`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Behavior {
    Single,
    Aoe,
    Chain,
    Laser,
    Homing,
    Slow,
    Knockback,
    Freeze,
    Curse,
    Heal,
    Detect,
    Poison,
    Fire,
    Summon,
    /// Raises slain nearby enemies as allied units instead of attacking directly.
    Necromancer,
}

/// All stats for a tower kind. Behavior-specific fields are `0.0`/`0` when unused,
/// exactly like the optional properties in the JS object literals.
#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub struct TowerDef {
    pub kind: TowerKind,
    pub name: &'static str,
    pub icon: &'static str,
    pub color: Color,
    pub cost: i32,
    pub damage: f32,
    pub range: f32,
    /// Cooldown between shots, in milliseconds (original `speed`).
    pub cooldown_ms: f32,
    pub category: Category,
    pub behavior: Behavior,
    pub magic: bool,
    pub element: Element,
    pub max_hp: f32,
    pub armor: f32,
    /// Footprint in grid cells per side (1 = 1×1, 2 = 2×2). Multi-cell towers need
    /// a clear `footprint`×`footprint` block of buildable tiles.
    pub footprint: i32,
    // behavior extras
    pub aoe_radius: f32,
    pub chain_count: i32,
    pub chain_range: f32,
    pub slow_factor: f32,
    pub slow_duration: f32,
    pub knock_dist: f32,
    pub stun_duration: f32,
    pub freeze_duration: f32,
    pub armor_reduce: f32,
    pub curse_duration: f32,
    pub heal_amount: f32,
    pub buff_range: f32,
    pub dot_damage: f32,
    pub poison_duration: f32,
    pub fire_duration: f32,
    pub summon_hp: f32,
    pub summon_speed: f32,
    pub max_summons: i32,
    pub desc: &'static str,
}

/// Base for a `TowerDef` so the table below only sets the fields it cares about.
const fn base() -> TowerDef {
    TowerDef {
        kind: TowerKind::Arrow,
        name: "",
        icon: "",
        color: hex(0xffffff),
        cost: 0,
        damage: 0.0,
        range: 0.0,
        cooldown_ms: 0.0,
        category: Category::Attack,
        behavior: Behavior::Single,
        magic: false,
        element: Element::Physical,
        max_hp: 120.0,
        armor: 5.0,
        footprint: 1,
        aoe_radius: 0.0,
        chain_count: 0,
        chain_range: 0.0,
        slow_factor: 0.0,
        slow_duration: 0.0,
        knock_dist: 0.0,
        stun_duration: 0.0,
        freeze_duration: 0.0,
        armor_reduce: 0.0,
        curse_duration: 0.0,
        heal_amount: 0.0,
        buff_range: 0.0,
        dot_damage: 0.0,
        poison_duration: 0.0,
        fire_duration: 0.0,
        summon_hp: 0.0,
        summon_speed: 0.0,
        max_summons: 0,
        desc: "",
    }
}

impl TowerKind {
    pub const ALL: [TowerKind; 19] = [
        TowerKind::Arrow,
        TowerKind::Cannon,
        TowerKind::Magic,
        TowerKind::Sniper,
        TowerKind::Thunder,
        TowerKind::Laser,
        TowerKind::Missile,
        TowerKind::Fortress,
        TowerKind::Ice,
        TowerKind::Wind,
        TowerKind::FrostNova,
        TowerKind::Shadow,
        TowerKind::Holy,
        TowerKind::Detection,
        TowerKind::Poison,
        TowerKind::Fire,
        TowerKind::Summon,
        TowerKind::Prism,
        TowerKind::Necromancer,
    ];

    pub fn def(self) -> &'static TowerDef {
        // Linear scan over a 16-element table; trivial and avoids index/enum drift.
        TOWER_DEFS.iter().find(|d| d.kind == self).unwrap()
    }

    /// Sprite file stem under `assets/sprites/towers/`.
    pub fn sprite_name(self) -> &'static str {
        use TowerKind::*;
        match self {
            Arrow => "arrow",
            Cannon => "cannon",
            Magic => "magic",
            Sniper => "sniper",
            Thunder => "thunder",
            Laser => "laser",
            Missile => "missile",
            Ice => "ice",
            Wind => "wind",
            FrostNova => "frostnova",
            Shadow => "shadow",
            Holy => "holy",
            Detection => "detection",
            Poison => "poison",
            Fire => "fire",
            Summon => "summon",
            Fortress => "fortress",
            Prism => "prism",
            Necromancer => "necromancer",
        }
    }
}

pub static TOWER_DEFS: &[TowerDef] = &[
    // ---- attack ----
    TowerDef {
        kind: TowerKind::Arrow,
        name: "箭塔",
        icon: "🏹",
        color: hex(0xe74c3c),
        cost: 50,
        damage: 18.0,
        range: 120.0,
        cooldown_ms: 350.0,
        behavior: Behavior::Single,
        desc: "快速单体物理伤害",
        ..base()
    },
    TowerDef {
        kind: TowerKind::Cannon,
        name: "炮塔",
        icon: "💣",
        color: hex(0xe67e22),
        cost: 120,
        damage: 45.0,
        range: 100.0,
        cooldown_ms: 1100.0,
        behavior: Behavior::Aoe,
        aoe_radius: 70.0,
        armor: 8.0,
        desc: "范围爆炸物理伤害",
        ..base()
    },
    TowerDef {
        kind: TowerKind::Magic,
        name: "魔法塔",
        icon: "🔮",
        color: hex(0x9b59b6),
        cost: 200,
        damage: 70.0,
        range: 140.0,
        cooldown_ms: 750.0,
        behavior: Behavior::Single,
        magic: true,
        element: Element::Arcane,
        desc: "高单体魔法伤害",
        ..base()
    },
    TowerDef {
        kind: TowerKind::Sniper,
        name: "狙击塔",
        icon: "🎯",
        color: hex(0x2ecc71),
        cost: 180,
        damage: 95.0,
        range: 220.0,
        cooldown_ms: 1400.0,
        behavior: Behavior::Single,
        desc: "超远射程物理伤害",
        ..base()
    },
    TowerDef {
        kind: TowerKind::Thunder,
        name: "雷塔",
        icon: "⚡",
        color: hex(0xf1c40f),
        cost: 220,
        damage: 35.0,
        range: 130.0,
        cooldown_ms: 900.0,
        behavior: Behavior::Chain,
        chain_count: 4,
        chain_range: 100.0,
        magic: true,
        element: Element::Storm,
        desc: "命中后弹射多个敌人",
        ..base()
    },
    TowerDef {
        kind: TowerKind::Laser,
        name: "激光塔",
        icon: "🔦",
        color: hex(0xe84393),
        cost: 250,
        damage: 25.0,
        range: 160.0,
        cooldown_ms: 100.0,
        behavior: Behavior::Laser,
        magic: true,
        element: Element::Arcane,
        desc: "持续穿透直线伤害",
        ..base()
    },
    TowerDef {
        kind: TowerKind::Missile,
        name: "导弹塔",
        icon: "🚀",
        color: hex(0xd35400),
        cost: 300,
        damage: 220.0,
        range: 200.0,
        cooldown_ms: 2000.0,
        behavior: Behavior::Homing,
        aoe_radius: 75.0,
        footprint: 2, // 2×2 heavy emplacement
        max_hp: 260.0,
        armor: 14.0,
        desc: "2×2 追踪导弹·高伤范围",
        ..base()
    },
    // ---- control ----
    TowerDef {
        kind: TowerKind::Ice,
        name: "冰塔",
        icon: "❄️",
        color: hex(0x3498db),
        cost: 100,
        damage: 10.0,
        range: 100.0,
        cooldown_ms: 600.0,
        category: Category::Control,
        behavior: Behavior::Slow,
        element: Element::Frost,
        slow_factor: 0.5,
        slow_duration: 2000.0,
        desc: "减速敌人",
        ..base()
    },
    TowerDef {
        kind: TowerKind::Wind,
        name: "风塔",
        icon: "🌪️",
        color: hex(0x00cec9),
        cost: 160,
        damage: 12.0,
        range: 90.0,
        cooldown_ms: 800.0,
        category: Category::Control,
        behavior: Behavior::Knockback,
        element: Element::Storm,
        knock_dist: 40.0,
        stun_duration: 400.0,
        desc: "击退并短暂眩晕",
        ..base()
    },
    TowerDef {
        kind: TowerKind::FrostNova,
        name: "冰霜新星",
        icon: "💥",
        color: hex(0x74b9ff),
        cost: 240,
        damage: 30.0,
        range: 110.0,
        cooldown_ms: 1500.0,
        category: Category::Control,
        behavior: Behavior::Freeze,
        element: Element::Frost,
        aoe_radius: 80.0,
        freeze_duration: 1500.0,
        desc: "范围冰冻敌人",
        ..base()
    },
    TowerDef {
        kind: TowerKind::Shadow,
        name: "暗影塔",
        icon: "🌑",
        color: hex(0x636e72),
        cost: 180,
        damage: 20.0,
        range: 120.0,
        cooldown_ms: 700.0,
        category: Category::Control,
        behavior: Behavior::Curse,
        element: Element::Shadow,
        armor_reduce: 8.0,
        curse_duration: 3000.0,
        desc: "降低敌人护甲/魔抗",
        ..base()
    },
    // ---- support ----
    TowerDef {
        kind: TowerKind::Holy,
        name: "圣光塔",
        icon: "✨",
        color: hex(0xfdcb6e),
        cost: 150,
        damage: 15.0,
        range: 130.0,
        cooldown_ms: 1200.0,
        category: Category::Support,
        behavior: Behavior::Heal,
        element: Element::Arcane,
        heal_amount: 1.0,
        buff_range: 120.0,
        desc: "治疗萝卜并增益周围塔",
        ..base()
    },
    TowerDef {
        kind: TowerKind::Detection,
        name: "侦测塔",
        icon: "👁️",
        color: hex(0xa29bfe),
        cost: 80,
        damage: 8.0,
        range: 150.0,
        cooldown_ms: 500.0,
        category: Category::Support,
        behavior: Behavior::Detect,
        element: Element::Arcane,
        desc: "使隐形敌人显形",
        ..base()
    },
    // ---- special ----
    TowerDef {
        kind: TowerKind::Poison,
        name: "毒塔",
        icon: "🧪",
        color: hex(0x6c5ce7),
        cost: 140,
        damage: 8.0,
        range: 110.0,
        cooldown_ms: 650.0,
        category: Category::Special,
        behavior: Behavior::Poison,
        element: Element::Toxic,
        dot_damage: 16.0,
        poison_duration: 4000.0,
        desc: "剧毒·持续掉血(可叠加时长)",
        ..base()
    },
    TowerDef {
        kind: TowerKind::Fire,
        name: "火塔",
        icon: "🔥",
        color: hex(0xe17055),
        cost: 170,
        damage: 15.0,
        range: 100.0,
        cooldown_ms: 900.0,
        category: Category::Special,
        behavior: Behavior::Fire,
        element: Element::Fire,
        aoe_radius: 60.0,
        dot_damage: 18.0,
        fire_duration: 3000.0,
        desc: "点燃范围敌人，并留下持续火场",
        ..base()
    },
    TowerDef {
        kind: TowerKind::Summon,
        name: "召唤塔",
        icon: "🐺",
        color: hex(0xb2bec3),
        cost: 200,
        damage: 25.0,
        range: 100.0,
        cooldown_ms: 1500.0,
        category: Category::Special,
        behavior: Behavior::Summon,
        max_hp: 150.0,
        summon_hp: 120.0,
        summon_speed: 1.5,
        max_summons: 1,
        desc: "召唤狼魂阻挡敌人",
        ..base()
    },
    // ---- large emplacements (multi-cell) ----
    TowerDef {
        kind: TowerKind::Fortress,
        name: "要塞炮",
        icon: "🏰",
        color: hex(0x8e6e3c),
        cost: 400,
        damage: 90.0,
        range: 150.0,
        cooldown_ms: 1300.0,
        category: Category::Attack,
        behavior: Behavior::Aoe,
        aoe_radius: 95.0,
        footprint: 2,
        max_hp: 360.0,
        armor: 24.0,
        desc: "2×2 重炮·超大范围爆炸",
        ..base()
    },
    TowerDef {
        kind: TowerKind::Prism,
        name: "光棱塔",
        icon: "🔆",
        color: hex(0x00d2ff),
        cost: 650,
        damage: 55.0,
        range: 210.0,
        cooldown_ms: 100.0,
        category: Category::Special,
        behavior: Behavior::Laser,
        magic: true,
        element: Element::Arcane,
        footprint: 3,
        max_hp: 520.0,
        armor: 18.0,
        desc: "3×3 持续高能激光",
        ..base()
    },
    TowerDef {
        kind: TowerKind::Necromancer,
        name: "死灵塔",
        icon: "💀",
        color: hex(0x4b3b5a),
        cost: 260,
        damage: 0.0,
        range: 130.0,
        cooldown_ms: 3500.0, // raise interval
        category: Category::Special,
        behavior: Behavior::Necromancer,
        element: Element::Shadow,
        desc: "击杀范围内敌人→复活为我方作战",
        ..base()
    },
];

/// Upgrade multipliers (original `UPGRADE_MULTIPLIERS`).
pub struct UpgradeMul;
#[allow(dead_code)]
impl UpgradeMul {
    pub const DAMAGE: f32 = 1.5;
    pub const RANGE: f32 = 1.12;
    pub const COOLDOWN: f32 = 0.85; // original `speed` (lower = faster)
    pub const COST: f32 = 0.7;
    pub const AOE_RADIUS: f32 = 1.15;
    pub const DOT_DAMAGE: f32 = 1.5;
    pub const HEAL_AMOUNT: f32 = 1.5;
    pub const SUMMON_HP: f32 = 1.5;
}

// ============================ Enemies ============================

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum EnemyKind {
    Normal,
    Fast,
    Tank,
    Flying,
    Invisible,
    Regenerating,
    Armored,
    Swarmer,
    Boss,
    Shielded,
    Splitter,
    Healer,
    Charger,
    Climber,
    Silencer,
    Ranged,
    Exploder,
    Moss,
}

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub struct EnemyDef {
    pub kind: EnemyKind,
    pub name: &'static str,
    pub color: Color,
    pub size: f32,
    pub hp_mod: f32,
    pub speed_mod: f32,
    pub reward_mod: f32,
    pub armor: f32,
    pub magic_resist: f32,
    pub resist: ElementProfile,
    pub flying: bool,
    pub invisible: bool,
    pub regen: f32,
    pub boss: bool,
    /// Base shield pool (absorbs damage before hp; scaled by wave like hp).
    pub shield: f32,
    /// Number of small enemies spawned on death (0 = none).
    pub splits: i32,
    /// HP per second this enemy heals to nearby allies (0 = none).
    pub heal_aura: f32,
    /// Periodically bursts forward at higher speed.
    pub charger: bool,
    /// Leaves the path briefly to chew on nearby defensive towers.
    pub tower_raider: bool,
    pub tower_dps: f32,
    /// Radius that disables tower attacks while this enemy is nearby.
    pub silence_aura: f32,
    /// Attacks defensive towers from the path without leaving formation.
    pub ranged_tower: bool,
    pub ranged_range: f32,
    pub ranged_damage: f32,
    pub ranged_cooldown: f32,
    /// Active self-detonation: leaves the path to approach towers/heroes, then
    /// explodes while alive. Death by tower fire does not trigger this.
    pub explosive: bool,
    pub explode_damage: f32,
    pub explode_radius: f32,
    pub explode_sense: f32,
    pub explode_trigger: f32,
    /// One-shot boss skill: obliterates the first tower it reaches.
    pub moss_destroy: bool,
    /// 孵化：存活超过固定时间会变强（普通/中级周期性强化，高级直接孵化为本关
    /// boss）。详见 enemy.rs 的 incubation 系统。
    pub incubate: bool,
}

const fn enemy_base() -> EnemyDef {
    EnemyDef {
        kind: EnemyKind::Normal,
        name: "",
        color: hex(0xffffff),
        size: 10.0,
        hp_mod: 1.0,
        speed_mod: 1.0,
        reward_mod: 1.0,
        armor: 0.0,
        magic_resist: 0.0,
        resist: ElementProfile::none(),
        flying: false,
        invisible: false,
        regen: 0.0,
        boss: false,
        shield: 0.0,
        splits: 0,
        heal_aura: 0.0,
        charger: false,
        tower_raider: false,
        tower_dps: 0.0,
        silence_aura: 0.0,
        ranged_tower: false,
        ranged_range: 0.0,
        ranged_damage: 0.0,
        ranged_cooldown: 1.5,
        explosive: false,
        explode_damage: 0.0,
        explode_radius: 0.0,
        explode_sense: 0.0,
        explode_trigger: 0.0,
        moss_destroy: false,
        incubate: false,
    }
}

impl EnemyKind {
    pub const ALL: [EnemyKind; 18] = [
        EnemyKind::Normal,
        EnemyKind::Fast,
        EnemyKind::Tank,
        EnemyKind::Flying,
        EnemyKind::Invisible,
        EnemyKind::Regenerating,
        EnemyKind::Armored,
        EnemyKind::Swarmer,
        EnemyKind::Boss,
        EnemyKind::Shielded,
        EnemyKind::Splitter,
        EnemyKind::Healer,
        EnemyKind::Charger,
        EnemyKind::Climber,
        EnemyKind::Silencer,
        EnemyKind::Ranged,
        EnemyKind::Exploder,
        EnemyKind::Moss,
    ];

    pub fn def(self) -> &'static EnemyDef {
        ENEMY_DEFS.iter().find(|d| d.kind == self).unwrap()
    }

    /// Sprite file stem under `assets/sprites/enemies/`.
    pub fn sprite_name(self) -> &'static str {
        use EnemyKind::*;
        match self {
            Normal => "normal",
            Fast => "fast",
            Tank => "tank",
            Flying => "flying",
            Invisible => "invisible",
            Regenerating => "regenerating",
            Armored => "armored",
            Swarmer => "swarmer",
            Boss => "boss",
            Shielded => "shielded",
            Splitter => "splitter",
            Healer => "healer",
            Charger => "charger",
            Climber => "climber",
            Silencer => "silencer",
            // Reuse the existing occult caster sprite until a dedicated ranged
            // monster sheet exists.
            Ranged => "silencer",
            // Reuse the fireworm sheet for the active self-detonator.
            Exploder => "charger",
            Moss => "moss",
        }
    }
}

pub static ENEMY_DEFS: &[EnemyDef] = &[
    EnemyDef {
        kind: EnemyKind::Normal,
        name: "普通怪",
        color: hex(0xe74c3c),
        size: 10.0,
        ..enemy_base()
    },
    EnemyDef {
        kind: EnemyKind::Fast,
        name: "快速怪",
        color: hex(0xf39c12),
        size: 8.0,
        hp_mod: 0.6,
        speed_mod: 1.6,
        reward_mod: 1.3,
        resist: ElementProfile::new(-0.10, 0.0, 0.0, 0.25, -0.10, 0.0, 0.0),
        ..enemy_base()
    },
    EnemyDef {
        kind: EnemyKind::Tank,
        name: "坦克怪",
        color: hex(0x8e44ad),
        size: 14.0,
        hp_mod: 2.8,
        speed_mod: 0.6,
        reward_mod: 2.0,
        armor: 18.0,
        magic_resist: 8.0,
        resist: ElementProfile::new(0.18, -0.10, 0.05, 0.05, -0.15, 0.0, 0.0),
        ..enemy_base()
    },
    EnemyDef {
        kind: EnemyKind::Flying,
        name: "飞行怪",
        color: hex(0x3498db),
        size: 9.0,
        hp_mod: 0.8,
        speed_mod: 1.4,
        reward_mod: 1.5,
        flying: true,
        resist: ElementProfile::new(0.0, 0.0, 0.0, -0.10, 0.25, 0.0, 0.0),
        ..enemy_base()
    },
    EnemyDef {
        kind: EnemyKind::Invisible,
        name: "隐形怪",
        color: hex(0x95a5a6),
        size: 9.0,
        hp_mod: 0.7,
        speed_mod: 1.3,
        reward_mod: 1.6,
        invisible: true,
        resist: ElementProfile::new(0.0, -0.10, 0.0, 0.0, 0.0, 0.30, 0.0),
        ..enemy_base()
    },
    EnemyDef {
        kind: EnemyKind::Regenerating,
        name: "再生怪",
        color: hex(0x2ecc71),
        size: 11.0,
        hp_mod: 1.3,
        speed_mod: 1.0,
        reward_mod: 1.4,
        armor: 8.0,
        magic_resist: 6.0,
        regen: 0.003,
        resist: ElementProfile::new(0.0, 0.0, -0.20, 0.0, 0.0, 0.0, 0.35),
        ..enemy_base()
    },
    EnemyDef {
        kind: EnemyKind::Armored,
        name: "重甲怪",
        color: hex(0x7f8c8d),
        size: 12.0,
        hp_mod: 1.5,
        speed_mod: 0.8,
        reward_mod: 1.6,
        armor: 40.0,
        resist: ElementProfile::new(0.30, -0.15, 0.10, 0.0, -0.20, 0.0, 0.0),
        ..enemy_base()
    },
    EnemyDef {
        kind: EnemyKind::Swarmer,
        name: "集群怪",
        color: hex(0xe67e22),
        size: 7.0,
        hp_mod: 0.35,
        speed_mod: 1.2,
        reward_mod: 0.6,
        resist: ElementProfile::new(-0.15, 0.0, 0.0, 0.0, 0.0, 0.0, -0.10),
        // 孵化：虫群是“卵/幼体”，放着不杀会越长越壮，高级虫群更会孵化成首领。
        incubate: true,
        ..enemy_base()
    },
    EnemyDef {
        kind: EnemyKind::Boss,
        name: "旧日巨像",
        color: hex(0xc0392b),
        size: 20.0,
        hp_mod: 8.0,
        speed_mod: 0.5,
        reward_mod: 10.0,
        armor: 45.0,
        magic_resist: 30.0,
        boss: true,
        resist: ElementProfile::new(0.25, 0.18, 0.10, 0.10, 0.10, 0.25, 0.15),
        ..enemy_base()
    },
    EnemyDef {
        kind: EnemyKind::Shielded,
        name: "护盾怪",
        color: hex(0x5dade2),
        size: 11.0,
        hp_mod: 1.0,
        speed_mod: 1.0,
        reward_mod: 1.7,
        shield: 50.0,
        resist: ElementProfile::new(0.12, 0.0, 0.0, 0.0, -0.20, 0.0, 0.0),
        ..enemy_base()
    },
    EnemyDef {
        kind: EnemyKind::Splitter,
        name: "分裂怪",
        color: hex(0xaf7ac5),
        size: 12.0,
        hp_mod: 1.4,
        speed_mod: 0.9,
        reward_mod: 1.8,
        splits: 3,
        resist: ElementProfile::new(0.0, 0.0, -0.15, 0.0, 0.0, 0.10, 0.0),
        ..enemy_base()
    },
    EnemyDef {
        kind: EnemyKind::Healer,
        name: "治疗怪",
        color: hex(0x58d68d),
        size: 10.0,
        hp_mod: 1.0,
        speed_mod: 1.0,
        reward_mod: 1.9,
        heal_aura: 14.0,
        resist: ElementProfile::new(0.0, 0.12, -0.10, 0.0, 0.0, 0.18, 0.0),
        ..enemy_base()
    },
    EnemyDef {
        kind: EnemyKind::Charger,
        name: "冲锋怪",
        color: hex(0xf5b041),
        size: 10.0,
        hp_mod: 0.9,
        speed_mod: 1.1,
        reward_mod: 1.6,
        charger: true,
        resist: ElementProfile::new(0.0, 0.0, 0.20, -0.25, 0.0, 0.0, 0.0),
        tower_raider: true,
        tower_dps: 18.0,
        ..enemy_base()
    },
    EnemyDef {
        kind: EnemyKind::Climber,
        name: "爬墙怪",
        color: hex(0xb9770e),
        size: 10.0,
        hp_mod: 1.15,
        speed_mod: 1.15,
        reward_mod: 2.0,
        armor: 12.0,
        resist: ElementProfile::new(0.10, 0.0, 0.0, -0.10, 0.0, 0.0, 0.0),
        tower_raider: true,
        tower_dps: 34.0,
        ..enemy_base()
    },
    EnemyDef {
        kind: EnemyKind::Silencer,
        name: "静默怪",
        color: hex(0x6c5ce7),
        size: 10.0,
        hp_mod: 1.25,
        speed_mod: 0.95,
        reward_mod: 2.2,
        magic_resist: 20.0,
        resist: ElementProfile::new(0.0, 0.35, 0.0, 0.0, -0.20, 0.28, 0.0),
        silence_aura: 95.0,
        ..enemy_base()
    },
    EnemyDef {
        kind: EnemyKind::Ranged,
        name: "远射怪",
        color: hex(0x9b59b6),
        size: 10.0,
        hp_mod: 0.95,
        speed_mod: 0.92,
        reward_mod: 2.15,
        magic_resist: 14.0,
        resist: ElementProfile::new(0.0, 0.20, -0.10, 0.0, -0.10, 0.24, 0.0),
        ranged_tower: true,
        ranged_range: TILE_SIZE * 4.25,
        ranged_damage: 28.0,
        ranged_cooldown: 1.45,
        ..enemy_base()
    },
    EnemyDef {
        kind: EnemyKind::Exploder,
        name: "自爆怪",
        color: hex(0xff6a18),
        size: 9.0,
        hp_mod: 0.85,
        speed_mod: 1.18,
        reward_mod: 1.85,
        armor: 4.0,
        magic_resist: 4.0,
        resist: ElementProfile::new(-0.08, 0.0, 0.35, -0.25, 0.0, 0.0, -0.12),
        explosive: true,
        explode_damage: 82.0,
        explode_radius: TILE_SIZE * 1.15,
        explode_sense: TILE_SIZE * 3.2,
        explode_trigger: TILE_SIZE * 0.72,
        ..enemy_base()
    },
    EnemyDef {
        kind: EnemyKind::Moss,
        name: "MOSS·吞塔者",
        color: hex(0x145a32),
        size: 22.0,
        hp_mod: 10.0,
        speed_mod: 0.48,
        reward_mod: 13.0,
        armor: 38.0,
        magic_resist: 35.0,
        boss: true,
        resist: ElementProfile::new(0.20, 0.18, -0.10, 0.15, 0.10, 0.35, 0.30),
        tower_raider: true,
        tower_dps: 55.0,
        moss_destroy: true,
        ..enemy_base()
    },
];

/// Every Nth wave spawns a boss; each level's final wave is also a boss wave.
pub const BOSS_WAVE_INTERVAL: i32 = 5;

// ============================ Levels ============================

#[derive(Clone, Copy, Debug)]
pub struct EnemyBase {
    pub hp: f32,
    pub speed: f32,
    pub reward: f32,
    pub count: i32,
}

#[derive(Clone, Debug)]
pub struct Level {
    pub name: &'static str,
    pub gold: i32,
    pub lives: i32,
    pub waves: i32,
    /// Path waypoints in (col,row) grid coordinates.
    pub path: Vec<(i32, i32)>,
    pub enemies: EnemyBase,
    pub spawn_interval_ms: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct LevelTheme {
    pub backdrop: Color,
    pub buildable: Color,
    pub buildable_alt: Color,
    pub path: Color,
    pub path_edge: Color,
    pub accent: Color,
    pub seal: Color,
}

/// Visual mood for each campaign level. These are intentionally data-only so the
/// board renderer can make every stage feel like a different Lovecraftian front
/// without hardcoding level-specific branches.
pub const LEVEL_THEMES: [LevelTheme; 20] = [
    LevelTheme {
        backdrop: hex(0x071209),
        buildable: hex(0x27442a),
        buildable_alt: hex(0x315632),
        path: hex(0x665338),
        path_edge: hex(0x806941),
        accent: hex(0x7fe17a),
        seal: hex(0xff8c1a),
    },
    LevelTheme {
        backdrop: hex(0x0b1114),
        buildable: hex(0x26383a),
        buildable_alt: hex(0x31484a),
        path: hex(0x594230),
        path_edge: hex(0x735744),
        accent: hex(0x91d7c3),
        seal: hex(0xf6a24a),
    },
    LevelTheme {
        backdrop: hex(0x08110c),
        buildable: hex(0x1f3a2a),
        buildable_alt: hex(0x2a4a36),
        path: hex(0x493b30),
        path_edge: hex(0x62503c),
        accent: hex(0x66c778),
        seal: hex(0xe6a14f),
    },
    LevelTheme {
        backdrop: hex(0x07111b),
        buildable: hex(0x173748),
        buildable_alt: hex(0x20495a),
        path: hex(0x37475a),
        path_edge: hex(0x4b6380),
        accent: hex(0x5cc7ff),
        seal: hex(0xff9e45),
    },
    LevelTheme {
        backdrop: hex(0x110c0b),
        buildable: hex(0x332e26),
        buildable_alt: hex(0x40382e),
        path: hex(0x5c402c),
        path_edge: hex(0x7a5639),
        accent: hex(0xc98b4a),
        seal: hex(0xff8f32),
    },
    LevelTheme {
        backdrop: hex(0x151009),
        buildable: hex(0x4a3920),
        buildable_alt: hex(0x5b4728),
        path: hex(0x81613a),
        path_edge: hex(0xa27a45),
        accent: hex(0xffca62),
        seal: hex(0xff7e2e),
    },
    LevelTheme {
        backdrop: hex(0x08121a),
        buildable: hex(0x254153),
        buildable_alt: hex(0x31576b),
        path: hex(0x9bb1bd),
        path_edge: hex(0xc2d9e4),
        accent: hex(0xa7ecff),
        seal: hex(0xffa44f),
    },
    LevelTheme {
        backdrop: hex(0x160707),
        buildable: hex(0x3d1f1b),
        buildable_alt: hex(0x522820),
        path: hex(0x6f2d1c),
        path_edge: hex(0xa64724),
        accent: hex(0xff6230),
        seal: hex(0xffbd4b),
    },
    LevelTheme {
        backdrop: hex(0x08110d),
        buildable: hex(0x1d382f),
        buildable_alt: hex(0x25493e),
        path: hex(0x314638),
        path_edge: hex(0x4d6f55),
        accent: hex(0x72d572),
        seal: hex(0xd8a753),
    },
    LevelTheme {
        backdrop: hex(0x120c13),
        buildable: hex(0x372a36),
        buildable_alt: hex(0x463545),
        path: hex(0x554035),
        path_edge: hex(0x765a46),
        accent: hex(0xdb8cff),
        seal: hex(0xff9d3d),
    },
    LevelTheme {
        backdrop: hex(0x050712),
        buildable: hex(0x1b2540),
        buildable_alt: hex(0x26325a),
        path: hex(0x30334c),
        path_edge: hex(0x4c5272),
        accent: hex(0x83a8ff),
        seal: hex(0xffa54a),
    },
    LevelTheme {
        backdrop: hex(0x07121a),
        buildable: hex(0x20384a),
        buildable_alt: hex(0x2a4b61),
        path: hex(0x435366),
        path_edge: hex(0x617d9a),
        accent: hex(0x91e8ff),
        seal: hex(0xffaf5a),
    },
    LevelTheme {
        backdrop: hex(0x190808),
        buildable: hex(0x3f201f),
        buildable_alt: hex(0x512927),
        path: hex(0x7b351f),
        path_edge: hex(0xb64d28),
        accent: hex(0xff7a2f),
        seal: hex(0xffd05a),
    },
    LevelTheme {
        backdrop: hex(0x041019),
        buildable: hex(0x163142),
        buildable_alt: hex(0x1d4258),
        path: hex(0x253a53),
        path_edge: hex(0x375f7e),
        accent: hex(0x4cc8d8),
        seal: hex(0xff9a44),
    },
    LevelTheme {
        backdrop: hex(0x0f080c),
        buildable: hex(0x2f1f2a),
        buildable_alt: hex(0x442b3a),
        path: hex(0x493434),
        path_edge: hex(0x6e494d),
        accent: hex(0xd94f77),
        seal: hex(0xff9640),
    },
    LevelTheme {
        backdrop: hex(0x0c0b09),
        buildable: hex(0x302f28),
        buildable_alt: hex(0x423f35),
        path: hex(0x554b3a),
        path_edge: hex(0x77684d),
        accent: hex(0xc7b37a),
        seal: hex(0xffa13b),
    },
    LevelTheme {
        backdrop: hex(0x050b17),
        buildable: hex(0x1d2b45),
        buildable_alt: hex(0x283a5d),
        path: hex(0x35415a),
        path_edge: hex(0x52688b),
        accent: hex(0xffeb6b),
        seal: hex(0xff8f3a),
    },
    LevelTheme {
        backdrop: hex(0x050512),
        buildable: hex(0x1d1832),
        buildable_alt: hex(0x2a2248),
        path: hex(0x2b2944),
        path_edge: hex(0x494575),
        accent: hex(0xb58cff),
        seal: hex(0xff7f4a),
    },
    LevelTheme {
        backdrop: hex(0x07090f),
        buildable: hex(0x1d2733),
        buildable_alt: hex(0x293646),
        path: hex(0x393a3d),
        path_edge: hex(0x5a5b5e),
        accent: hex(0x9ba6b8),
        seal: hex(0xff8b35),
    },
    LevelTheme {
        backdrop: hex(0x030309),
        buildable: hex(0x171426),
        buildable_alt: hex(0x211c37),
        path: hex(0x241f2d),
        path_edge: hex(0x49344f),
        accent: hex(0xff4f6d),
        seal: hex(0xffbe4f),
    },
];

/// Prologue shown on the menu — Cthulhu-flavored reframing of the carrot premise.
pub const PROLOGUE: &str = "序章 · 最后的封印\n\
萝卜并非萝卜，而是诸神陨落前埋下的封印之种。\n\
群星归位，裂隙张开，古神的眷族正涌向人间——\n\
守住它，便守住了尚未被吞没的现实。";

/// One-line atmospheric lore per level (Cthulhu style), shown when a level loads.
pub const LEVEL_LORE: [&str; 20] = [
    "草原静得反常。土壤之下，有什么在数着你的心跳。",
    "小径蜿蜒如肠道，每一次转弯都更靠近那不可名状之物。",
    "森林里树木朝同一方向低头——朝向封印。",
    "湖面倒映的不是天空，而是一只缓缓睁开的眼。",
    "山谷回响着并非风的低语：它们饿了。",
    "黄沙之下封存着旧日支配者的呼吸，每一粒都在尖叫。",
    "雪不是白色，是无数苍白的手指从天而降。",
    "火山的脉动与你的脉搏同频——它在模仿你。",
    "沼气中漂浮着旧神祈祷文的残片，读它的人都疯了。",
    "传说龙曾在此封印更古老的东西。龙已不在。",
    "回廊悬于虚空，脚下是会回望你的深渊。",
    "水晶折射出你尚未发生的死亡，一千种。",
    "熔岩里游动着不该存在的几何体，棱角刺痛理智。",
    "海沟深处的灯火，是诱饵，也是邀请。",
    "荆棘以血为养料，而它今天格外口渴。",
    "守卫遗迹的不是石像，是被时间遗忘的祭品。",
    "雷暴拼写出一个名字，念出它便会被听见。",
    "裂隙在此最薄。透过它，群星正注视着你。",
    "堡垒是人类最后的妄想，而潮水已没过城墙。",
    "终焉将至。守住封印之种，或让现实随你一同沉没。",
];

/// All 20 levels (original `LEVELS`). Built on demand to keep `Vec` allocation out
/// of `const` context.
pub fn levels() -> Vec<Level> {
    fn p(pairs: &[(i32, i32)]) -> Vec<(i32, i32)> {
        pairs.to_vec()
    }
    let mut levels = vec![
        Level {
            name: "初入草原",
            gold: 180,
            lives: 10,
            waves: 5,
            path: p(&[
                (0, 2),
                (3, 2),
                (3, 5),
                (7, 5),
                (7, 2),
                (11, 2),
                (11, 7),
                (15, 7),
                (15, 4),
                (19, 4),
            ]),
            enemies: EnemyBase {
                hp: 60.0,
                speed: 1.2,
                reward: 8.0,
                count: 4,
            },
            spawn_interval_ms: 1200.0,
        },
        Level {
            name: "蜿蜒小径",
            gold: 200,
            lives: 10,
            waves: 6,
            path: p(&[
                (0, 1),
                (2, 1),
                (2, 4),
                (5, 4),
                (5, 1),
                (8, 1),
                (8, 6),
                (12, 6),
                (12, 3),
                (16, 3),
                (16, 7),
                (19, 7),
            ]),
            enemies: EnemyBase {
                hp: 80.0,
                speed: 1.3,
                reward: 9.0,
                count: 5,
            },
            spawn_interval_ms: 1100.0,
        },
        Level {
            name: "森林迷宫",
            gold: 220,
            lives: 10,
            waves: 7,
            path: p(&[
                (0, 3),
                (4, 3),
                (4, 1),
                (8, 1),
                (8, 5),
                (4, 5),
                (4, 9),
                (10, 9),
                (10, 5),
                (14, 5),
                (14, 1),
                (18, 1),
                (18, 7),
                (19, 7),
            ]),
            enemies: EnemyBase {
                hp: 100.0,
                speed: 1.4,
                reward: 10.0,
                count: 5,
            },
            spawn_interval_ms: 1000.0,
        },
        Level {
            name: "湖边防线",
            gold: 250,
            lives: 10,
            waves: 8,
            path: p(&[
                (0, 7),
                (3, 7),
                (3, 4),
                (6, 4),
                (6, 10),
                (10, 10),
                (10, 4),
                (14, 4),
                (14, 7),
                (17, 7),
                (17, 3),
                (19, 3),
            ]),
            enemies: EnemyBase {
                hp: 130.0,
                speed: 1.5,
                reward: 11.0,
                count: 6,
            },
            spawn_interval_ms: 950.0,
        },
        Level {
            name: "山谷伏击",
            gold: 280,
            lives: 10,
            waves: 8,
            path: p(&[
                (0, 1),
                (5, 1),
                (5, 5),
                (2, 5),
                (2, 9),
                (7, 9),
                (7, 3),
                (12, 3),
                (12, 8),
                (16, 8),
                (16, 4),
                (19, 4),
            ]),
            enemies: EnemyBase {
                hp: 160.0,
                speed: 1.6,
                reward: 12.0,
                count: 6,
            },
            spawn_interval_ms: 900.0,
        },
        Level {
            name: "沙漠风暴",
            gold: 300,
            lives: 10,
            waves: 9,
            path: p(&[
                (0, 5),
                (3, 5),
                (3, 2),
                (7, 2),
                (7, 8),
                (11, 8),
                (11, 3),
                (15, 3),
                (15, 6),
                (18, 6),
                (18, 1),
                (19, 1),
            ]),
            enemies: EnemyBase {
                hp: 200.0,
                speed: 1.8,
                reward: 13.0,
                count: 7,
            },
            spawn_interval_ms: 850.0,
        },
        Level {
            name: "雪域奇缘",
            gold: 320,
            lives: 10,
            waves: 10,
            path: p(&[
                (0, 3),
                (4, 3),
                (4, 7),
                (2, 7),
                (2, 11),
                (6, 11),
                (6, 5),
                (10, 5),
                (10, 9),
                (14, 9),
                (14, 4),
                (18, 4),
                (18, 7),
                (19, 7),
            ]),
            enemies: EnemyBase {
                hp: 250.0,
                speed: 1.7,
                reward: 14.0,
                count: 7,
            },
            spawn_interval_ms: 800.0,
        },
        Level {
            name: "火山危机",
            gold: 350,
            lives: 10,
            waves: 10,
            path: p(&[
                (0, 7),
                (3, 7),
                (3, 3),
                (7, 3),
                (7, 10),
                (11, 10),
                (11, 2),
                (15, 2),
                (15, 6),
                (18, 6),
                (18, 4),
                (19, 4),
            ]),
            enemies: EnemyBase {
                hp: 300.0,
                speed: 2.0,
                reward: 15.0,
                count: 8,
            },
            spawn_interval_ms: 750.0,
        },
        Level {
            name: "幽暗沼泽",
            gold: 380,
            lives: 10,
            waves: 11,
            path: p(&[
                (0, 1),
                (3, 1),
                (3, 5),
                (1, 5),
                (1, 9),
                (5, 9),
                (5, 3),
                (9, 3),
                (9, 7),
                (13, 7),
                (13, 2),
                (17, 2),
                (17, 6),
                (19, 6),
            ]),
            enemies: EnemyBase {
                hp: 360.0,
                speed: 2.1,
                reward: 16.0,
                count: 8,
            },
            spawn_interval_ms: 700.0,
        },
        Level {
            name: "龙之谷",
            gold: 420,
            lives: 10,
            waves: 12,
            path: p(&[
                (0, 7),
                (2, 7),
                (2, 3),
                (5, 3),
                (5, 10),
                (9, 10),
                (9, 2),
                (13, 2),
                (13, 8),
                (16, 8),
                (16, 4),
                (18, 4),
                (18, 7),
                (19, 7),
            ]),
            enemies: EnemyBase {
                hp: 450.0,
                speed: 2.3,
                reward: 18.0,
                count: 9,
            },
            spawn_interval_ms: 650.0,
        },
        Level {
            name: "天空回廊",
            gold: 450,
            lives: 10,
            waves: 12,
            path: p(&[
                (0, 3),
                (4, 3),
                (4, 1),
                (9, 1),
                (9, 6),
                (14, 6),
                (14, 2),
                (18, 2),
                (18, 10),
                (19, 10),
            ]),
            enemies: EnemyBase {
                hp: 520.0,
                speed: 2.4,
                reward: 19.0,
                count: 9,
            },
            spawn_interval_ms: 600.0,
        },
        Level {
            name: "水晶洞穴",
            gold: 480,
            lives: 10,
            waves: 13,
            path: p(&[
                (0, 10),
                (3, 10),
                (3, 4),
                (7, 4),
                (7, 8),
                (11, 8),
                (11, 2),
                (15, 2),
                (15, 6),
                (18, 6),
                (18, 3),
                (19, 3),
            ]),
            enemies: EnemyBase {
                hp: 600.0,
                speed: 2.5,
                reward: 20.0,
                count: 10,
            },
            spawn_interval_ms: 580.0,
        },
        Level {
            name: "熔岩地狱",
            gold: 520,
            lives: 10,
            waves: 14,
            path: p(&[
                (0, 1),
                (4, 1),
                (4, 4),
                (2, 4),
                (2, 8),
                (6, 8),
                (6, 3),
                (10, 3),
                (10, 9),
                (14, 9),
                (14, 5),
                (18, 5),
                (18, 7),
                (19, 7),
            ]),
            enemies: EnemyBase {
                hp: 700.0,
                speed: 2.6,
                reward: 22.0,
                count: 10,
            },
            spawn_interval_ms: 560.0,
        },
        Level {
            name: "深海迷航",
            gold: 560,
            lives: 10,
            waves: 14,
            path: p(&[
                (0, 5),
                (3, 5),
                (3, 2),
                (7, 2),
                (7, 9),
                (11, 9),
                (11, 4),
                (15, 4),
                (15, 8),
                (18, 8),
                (18, 6),
                (19, 6),
            ]),
            enemies: EnemyBase {
                hp: 800.0,
                speed: 2.7,
                reward: 24.0,
                count: 11,
            },
            spawn_interval_ms: 540.0,
        },
        Level {
            name: "荆棘丛林",
            gold: 600,
            lives: 10,
            waves: 15,
            path: p(&[
                (0, 7),
                (2, 7),
                (2, 2),
                (6, 2),
                (6, 6),
                (10, 6),
                (10, 1),
                (14, 1),
                (14, 9),
                (18, 9),
                (18, 4),
                (19, 4),
            ]),
            enemies: EnemyBase {
                hp: 920.0,
                speed: 2.8,
                reward: 26.0,
                count: 11,
            },
            spawn_interval_ms: 520.0,
        },
        Level {
            name: "遗迹守卫",
            gold: 650,
            lives: 10,
            waves: 15,
            path: p(&[
                (0, 3),
                (5, 3),
                (5, 1),
                (9, 1),
                (9, 7),
                (5, 7),
                (5, 11),
                (12, 11),
                (12, 5),
                (16, 5),
                (16, 9),
                (19, 9),
            ]),
            enemies: EnemyBase {
                hp: 1050.0,
                speed: 2.9,
                reward: 28.0,
                count: 12,
            },
            spawn_interval_ms: 500.0,
        },
        Level {
            name: "风暴之巅",
            gold: 700,
            lives: 10,
            waves: 16,
            path: p(&[
                (0, 10),
                (4, 10),
                (4, 4),
                (8, 4),
                (8, 8),
                (12, 8),
                (12, 2),
                (16, 2),
                (16, 6),
                (19, 6),
            ]),
            enemies: EnemyBase {
                hp: 1200.0,
                speed: 3.0,
                reward: 30.0,
                count: 12,
            },
            spawn_interval_ms: 480.0,
        },
        Level {
            name: "虚空裂隙",
            gold: 760,
            lives: 10,
            waves: 17,
            path: p(&[
                (0, 1),
                (3, 1),
                (3, 7),
                (1, 7),
                (1, 12),
                (6, 12),
                (6, 4),
                (11, 4),
                (11, 9),
                (16, 9),
                (16, 3),
                (19, 3),
            ]),
            enemies: EnemyBase {
                hp: 1400.0,
                speed: 3.1,
                reward: 33.0,
                count: 13,
            },
            spawn_interval_ms: 460.0,
        },
        Level {
            name: "末日堡垒",
            gold: 850,
            lives: 10,
            waves: 18,
            path: p(&[
                (0, 7),
                (3, 7),
                (3, 3),
                (7, 3),
                (7, 10),
                (11, 10),
                (11, 2),
                (15, 2),
                (15, 8),
                (18, 8),
                (18, 5),
                (19, 5),
            ]),
            enemies: EnemyBase {
                hp: 1700.0,
                speed: 3.3,
                reward: 36.0,
                count: 13,
            },
            spawn_interval_ms: 440.0,
        },
        Level {
            name: "萝卜保卫战",
            gold: 1000,
            lives: 10,
            waves: 20,
            path: p(&[
                (0, 7),
                (2, 7),
                (2, 3),
                (5, 3),
                (5, 10),
                (9, 10),
                (9, 2),
                (13, 2),
                (13, 8),
                (16, 8),
                (16, 4),
                (18, 4),
                (18, 7),
                (19, 7),
            ]),
            enemies: EnemyBase {
                hp: 2200.0,
                speed: 3.5,
                reward: 40.0,
                count: 15,
            },
            spawn_interval_ms: 400.0,
        },
    ];
    // Baked per-level economy from the Layer-3 optimizer (`sim … opt`): the minimum
    // starting gold + kill reward at which the greedy player can clear each level,
    // replacing the original near-linear curve that left levels 10+ unwinnable
    // against geometric HP scaling. Re-baked after the tiered-skill enemy buffs
    // (split generations, flying short-cut, scaled regen/shield/armor): the curve
    // follows a descending target win-rate (0.95→0.57) so later levels stay tense
    // without being gold-starved; 11 & 15 are composition spikes already high.
    const ECON: [(i32, f32); 20] = [
        (180, 8.0),
        (200, 9.0),
        (220, 10.0),
        (250, 12.0),
        (280, 12.0),
        (300, 13.0),
        (320, 15.0),
        (364, 18.0),
        (437, 22.0),
        (546, 27.0),
        (800, 40.0),
        (923, 46.0),
        (1076, 54.0),
        (1100, 55.0),
        (1564, 78.0),
        (1444, 72.0),
        (1650, 83.0),
        (1925, 96.0),
        (2338, 117.0),
        (3025, 151.0),
    ];
    for (i, lvl) in levels.iter_mut().enumerate() {
        if let Some(&(gold, reward)) = ECON.get(i) {
            lvl.gold = gold;
            lvl.enemies.reward = reward;
        }
    }
    levels
}
