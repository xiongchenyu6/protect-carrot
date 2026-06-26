//! Equipment loot: enemies drop named relics. Equipment is
//! socketed into towers (up to 3 pieces) and can change stats, durability,
//! armor-piercing, and elemental damage.

use crate::data::{Element, hex};
use bevy::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
    Mythic,
}

impl Rarity {
    pub fn name(self) -> &'static str {
        match self {
            Rarity::Common => "普通",
            Rarity::Uncommon => "精良",
            Rarity::Rare => "稀有",
            Rarity::Epic => "史诗",
            Rarity::Legendary => "传说",
            Rarity::Mythic => "神话",
        }
    }

    /// Localized rarity name for display. Common's Chinese "普通" collides with the
    /// difficulty "普通" (Normal) in the flat dictionary, so override it to "Common"
    /// in English; everything else goes through the dictionary normally.
    pub fn label(self) -> String {
        if self == Rarity::Common && crate::i18n::current_lang() == crate::i18n::Lang::En {
            return "Common".to_string();
        }
        crate::i18n::t(self.name())
    }

    pub fn color(self) -> Color {
        match self {
            Rarity::Common => hex(0xb8c0aa),
            Rarity::Uncommon => hex(0x45d483),
            Rarity::Rare => hex(0x4aa3ff),
            Rarity::Epic => hex(0xb66dff),
            Rarity::Legendary => hex(0xffa93d),
            Rarity::Mythic => hex(0xff4f6d),
        }
    }

    fn tier(self) -> i32 {
        match self {
            Rarity::Common => 0,
            Rarity::Uncommon => 1,
            Rarity::Rare => 2,
            Rarity::Epic => 3,
            Rarity::Legendary => 4,
            Rarity::Mythic => 5,
        }
    }

    fn next(self) -> Option<Rarity> {
        match self {
            Rarity::Common => Some(Rarity::Uncommon),
            Rarity::Uncommon => Some(Rarity::Rare),
            Rarity::Rare => Some(Rarity::Epic),
            Rarity::Epic => Some(Rarity::Legendary),
            Rarity::Legendary => Some(Rarity::Mythic),
            Rarity::Mythic => None,
        }
    }
}

pub fn drop_source_hint(rarity: Rarity) -> &'static str {
    match rarity {
        Rarity::Common => "来源：普通怪常见掉落；通关宝箱基础奖励",
        Rarity::Uncommon => "来源：普通怪、精英怪、1印+通关宝箱",
        Rarity::Rare => "来源：普通怪、精英怪、首领、2印+通关宝箱",
        Rarity::Epic => "来源：10波后普通怪小概率、精英怪、首领、3印宝箱",
        Rarity::Legendary => "来源：12波后精英怪、首领、噩梦/后期3印宝箱",
        Rarity::Mythic => "来源：15波后首领或终局3印宝箱小概率掉落",
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Equipment {
    RustySight,
    CarrotSigil,
    BoneFletching,
    SaltpeterKeg,
    PrismShard,
    FrostLens,
    EmberCore,
    VenomVial,
    ThunderCoil,
    ShadowSeal,
    BulwarkPlate,
    ClockworkTrigger,
    WitchSalt,
    DeepOneScale,
    CultistManual,
    StarMetalBarrel,
    VoidCapacitor,
    SaintedGear,
    KrakenHeart,
    AzathothEye,
}

impl Equipment {
    pub const ALL: [Equipment; 20] = [
        Equipment::RustySight,
        Equipment::CarrotSigil,
        Equipment::BoneFletching,
        Equipment::SaltpeterKeg,
        Equipment::PrismShard,
        Equipment::FrostLens,
        Equipment::EmberCore,
        Equipment::VenomVial,
        Equipment::ThunderCoil,
        Equipment::ShadowSeal,
        Equipment::BulwarkPlate,
        Equipment::ClockworkTrigger,
        Equipment::WitchSalt,
        Equipment::DeepOneScale,
        Equipment::CultistManual,
        Equipment::StarMetalBarrel,
        Equipment::VoidCapacitor,
        Equipment::SaintedGear,
        Equipment::KrakenHeart,
        Equipment::AzathothEye,
    ];

    pub fn idx(self) -> usize {
        Self::ALL.iter().position(|e| *e == self).unwrap()
    }

    pub fn def(self) -> &'static EquipmentDef {
        EQUIPMENT_DEFS.iter().find(|d| d.item == self).unwrap()
    }

    pub fn short(self) -> &'static str {
        self.def().short
    }

    pub fn sprite_name(self) -> &'static str {
        match self {
            Equipment::RustySight => "rusty_sight",
            Equipment::CarrotSigil => "carrot_sigil",
            Equipment::BoneFletching => "bone_fletching",
            Equipment::SaltpeterKeg => "saltpeter_keg",
            Equipment::PrismShard => "prism_shard",
            Equipment::FrostLens => "frost_lens",
            Equipment::EmberCore => "ember_core",
            Equipment::VenomVial => "venom_vial",
            Equipment::ThunderCoil => "thunder_coil",
            Equipment::ShadowSeal => "shadow_seal",
            Equipment::BulwarkPlate => "bulwark_plate",
            Equipment::ClockworkTrigger => "clockwork_trigger",
            Equipment::WitchSalt => "witch_salt",
            Equipment::DeepOneScale => "deep_one_scale",
            Equipment::CultistManual => "cultist_manual",
            Equipment::StarMetalBarrel => "star_metal_barrel",
            Equipment::VoidCapacitor => "void_capacitor",
            Equipment::SaintedGear => "sainted_gear",
            Equipment::KrakenHeart => "kraken_heart",
            Equipment::AzathothEye => "azathoth_eye",
        }
    }

    pub fn visual(self) -> EquipmentVisual {
        match self {
            Equipment::RustySight => EquipmentVisual::Crosshair,
            Equipment::CarrotSigil => EquipmentVisual::WardSigil,
            Equipment::BoneFletching => EquipmentVisual::Feather,
            Equipment::SaltpeterKeg => EquipmentVisual::FuseSpark,
            Equipment::PrismShard => EquipmentVisual::Prism,
            Equipment::FrostLens => EquipmentVisual::FrostLens,
            Equipment::EmberCore => EquipmentVisual::EmberCore,
            Equipment::VenomVial => EquipmentVisual::VenomDrop,
            Equipment::ThunderCoil => EquipmentVisual::ThunderCoil,
            Equipment::ShadowSeal => EquipmentVisual::ShadowSeal,
            Equipment::BulwarkPlate => EquipmentVisual::BulwarkPlate,
            Equipment::ClockworkTrigger => EquipmentVisual::ClockworkGear,
            Equipment::WitchSalt => EquipmentVisual::SaltCrystal,
            Equipment::DeepOneScale => EquipmentVisual::DeepScale,
            Equipment::CultistManual => EquipmentVisual::ForbiddenTome,
            Equipment::StarMetalBarrel => EquipmentVisual::StarBarrel,
            Equipment::VoidCapacitor => EquipmentVisual::VoidCapacitor,
            Equipment::SaintedGear => EquipmentVisual::SaintedGear,
            Equipment::KrakenHeart => EquipmentVisual::KrakenHeart,
            Equipment::AzathothEye => EquipmentVisual::AzathothEye,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EquipmentVisual {
    Crosshair,
    WardSigil,
    Feather,
    FuseSpark,
    Prism,
    FrostLens,
    EmberCore,
    VenomDrop,
    ThunderCoil,
    ShadowSeal,
    BulwarkPlate,
    ClockworkGear,
    SaltCrystal,
    DeepScale,
    ForbiddenTome,
    StarBarrel,
    VoidCapacitor,
    SaintedGear,
    KrakenHeart,
    AzathothEye,
}

#[derive(Clone, Copy, Debug)]
pub struct EquipmentDef {
    pub item: Equipment,
    pub name: &'static str,
    pub short: &'static str,
    pub rarity: Rarity,
    pub element: Option<Element>,
    pub damage_mult: f32,
    pub range_mult: f32,
    pub cooldown_mult: f32,
    pub armor_pierce: f32,
    pub hp_mult: f32,
    pub armor_add: f32,
    pub desc: &'static str,
}

const fn base(
    item: Equipment,
    name: &'static str,
    short: &'static str,
    rarity: Rarity,
) -> EquipmentDef {
    EquipmentDef {
        item,
        name,
        short,
        rarity,
        element: None,
        damage_mult: 1.0,
        range_mult: 1.0,
        cooldown_mult: 1.0,
        armor_pierce: 0.0,
        hp_mult: 1.0,
        armor_add: 0.0,
        desc: "",
    }
}

pub static EQUIPMENT_DEFS: &[EquipmentDef] = &[
    EquipmentDef {
        damage_mult: 1.08,
        desc: "早期瞄具，稳定提升伤害。",
        ..base(Equipment::RustySight, "锈蚀准星", "准星", Rarity::Common)
    },
    EquipmentDef {
        hp_mult: 1.18,
        armor_add: 2.0,
        desc: "封印护符，让塔更耐打。",
        ..base(
            Equipment::CarrotSigil,
            "封印萝卜徽记",
            "徽记",
            Rarity::Common,
        )
    },
    EquipmentDef {
        damage_mult: 1.10,
        range_mult: 1.05,
        desc: "骨质尾羽，适合远程塔。",
        ..base(
            Equipment::BoneFletching,
            "食尸鬼骨羽",
            "骨羽",
            Rarity::Common,
        )
    },
    EquipmentDef {
        damage_mult: 1.15,
        element: Some(Element::Physical),
        armor_pierce: 4.0,
        desc: "黑火药桶，提升爆破与穿甲。",
        ..base(
            Equipment::SaltpeterKeg,
            "盐硝火药桶",
            "火药",
            Rarity::Uncommon,
        )
    },
    EquipmentDef {
        damage_mult: 1.10,
        element: Some(Element::Arcane),
        range_mult: 1.08,
        desc: "把攻击折射为秘法能量。",
        ..base(Equipment::PrismShard, "裂光棱晶", "棱晶", Rarity::Uncommon)
    },
    EquipmentDef {
        damage_mult: 1.08,
        element: Some(Element::Frost),
        cooldown_mult: 0.94,
        desc: "附加冰霜属性并略微加速。",
        ..base(Equipment::FrostLens, "寒雾透镜", "寒镜", Rarity::Uncommon)
    },
    EquipmentDef {
        damage_mult: 1.14,
        element: Some(Element::Fire),
        desc: "让攻击带上火焰属性。",
        ..base(Equipment::EmberCore, "余烬核心", "余烬", Rarity::Rare)
    },
    EquipmentDef {
        damage_mult: 1.12,
        element: Some(Element::Toxic),
        armor_pierce: 6.0,
        desc: "毒囊改造，克制再生与血肉怪。",
        ..base(Equipment::VenomVial, "深渊毒瓶", "毒瓶", Rarity::Rare)
    },
    EquipmentDef {
        damage_mult: 1.10,
        element: Some(Element::Storm),
        cooldown_mult: 0.88,
        desc: "雷风线圈，明显提升攻速。",
        ..base(Equipment::ThunderCoil, "雷鸣线圈", "雷圈", Rarity::Rare)
    },
    EquipmentDef {
        damage_mult: 1.16,
        element: Some(Element::Shadow),
        desc: "把弹道染成暗影，压制施法怪。",
        ..base(Equipment::ShadowSeal, "暗影蜡印", "蜡印", Rarity::Rare)
    },
    EquipmentDef {
        hp_mult: 1.45,
        armor_add: 8.0,
        desc: "厚重装甲，专防爬墙怪拆塔。",
        ..base(Equipment::BulwarkPlate, "堡垒装甲板", "装甲", Rarity::Epic)
    },
    EquipmentDef {
        cooldown_mult: 0.78,
        damage_mult: 1.06,
        desc: "危险但高效的机械扳机。",
        ..base(
            Equipment::ClockworkTrigger,
            "疯匠发条扳机",
            "扳机",
            Rarity::Epic,
        )
    },
    EquipmentDef {
        damage_mult: 1.18,
        armor_pierce: 12.0,
        desc: "驱邪盐晶，打穿重甲与护盾。",
        ..base(Equipment::WitchSalt, "女巫盐晶", "盐晶", Rarity::Epic)
    },
    EquipmentDef {
        hp_mult: 1.25,
        armor_add: 5.0,
        element: Some(Element::Frost),
        desc: "深潜者鳞片，提供冰冷护甲。",
        ..base(Equipment::DeepOneScale, "深潜者鳞片", "鳞片", Rarity::Epic)
    },
    EquipmentDef {
        damage_mult: 1.22,
        element: Some(Element::Shadow),
        range_mult: 1.10,
        desc: "邪教手册，扩大射程并转为暗影。",
        ..base(
            Equipment::CultistManual,
            "黄衣教团手册",
            "手册",
            Rarity::Legendary,
        )
    },
    EquipmentDef {
        damage_mult: 1.28,
        armor_pierce: 18.0,
        desc: "星金炮管，大幅提升穿透火力。",
        ..base(
            Equipment::StarMetalBarrel,
            "星金炮管",
            "星管",
            Rarity::Legendary,
        )
    },
    EquipmentDef {
        damage_mult: 1.20,
        cooldown_mult: 0.82,
        element: Some(Element::Arcane),
        desc: "虚空电容，让高频塔质变。",
        ..base(
            Equipment::VoidCapacitor,
            "虚空电容器",
            "电容",
            Rarity::Legendary,
        )
    },
    EquipmentDef {
        damage_mult: 1.12,
        hp_mult: 1.30,
        armor_add: 6.0,
        element: Some(Element::Arcane),
        desc: "圣齿轮，兼顾输出与生存。",
        ..base(
            Equipment::SaintedGear,
            "圣约齿轮",
            "圣轮",
            Rarity::Legendary,
        )
    },
    EquipmentDef {
        damage_mult: 1.35,
        hp_mult: 1.35,
        armor_add: 10.0,
        element: Some(Element::Toxic),
        desc: "克拉肯心脏，血肉与毒性一起增殖。",
        ..base(Equipment::KrakenHeart, "克拉肯心脏", "海心", Rarity::Mythic)
    },
    EquipmentDef {
        damage_mult: 1.42,
        range_mult: 1.18,
        cooldown_mult: 0.86,
        element: Some(Element::Arcane),
        armor_pierce: 16.0,
        desc: "阿撒托斯之眼，终局级全能遗物。",
        ..base(
            Equipment::AzathothEye,
            "阿撒托斯之眼",
            "神眼",
            Rarity::Mythic,
        )
    },
];

#[derive(Resource)]
pub struct EquipmentInventory {
    pub counts: [u32; 20],
}

impl Default for EquipmentInventory {
    fn default() -> Self {
        EquipmentInventory {
            counts: load_inventory_counts(),
        }
    }
}

impl EquipmentInventory {
    pub fn add(&mut self, item: Equipment) {
        self.counts[item.idx()] += 1;
        save_inventory_counts(&self.counts);
    }

    pub fn take(&mut self, item: Equipment) -> bool {
        let i = item.idx();
        if self.counts[i] == 0 {
            return false;
        }
        self.counts[i] -= 1;
        save_inventory_counts(&self.counts);
        true
    }

    fn consume_many(&mut self, item: Equipment, count: u32) -> bool {
        let i = item.idx();
        if self.counts[i] < count {
            return false;
        }
        self.counts[i] -= count;
        save_inventory_counts(&self.counts);
        true
    }

    pub fn total(&self) -> u32 {
        self.counts.iter().sum()
    }
}

pub fn refine_equipment(
    inv: &mut EquipmentInventory,
    rng: &mut crate::game::Rng,
    item: Equipment,
) -> Result<Equipment, &'static str> {
    let Some(next_rarity) = item.def().rarity.next() else {
        return Err("神话装备无法继续精炼");
    };
    if !inv.consume_many(item, 3) {
        return Err("需要3件同名装备");
    }
    let reward = pick_item_by_rarity(rng, next_rarity);
    inv.add(reward);
    Ok(reward)
}

fn encode_counts(counts: &[u32; 20]) -> String {
    counts
        .iter()
        .map(|count| count.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

fn decode_counts(raw: &str) -> [u32; 20] {
    let mut counts = [0; 20];
    for (slot, value) in counts.iter_mut().zip(
        raw.split(|c: char| c == ',' || c.is_ascii_whitespace())
            .filter(|value| !value.is_empty()),
    ) {
        *slot = value.parse().unwrap_or(0);
    }
    counts
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(inline_js = r#"
export function load_equipment_inventory() {
  try {
    return globalThis.localStorage?.getItem('protect_carrot_equipment') || '';
  } catch (_) {
    return '';
  }
}
export function save_equipment_inventory(value) {
  try {
    globalThis.localStorage?.setItem('protect_carrot_equipment', value);
  } catch (_) {}
}
"#)]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = load_equipment_inventory)]
    fn load_equipment_inventory_js() -> String;
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = save_equipment_inventory)]
    fn save_equipment_inventory_js(value: &str);
}

#[cfg(target_arch = "wasm32")]
fn load_inventory_counts() -> [u32; 20] {
    decode_counts(&load_equipment_inventory_js())
}

#[cfg(target_arch = "wasm32")]
fn save_inventory_counts(counts: &[u32; 20]) {
    save_equipment_inventory_js(&encode_counts(counts));
}

#[cfg(not(target_arch = "wasm32"))]
fn load_inventory_counts() -> [u32; 20] {
    std::fs::read_to_string("tmp/equipment_counts.txt")
        .map(|raw| decode_counts(&raw))
        .unwrap_or([0; 20])
}

#[cfg(not(target_arch = "wasm32"))]
fn save_inventory_counts(counts: &[u32; 20]) {
    let _ = std::fs::create_dir_all("tmp");
    let _ = std::fs::write("tmp/equipment_counts.txt", encode_counts(counts));
}

/// Per-item "best used on" hint, derived from the item's dominant stat so it stays
/// accurate if stats are tuned. Also nudges toward elemental resonance synergy.
pub fn recommend_text(d: &EquipmentDef) -> String {
    let mut tip = if d.range_mult >= 1.06 {
        crate::i18n::t("推荐：狙击塔 / 弓箭塔等远程高伤塔")
    } else if d.cooldown_mult <= 0.93 {
        crate::i18n::t("推荐：激光 / 闪电 / 连射等高频塔")
    } else if d.hp_mult >= 1.15 || d.armor_add >= 3.0 {
        crate::i18n::t("推荐：前线肉盾 / 堡垒塔（常被怪攻击的位置）")
    } else if d.armor_pierce >= 4.0 {
        crate::i18n::t("推荐：对重甲怪的主力塔（加农 / 火炮）")
    } else if let Some(el) = d.element {
        crate::i18n::tf(
            "推荐：{}系塔，强化其元素打击",
            &[&crate::i18n::t(el.name())],
        )
    } else {
        crate::i18n::t("推荐：通用，装在主输出塔上最划算")
    };
    if let Some(el) = d.element {
        tip.push_str(&crate::i18n::tf(
            "\n联动：搭配 2-3 件{}系装备触发『{}共鸣』全塔加成",
            &[&crate::i18n::t(el.name()), &crate::i18n::t(el.name())],
        ));
    } else {
        tip.push_str(&crate::i18n::t(
            "\n联动：凑齐 3 件 / 多件高阶遗物触发套装加成",
        ));
    }
    tip
}

#[derive(Clone, Copy, Debug)]
pub struct EquipmentSetBonus {
    pub damage_mult: f32,
    pub armor_add: f32,
    pub resonance_element: Option<Element>,
    pub resonance_count: usize,
    pub grade_tier: u8,
}

impl Default for EquipmentSetBonus {
    fn default() -> Self {
        Self {
            damage_mult: 1.0,
            armor_add: 0.0,
            resonance_element: None,
            resonance_count: 0,
            grade_tier: 0,
        }
    }
}

impl EquipmentSetBonus {
    pub fn active(self) -> bool {
        self.damage_mult > 1.001 || self.armor_add > 0.0
    }
}

/// Derived set bonuses reward deliberate equipment builds without adding another
/// permanent mutation path to tower stats.
pub fn equipment_set_bonus(slots: &[Option<Equipment>; 3]) -> EquipmentSetBonus {
    let equipped: Vec<Equipment> = slots.iter().flatten().copied().collect();
    if equipped.len() < 2 {
        return EquipmentSetBonus::default();
    }

    let mut bonus = EquipmentSetBonus::default();

    let resonance = Element::ALL
        .into_iter()
        .filter_map(|element| {
            let count = equipped
                .iter()
                .filter(|item| item.def().element == Some(element))
                .count();
            (count >= 2).then_some((element, count))
        })
        .max_by_key(|(_, count)| *count);
    if let Some((element, count)) = resonance {
        bonus.resonance_element = Some(element);
        bonus.resonance_count = count;
        if count >= 3 {
            bonus.damage_mult *= 1.18;
            bonus.armor_add += 3.0;
        } else {
            bonus.damage_mult *= 1.10;
            bonus.armor_add += 1.0;
        }
    }

    let high_grade = equipped
        .iter()
        .filter(|item| item.def().rarity.tier() >= Rarity::Epic.tier())
        .count();
    if equipped.len() >= 3 && high_grade >= 3 {
        bonus.grade_tier = 3;
        bonus.damage_mult *= 1.12;
        bonus.armor_add += 8.0;
    } else if high_grade >= 2 {
        bonus.grade_tier = 2;
        bonus.damage_mult *= 1.07;
        bonus.armor_add += 4.0;
    } else if equipped.len() >= 3 {
        bonus.grade_tier = 1;
        bonus.damage_mult *= 1.04;
        bonus.armor_add += 2.0;
    }

    bonus
}

pub fn equipment_set_bonus_summary(slots: &[Option<Equipment>; 3]) -> String {
    let bonus = equipment_set_bonus(slots);
    if !bonus.active() {
        return crate::i18n::t("共鸣：无");
    }

    let mut parts = Vec::new();
    if let Some(element) = bonus.resonance_element {
        let pct = if bonus.resonance_count >= 3 { 18 } else { 10 };
        parts.push(crate::i18n::tf(
            "{}共鸣+{}%",
            &[&crate::i18n::t(element.name()), &pct.to_string()],
        ));
    }
    match bonus.grade_tier {
        3 => parts.push(crate::i18n::t("禁忌三件+12%/护甲+8")),
        2 => parts.push(crate::i18n::t("高阶二件+7%/护甲+4")),
        1 => parts.push(crate::i18n::t("整备三件+4%/护甲+2")),
        _ => {}
    }
    parts.push(crate::i18n::tf(
        "总伤害×{} 护甲+{}",
        &[
            &format!("{:.2}", bonus.damage_mult),
            &format!("{:.0}", bonus.armor_add),
        ],
    ));
    crate::i18n::tf("共鸣：{}", &[&parts.join("  ")])
}

pub fn return_equipment_to_inventory(
    inv: &mut EquipmentInventory,
    tower: &crate::tower::Tower,
) -> usize {
    let mut returned = 0;
    for item in tower.equipment.iter().flatten() {
        inv.add(*item);
        returned += 1;
    }
    returned
}

pub fn unequip_all_to_inventory(
    inv: &mut EquipmentInventory,
    tower: &mut crate::tower::Tower,
) -> usize {
    let removed: Vec<Equipment> = tower.equipment.iter().flatten().copied().collect();
    if removed.is_empty() {
        return 0;
    }

    for item in &removed {
        inv.add(*item);
    }

    tower.equipment = [None, None, None];
    remove_equipment_effects(tower, &removed);
    recompute_tower_element(tower);
    removed.len()
}

pub fn unequip_slot_to_inventory(
    inv: &mut EquipmentInventory,
    tower: &mut crate::tower::Tower,
    slot: usize,
) -> Option<Equipment> {
    let item = tower.equipment.get_mut(slot)?.take()?;
    inv.add(item);
    remove_equipment_effects(tower, &[item]);
    recompute_tower_element(tower);
    Some(item)
}

fn remove_equipment_effects(tower: &mut crate::tower::Tower, removed: &[Equipment]) {
    let hp_frac = if tower.max_hp > 0.0 {
        (tower.hp / tower.max_hp).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let mut damage_mult = 1.0;
    let mut range_mult = 1.0;
    let mut cooldown_mult = 1.0;
    let mut hp_mult = 1.0;
    let mut armor_pierce = 0.0;
    let mut armor_add = 0.0;

    for item in removed {
        let d = item.def();
        damage_mult *= d.damage_mult;
        range_mult *= d.range_mult;
        cooldown_mult *= d.cooldown_mult;
        hp_mult *= d.hp_mult;
        armor_pierce += d.armor_pierce;
        armor_add += d.armor_add;
    }

    tower.base_damage = (tower.base_damage / damage_mult).max(1.0).floor();
    tower.damage = tower.base_damage;
    tower.range = (tower.range / range_mult).max(1.0).floor();
    tower.cooldown = (tower.cooldown / cooldown_mult).max(0.03);
    tower.armor_pierce = (tower.armor_pierce - armor_pierce).max(0.0);
    tower.armor = (tower.armor - armor_add).max(0.0);
    if hp_mult > 1.0 {
        tower.max_hp = (tower.max_hp / hp_mult).max(1.0).floor();
        tower.hp = (tower.max_hp * hp_frac).clamp(1.0, tower.max_hp);
    }
    if tower.dot_damage > 0.0 {
        tower.dot_damage = (tower.dot_damage / damage_mult.sqrt()).max(1.0).floor();
    }
    if tower.summon_hp > 0.0 {
        tower.summon_hp = (tower.summon_hp / hp_mult.max(1.0)).max(1.0).floor();
    }
}

fn recompute_tower_element(tower: &mut crate::tower::Tower) {
    let def = tower.kind.def();
    tower.element = def.element;
    tower.magic = def.magic;
    for item in tower.equipment.iter().flatten() {
        if let Some(element) = item.def().element {
            tower.element = element;
            tower.magic = element != Element::Physical;
        }
    }
}

pub fn apply_equipment_stats(tower: &mut crate::tower::Tower) {
    let equipped = tower
        .equipment
        .iter()
        .flatten()
        .copied()
        .collect::<Vec<_>>();
    for item in equipped {
        apply_item_stats(tower, item);
    }
}

pub fn equip_into(tower: &mut crate::tower::Tower, item: Equipment) -> bool {
    let Some(slot) = tower.equipment.iter_mut().find(|slot| slot.is_none()) else {
        return false;
    };
    *slot = Some(item);
    apply_item_stats(tower, item);
    true
}

fn apply_item_stats(tower: &mut crate::tower::Tower, item: Equipment) {
    let d = item.def();
    tower.base_damage = (tower.base_damage * d.damage_mult).ceil();
    tower.range = (tower.range * d.range_mult).ceil();
    tower.cooldown = (tower.cooldown * d.cooldown_mult).max(0.03);
    tower.armor_pierce += d.armor_pierce;
    tower.armor += d.armor_add;
    if d.hp_mult > 1.0 {
        let old = tower.max_hp;
        tower.max_hp = (tower.max_hp * d.hp_mult).ceil();
        tower.hp += tower.max_hp - old;
    }
    if tower.dot_damage > 0.0 {
        tower.dot_damage = (tower.dot_damage * d.damage_mult.sqrt()).ceil();
    }
    if tower.summon_hp > 0.0 {
        tower.summon_hp = (tower.summon_hp * d.hp_mult.max(1.0)).ceil();
    }
    if let Some(element) = d.element {
        tower.element = element;
        tower.magic = element != Element::Physical;
    }
}

pub fn roll_drop(
    rng: &mut crate::game::Rng,
    boss: bool,
    elite: bool,
    wave: i32,
) -> Option<Equipment> {
    let chance = if boss {
        1.0
    } else if elite {
        0.34
    } else {
        (0.08 + wave as f32 * 0.006).min(0.22)
    };
    if rng.frac() >= chance {
        return None;
    }

    let rarity = roll_rarity(rng.frac(), boss, elite, wave);
    Some(pick_item_by_rarity(rng, rarity))
}

pub fn roll_clear_rewards(
    rng: &mut crate::game::Rng,
    stars: u8,
    difficulty_bonus: i32,
    level_index: usize,
) -> Vec<Equipment> {
    let stars = stars.clamp(1, 3);
    let mut rewards = Vec::new();
    for slot in 0..stars {
        let rarity = roll_clear_rarity(rng.frac(), stars, difficulty_bonus, level_index, slot);
        rewards.push(pick_item_by_rarity(rng, rarity));
    }
    if difficulty_bonus >= 2 && stars >= 3 && level_index >= 10 && rng.frac() < 0.35 {
        rewards.push(pick_item_by_rarity(rng, Rarity::Epic));
    }
    rewards
}

fn pick_item_by_rarity(rng: &mut crate::game::Rng, rarity: Rarity) -> Equipment {
    let mut candidates: Vec<Equipment> = Equipment::ALL
        .into_iter()
        .filter(|item| item.def().rarity == rarity)
        .collect();
    if candidates.is_empty() {
        candidates = Equipment::ALL
            .into_iter()
            .filter(|item| item.def().rarity.tier() <= rarity.tier())
            .collect();
    }
    candidates[rng.range(candidates.len())]
}

fn roll_clear_rarity(
    p: f32,
    stars: u8,
    difficulty_bonus: i32,
    level_index: usize,
    slot: u8,
) -> Rarity {
    let depth_bonus = if level_index >= 15 {
        2
    } else if level_index >= 8 {
        1
    } else {
        0
    };
    let tier_bonus = stars as i32 - 1 + difficulty_bonus + depth_bonus + slot as i32 / 2;
    match tier_bonus {
        0 => {
            if p < 0.24 {
                Rarity::Uncommon
            } else {
                Rarity::Common
            }
        }
        1 => {
            if p < 0.12 {
                Rarity::Rare
            } else if p < 0.58 {
                Rarity::Uncommon
            } else {
                Rarity::Common
            }
        }
        2 => {
            if p < 0.08 {
                Rarity::Epic
            } else if p < 0.38 {
                Rarity::Rare
            } else {
                Rarity::Uncommon
            }
        }
        3 => {
            if p < 0.05 {
                Rarity::Legendary
            } else if p < 0.24 {
                Rarity::Epic
            } else if p < 0.68 {
                Rarity::Rare
            } else {
                Rarity::Uncommon
            }
        }
        4 => {
            if p < 0.03 {
                Rarity::Mythic
            } else if p < 0.16 {
                Rarity::Legendary
            } else if p < 0.44 {
                Rarity::Epic
            } else {
                Rarity::Rare
            }
        }
        _ => {
            if p < 0.07 {
                Rarity::Mythic
            } else if p < 0.28 {
                Rarity::Legendary
            } else if p < 0.62 {
                Rarity::Epic
            } else {
                Rarity::Rare
            }
        }
    }
}

fn roll_rarity(p: f32, boss: bool, elite: bool, wave: i32) -> Rarity {
    if boss {
        if wave >= 15 && p < 0.16 {
            Rarity::Mythic
        } else if p < 0.48 {
            Rarity::Legendary
        } else if p < 0.82 {
            Rarity::Epic
        } else {
            Rarity::Rare
        }
    } else if elite {
        if wave >= 12 && p < 0.06 {
            Rarity::Legendary
        } else if p < 0.28 {
            Rarity::Epic
        } else if p < 0.70 {
            Rarity::Rare
        } else {
            Rarity::Uncommon
        }
    } else if wave >= 10 && p < 0.04 {
        Rarity::Epic
    } else if p < 0.22 {
        Rarity::Rare
    } else if p < 0.62 {
        Rarity::Uncommon
    } else {
        Rarity::Common
    }
}
