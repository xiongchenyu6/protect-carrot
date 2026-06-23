//! All `bevy_ui` screens: the in-game HUD + build palette, the level-select menu,
//! and the win/lose overlays. UI here is screen-space `Node` trees; clicks drive
//! the same `Selection`/`RunState` the keyboard shortcuts do.

use crate::bestiary::{brief, Bestiary};
use crate::build::{repair_tower, upgrade_tower, upgrade_unlock_note, Selection};
use crate::components::Enemy;
use crate::data::{
    Behavior, Category, Element, Level, TowerDef, TowerKind, BOARD_W, BOSS_WAVE_INTERVAL, COLS,
    LEVEL_LORE, LEVEL_THEMES, PROLOGUE, ROWS, TILE_SIZE,
};
use crate::enemy::PendingBossCast;
use crate::equipment::{
    drop_source_hint, equipment_set_bonus, equipment_set_bonus_summary, refine_equipment,
    return_equipment_to_inventory, roll_clear_rewards, unequip_all_to_inventory,
    unequip_slot_to_inventory, Equipment, EquipmentInventory, EquipmentVisual,
};
use crate::game::{
    start_wave, toggle_auto_wave, CurrentLevel, Difficulty, GameDifficulty, GameMode, Rng, RunMode,
    RunState, KILL_COMBO_WINDOW,
};
use crate::hero::{Class, HeroLoadout, Race};
use crate::meta::{talent_cost, Abilities, Ability, Talents};
use crate::monster::{
    boss_skill, elite_affix_pool, is_boss_wave, species_by_id, BossSkill, MONSTER_SPECIES,
};
use crate::audio::AudioSettings;
use crate::i18n::{tr, Language};
use crate::quality::GraphicsQuality;
use crate::sprites::Sprites;
use crate::states::GameState;
use crate::tower::{BuffTower, Damage, Status, StatusKind};
use crate::Levels;
use bevy::audio::{PlaybackMode, Volume};
use bevy::prelude::*;
use bevy::window::{MonitorSelection, WindowMode};

/// Persistent progression (how many levels are unlocked).
#[derive(Resource)]
pub struct Progress {
    pub unlocked: usize,
    pub stars: [u8; 20],
}
impl Default for Progress {
    fn default() -> Self {
        Progress {
            unlocked: load_progress_unlocked().max(1),
            stars: load_progress_stars(),
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(inline_js = r#"
export function load_progress() {
  try {
    return globalThis.localStorage?.getItem('protect_carrot_unlocked') || '';
  } catch (_) {
    return '';
  }
}
export function save_progress(value) {
  try {
    globalThis.localStorage?.setItem('protect_carrot_unlocked', value);
  } catch (_) {}
}
export function load_progress_stars() {
  try {
    return globalThis.localStorage?.getItem('protect_carrot_stars') || '';
  } catch (_) {
    return '';
  }
}
export function save_progress_stars(value) {
  try {
    globalThis.localStorage?.setItem('protect_carrot_stars', value);
  } catch (_) {}
}
"#)]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = load_progress)]
    fn load_progress_js() -> String;
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = save_progress)]
    fn save_progress_js(value: &str);
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = load_progress_stars)]
    fn load_progress_stars_js() -> String;
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = save_progress_stars)]
    fn save_progress_stars_js(value: &str);
}

#[cfg(target_arch = "wasm32")]
fn load_progress_unlocked() -> usize {
    load_progress_js().trim().parse().unwrap_or(1)
}

#[cfg(target_arch = "wasm32")]
fn save_progress_unlocked(unlocked: usize) {
    save_progress_js(&unlocked.to_string());
}

#[cfg(target_arch = "wasm32")]
fn load_progress_stars() -> [u8; 20] {
    decode_stars(&load_progress_stars_js())
}

#[cfg(target_arch = "wasm32")]
fn save_progress_stars(stars: &[u8; 20]) {
    save_progress_stars_js(&encode_stars(stars));
}

#[cfg(not(target_arch = "wasm32"))]
fn load_progress_unlocked() -> usize {
    std::fs::read_to_string("tmp/progress_unlocked.txt")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(1)
}

#[cfg(not(target_arch = "wasm32"))]
fn save_progress_unlocked(unlocked: usize) {
    let _ = std::fs::create_dir_all("tmp");
    let _ = std::fs::write("tmp/progress_unlocked.txt", unlocked.to_string());
}

#[cfg(not(target_arch = "wasm32"))]
fn load_progress_stars() -> [u8; 20] {
    std::fs::read_to_string("tmp/progress_stars.txt")
        .map(|raw| decode_stars(&raw))
        .unwrap_or([0; 20])
}

#[cfg(not(target_arch = "wasm32"))]
fn save_progress_stars(stars: &[u8; 20]) {
    let _ = std::fs::create_dir_all("tmp");
    let _ = std::fs::write("tmp/progress_stars.txt", encode_stars(stars));
}

fn encode_stars(stars: &[u8; 20]) -> String {
    stars
        .iter()
        .map(|star| star.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

fn decode_stars(raw: &str) -> [u8; 20] {
    let mut stars = [0; 20];
    for (slot, value) in stars.iter_mut().zip(
        raw.split(|c: char| c == ',' || c.is_ascii_whitespace())
            .filter(|value| !value.is_empty()),
    ) {
        *slot = value.parse::<u8>().unwrap_or(0).min(3);
    }
    stars
}

/// Handle to the CJK-capable UI font. Bevy's built-in default font has no Chinese
/// glyphs, so we embed WenQuanYi Micro Hei (registered in `main`) and use it for
/// every `Text`.
#[derive(Resource)]
pub struct UiFont(pub Handle<Font>);

// ---- markers (public so `despawn_with::<_>` can target them from main) ----
#[derive(Component)]
pub struct HudRoot;
/// Full-screen red overlay that flashes when the carrot loses a life. `level` is
/// the current alpha (decays each frame); driven by [`update_screen_flash`].
#[derive(Component)]
pub struct ScreenFlash {
    pub level: f32,
}
/// Bottom touch-control bar shown only after touch input is detected (mobile).
#[derive(Component)]
pub struct MobileHudRoot;
/// Panel rows that are redundant once the mobile touch bar is shown (the bar hosts
/// wave/pause/speed + abilities), so they're hidden in touch mode to avoid clutter.
#[derive(Component)]
pub struct TouchHiddenRow;
/// The bottom loadout dock (hero + selected unit + equipment). Toggled open/closed
/// by [`HudPanels::dock_open`] so it doesn't permanently cover the board.
#[derive(Component)]
pub struct DockRoot;
/// The settings panel opened from the top-right gear. Toggled by
/// [`HudPanels::settings_open`].
#[derive(Component)]
pub struct SettingsRoot;
/// Floating top-right gear button that opens [`SettingsRoot`].
#[derive(Component)]
pub struct SettingsGear;
/// Which collapsible HUD panels are currently open. Both default closed so the
/// board is fully reachable; the player opens them on demand.
#[derive(Resource, Default)]
pub struct HudPanels {
    pub dock_open: bool,
    pub settings_open: bool,
}
#[derive(Component)]
pub struct MenuRoot;
/// The settings popup opened from the menu's top-right gear (画质/音量/语言/全屏).
#[derive(Component)]
pub struct MenuSettingsRoot;
/// Set true to force the menu to rebuild next frame (used after a language change
/// so every translated string re-renders).
#[derive(Resource, Default)]
pub struct MenuDirty(pub bool);
/// Menu label showing the current master-volume percentage.
#[derive(Component)]
pub struct VolumeLabel;
/// Menu label showing the current language.
#[derive(Component)]
pub struct LanguageLabel;
#[derive(Component)]
pub struct StoryRoot;
#[derive(Component)]
pub struct BriefingRoot;
#[derive(Component)]
pub struct OverlayRoot;
#[derive(Component)]
pub struct BestiaryRoot;
#[derive(Component)]
pub struct ArmoryRoot;
#[derive(Component)]
pub struct TowerArchiveRoot;
#[derive(Component)]
pub struct MilestonesRoot;
#[derive(Component)]
pub struct CampaignDossierRoot;
/// Menu label showing the currently selected difficulty.
#[derive(Component)]
pub struct DiffLabel;
/// Label (menu + in-game panel) showing the current graphics-quality tier.
#[derive(Component)]
pub struct QualityLabel;
/// Menu label showing the chosen hero race + class.
#[derive(Component)]
pub struct HeroLabel;
/// In-game label showing hero level, XP, talent points, and skill cooldown.
#[derive(Component)]
pub struct HeroInfoText;
/// Root of the on-screen movement joystick (touch only).
#[derive(Component)]
pub struct JoystickBase;
/// The draggable knob inside the joystick.
#[derive(Component)]
pub struct JoystickKnob;

/// Joystick geometry (window-logical px) and current normalized direction.
#[derive(Resource, Default)]
pub struct JoystickState {
    /// The touch id currently driving the stick, if any.
    pub touch: Option<u64>,
    /// Normalized direction (-1..1 each axis, world-space: +y up).
    pub dir: Vec2,
    /// Screen-space anchor where the floating joystick appeared (touch-down point).
    pub origin: Vec2,
}

/// Radius of the floating movement joystick (window-logical px).
pub const JOY_RADIUS: f32 = 64.0;

#[derive(Component)]
pub struct GoldText;
#[derive(Component)]
pub struct LivesText;
#[derive(Component)]
pub struct WaveText;
#[derive(Component)]
pub struct WaveIntelText;
#[derive(Component)]
pub struct SpeedText;
#[derive(Component)]
pub struct ComboMeterRoot;
#[derive(Component)]
pub struct ComboMeterFill;
#[derive(Component)]
pub struct ComboMeterText;
#[derive(Component)]
pub struct SelInfoText;
/// One number in the selected-unit icon stat-strip. Updated by `update_unit_stats`
/// so the figures live next to a glyph instead of inside a text wall (goal: 文字→图标).
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum UnitStat {
    Damage,
    Range,
    Armor,
    Speed,
    Dps,
}
/// Marks the text inside the 升级 (upgrade) dock button so it can show the live cost.
#[derive(Component)]
pub struct UpgradeBtnText;
/// The hover tooltip container + its text.
#[derive(Component)]
pub struct TooltipBox;
#[derive(Component)]
pub struct TooltipText;
/// Equipment inventory readout.
#[derive(Component)]
pub struct InvText;
#[derive(Component)]
pub struct EquipmentButtonText {
    item: Equipment,
}
#[derive(Component)]
pub struct EquipmentButtonIcon {
    item: Equipment,
}
#[derive(Component)]
pub struct EquippedSlotFrame {
    slot: usize,
}
#[derive(Component)]
pub struct EquippedSlotIcon {
    slot: usize,
}
#[derive(Component)]
pub struct EquippedSlotText {
    slot: usize,
}
/// Big always-visible feedback message (sits over the board, not in the panel).
#[derive(Component)]
pub struct BannerText;
#[derive(Component)]
pub struct BossBarRoot;
#[derive(Component)]
pub struct BossBarFill;
#[derive(Component)]
pub struct BossSkillFill;
#[derive(Component)]
pub struct BossBarText;
#[derive(Component)]
pub struct BossPortrait;

/// What a clickable button does.
#[derive(Component, Clone)]
pub enum UiAction {
    Build(TowerKind),
    StartWave,
    TogglePause,
    ToggleAutoWave,
    CycleSpeed,
    Upgrade,
    Repair,
    CycleTargetPriority,
    Unequip,
    UnequipSlot(usize),
    Sell,
    Equip(Equipment),
    Fullscreen,
    OpenBestiary,
    OpenArmory,
    OpenTowerArchive,
    OpenMilestones,
    OpenCampaignDossier,
    OpenHeroCodex,
    RefineEquipment(Equipment),
    SetDifficulty(Difficulty),
    TalentDamage,
    TalentRange,
    TalentSpeed,
    Cast(Ability),
    CycleQuality,
    CycleVolume,
    CycleLanguage,
    /// Open/close the menu settings popup (画质/音量/语言/全屏).
    ToggleMenuSettings,
    /// A non-interactive stat/icon that only exists to show a tooltip on hover/tap.
    /// The payload is the tooltip text itself.
    Info(&'static str),
    /// Show/hide the bottom loadout dock (hero + selected unit + equipment) so the
    /// board underneath is reachable for building.
    ToggleDock,
    /// Open/close the settings panel (gear icon, top-right).
    ToggleSettings,
    SummonHero,
    HeroTalent(usize),
    HeroSkill,
    ResetHeroTalents,
    /// Pick a specific hero class/race on the briefing selection screen.
    SelectHeroClass(Class),
    SelectHeroRace(Race),
    PlayLevel(usize),
    PlayEndless,
    BeginMission,
    Restart,
    NextLevel,
    ToMenu,
}

const UI_BG: Color = Color::srgb(0.045, 0.052, 0.047);
const UI_PANEL: Color = Color::srgba(0.075, 0.086, 0.078, 0.93);
const UI_PANEL_DARK: Color = Color::srgba(0.025, 0.032, 0.030, 0.94);
const UI_CARD: Color = Color::srgba(0.105, 0.116, 0.104, 0.90);
const UI_CARD_SOFT: Color = Color::srgba(0.13, 0.125, 0.096, 0.88);
const UI_ACCENT_GOLD: Color = Color::srgb(0.96, 0.72, 0.28);
const UI_ACCENT_TEAL: Color = Color::srgb(0.33, 0.82, 0.78);
const UI_ACCENT_RED: Color = Color::srgb(0.78, 0.22, 0.18);
const UI_TEXT: Color = Color::srgb(0.90, 0.92, 0.86);
const UI_TEXT_DIM: Color = Color::srgb(0.62, 0.68, 0.62);
const PANEL_BG: Color = UI_PANEL;
const BTN_BG: Color = Color::srgb(0.16, 0.23, 0.20);

fn difficulty_color(difficulty: Difficulty) -> Color {
    match difficulty {
        Difficulty::Easy => Color::srgb(0.2, 0.45, 0.25),
        Difficulty::Normal => Color::srgb(0.3, 0.4, 0.5),
        Difficulty::Hard => Color::srgb(0.5, 0.2, 0.2),
    }
}

/// Sprite key for a difficulty's icon (text → icon, name shown on hover tooltip).
fn difficulty_icon_key(difficulty: Difficulty) -> &'static str {
    match difficulty {
        Difficulty::Easy => "diff_easy",
        Difficulty::Normal => "diff_normal",
        Difficulty::Hard => "diff_hard",
    }
}

fn rating_label(stars: u8) -> &'static str {
    match stars {
        3 => "3印",
        2 => "2印",
        1 => "1印",
        _ => "未封印",
    }
}

fn victory_rating(lives: i32, start_lives: i32) -> u8 {
    if start_lives <= 1 || lives >= start_lives {
        3
    } else if lives * 2 >= start_lives {
        2
    } else {
        1
    }
}

fn settlement_summary(
    heading: &str,
    current: usize,
    levels: &Levels,
    run: &RunState,
    difficulty: Difficulty,
) -> String {
    let level_name = levels
        .0
        .get(current)
        .map(|level| level.name)
        .unwrap_or("未知关卡");
    if run.is_endless() {
        return crate::i18n::tf(
            "{}：无尽模式  战场：{}  难度：{}  坚持：第{}波\n击杀：{}  最高连杀：{}  剩余生命：{}  金币：{}",
            &[
                heading,
                &crate::i18n::t(level_name),
                &crate::i18n::t(difficulty.name()),
                &run.wave.max(0).to_string(),
                &run.kills.max(0).to_string(),
                &run.best_combo.max(0).to_string(),
                &run.lives.max(0).to_string(),
                &run.gold.max(0).to_string(),
            ],
        );
    }
    let total_waves = run.total_waves.max(0);
    let wave = run.wave.max(0).min(total_waves);
    crate::i18n::tf(
        "{}：{}  难度：{}  波次：{}/{}\n击杀：{}  最高连杀：{}  剩余生命：{}  金币：{}",
        &[
            heading,
            &crate::i18n::t(level_name),
            &crate::i18n::t(difficulty.name()),
            &wave.to_string(),
            &total_waves.to_string(),
            &run.kills.max(0).to_string(),
            &run.best_combo.max(0).to_string(),
            &run.lives.max(0).to_string(),
            &run.gold.max(0).to_string(),
        ],
    )
}

fn clear_reward_bonus(difficulty: Difficulty) -> i32 {
    match difficulty {
        Difficulty::Easy => 0,
        Difficulty::Normal => 1,
        Difficulty::Hard => 2,
    }
}

fn equipment_reward_summary(items: &[Equipment]) -> String {
    if items.is_empty() {
        return crate::i18n::t("无");
    }
    items
        .iter()
        .map(|item| {
            let d = item.def();
            crate::i18n::tf("{}·{}", &[&d.rarity.label(), &crate::i18n::t(d.name)])
        })
        .collect::<Vec<_>>()
        .join("、")
}

struct RewardCard {
    image: Handle<Image>,
    color: Color,
    title: String,
    subtitle: String,
}

#[derive(Resource, Default)]
pub struct StoryTimeline {
    start: f32,
    played_mask: u8,
}

#[derive(Component)]
pub struct StoryImageMotion {
    from_left: f32,
    to_left: f32,
    from_bottom: f32,
    to_bottom: f32,
    delay: f32,
    duration: f32,
    tint: Color,
    alpha: f32,
    float_amp: f32,
    float_speed: f32,
    /// If set, this portrait dims when another character is speaking (VN highlight).
    speaker: Option<Speaker>,
}

#[derive(Component)]
pub struct StoryBackdrop {
    delay: f32,
    duration: f32,
    alpha: f32,
}

#[derive(Resource, Default)]
pub struct BriefingTimeline {
    start: f32,
}

#[derive(Component)]
pub struct BriefingTextFade {
    delay: f32,
    duration: f32,
    color: Color,
    alpha: f32,
}

#[derive(Component)]
pub struct BriefingPanelFade {
    delay: f32,
    duration: f32,
    color: Color,
    alpha: f32,
}

#[derive(Component)]
pub struct BriefingSweep {
    base_left: f32,
    span: f32,
    speed: f32,
    width: f32,
    color: Color,
    alpha: f32,
}

#[derive(Component)]
pub struct BriefingMeter {
    delay: f32,
    duration: f32,
}

struct MilestoneRow {
    category: &'static str,
    title: &'static str,
    detail: String,
    current: u32,
    target: u32,
    color: Color,
}

impl MilestoneRow {
    fn fraction(&self) -> f32 {
        if self.target == 0 {
            1.0
        } else {
            self.current as f32 / self.target as f32
        }
    }

    fn complete(&self) -> bool {
        self.current >= self.target
    }
}

fn milestone(
    rows: &mut Vec<MilestoneRow>,
    category: &'static str,
    title: &'static str,
    detail: impl Into<String>,
    current: u32,
    target: u32,
    color: Color,
) {
    rows.push(MilestoneRow {
        category,
        title,
        detail: detail.into(),
        current,
        target: target.max(1),
        color,
    });
}

fn milestone_rows(
    levels: &Levels,
    progress: &Progress,
    inv: &EquipmentInventory,
    bestiary: &Bestiary,
) -> Vec<MilestoneRow> {
    let level_count = levels.0.len() as u32;
    let completed_levels = progress
        .stars
        .iter()
        .take(levels.0.len())
        .filter(|stars| **stars > 0)
        .count() as u32;
    let perfect_levels = progress
        .stars
        .iter()
        .take(levels.0.len())
        .filter(|stars| **stars >= 3)
        .count() as u32;
    let total_rating: u32 = progress
        .stars
        .iter()
        .take(levels.0.len())
        .map(|stars| *stars as u32)
        .sum();
    let discovered = MONSTER_SPECIES
        .iter()
        .filter(|species| bestiary.count(species.id) > 0)
        .count() as u32;
    let total_kills: u32 = MONSTER_SPECIES
        .iter()
        .map(|species| bestiary.count(species.id))
        .sum();
    let boss_kills: u32 = MONSTER_SPECIES
        .iter()
        .filter(|species| species.is_boss())
        .map(|species| bestiary.count(species.id))
        .sum();
    let equipment_kinds = Equipment::ALL
        .iter()
        .filter(|item| inv.counts[item.idx()] > 0)
        .count() as u32;
    let mythic_kinds = Equipment::ALL
        .iter()
        .filter(|item| {
            item.def().rarity == crate::equipment::Rarity::Mythic && inv.counts[item.idx()] > 0
        })
        .count() as u32;
    let mythic_target = Equipment::ALL
        .iter()
        .filter(|item| item.def().rarity == crate::equipment::Rarity::Mythic)
        .count() as u32;

    let mut rows = Vec::new();
    milestone(
        &mut rows,
        "战役",
        "封印推进",
        "通关所有萝卜防线",
        completed_levels,
        level_count,
        Color::srgb(0.42, 0.82, 0.52),
    );
    milestone(
        &mut rows,
        "战役",
        "完美封印",
        "所有关卡达成3印",
        perfect_levels,
        level_count,
        Color::srgb(1.0, 0.74, 0.28),
    );
    milestone(
        &mut rows,
        "战役",
        "总封印力",
        "累计收集全部评级印记",
        total_rating,
        level_count * 3,
        Color::srgb(0.88, 0.70, 1.0),
    );
    milestone(
        &mut rows,
        "图鉴",
        "百怪目击",
        "在战斗中发现全部怪物",
        discovered,
        MONSTER_SPECIES.len() as u32,
        Color::srgb(0.72, 0.56, 1.0),
    );
    milestone(
        &mut rows,
        "图鉴",
        "旧日猎手",
        "累计击杀首领与MOSS级威胁",
        boss_kills,
        30,
        Color::srgb(1.0, 0.36, 0.28),
    );
    milestone(
        &mut rows,
        "图鉴",
        "战线清剿",
        "累计击杀怪物",
        total_kills,
        2000,
        Color::srgb(0.62, 0.84, 0.78),
    );
    milestone(
        &mut rows,
        "装备",
        "遗物收藏家",
        "获得全部装备种类",
        equipment_kinds,
        Equipment::ALL.len() as u32,
        Color::srgb(1.0, 0.82, 0.36),
    );
    milestone(
        &mut rows,
        "装备",
        "深库整备",
        "累计持有装备件数",
        inv.total(),
        120,
        Color::srgb(0.76, 0.86, 0.62),
    );
    milestone(
        &mut rows,
        "装备",
        "神话封存",
        "获得全部神话遗物",
        mythic_kinds,
        mythic_target,
        Color::srgb(1.0, 0.38, 0.52),
    );
    milestone(
        &mut rows,
        "档案",
        "全塔校阅",
        "防御塔档案已完整收录",
        TowerKind::ALL.len() as u32,
        TowerKind::ALL.len() as u32,
        Color::srgb(0.48, 0.86, 1.0),
    );
    rows
}

fn campaign_boss_waves(level: &Level) -> Vec<i32> {
    (1..=level.waves)
        .filter(|wave| is_boss_wave(*wave, level.waves))
        .collect()
}

fn campaign_bosses(
    level_index: usize,
    level: &Level,
) -> Vec<&'static crate::monster::MonsterSpecies> {
    let mut out = Vec::new();
    for wave in campaign_boss_waves(level) {
        for boss in boss_candidates(wave, level.waves, level_index) {
            if !out
                .iter()
                .any(|seen: &&crate::monster::MonsterSpecies| seen.id == boss.id)
            {
                out.push(boss);
            }
        }
    }
    out.sort_by_key(|species| species.id);
    out
}

fn campaign_boss_line(level_index: usize, level: &Level) -> String {
    let waves = campaign_boss_waves(level);
    let wave_text = if waves.is_empty() {
        crate::i18n::t("无")
    } else {
        waves
            .iter()
            .map(|wave| wave.to_string())
            .collect::<Vec<_>>()
            .join("、")
    };
    let bosses = campaign_bosses(level_index, level);
    if bosses.is_empty() {
        return crate::i18n::tf("首领波：{}", &[&wave_text]);
    }
    let mut names = bosses
        .iter()
        .take(3)
        .map(|boss| {
            let skill = boss_skill(boss.id);
            if skill == crate::monster::BossSkill::None {
                crate::i18n::t(boss.name)
            } else {
                crate::i18n::tf("{}·{}", &[&crate::i18n::t(boss.name), &crate::i18n::t(skill.name())])
            }
        })
        .collect::<Vec<_>>();
    if bosses.len() > names.len() {
        names.push(crate::i18n::tf("另{}名", &[&(bosses.len() - names.len()).to_string()]));
    }
    crate::i18n::tf("首领波：{}\n首领威胁：{}", &[&wave_text, &names.join(" / ")])
}

fn campaign_recommendation(level_index: usize, level: &Level) -> String {
    let bosses = campaign_bosses(level_index, level);
    if !bosses.is_empty() {
        return recommended_elements(&bosses);
    }
    let mut featured = MONSTER_SPECIES
        .iter()
        .filter(|species| !species.is_boss() && species.available(level.waves, level_index))
        .collect::<Vec<_>>();
    featured.sort_by(|a, b| {
        wave_threat_score(b, level.waves, level_index).cmp(&wave_threat_score(
            a,
            level.waves,
            level_index,
        ))
    });
    let shortlist = featured.into_iter().take(4).collect::<Vec<_>>();
    recommended_elements(&shortlist)
}

fn campaign_level_stats(level: &Level) -> String {
    crate::i18n::tf(
        "波次{}  金{}  生命{}  基础怪{}  路径{}点  首领周期{}",
        &[
            &level.waves.to_string(),
            &level.gold.to_string(),
            &level.lives.to_string(),
            &level.enemies.count.to_string(),
            &level.path.len().to_string(),
            &BOSS_WAVE_INTERVAL.to_string(),
        ],
    )
}

fn equipment_stat_line(d: &crate::equipment::EquipmentDef) -> String {
    let mut parts = Vec::new();
    if (d.damage_mult - 1.0).abs() > 0.001 {
        parts.push(crate::i18n::tf("伤害×{}", &[&format!("{:.2}", d.damage_mult)]));
    }
    if (d.range_mult - 1.0).abs() > 0.001 {
        parts.push(crate::i18n::tf("射程×{}", &[&format!("{:.2}", d.range_mult)]));
    }
    if (d.cooldown_mult - 1.0).abs() > 0.001 {
        parts.push(crate::i18n::tf("冷却×{}", &[&format!("{:.2}", d.cooldown_mult)]));
    }
    if d.armor_pierce > 0.0 {
        parts.push(crate::i18n::tf("穿甲+{}", &[&format!("{:.0}", d.armor_pierce)]));
    }
    if (d.hp_mult - 1.0).abs() > 0.001 {
        parts.push(crate::i18n::tf("HP×{}", &[&format!("{:.2}", d.hp_mult)]));
    }
    if d.armor_add > 0.0 {
        parts.push(crate::i18n::tf("护甲+{}", &[&format!("{:.0}", d.armor_add)]));
    }
    if let Some(element) = d.element {
        parts.push(crate::i18n::tf("转{}", &[&crate::i18n::t(element.name())]));
    }
    if parts.is_empty() {
        crate::i18n::t("稳定遗物")
    } else {
        parts.join("  ")
    }
}

fn equipment_visual_line(item: Equipment) -> &'static str {
    match item.visual() {
        EquipmentVisual::Crosshair => "视觉：塔身准星环，攻击型基础件一眼可见",
        EquipmentVisual::WardSigil => "视觉：护盾符文，表示耐久和护甲加成",
        EquipmentVisual::Feather => "视觉：骨羽刻纹，提示远程射程强化",
        EquipmentVisual::FuseSpark => "视觉：引线火星，提示爆破和穿甲",
        EquipmentVisual::Prism => "视觉：三棱折射光，提示秘法转化",
        EquipmentVisual::FrostLens => "视觉：冰晶透镜，提示冰霜属性",
        EquipmentVisual::EmberCore => "视觉：脉动火芯，提示火焰属性",
        EquipmentVisual::VenomDrop => "视觉：毒液滴泡，提示剧毒属性",
        EquipmentVisual::ThunderCoil => "视觉：线圈电弧，提示雷电攻速",
        EquipmentVisual::ShadowSeal => "视觉：暗影蜡印，提示压制和暗影转化",
        EquipmentVisual::BulwarkPlate => "视觉：三片护板，提示重甲防护",
        EquipmentVisual::ClockworkGear => "视觉：旋转齿轮，提示高频攻击",
        EquipmentVisual::SaltCrystal => "视觉：盐晶星芒，提示破甲驱邪",
        EquipmentVisual::DeepScale => "视觉：深海鳞盾，提示冰冷护甲",
        EquipmentVisual::ForbiddenTome => "视觉：禁书三角符，提示远程暗影",
        EquipmentVisual::StarBarrel => "视觉：星金炮管，提示高穿透火力",
        EquipmentVisual::VoidCapacitor => "视觉：双极电容，提示高频秘法",
        EquipmentVisual::SaintedGear => "视觉：圣齿轮十字，提示攻防兼备",
        EquipmentVisual::KrakenHeart => "视觉：跳动海心和触须，提示神话生命毒性",
        EquipmentVisual::AzathothEye => "视觉：神眼凝视，提示终局级全能遗物",
    }
}

fn tower_stat_line(d: &TowerDef) -> String {
    crate::i18n::tf(
        "花费{}  伤害{}  射程{}  攻速{}/秒\nHP{}  护甲{}  占地{}×{}",
        &[
            &d.cost.to_string(),
            &format!("{:.0}", d.damage),
            &format!("{:.0}", d.range),
            &format!("{:.2}", 1000.0 / d.cooldown_ms.max(1.0)),
            &format!("{:.0}", d.max_hp),
            &format!("{:.0}", d.armor),
            &d.footprint.to_string(),
            &d.footprint.to_string(),
        ],
    )
}

fn tower_behavior_line(d: &TowerDef) -> String {
    match d.behavior {
        Behavior::Single => crate::i18n::t("职责：单体点杀"),
        Behavior::Aoe => crate::i18n::tf("职责：范围爆炸  半径{}", &[&format!("{:.0}", d.aoe_radius)]),
        Behavior::Chain => crate::i18n::tf(
            "职责：连锁弹射  {}跳/{}距",
            &[&d.chain_count.to_string(), &format!("{:.0}", d.chain_range)],
        ),
        Behavior::Laser => crate::i18n::t("职责：持续穿透光束"),
        Behavior::Homing => crate::i18n::tf("职责：追踪爆破  半径{}", &[&format!("{:.0}", d.aoe_radius)]),
        Behavior::Slow => crate::i18n::tf(
            "职责：减速  {}%/{}s",
            &[
                &format!("{:.0}", (1.0 - d.slow_factor) * 100.0),
                &format!("{:.1}", d.slow_duration / 1000.0),
            ],
        ),
        Behavior::Knockback => crate::i18n::tf(
            "职责：击退{}并眩晕{}s",
            &[
                &format!("{:.0}", d.knock_dist),
                &format!("{:.1}", d.stun_duration / 1000.0),
            ],
        ),
        Behavior::Freeze => crate::i18n::tf("职责：范围冰冻  {}s", &[&format!("{:.1}", d.freeze_duration / 1000.0)]),
        Behavior::Curse => crate::i18n::tf(
            "职责：破甲破抗  -{}/{}s",
            &[
                &format!("{:.0}", d.armor_reduce),
                &format!("{:.1}", d.curse_duration / 1000.0),
            ],
        ),
        Behavior::Heal => crate::i18n::tf(
            "职责：治疗萝卜{}  增益{}",
            &[&d.heal_amount.to_string(), &format!("{:.0}", d.buff_range)],
        ),
        Behavior::Detect => crate::i18n::t("职责：反隐侦测"),
        Behavior::Poison => crate::i18n::tf(
            "职责：剧毒持续伤害  {}/s {}s",
            &[
                &format!("{:.0}", d.dot_damage),
                &format!("{:.1}", d.poison_duration / 1000.0),
            ],
        ),
        Behavior::Fire => crate::i18n::tf(
            "职责：火场持续伤害  {}/s {}s",
            &[
                &format!("{:.0}", d.dot_damage),
                &format!("{:.1}", d.fire_duration / 1000.0),
            ],
        ),
        Behavior::Summon => crate::i18n::tf("职责：召唤阻挡  上限{}", &[&d.max_summons.to_string()]),
        Behavior::Necromancer => crate::i18n::t("职责：复活范围内阵亡怪物为友军"),
    }
}

fn tower_counter_line(element: Element) -> String {
    let mut weak: Vec<(&str, f32)> = MONSTER_SPECIES
        .iter()
        .map(|s| (s.name, s.resist_profile().get(element)))
        .filter(|(_, resist)| *resist <= -0.15)
        .collect();
    weak.sort_by(|a, b| a.1.total_cmp(&b.1));

    let mut resist: Vec<(&str, f32)> = MONSTER_SPECIES
        .iter()
        .map(|s| (s.name, s.resist_profile().get(element)))
        .filter(|(_, resist)| *resist >= 0.20)
        .collect();
    resist.sort_by(|a, b| b.1.total_cmp(&a.1));

    let weak_names: Vec<String> = weak.iter().take(2).map(|(name, _)| crate::i18n::t(name)).collect();
    let resist_names: Vec<String> = resist.iter().take(2).map(|(name, _)| crate::i18n::t(name)).collect();
    let weak_text = if weak_names.is_empty() {
        crate::i18n::t("克制：少见明显易伤")
    } else {
        crate::i18n::tf("克制：{}", &[&weak_names.join("、")])
    };
    let resist_text = if resist_names.is_empty() {
        crate::i18n::t("强抗：少")
    } else {
        crate::i18n::tf("强抗：{}", &[&resist_names.join("、")])
    };
    format!("{}；{}", weak_text, resist_text)
}

/// Generic cleanup: despawn every entity carrying marker `T`.
pub fn despawn_with<T: Component>(mut commands: Commands, q: Query<Entity, With<T>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

/// Bevy 0.19's text backend (parley) does Unicode word-boundary line-breaking via
/// ICU4X, which has no bundled CJK segmentation model and spams "No segmentation
/// model for language: ja". All our text is Chinese, which line-breaks per glyph
/// anyway, so force `AnyCharacter` on every text the moment it spawns — this both
/// fixes wrapping and bypasses the missing word segmenter.
pub fn cjk_linebreak(mut texts: Query<&mut TextLayout, Added<TextLayout>>) {
    for mut tl in &mut texts {
        if tl.linebreak != LineBreak::AnyCharacter {
            tl.linebreak = LineBreak::AnyCharacter;
        }
    }
}

fn text_font(f: &Handle<Font>, size: f32) -> TextFont {
    TextFont {
        font: FontSource::Handle(f.clone()),
        font_size: FontSize::Px(size),
        ..default()
    }
}

/// A HUD stat shown as [icon] number, replacing a text label (e.g. 金币 → 🪙). The
/// `marker` goes on the number text so `update_hud` can update just the value.
fn stat_icon(
    r: &mut ChildSpawnerCommands,
    f: &Handle<Font>,
    icon: Handle<Image>,
    text: &str,
    color: Color,
    marker: impl Bundle,
) {
    r.spawn(Node {
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        column_gap: Val::Px(3.0),
        ..default()
    })
    .with_children(|c| {
        c.spawn((
            ImageNode {
                image: icon,
                ..default()
            },
            Node {
                width: Val::Px(18.0),
                height: Val::Px(18.0),
                ..default()
            },
        ));
        c.spawn((Text::new(text), text_font(f, 15.0), TextColor(color), marker));
    });
}

/// Spawn a labeled button as a child of `parent`.
fn button(
    parent: &mut ChildSpawnerCommands,
    f: &Handle<Font>,
    label: &str,
    action: UiAction,
    bg: Color,
) {
    parent
        .spawn((
            Button,
            Node {
                // Roomier hit area so buttons stay tappable on phones/touchscreens
                // (the whole UI scales with the window via `UiScale`).
                padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                margin: UiRect::all(Val::Px(2.0)),
                min_height: Val::Px(32.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(bg.with_alpha(0.92)),
            action,
        ))
        .with_children(|b| {
            b.spawn((Text::new(label), text_font(f, 13.0), TextColor(UI_TEXT)));
        });
}

fn dock_button(
    parent: &mut ChildSpawnerCommands,
    f: &Handle<Font>,
    label: &str,
    action: UiAction,
    bg: Color,
) {
    parent
        .spawn((
            Button,
            Node {
                padding: UiRect::axes(Val::Px(7.0), Val::Px(5.0)),
                margin: UiRect::all(Val::Px(1.0)),
                min_height: Val::Px(24.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(bg),
            action,
        ))
        .with_children(|b| {
            b.spawn((
                Text::new(label),
                text_font(f, 12.0),
                TextColor(Color::WHITE),
            ));
        });
}

/// Like [`dock_button`], but attaches `marker` to the inner text so a system can
/// update the label live (e.g. show the upgrade cost on the 升级 button).
fn dock_button_tagged(
    parent: &mut ChildSpawnerCommands,
    f: &Handle<Font>,
    label: &str,
    action: UiAction,
    bg: Color,
    marker: impl Bundle,
) {
    parent
        .spawn((
            Button,
            Node {
                padding: UiRect::axes(Val::Px(7.0), Val::Px(5.0)),
                margin: UiRect::all(Val::Px(1.0)),
                min_height: Val::Px(24.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(bg),
            action,
        ))
        .with_children(|b| {
            b.spawn((
                Text::new(label),
                text_font(f, 12.0),
                TextColor(Color::WHITE),
                marker,
            ));
        });
}

/// Icon tile for the build palette: tower sprite + small cost badge. The full
/// stats live in the hover/tap tooltip (`tooltip_text`), so no name text here.
fn build_button(
    parent: &mut ChildSpawnerCommands,
    f: &Handle<Font>,
    icon: Handle<Image>,
    kind: TowerKind,
    def: &TowerDef,
) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Px(54.0),
                height: Val::Px(56.0),
                margin: UiRect::all(Val::Px(3.0)),
                padding: UiRect::all(Val::Px(3.0)),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(1.0),
                ..default()
            },
            BackgroundColor(def.color.with_alpha(0.85)),
            UiAction::Build(kind),
        ))
        .with_children(|b| {
            b.spawn((
                ImageNode {
                    image: icon,
                    ..default()
                },
                Node {
                    width: Val::Px(34.0),
                    height: Val::Px(34.0),
                    ..default()
                },
            ));
            b.spawn((
                Text::new(format!("{}", def.cost)),
                text_font(f, 12.0),
                TextColor(Color::srgb(1.0, 0.92, 0.55)),
            ));
        });
}

/// A square icon button (abilities / talents). Stats are in the tap/hover tooltip,
/// so there's no label text. `extra` lets callers attach a marker component.
fn icon_button(
    parent: &mut ChildSpawnerCommands,
    icon: Handle<Image>,
    action: UiAction,
    bg: Color,
    extra: impl Bundle,
) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Px(50.0),
                height: Val::Px(50.0),
                margin: UiRect::all(Val::Px(3.0)),
                padding: UiRect::all(Val::Px(5.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(bg),
            action,
            extra,
        ))
        .with_children(|b| {
            b.spawn((
                ImageNode {
                    image: icon,
                    ..default()
                },
                Node {
                    width: Val::Px(36.0),
                    height: Val::Px(36.0),
                    ..default()
                },
            ));
        });
}

fn dock_icon_button(
    parent: &mut ChildSpawnerCommands,
    icon: Handle<Image>,
    action: UiAction,
    bg: Color,
    extra: impl Bundle,
) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Px(34.0),
                height: Val::Px(34.0),
                margin: UiRect::all(Val::Px(1.0)),
                padding: UiRect::all(Val::Px(4.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(bg),
            action,
            extra,
        ))
        .with_children(|b| {
            b.spawn((
                ImageNode {
                    image: icon,
                    ..default()
                },
                Node {
                    width: Val::Px(25.0),
                    height: Val::Px(25.0),
                    ..default()
                },
            ));
        });
}

fn equipment_button(
    parent: &mut ChildSpawnerCommands,
    f: &Handle<Font>,
    icon: Handle<Image>,
    item: Equipment,
    bg: Color,
) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Px(38.0),
                height: Val::Px(50.0),
                padding: UiRect::all(Val::Px(2.0)),
                margin: UiRect::all(Val::Px(0.0)),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(2.0),
                ..default()
            },
            BackgroundColor(bg),
            UiAction::Equip(item),
        ))
        .with_children(|b| {
            b.spawn((
                ImageNode {
                    image: icon,
                    color: Color::srgba(0.55, 0.55, 0.55, 0.65),
                    ..default()
                },
                Node {
                    width: Val::Px(24.0),
                    height: Val::Px(24.0),
                    ..default()
                },
                EquipmentButtonIcon { item },
            ));
            b.spawn((
                Text::new(format!("{}×0", item.short())),
                text_font(f, 8.5),
                TextColor(Color::srgb(0.55, 0.55, 0.55)),
                EquipmentButtonText { item },
            ));
        });
}

// ============================ HUD ============================

const PANEL_W_UI: f32 = 256.0;

pub fn spawn_hud(
    mut commands: Commands,
    fonts: Res<UiFont>,
    sprites: Res<Sprites>,
    hero: Res<HeroLoadout>,
) {
    let f = &fonts.0;
    // Start every level with the collapsible panels closed so the board is clear.
    commands.insert_resource(HudPanels::default());
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Px(PANEL_W_UI),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(8.0)),
                row_gap: Val::Px(3.0),
                // Scroll vertically so the whole palette is reachable even when the
                // panel is taller than the (scaled) window — fixes off-screen towers.
                overflow: Overflow::scroll_y(),
                ..default()
            },
            BackgroundColor(PANEL_BG),
            ScrollPosition::default(),
            HudRoot,
        ))
        .with_children(|p| {
            // --- stats (compact, two rows) ---
            // Panel gold/lives/wave — hidden in touch mode, where the left status
            // column shows them instead (avoids a left+right duplicate readout).
            p.spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(10.0),
                    ..default()
                },
                TouchHiddenRow,
            ))
            .with_children(|r| {
                stat_icon(
                    r,
                    f,
                    sprites.ui["coin"].clone(),
                    "0",
                    Color::srgb(1.0, 0.84, 0.0),
                    GoldText,
                );
                stat_icon(
                    r,
                    f,
                    sprites.ui["heart"].clone(),
                    "0",
                    Color::srgb(0.9, 0.3, 0.3),
                    LivesText,
                );
            });
            p.spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(10.0),
                    ..default()
                },
                TouchHiddenRow,
            ))
            .with_children(|r| {
                stat_icon(
                    r,
                    f,
                    sprites.ui["wave"].clone(),
                    "0/0",
                    Color::WHITE,
                    WaveText,
                );
                r.spawn((
                    Text::new("x1"),
                    text_font(f, 13.0),
                    TextColor(Color::srgb(0.6, 0.9, 1.0)),
                    SpeedText,
                ));
            });
            p.spawn((
                Node {
                    display: Display::None,
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(3.0),
                    margin: UiRect::bottom(Val::Px(2.0)),
                    ..default()
                },
                ComboMeterRoot,
            ))
            .with_children(|combo| {
                combo.spawn((
                    Text::new(crate::i18n::t("连杀 x0")),
                    text_font(f, 12.0),
                    TextColor(Color::srgb(1.0, 0.82, 0.28)),
                    ComboMeterText,
                ));
                combo
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(5.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.32, 0.21, 0.04, 0.76)),
                    ))
                    .with_children(|bar| {
                        bar.spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(1.0, 0.68, 0.16)),
                            ComboMeterFill,
                        ));
                    });
            });
            p.spawn((
                Text::new(crate::i18n::t("侦察：等待关卡载入")),
                text_font(f, 12.0),
                TextColor(Color::srgb(0.72, 0.9, 0.78)),
                WaveIntelText,
            ));

            // (Wave/pause/speed controls moved to the pinned top-left bar so they're
            // always reachable regardless of rail scroll.)

            // --- abilities (icons; tap/hover for details). Hidden on touch since
            // the bottom bar carries them. ---
            p.spawn((section_icon_node(sprites.ui["sec_skill"].clone()), TouchHiddenRow));
            p.spawn((row_node(), TouchHiddenRow)).with_children(|row| {
                icon_button(
                    row,
                    sprites.abilities[&Ability::Meteor].clone(),
                    UiAction::Cast(Ability::Meteor),
                    ability_color(Ability::Meteor),
                    (),
                );
                icon_button(
                    row,
                    sprites.abilities[&Ability::Freeze].clone(),
                    UiAction::Cast(Ability::Freeze),
                    ability_color(Ability::Freeze),
                    (),
                );
                icon_button(
                    row,
                    sprites.abilities[&Ability::GoldRush].clone(),
                    UiAction::Cast(Ability::GoldRush),
                    ability_color(Ability::GoldRush),
                    (),
                );
            });

            // --- talents (icons; tap/hover for details) ---
            p.spawn(section_icon_node(sprites.ui["sec_talent"].clone()));
            p.spawn(row_node()).with_children(|row| {
                icon_button(
                    row,
                    sprites.talents["damage"].clone(),
                    UiAction::TalentDamage,
                    BTN_BG,
                    (),
                );
                icon_button(
                    row,
                    sprites.talents["range"].clone(),
                    UiAction::TalentRange,
                    BTN_BG,
                    (),
                );
                icon_button(
                    row,
                    sprites.talents["speed"].clone(),
                    UiAction::TalentSpeed,
                    BTN_BG,
                    (),
                );
            });

            // --- build palette: icon grid; tap/hover a tower for its tooltip. Each
            // category header is an icon (swords/snowflake/plus/star) instead of text.
            for cat in Category::ALL {
                let key = match cat {
                    Category::Attack => "cat_attack",
                    Category::Control => "cat_control",
                    Category::Support => "cat_support",
                    Category::Special => "cat_special",
                };
                p.spawn(section_icon_node(sprites.ui[key].clone()));
                p.spawn(row_node()).with_children(|row| {
                    for kind in TowerKind::ALL {
                        let def = kind.def();
                        if def.category != cat {
                            continue;
                        }
                        build_button(row, f, sprites.towers[&kind].clone(), kind, def);
                    }
                });
            }
        });

    // --- loadout dock: hero, selected unit, equipment slots, and inventory. Hidden
    // by default (`HudPanels::dock_open`) so it never covers the board; the player
    // opens it with the 英雄/装备 button. Works the same on desktop and touch.
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(8.0),
                right: Val::Px(PANEL_W_UI + 8.0),
                bottom: Val::Px(6.0),
                height: Val::Px(190.0),
                display: Display::None,
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(7.0)),
                row_gap: Val::Px(4.0),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(UI_PANEL_DARK),
            GlobalZIndex(24),
            HudRoot,
            DockRoot,
        ))
        .with_children(|dock| {
            dock.spawn(Node {
                height: Val::Px(112.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(6.0),
                ..default()
            })
            .with_children(|top| {
                top.spawn((
                    Node {
                        width: Val::Px(380.0),
                        height: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(7.0),
                        padding: UiRect::all(Val::Px(6.0)),
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(UI_CARD_SOFT),
                ))
                .with_children(|card| {
                    card.spawn((
                        ImageNode {
                            image: sprites.heroes[&hero.class].clone(),
                            ..default()
                        },
                        Node {
                            width: Val::Px(58.0),
                            height: Val::Px(58.0),
                            flex_shrink: 0.0,
                            ..default()
                        },
                    ));
                    card.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(2.0),
                        flex_grow: 1.0,
                        ..default()
                    })
                    .with_children(|info| {
                        info.spawn((
                            Text::new(crate::i18n::t("英雄")),
                            text_font(f, 11.0),
                            TextColor(Color::srgb(0.84, 0.94, 1.0)),
                            HeroInfoText,
                        ));
                        info.spawn(Node {
                            flex_direction: FlexDirection::Row,
                            flex_wrap: FlexWrap::Wrap,
                            ..default()
                        })
                        .with_children(|talents| {
                            dock_icon_button(
                                talents,
                                sprites.hero_skills[&hero.class].clone(),
                                UiAction::HeroSkill,
                                hero.class.skill_color(),
                                (),
                            );
                            for i in 0..HeroLoadout::TALENT_SLOTS {
                                dock_icon_button(
                                    talents,
                                    sprites.hero_talents[&(hero.class, i)].clone(),
                                    UiAction::HeroTalent(i),
                                    Color::srgba(0.14, 0.18, 0.24, 0.92),
                                    (),
                                );
                            }
                        });
                    });
                });

                top.spawn((
                    Node {
                        height: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(3.0),
                        padding: UiRect::all(Val::Px(6.0)),
                        flex_grow: 1.0,
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BackgroundColor(UI_CARD),
                ))
                .with_children(|card| {
                    card.spawn((
                        Text::new(crate::i18n::t("未选择单位")),
                        text_font(f, 11.0),
                        TextColor(Color::WHITE),
                        SelInfoText,
                    ));
                    // Icon stat-strip: glyph + live number for the 5 key figures,
                    // replacing the old "伤害 X 范围 Y …" text wall (文字→图标).
                    card.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(9.0),
                        ..default()
                    })
                    .with_children(|strip| {
                        for (key, stat) in [
                            ("st_damage", UnitStat::Damage),
                            ("st_range", UnitStat::Range),
                            ("st_armor", UnitStat::Armor),
                            ("st_speed", UnitStat::Speed),
                            ("st_dps", UnitStat::Dps),
                        ] {
                            stat_icon(strip, f, sprites.ui[key].clone(), "—", UI_TEXT, stat);
                        }
                    });
                    card.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(5.0),
                        ..default()
                    })
                    .with_children(|bar| {
                        for slot in 0..3 {
                            bar.spawn((
                                Button,
                                Node {
                                    width: Val::Px(44.0),
                                    height: Val::Px(44.0),
                                    padding: UiRect::all(Val::Px(3.0)),
                                    flex_direction: FlexDirection::Column,
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::Center,
                                    row_gap: Val::Px(1.0),
                                    flex_shrink: 0.0,
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.04)),
                                EquippedSlotFrame { slot },
                                UiAction::UnequipSlot(slot),
                            ))
                            .with_children(|slot_btn| {
                                slot_btn.spawn((
                                    ImageNode {
                                        image: sprites.equipment[&Equipment::RustySight].clone(),
                                        color: Color::srgba(1.0, 1.0, 1.0, 0.0),
                                        ..default()
                                    },
                                    Node {
                                        width: Val::Px(24.0),
                                        height: Val::Px(24.0),
                                        ..default()
                                    },
                                    EquippedSlotIcon { slot },
                                ));
                                slot_btn.spawn((
                                    Text::new(crate::i18n::t("空")),
                                    text_font(f, 8.5),
                                    TextColor(Color::srgb(0.45, 0.45, 0.45)),
                                    EquippedSlotText { slot },
                                ));
                            });
                        }

                        bar.spawn(Node {
                            flex_direction: FlexDirection::Row,
                            flex_wrap: FlexWrap::Wrap,
                            flex_grow: 1.0,
                            ..default()
                        })
                        .with_children(|actions| {
                            dock_button_tagged(
                                actions,
                                f,
                                &crate::i18n::t("升级"),
                                UiAction::Upgrade,
                                Color::srgb(0.42, 0.30, 0.10),
                                UpgradeBtnText,
                            );
                            dock_button(actions, f, &crate::i18n::t("修理"), UiAction::Repair, BTN_BG);
                            dock_button(actions, f, &crate::i18n::t("目标"), UiAction::CycleTargetPriority, BTN_BG);
                            dock_button(actions, f, &crate::i18n::t("卸装"), UiAction::Unequip, BTN_BG);
                            dock_button(
                                actions,
                                f,
                                &crate::i18n::t("卖出"),
                                UiAction::Sell,
                                Color::srgb(0.35, 0.12, 0.12),
                            );
                            dock_button(actions, f, &crate::i18n::t("重生"), UiAction::SummonHero, BTN_BG);
                            dock_button(actions, f, &crate::i18n::t("重置"), UiAction::ResetHeroTalents, BTN_BG);
                        });
                    });
                });
            });

            dock.spawn(Node {
                height: Val::Px(14.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(7.0),
                overflow: Overflow::clip(),
                ..default()
            })
            .with_children(|row| {
                row.spawn((
                    Text::new(crate::i18n::t("装备栏")),
                    text_font(f, 11.0),
                    TextColor(Color::srgb(0.92, 0.84, 0.58)),
                ));
                row.spawn((
                    Text::new(crate::i18n::t("装备 0")),
                    text_font(f, 10.0),
                    TextColor(Color::srgb(0.9, 0.7, 0.9)),
                    InvText,
                ));
                row.spawn((
                    Text::new(crate::i18n::t("选中英雄或塔后点击装备；点击槽位单独卸下")),
                    text_font(f, 10.0),
                    TextColor(Color::srgb(0.58, 0.68, 0.62)),
                ));
            });

            dock.spawn(Node {
                height: Val::Px(50.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Stretch,
                justify_content: JustifyContent::SpaceBetween,
                overflow: Overflow::clip(),
                ..default()
            })
            .with_children(|row| {
                for item in Equipment::ALL {
                    equipment_button(
                        row,
                        f,
                        sprites.equipment[&item].clone(),
                        item,
                        item.def().rarity.color().with_alpha(0.72),
                    );
                }
            });
        });

    // --- fixed feedback banner over the board (always visible, not in panel) ---
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(12.0),
                left: Val::Px(0.0),
                width: Val::Px(BOARD_W),
                justify_content: JustifyContent::Center,
                ..default()
            },
            HudRoot,
        ))
        .with_children(|b| {
            b.spawn((
                Text::new(""),
                text_font(f, 22.0),
                TextColor(Color::srgb(1.0, 0.95, 0.5)),
                BannerText,
            ));
        });

    // --- boss pressure bar over the board, hidden until a boss is alive ---
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(46.0),
                left: Val::Px(0.0),
                width: Val::Px(BOARD_W),
                justify_content: JustifyContent::Center,
                display: Display::None,
                ..default()
            },
            BossBarRoot,
            HudRoot,
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Px(440.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(7.0)),
                    row_gap: Val::Px(4.0),
                    ..default()
                },
                // Fully transparent: show only the portrait / name / HP bar, no black box.
                BackgroundColor(Color::NONE),
            ))
            .with_children(|panel| {
                // Boss portrait (image set per-boss in update_boss_bar).
                panel.spawn((
                    ImageNode {
                        image: sprites.bosses[&BossSkill::SerpentRush].clone(),
                        ..default()
                    },
                    Node {
                        width: Val::Px(72.0),
                        height: Val::Px(72.0),
                        align_self: AlignSelf::Center,
                        ..default()
                    },
                    BossPortrait,
                ));
                panel.spawn((
                    Text::new(""),
                    text_font(f, 13.0),
                    TextColor(Color::srgb(1.0, 0.92, 0.72)),
                    BossBarText,
                ));
                panel
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(9.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.35, 0.03, 0.05, 0.78)),
                    ))
                    .with_children(|bar| {
                        bar.spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.80, 0.05, 0.08)),
                            BossBarFill,
                        ));
                    });
                panel
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(4.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.95, 0.55, 0.08, 0.20)),
                    ))
                    .with_children(|bar| {
                        bar.spawn((
                            Node {
                                width: Val::Percent(0.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(1.0, 0.55, 0.14)),
                            BossSkillFill,
                        ));
                    });
            });
        });

    // --- hover tooltip (separate absolute node, sits just left of the panel) ---
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(PANEL_W_UI + 6.0),
                // Sit above the mobile touch bar (bottom 3 + height 38) so a
                // tapped tooltip never hides behind it.
                bottom: Val::Px(48.0),
                max_width: Val::Px(300.0),
                padding: UiRect::all(Val::Px(8.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(UI_PANEL_DARK),
            GlobalZIndex(80),
            TooltipBox,
            HudRoot, // tagged so it despawns with the HUD on level exit
        ))
        .with_children(|t| {
            t.spawn((
                Text::new(""),
                text_font(f, 13.0),
                TextColor(Color::srgb(0.95, 0.95, 0.85)),
                TooltipText,
            ));
        });

    // --- mobile touch controls: a VERTICAL strip down the LEFT edge of the screen.
    // On phones (forced landscape) the board fills the height, so top/bottom bars
    // cover gameplay cells — but the left/right have letterbox margin. Putting the
    // controls in a thin left column keeps the core board grid unobstructed and is
    // reachable by either thumb. Hidden until touch is detected. ---
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(4.0),
                top: Val::Px(88.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                align_items: AlignItems::Center,
                display: Display::None, // revealed once touch input is seen
                ..default()
            },
            MobileHudRoot,
        ))
        .with_children(|bar| {
            side_icon_button(
                bar,
                sprites.ui["ctrl_start"].clone(),
                UiAction::StartWave,
                Color::srgb(0.18, 0.5, 0.2),
            );
            side_icon_button(bar, sprites.ui["ctrl_auto"].clone(), UiAction::ToggleAutoWave, BTN_BG);
            side_icon_button(bar, sprites.ui["ctrl_pause"].clone(), UiAction::TogglePause, BTN_BG);
            side_icon_button(bar, sprites.ui["ctrl_speed"].clone(), UiAction::CycleSpeed, BTN_BG);
            // Abilities + hero skill as icons (tap shows the tooltip; cooldown greys bg).
            for ab in [Ability::Meteor, Ability::Freeze, Ability::GoldRush] {
                side_icon_button(
                    bar,
                    sprites.abilities[&ab].clone(),
                    UiAction::Cast(ab),
                    ability_color(ab),
                );
            }
            side_icon_button(
                bar,
                sprites.hero_skills[&hero.class].clone(),
                UiAction::HeroSkill,
                hero.class.skill_color(),
            );
        });

    // --- fixed status (touch only): 金/命/波 stacked VERTICALLY at the top-left,
    // above the control strip, so they never cover board cells. Toggles with touch. ---
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(4.0),
                top: Val::Px(4.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(UI_PANEL),
            GlobalZIndex(40),
            MobileHudRoot,
        ))
        .with_children(|r| {
            stat_icon(
                r,
                f,
                sprites.ui["coin"].clone(),
                "0",
                Color::srgb(1.0, 0.84, 0.0),
                GoldText,
            );
            stat_icon(
                r,
                f,
                sprites.ui["heart"].clone(),
                "0",
                Color::srgb(1.0, 0.45, 0.45),
                LivesText,
            );
            stat_icon(
                r,
                f,
                sprites.ui["wave"].clone(),
                "0/0",
                Color::WHITE,
                WaveText,
            );
        });

    // --- movement joystick (touch only): drag to steer the hero (王者荣耀-style) ---
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Px(JOY_RADIUS * 2.0),
                height: Val::Px(JOY_RADIUS * 2.0),
                // Floating joystick: hidden until a finger presses the move zone; then
                // `hero_joystick` positions it at the touch point (王者/原神-style).
                display: Display::None,
                ..default()
            },
            // Circular base: a white disk texture tinted translucent (Node background
            // can't be rounded — BorderRadius isn't a Component in this Bevy build).
            ImageNode {
                image: sprites.ui["circle"].clone(),
                color: Color::srgba(0.9, 0.95, 1.0, 0.14),
                ..default()
            },
            GlobalZIndex(45),
            JoystickBase,
            HudRoot,
        ))
        .with_children(|b| {
            b.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(JOY_RADIUS - 24.0),
                    top: Val::Px(JOY_RADIUS - 24.0),
                    width: Val::Px(48.0),
                    height: Val::Px(48.0),
                    ..default()
                },
                ImageNode {
                    image: sprites.ui["circle"].clone(),
                    color: Color::srgba(0.8, 0.9, 1.0, 0.55),
                    ..default()
                },
                JoystickKnob,
            ));
        });

    // --- build-placement ghost (world-space translucent tower preview) ---
    commands.spawn((
        Sprite {
            image: sprites.towers[&TowerKind::ALL[0]].clone(),
            color: Color::srgba(1.0, 1.0, 1.0, 0.0),
            custom_size: Some(Vec2::splat(40.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 7.0),
        Visibility::Hidden,
        crate::build::BuildGhost,
    ));

    // --- full-screen danger flash (red pulse when a life is lost) ---
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(Color::srgba(1.0, 0.1, 0.08, 0.0)),
        GlobalZIndex(50),
        ScreenFlash { level: 0.0 },
        HudRoot,
    ));

    // --- pinned control bar (top-left, always visible): the next-wave / pause /
    // speed controls live here so they're reachable no matter how the rail scrolls.
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(6.0),
                top: Val::Px(6.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                ..default()
            },
            GlobalZIndex(55),
            HudRoot,
            // Hidden on touch: the mobile left strip provides these controls, so the
            // desktop top-left bar would otherwise duplicate "start" and cover cells.
            TouchHiddenRow,
        ))
        .with_children(|row| {
            // Icon controls (hover/tap shows the tooltip with the hotkey). The current
            // speed multiplier is shown by SpeedText in the top status row.
            icon_button(
                row,
                sprites.ui["ctrl_start"].clone(),
                UiAction::StartWave,
                Color::srgb(0.2, 0.5, 0.2),
                (),
            );
            icon_button(
                row,
                sprites.ui["ctrl_auto"].clone(),
                UiAction::ToggleAutoWave,
                BTN_BG,
                (),
            );
            icon_button(
                row,
                sprites.ui["ctrl_pause"].clone(),
                UiAction::TogglePause,
                BTN_BG,
                (),
            );
            icon_button(
                row,
                sprites.ui["ctrl_speed"].clone(),
                UiAction::CycleSpeed,
                BTN_BG,
                (),
            );
        });

    // --- settings: a floating gear button (top-right) that opens a panel holding
    // all settings (quality, fullscreen, difficulty) instead of cluttering the rail.
    // Floating hero-panel button (top-right, left of the gear): the hero portrait;
    // tapping opens the loadout dock (hero stats / equipment / talents / summon).
    commands
        .spawn((
            Button,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(70.0),
                top: Val::Px(4.0),
                width: Val::Px(38.0),
                height: Val::Px(38.0),
                padding: UiRect::all(Val::Px(3.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.16, 0.22, 0.30, 0.92)),
            GlobalZIndex(60),
            UiAction::ToggleDock,
            HudRoot,
        ))
        .with_children(|b| {
            b.spawn((
                ImageNode {
                    image: sprites.heroes[&hero.class].clone(),
                    ..default()
                },
                Node {
                    width: Val::Px(32.0),
                    height: Val::Px(32.0),
                    ..default()
                },
            ));
        });

    commands
        .spawn((
            Button,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(6.0),
                top: Val::Px(6.0),
                width: Val::Px(56.0),
                height: Val::Px(34.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.12, 0.14, 0.16, 0.92)),
            GlobalZIndex(60),
            UiAction::ToggleSettings,
            SettingsGear,
            HudRoot,
        ))
        .with_children(|g| {
            g.spawn((Text::new(crate::i18n::t("设置")), text_font(f, 14.0), TextColor(Color::WHITE)));
        });

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(6.0),
                top: Val::Px(46.0),
                width: Val::Px(232.0),
                display: Display::None,
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                row_gap: Val::Px(7.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.07, 0.08, 0.09, 0.98)),
            GlobalZIndex(59),
            SettingsRoot,
            HudRoot,
        ))
        .with_children(|s| {
            s.spawn((
                Text::new(crate::i18n::t("设置")),
                text_font(f, 16.0),
                TextColor(Color::srgb(0.9, 0.95, 1.0)),
            ));
            s.spawn((
                Text::new(crate::i18n::t("画质：标准")),
                text_font(f, 12.0),
                TextColor(Color::srgb(0.7, 0.85, 1.0)),
                QualityLabel,
            ));
            s.spawn(row_node()).with_children(|row| {
                button(row, f, &crate::i18n::t("切换画质"), UiAction::CycleQuality, BTN_BG);
                button(row, f, &crate::i18n::t("全屏"), UiAction::Fullscreen, BTN_BG);
            });
            s.spawn((
                Text::new(crate::i18n::t("难度")),
                text_font(f, 12.0),
                TextColor(Color::srgb(0.8, 0.85, 0.9)),
                DiffLabel,
            ));
            s.spawn(row_node()).with_children(|row| {
                for difficulty in Difficulty::ALL {
                    icon_button(
                        row,
                        sprites.ui[difficulty_icon_key(difficulty)].clone(),
                        UiAction::SetDifficulty(difficulty),
                        difficulty_color(difficulty),
                        (),
                    );
                }
            });
            s.spawn((
                Text::new(crate::i18n::t("关卡")),
                text_font(f, 12.0),
                TextColor(Color::srgb(0.8, 0.85, 0.9)),
            ));
            s.spawn(row_node()).with_children(|row| {
                button(row, f, &crate::i18n::t("重新开始"), UiAction::Restart, Color::srgb(0.30, 0.22, 0.10));
                button(row, f, &crate::i18n::t("返回主页"), UiAction::ToMenu, Color::srgb(0.30, 0.12, 0.12));
            });
            s.spawn(row_node()).with_children(|row| {
                button(row, f, &crate::i18n::t("关闭"), UiAction::ToggleSettings, BTN_BG);
            });
        });
}

/// An equal-width icon button for the mobile bottom bar (abilities).
/// A fixed-size square icon button for the mobile LEFT control strip (vertical).
/// Unlike `mobile_icon_button` it does not flex-grow, so it stays compact when
/// stacked in a column down the side of the board (keeps the playfield clear).
fn side_icon_button(
    parent: &mut ChildSpawnerCommands,
    icon: Handle<Image>,
    action: UiAction,
    bg: Color,
) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Px(40.0),
                height: Val::Px(40.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(bg.with_alpha(0.92)),
            action,
        ))
        .with_children(|b| {
            b.spawn((
                ImageNode {
                    image: icon,
                    ..default()
                },
                Node {
                    width: Val::Px(28.0),
                    height: Val::Px(28.0),
                    ..default()
                },
            ));
        });
}

/// One large, equal-width touch button for the mobile bottom bar.
/// Sticky flag: becomes true the first time any touch input is seen, and the
/// mobile touch bar reveals itself. Mouse-only sessions never flip it.
#[derive(Resource, Default)]
pub struct TouchMode(pub bool);

/// Identity of a costly/irreversible action that requires a confirming second tap
/// on touch (so a stray tap never spends gold or sells a tower). Desktop acts
/// immediately since hover already reveals the tooltip.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ConfirmId {
    /// 0 = damage, 1 = range, 2 = speed talent.
    Talent(u8),
    /// Equip the inventory item with this index onto the selected tower.
    Equip(usize),
    /// Refine the inventory item with this index.
    Refine(usize),
    /// Sell the selected tower.
    Sell,
}

/// Pending action awaiting a confirming second tap (touch only). Cleared on a
/// timeout or once confirmed; see [`tick_talent_confirm`].
#[derive(Resource, Default)]
pub struct TalentConfirm {
    pub pending: Option<ConfirmId>,
    pub timer: f32,
}

/// Map a `UiAction` to its [`ConfirmId`] when it needs tap-to-confirm on touch.
fn confirm_id(action: &UiAction) -> Option<ConfirmId> {
    match action {
        UiAction::TalentDamage => Some(ConfirmId::Talent(0)),
        UiAction::TalentRange => Some(ConfirmId::Talent(1)),
        UiAction::TalentSpeed => Some(ConfirmId::Talent(2)),
        UiAction::Equip(item) => Some(ConfirmId::Equip(item.idx())),
        UiAction::RefineEquipment(item) => Some(ConfirmId::Refine(item.idx())),
        UiAction::Sell => Some(ConfirmId::Sell),
        _ => None,
    }
}

/// Short hint shown on the first (arming) tap of a confirmable action.
fn confirm_hint(id: ConfirmId) -> &'static str {
    match id {
        ConfirmId::Talent(_) => "再次点击确认强化",
        ConfirmId::Equip(_) => "再次点击确认装配",
        ConfirmId::Refine(_) => "再次点击确认精炼",
        ConfirmId::Sell => "再次点击确认出售",
    }
}

/// Decay the pending confirmation so it doesn't linger between taps.
pub fn tick_talent_confirm(time: Res<Time>, mut confirm: ResMut<TalentConfirm>) {
    if confirm.pending.is_some() {
        confirm.timer -= time.delta_secs();
        if confirm.timer <= 0.0 {
            confirm.pending = None;
        }
    }
}

/// Flip `TouchMode` on once a touch is detected (cheap; runs every frame).
pub fn detect_touch_mode(touches: Res<Touches>, mut mode: ResMut<TouchMode>) {
    if !mode.0 && (touches.iter().next().is_some() || touches.any_just_pressed()) {
        mode.0 = true;
    }
}

/// Show the mobile touch bar and hide the now-redundant panel rows in touch mode
/// (the bar carries wave/pause/speed + abilities, so they'd otherwise duplicate).
pub fn update_mobile_controls(
    mode: Res<TouchMode>,
    mut bar: Query<&mut Node, (With<MobileHudRoot>, Without<TouchHiddenRow>)>,
    mut rows: Query<&mut Node, (With<TouchHiddenRow>, Without<MobileHudRoot>)>,
) {
    let bar_want = if mode.0 { Display::Flex } else { Display::None };
    for mut node in &mut bar {
        if node.display != bar_want {
            node.display = bar_want;
        }
    }
    // Section labels default to Display::Block; rows to Flex. Restore the right
    // one when not in touch mode.
    for mut node in &mut rows {
        let want = if mode.0 { Display::None } else { Display::Flex };
        if node.display != want {
            node.display = want;
        }
    }
}

/// Show/hide the collapsible loadout dock and the settings panel per [`HudPanels`].
/// Both stay closed by default so the board is reachable; the player opens them on
/// demand (英雄/装备 button and the top-right gear). Disjoint `Without<>` filters
/// keep the two `&mut Node` queries from conflicting (B0001).
pub fn update_panel_visibility(
    panels: Res<HudPanels>,
    mut dock: Query<&mut Node, (With<DockRoot>, Without<SettingsRoot>)>,
    mut settings: Query<&mut Node, (With<SettingsRoot>, Without<DockRoot>)>,
) {
    if !panels.is_changed() {
        return;
    }
    let want = if panels.dock_open { Display::Flex } else { Display::None };
    for mut node in &mut dock {
        node.display = want;
    }
    let want = if panels.settings_open {
        Display::Flex
    } else {
        Display::None
    };
    for mut node in &mut settings {
        node.display = want;
    }
}

fn row_node() -> Node {
    Node {
        flex_direction: FlexDirection::Row,
        flex_wrap: FlexWrap::Wrap,
        ..default()
    }
}

/// A section header rendered as an icon instead of Chinese text (goal: text→icon).
/// Returns the bundle so callers can attach markers like `TouchHiddenRow`.
fn section_icon_node(icon: Handle<Image>) -> impl Bundle {
    (
        ImageNode {
            image: icon,
            ..default()
        },
        Node {
            width: Val::Px(26.0),
            height: Val::Px(26.0),
            margin: UiRect::top(Val::Px(4.0)),
            ..default()
        },
    )
}

fn element_marker(element: crate::data::Element) -> &'static str {
    match element {
        crate::data::Element::Physical => "物",
        crate::data::Element::Arcane => "秘",
        crate::data::Element::Fire => "火",
        crate::data::Element::Frost => "冰",
        crate::data::Element::Storm => "雷",
        crate::data::Element::Shadow => "影",
        crate::data::Element::Toxic => "毒",
    }
}

fn wave_threat_score(
    species: &crate::monster::MonsterSpecies,
    wave: i32,
    level_index: usize,
) -> i32 {
    let def = species.def();
    let mut score = 0;
    score += (species.min_wave * 3).min(wave * 3);
    score += (species.min_level as i32 * 2).min(level_index as i32 * 2);
    if def.boss {
        score += 50;
    }
    if def.tower_raider {
        score += 16;
    }
    if def.silence_aura > 0.0 {
        score += 14;
    }
    if def.heal_aura > 0.0 {
        score += 11;
    }
    if def.shield > 0.0 {
        score += 8;
    }
    if def.splits > 0 {
        score += 8;
    }
    if def.flying {
        score += 7;
    }
    if def.invisible {
        score += 7;
    }
    if def.regen > 0.0 {
        score += 6;
    }
    if def.charger {
        score += 6;
    }
    score
}

fn recommended_elements(species: &[&crate::monster::MonsterSpecies]) -> String {
    if species.is_empty() {
        return crate::i18n::t("推荐：均衡布防");
    }
    let mut scored: Vec<(Element, f32)> = Element::ALL
        .iter()
        .map(|element| {
            let total: f32 = species
                .iter()
                .map(|s| s.resist_profile().get(*element))
                .sum();
            (*element, total / species.len() as f32)
        })
        .collect();
    scored.sort_by(|a, b| a.1.total_cmp(&b.1));
    let best: Vec<String> = scored
        .iter()
        .filter(|(_, avg)| *avg <= 0.12)
        .take(3)
        .map(|(element, avg)| {
            if *avg <= -0.08 {
                crate::i18n::tf(
                    "{}弱{}%",
                    &[&crate::i18n::t(element.name()), &((-avg * 100.0).round() as i32).to_string()],
                )
            } else {
                crate::i18n::t(element.name())
            }
        })
        .collect();
    if best.is_empty() {
        crate::i18n::t("推荐：穿甲/减抗/召唤拖延")
    } else {
        crate::i18n::tf("推荐：{}", &[&best.join("、")])
    }
}

fn elite_affix_intel(wave: i32, level_index: usize) -> String {
    if wave < 4 {
        return crate::i18n::t("精英突变：尚未侦测");
    }
    let pool = elite_affix_pool(wave, level_index);
    if pool.is_empty() {
        return crate::i18n::t("精英突变：低风险");
    }
    let names = pool
        .iter()
        .map(|affix| crate::i18n::t(affix.name()))
        .collect::<Vec<_>>()
        .join("、");
    let focus = pool[0];
    crate::i18n::tf(
        "精英突变：{}\n重点：{} - {}",
        &[
            &names,
            &crate::i18n::t(focus.name()),
            &crate::i18n::t(focus.description()),
        ],
    )
}

fn boss_candidates(
    wave: i32,
    total_waves: i32,
    level_index: usize,
) -> Vec<&'static crate::monster::MonsterSpecies> {
    if wave == total_waves && wave >= 20 && level_index >= 18 {
        return species_by_id(99).into_iter().collect();
    }
    MONSTER_SPECIES
        .iter()
        .filter(|s| s.is_boss() && s.available(wave, level_index))
        .collect()
}

fn wave_intel_text(run: &RunState, level_index: usize) -> String {
    let wave = if run.wave_in_progress {
        run.wave
    } else if run.is_endless() {
        run.wave + 1
    } else {
        (run.wave + 1).min(run.total_waves)
    };
    if wave <= 0 {
        return crate::i18n::t("侦察：等待关卡载入");
    }
    if !run.is_endless() && run.wave >= run.total_waves && !run.wave_in_progress {
        return crate::i18n::t("侦察：本关威胁已清空");
    }

    if run.is_boss_wave_number(wave) {
        let boss_total = if run.is_endless() {
            (wave + BOSS_WAVE_INTERVAL).max(20)
        } else {
            run.total_waves
        };
        let boss = run
            .pending_boss_species
            .and_then(species_by_id)
            .filter(|s| s.is_boss())
            .or_else(|| {
                let mut candidates = boss_candidates(wave, boss_total, level_index);
                candidates.sort_by_key(|s| wave_threat_score(s, wave, level_index));
                candidates.pop()
            });
        if let Some(boss) = boss {
            let skill = boss_skill(boss.id);
            let resist = crate::monster::resistance_summary(boss.resist_profile());
            let resist_text = if resist.is_empty() {
                crate::i18n::t("抗性：无明显偏向")
            } else {
                crate::i18n::tf(
                    "抗性：{}",
                    &[&resist
                        .into_iter()
                        .take(3)
                        .collect::<Vec<_>>()
                        .join("、")],
                )
            };
            return crate::i18n::tf(
                "侦察：{}第{}波首领 {}\n技能：{} - {}\n特性：{}\n{}\n{}\n本局遭遇：{}种",
                &[
                    &if run.is_endless() { crate::i18n::t("无尽") } else { String::new() },
                    &wave.to_string(),
                    &crate::i18n::t(boss.name),
                    &crate::i18n::t(skill.name()),
                    &crate::i18n::t(skill.description()),
                    &boss.traits(),
                    &resist_text,
                    &recommended_elements(&[boss]),
                    &run.encountered_species.len().to_string(),
                ],
            );
        }
    }

    let mut pool: Vec<&crate::monster::MonsterSpecies> = MONSTER_SPECIES
        .iter()
        .filter(|s| !s.is_boss() && s.available(wave, level_index))
        .collect();
    pool.sort_by(|a, b| {
        wave_threat_score(b, wave, level_index).cmp(&wave_threat_score(a, wave, level_index))
    });
    let featured: Vec<&crate::monster::MonsterSpecies> = pool.iter().take(4).copied().collect();
    let names = if featured.is_empty() {
        crate::i18n::t("未知游荡者")
    } else {
        featured
            .iter()
            .map(|s| crate::i18n::t(s.name))
            .collect::<Vec<_>>()
            .join("、")
    };
    let traits = if featured.is_empty() {
        crate::i18n::t("普通")
    } else {
        let mut tags: Vec<String> = featured.iter().map(|s| s.traits()).collect();
        tags.sort();
        tags.dedup();
        tags.join(" / ")
    };
    crate::i18n::tf(
        "侦察：{}第{}波 {}\n特性：{}\n{}\n{}\n本局遭遇：{}种",
        &[
            &if run.is_endless() { crate::i18n::t("无尽") } else { String::new() },
            &wave.to_string(),
            &names,
            &traits,
            &recommended_elements(&featured),
            &elite_affix_intel(wave, level_index),
            &run.encountered_species.len().to_string(),
        ],
    )
}

fn ascii_bar(frac: f32, width: usize) -> String {
    let filled = (frac.clamp(0.0, 1.0) * width as f32).round() as usize;
    format!("{}{}", "#".repeat(filled), "-".repeat(width - filled))
}

struct BossHudInfo {
    name: &'static str,
    hp_frac: f32,
    shield_frac: Option<f32>,
    skill_state: String,
    skill_frac: f32,
    enraged: bool,
    casting: bool,
    skill: BossSkill,
}

fn active_boss_info(bosses: &Query<(&Enemy, Option<&PendingBossCast>)>) -> Option<BossHudInfo> {
    let (boss, cast) = bosses
        .iter()
        .filter(|(enemy, _)| enemy.boss && enemy.hp > 0.0)
        .max_by(|a, b| a.0.hp.total_cmp(&b.0.hp))?;
    let species = species_by_id(boss.species_id);
    let name = species.map(|s| s.name).unwrap_or("未知首领");
    let hp_frac = if boss.max_hp > 0.0 {
        (boss.hp / boss.max_hp).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let shield_frac = if boss.max_shield > 0.0 && boss.shield > 0.0 {
        Some((boss.shield / boss.max_shield).clamp(0.0, 1.0))
    } else {
        None
    };
    let skill = boss_skill(boss.species_id);
    let (skill_state, skill_frac, casting) = if let Some(cast) = cast {
        let progress = (1.0 - cast.timer / cast.max_timer).clamp(0.0, 1.0);
        (
            crate::i18n::tf(
                "施法中 {} {}%",
                &[&crate::i18n::t(cast.skill.name()), &format!("{:.0}", progress * 100.0)],
            ),
            progress,
            true,
        )
    } else if skill.name().is_empty() {
        (crate::i18n::t("技能：无"), 0.0, false)
    } else {
        let rate = if boss.enraged { 1.55 } else { 1.0 };
        let remain = ((skill.cooldown() - boss.boss_skill_timer).max(0.0) / rate).ceil();
        (
            crate::i18n::tf(
                "{}技能：{} 还需{}s",
                &[
                    &if boss.enraged { crate::i18n::t("狂怒·") } else { String::new() },
                    &crate::i18n::t(skill.name()),
                    &format!("{:.0}", remain),
                ],
            ),
            (boss.boss_skill_timer / skill.cooldown()).clamp(0.0, 1.0),
            false,
        )
    };
    Some(BossHudInfo {
        name,
        hp_frac,
        shield_frac,
        skill_state,
        skill_frac,
        enraged: boss.enraged,
        casting,
        skill,
    })
}

fn active_boss_status(bosses: &Query<(&Enemy, Option<&PendingBossCast>)>) -> Option<String> {
    let info = active_boss_info(bosses)?;
    let shield = info
        .shield_frac
        .map(|frac| crate::i18n::tf(" 护盾{}%", &[&format!("{:.0}", frac * 100.0)]))
        .unwrap_or_default();
    Some(crate::i18n::tf(
        "首领：{}{} [{}] HP{}%{}\n{}",
        &[
            &crate::i18n::t(info.name),
            &if info.enraged { crate::i18n::t(" · 狂怒") } else { String::new() },
            &ascii_bar(info.hp_frac, 12),
            &format!("{:.0}", info.hp_frac * 100.0),
            &shield,
            &info.skill_state,
        ],
    ))
}

fn boss_bar_color(info: &BossHudInfo) -> Color {
    if info.casting {
        Color::srgb(1.0, 0.54, 0.12)
    } else if info.enraged {
        Color::srgb(1.0, 0.13, 0.06)
    } else {
        Color::srgb(0.82, 0.06, 0.10)
    }
}

fn boss_bar_text(info: &BossHudInfo) -> String {
    let shield = info
        .shield_frac
        .map(|frac| crate::i18n::tf(" · 护盾{}%", &[&format!("{:.0}", frac * 100.0)]))
        .unwrap_or_default();
    crate::i18n::tf(
        "{}{} · HP {}%{} · {}",
        &[
            &crate::i18n::t(info.name),
            &if info.enraged { crate::i18n::t(" · 狂怒") } else { String::new() },
            &format!("{:.0}", info.hp_frac * 100.0),
            &shield,
            &info.skill_state,
        ],
    )
}

pub fn update_boss_bar(
    bosses: Query<(&Enemy, Option<&PendingBossCast>)>,
    sprites: Res<Sprites>,
    mut nodes: ParamSet<(
        Query<&mut Node, With<BossBarRoot>>,
        Query<(&mut Node, &mut BackgroundColor), With<BossBarFill>>,
        Query<(&mut Node, &mut BackgroundColor), With<BossSkillFill>>,
    )>,
    mut text: Query<&mut Text, With<BossBarText>>,
    // Must exclude every BossBar*-tagged Node so this `&mut Node` query is
    // provably disjoint from the ParamSet above (else B0001 panic at runtime).
    mut portrait: Query<
        (&mut ImageNode, &mut Node),
        (
            With<BossPortrait>,
            Without<BossBarRoot>,
            Without<BossBarFill>,
            Without<BossSkillFill>,
        ),
    >,
) {
    let boss_info = active_boss_info(&bosses);
    if let Ok(mut node) = nodes.p0().single_mut() {
        node.display = if boss_info.is_some() {
            Display::Flex
        } else {
            Display::None
        };
    }

    let Some(info) = boss_info.as_ref() else {
        return;
    };
    // Swap in this boss's portrait (hide it if the boss has no signature skill).
    if let Ok((mut img, mut pnode)) = portrait.single_mut() {
        if let Some(handle) = sprites.bosses.get(&info.skill) {
            img.image = handle.clone();
            pnode.display = Display::Flex;
        } else {
            pnode.display = Display::None;
        }
    }
    if let Ok((mut node, mut bg)) = nodes.p1().single_mut() {
        node.width = Val::Percent(info.hp_frac * 100.0);
        bg.0 = boss_bar_color(info);
    }
    if let Ok((mut node, mut bg)) = nodes.p2().single_mut() {
        node.width = Val::Percent(info.skill_frac * 100.0);
        bg.0 = if info.casting {
            Color::srgb(1.0, 0.76, 0.24)
        } else {
            Color::srgb(0.96, 0.44, 0.12)
        };
    }
    if let Ok(mut t) = text.single_mut() {
        t.0 = boss_bar_text(info);
    }
}

fn combo_meter_color(combo: i32) -> Color {
    if combo >= 20 {
        Color::srgb(1.0, 0.34, 0.10)
    } else if combo >= 10 {
        Color::srgb(1.0, 0.72, 0.16)
    } else {
        Color::srgb(1.0, 0.84, 0.28)
    }
}

pub fn update_combo_meter(
    run: Res<RunState>,
    mut nodes: ParamSet<(
        Query<&mut Node, With<ComboMeterRoot>>,
        Query<(&mut Node, &mut BackgroundColor), With<ComboMeterFill>>,
    )>,
    mut labels: Query<(&mut Text, &mut TextColor), With<ComboMeterText>>,
) {
    let active = run.kill_combo >= 3 && run.kill_combo_timer > 0.0;
    if let Ok(mut node) = nodes.p0().single_mut() {
        node.display = if active { Display::Flex } else { Display::None };
    }
    if !active {
        return;
    }

    let window = run.kill_combo_window.max(KILL_COMBO_WINDOW);
    let frac = (run.kill_combo_timer / window).clamp(0.0, 1.0);
    let color = combo_meter_color(run.kill_combo);
    if let Ok((mut node, mut bg)) = nodes.p1().single_mut() {
        node.width = Val::Percent(frac * 100.0);
        bg.0 = color;
    }
    if let Ok((mut text, mut text_color)) = labels.single_mut() {
        let next_reward = ((run.kill_combo / 5) + 1) * 5;
        let reward_hint = if run.kill_combo % 5 == 0 {
            crate::i18n::t("奖励已触发")
        } else {
            crate::i18n::tf("距奖励 x{}", &[&next_reward.to_string()])
        };
        text.0 = crate::i18n::tf(
            "连杀 x{} · {}s · {}",
            &[
                &run.kill_combo.to_string(),
                &format!("{:.1}", run.kill_combo_timer.max(0.0)),
                &reward_hint,
            ],
        );
        text_color.0 = color;
    }
}

fn element_matchup_text(element: crate::data::Element) -> String {
    let mut weak: Vec<(&str, f32)> = MONSTER_SPECIES
        .iter()
        .map(|s| (s.name, s.resist_profile().get(element)))
        .filter(|(_, resist)| *resist <= -0.15)
        .collect();
    weak.sort_by(|a, b| a.1.total_cmp(&b.1));

    let mut resist: Vec<(&str, f32)> = MONSTER_SPECIES
        .iter()
        .map(|s| (s.name, s.resist_profile().get(element)))
        .filter(|(_, resist)| *resist >= 0.20)
        .collect();
    resist.sort_by(|a, b| b.1.total_cmp(&a.1));

    let weak_names: Vec<String> = weak.iter().take(3).map(|(name, _)| crate::i18n::t(name)).collect();
    let resist_names: Vec<String> = resist.iter().take(3).map(|(name, _)| crate::i18n::t(name)).collect();
    let weak_text = if weak_names.is_empty() {
        crate::i18n::t("暂无明显易伤目标")
    } else {
        crate::i18n::tf("克制：{}", &[&weak_names.join("、")])
    };
    let resist_text = if resist_names.is_empty() {
        crate::i18n::t("少见强抗性")
    } else {
        crate::i18n::tf("慎打：{}", &[&resist_names.join("、")])
    };
    crate::i18n::tf(
        "元素：{} [{}]\n{}；{}",
        &[
            &crate::i18n::t(element.name()),
            &crate::i18n::t(element_marker(element)),
            &weak_text,
            &resist_text,
        ],
    )
}

pub fn update_hud(
    run: Res<RunState>,
    current: Res<CurrentLevel>,
    snap: Res<crate::tower::Snapshot>,
    sel: Res<Selection>,
    inv: Res<EquipmentInventory>,
    hero: Res<HeroLoadout>,
    towers: Query<&crate::tower::Tower>,
    bosses: Query<(&Enemy, Option<&PendingBossCast>)>,
    mut texts: ParamSet<(
        Query<&mut Text, With<GoldText>>,
        Query<&mut Text, With<LivesText>>,
        Query<&mut Text, With<WaveText>>,
        Query<&mut Text, With<SpeedText>>,
        Query<&mut Text, With<BannerText>>,
        Query<&mut Text, With<SelInfoText>>,
        Query<&mut Text, With<InvText>>,
        Query<&mut Text, With<WaveIntelText>>,
    )>,
) {
    // Gold/lives/wave may appear twice (panel + fixed mobile status bar), so update
    // every matching label, not just one.
    let gold_str = format!("{}", run.gold);
    for mut t in &mut texts.p0() {
        t.0 = gold_str.clone();
    }
    let lives_str = format!("{}", run.lives);
    for mut t in &mut texts.p1() {
        t.0 = lives_str.clone();
    }
    {
        let boss_hint = if run.wave_in_progress && run.is_boss_wave_number(run.wave) {
            if let Some(boss) = run.pending_boss_species.and_then(species_by_id) {
                crate::i18n::tf(" · {}", &[&crate::i18n::t(boss.name)])
            } else {
                crate::i18n::t(" · 首领")
            }
        } else if let Some(next_boss) = run.next_boss_wave_after(run.wave) {
            crate::i18n::tf(" · 首领{}", &[&next_boss.to_string()])
        } else {
            String::new()
        };
        let auto_hint = if run.auto_wave && run.can_start_next_wave() {
            crate::i18n::tf(" · 自动{}s", &[&format!("{:.0}", run.auto_wave_timer.max(0.0).ceil())])
        } else if run.auto_wave {
            crate::i18n::t(" · 自动")
        } else {
            String::new()
        };
        let wave_str = if run.is_endless() {
            crate::i18n::tf("第{}波{}{}", &[&run.wave.to_string(), &boss_hint, &auto_hint])
        } else {
            format!("{}/{}{}{}", run.wave, run.total_waves, boss_hint, auto_hint)
        };
        for mut t in &mut texts.p2() {
            t.0 = wave_str.clone();
        }
    }
    if let Ok(mut t) = texts.p3().single_mut() {
        t.0 = format!("x{}", run.game_speed as i32);
    }
    if let Ok(mut t) = texts.p4().single_mut() {
        t.0 = if run.message_timer > 0.0 {
            run.message.clone()
        } else {
            String::new()
        };
    }
    if let Ok(mut t) = texts.p5().single_mut() {
        t.0 = match sel.selected.and_then(|e| towers.get(e).ok()) {
            Some(tw) if tw.hero => {
                let skill = if hero.skill_cd > 0 {
                    crate::i18n::tf(
                        "{} 冷却{}波",
                        &[&crate::i18n::t(hero.class.skill_name()), &hero.skill_cd.to_string()],
                    )
                } else {
                    crate::i18n::tf("{} 就绪", &[&crate::i18n::t(hero.class.skill_name())])
                };
                crate::i18n::tf(
                    "英雄 {}·{} Lv{}  装备 {}/3  HP {}/{}\n{}  击杀 {}  {}",
                    &[
                        &crate::i18n::t(hero.race.name()),
                        &crate::i18n::t(hero.class.name()),
                        &hero.level.to_string(),
                        &tw.equipment_count().to_string(),
                        &(tw.hp.max(0.0) as i32).to_string(),
                        &(tw.max_hp as i32).to_string(),
                        &skill,
                        &tw.kills.to_string(),
                        &equipment_set_bonus_summary(&tw.equipment),
                    ],
                )
            }
            Some(tw) => {
                let silence = if snap.tower_silenced(tw.center()) {
                    crate::i18n::t("  静默中")
                } else {
                    String::new()
                };
                crate::i18n::tf(
                    "{} Lv{}/3  装备 {}/3  {}{}  目标:{}\nHP {}/{}  穿甲 {}  击杀 {}  修理{}  升级{}{}  {}",
                    &[
                        &crate::i18n::t(tw.kind.def().name),
                        &tw.level.to_string(),
                        &tw.equipment_count().to_string(),
                        &crate::i18n::t(tw.element.name()),
                        &silence,
                        &crate::i18n::t(tw.target_priority.label()),
                        &(tw.hp.max(0.0) as i32).to_string(),
                        &(tw.max_hp as i32).to_string(),
                        &format!("{:.0}", tw.armor_pierce),
                        &tw.kills.to_string(),
                        &tw.repair_cost().to_string(),
                        &if tw.level >= 3 { 0 } else { tw.upgrade_cost() }.to_string(),
                        &if tw.synergy > 0.0 {
                            crate::i18n::tf("  协同+{}%", &[&((tw.synergy * 100.0) as i32).to_string()])
                        } else {
                            String::new()
                        },
                        &equipment_set_bonus_summary(&tw.equipment),
                    ],
                )
            }
            None => crate::i18n::t("未选择单位\n点击英雄或防御塔查看属性，并为其装配右下方装备"),
        };
    }
    if let Ok(mut t) = texts.p6().single_mut() {
        t.0 = crate::i18n::tf("总数 {}", &[&inv.total().to_string()]);
    }
    if let Ok(mut t) = texts.p7().single_mut() {
        let intel = wave_intel_text(&run, current.0);
        t.0 = if let Some(boss) = active_boss_status(&bosses) {
            format!("{}\n{}", boss, intel)
        } else {
            intel
        };
    }
}

/// Fill the selected-unit icon stat-strip. Kept separate from `update_hud` because
/// that system's ParamSet is already at the ~8-query limit; this owns its own query.
pub fn update_unit_stats(
    sel: Res<Selection>,
    towers: Query<&crate::tower::Tower>,
    mut stats: Query<(&UnitStat, &mut Text)>,
) {
    let tw = sel.selected.and_then(|e| towers.get(e).ok());
    for (stat, mut text) in &mut stats {
        text.0 = match tw {
            Some(tw) => {
                let aps = if tw.cooldown > 0.0 { 1.0 / tw.cooldown } else { 0.0 };
                let armor = tw.armor + equipment_set_bonus(&tw.equipment).armor_add;
                match stat {
                    UnitStat::Damage => format!("{}", tw.damage as i32),
                    UnitStat::Range => format!("{}", tw.range as i32),
                    UnitStat::Armor => format!("{:.0}", armor),
                    UnitStat::Speed => format!("{:.2}", aps),
                    UnitStat::Dps => format!("{:.0}", tw.damage * aps),
                }
            }
            None => "—".to_string(),
        };
    }
}

pub fn update_hero_info(hero: Res<HeroLoadout>, mut info: Query<&mut Text, With<HeroInfoText>>) {
    if let Ok(mut t) = info.single_mut() {
        let skill = if hero.skill_cd > 0 {
            crate::i18n::tf(
                "{} 冷却{}波",
                &[&crate::i18n::t(hero.class.skill_name()), &hero.skill_cd.to_string()],
            )
        } else {
            crate::i18n::tf("{} 就绪", &[&crate::i18n::t(hero.class.skill_name())])
        };
        let doc = hero.class.doctrine();
        let ult = if hero.level >= crate::hero::HeroLoadout::MAX_LEVEL {
            crate::i18n::tf("  终极·{}✓", &[&crate::i18n::t(hero.class.ultimate_name())])
        } else {
            crate::i18n::tf("  终极·{}(30级)", &[&crate::i18n::t(hero.class.ultimate_name())])
        };
        t.0 = crate::i18n::tf(
            "英雄 {}·{} Lv{}  XP {}/{}  点数 {}\n天赋【{}】{}{}\n技能：{}  本职业已投 {}",
            &[
                &crate::i18n::t(hero.race.name()),
                &crate::i18n::t(hero.class.name()),
                &hero.level.to_string(),
                &hero.xp.to_string(),
                &hero.xp_to_next().to_string(),
                &hero.talent_points.to_string(),
                &crate::i18n::t(doc.name),
                &crate::i18n::t(doc.desc),
                &ult,
                &skill,
                &hero.spent_in_current_class().to_string(),
            ],
        );
    }
}

pub fn update_equipment_button_labels(
    inv: Res<EquipmentInventory>,
    mut labels: Query<(&EquipmentButtonText, &mut Text, &mut TextColor)>,
    mut icons: Query<(&EquipmentButtonIcon, &mut ImageNode)>,
) {
    for (label, mut text, mut color) in &mut labels {
        let count = inv.counts[label.item.idx()];
        text.0 = format!("{}×{}", crate::i18n::t(label.item.short()), count);
        color.0 = if count > 0 {
            Color::WHITE
        } else {
            Color::srgb(0.55, 0.55, 0.55)
        };
    }
    for (icon, mut image) in &mut icons {
        let count = inv.counts[icon.item.idx()];
        image.color = if count > 0 {
            Color::WHITE
        } else {
            Color::srgba(0.55, 0.55, 0.55, 0.65)
        };
    }
}

/// Show the live upgrade cost on the 升级 button (e.g. "升级 50"), or "满级" when
/// the selected tower is maxed / "升级" when nothing upgradeable is selected.
pub fn update_upgrade_button_label(
    sel: Res<Selection>,
    towers: Query<&crate::tower::Tower>,
    mut labels: Query<&mut Text, With<UpgradeBtnText>>,
) {
    let text = match sel.selected.and_then(|e| towers.get(e).ok()) {
        Some(t) if t.hero => crate::i18n::t("升级"),
        Some(t) if t.level >= 3 => crate::i18n::t("满级"),
        Some(t) => crate::i18n::tf("升级 {}", &[&t.upgrade_cost().to_string()]),
        None => crate::i18n::t("升级"),
    };
    for mut label in &mut labels {
        if label.0 != text {
            label.0 = text.clone();
        }
    }
}

pub fn update_equipped_slot_icons(
    sel: Res<Selection>,
    sprites: Res<Sprites>,
    towers: Query<&crate::tower::Tower>,
    mut frames: Query<(&EquippedSlotFrame, &mut BackgroundColor)>,
    mut icons: Query<(&EquippedSlotIcon, &mut ImageNode)>,
    mut labels: Query<(&EquippedSlotText, &mut Text, &mut TextColor)>,
) {
    let slots = sel
        .selected
        .and_then(|entity| towers.get(entity).ok().map(|tower| tower.equipment))
        .unwrap_or([None, None, None]);

    for (frame, mut bg) in &mut frames {
        bg.0 = match slots.get(frame.slot).copied().flatten() {
            Some(item) => item.def().rarity.color().with_alpha(0.20),
            None => Color::srgba(1.0, 1.0, 1.0, 0.04),
        };
    }
    for (icon, mut image) in &mut icons {
        if let Some(item) = slots.get(icon.slot).copied().flatten() {
            image.image = sprites.equipment[&item].clone();
            image.color = Color::WHITE;
        } else {
            image.image = sprites.equipment[&Equipment::RustySight].clone();
            image.color = Color::srgba(1.0, 1.0, 1.0, 0.0);
        }
    }
    for (label, mut text, mut color) in &mut labels {
        if let Some(item) = slots.get(label.slot).copied().flatten() {
            let def = item.def();
            text.0 = crate::i18n::t(def.short);
            color.0 = def.rarity.color();
        } else {
            text.0 = crate::i18n::t("空");
            color.0 = Color::srgb(0.45, 0.45, 0.45);
        }
    }
}

pub fn hud_buttons(
    mut commands: Commands,
    mut interactions: Query<(&Interaction, &UiAction, &mut BackgroundColor), Changed<Interaction>>,
    mut run: ResMut<RunState>,
    current: Res<CurrentLevel>,
    mut rng: ResMut<Rng>,
    mut sel: ResMut<Selection>,
    mut paused: ResMut<crate::game::Paused>,
    mut towers: Query<(Entity, &mut crate::tower::Tower)>,
    mut windows: Query<&mut Window>,
    mut talents: ResMut<Talents>,
    mut abilities: ResMut<Abilities>,
    mut inv: ResMut<EquipmentInventory>,
    mut quality: ResMut<GraphicsQuality>,
    // Bundled into one tuple param to stay within Bevy's 16-param system limit.
    mut confirm_state: (
        Res<TouchMode>,
        ResMut<TalentConfirm>,
        Res<HeroLoadout>,
        ResMut<HudPanels>,
    ),
    mut sfx: MessageWriter<crate::audio::SfxEvent>,
    mut vfx: MessageWriter<crate::vfx::VfxEvent>,
) {
    use crate::audio::Sound;
    for (interaction, action, mut bg) in &mut interactions {
        match *interaction {
            Interaction::Pressed => {
                bg.0 = bg.0.with_alpha(1.0);
                sfx.write(crate::audio::SfxEvent(Sound::Click));
                // Touch: costly/irreversible actions (talents, equip, refine, sell)
                // need a confirming second tap so a stray tap never spends gold or
                // sells a tower. The first tap also pins the tooltip.
                if confirm_state.0 .0 {
                    if let Some(id) = confirm_id(action) {
                        if confirm_state.1.pending != Some(id) {
                            confirm_state.1.pending = Some(id);
                            confirm_state.1.timer = 3.0;
                            run.show(crate::i18n::t(confirm_hint(id)));
                            continue;
                        }
                        confirm_state.1.pending = None; // confirmed → fall through
                    }
                }
                match action {
                    // Build arming + drag-to-place is owned by `mouse_build` (it
                    // reads the pressed Build icon directly), so nothing here.
                    UiAction::Build(_) => {}
                    UiAction::StartWave => {
                        start_wave(&mut run, current.0, &mut rng);
                        sfx.write(crate::audio::SfxEvent(Sound::Wave));
                    }
                    UiAction::ToggleAutoWave => toggle_auto_wave(&mut run),
                    UiAction::ToggleDock => {
                        confirm_state.3.dock_open = !confirm_state.3.dock_open;
                    }
                    UiAction::ToggleSettings => {
                        confirm_state.3.settings_open = !confirm_state.3.settings_open;
                    }
                    UiAction::CycleQuality => {
                        quality.cycle();
                        run.show(crate::i18n::tf("画质：{}", &[&crate::i18n::t(quality.level.name())]));
                    }
                    UiAction::TogglePause => paused.0 = !paused.0,
                    UiAction::CycleSpeed => {
                        run.game_speed = match run.game_speed as i32 {
                            1 => 2.0,
                            2 => 4.0,
                            4 => 8.0,
                            _ => 1.0,
                        };
                    }
                    UiAction::Upgrade => {
                        if let Some(e) = sel.selected {
                            if let Ok((_, mut t)) = towers.get_mut(e) {
                                if t.hero {
                                    run.show(crate::i18n::t("英雄通过经验升级；请使用英雄天赋点强化"));
                                    continue;
                                }
                                let cost = t.upgrade_cost();
                                if t.level < 3 && run.gold >= cost {
                                    run.gold -= cost;
                                    upgrade_tower(&mut t);
                                    sfx.write(crate::audio::SfxEvent(Sound::Upgrade));
                                    // Golden upgrade burst, sized to the footprint.
                                    vfx.write(crate::vfx::VfxEvent::Burst {
                                        pos: t.center(),
                                        radius: TILE_SIZE * t.footprint as f32 * 0.66,
                                        color: Color::srgb(1.0, 0.85, 0.36),
                                    });
                                    if let Some(note) = upgrade_unlock_note(t.kind, t.level) {
                                        run.show(crate::i18n::t(note));
                                    }
                                }
                            }
                        }
                    }
                    UiAction::Repair => {
                        if let Some(e) = sel.selected {
                            if let Ok((_, mut t)) = towers.get_mut(e) {
                                repair_tower(&mut t, &mut run);
                            }
                        } else {
                            run.show(crate::i18n::t("先选中一座塔"));
                        }
                    }
                    UiAction::CycleTargetPriority => {
                        if let Some(e) = sel.selected {
                            if let Ok((_, mut t)) = towers.get_mut(e) {
                                let priority = t.cycle_target_priority();
                                run.show(crate::i18n::tf(
                                    "目标优先：{} - {}",
                                    &[
                                        &crate::i18n::t(priority.label()),
                                        &crate::i18n::t(priority.description()),
                                    ],
                                ));
                            } else {
                                run.show(crate::i18n::t("先选中一座塔"));
                            }
                        } else {
                            run.show(crate::i18n::t("先选中一座塔"));
                        }
                    }
                    UiAction::Unequip => {
                        if let Some(e) = sel.selected {
                            if let Ok((_, mut t)) = towers.get_mut(e) {
                                let returned = unequip_all_to_inventory(&mut inv, &mut t);
                                if returned > 0 && t.hero {
                                    crate::hero::apply_loadout_to_tower(&confirm_state.2, &mut t);
                                }
                                if returned > 0 {
                                    run.show(crate::i18n::tf("卸下装备 {} 件", &[&returned.to_string()]));
                                } else {
                                    run.show(crate::i18n::t("没有可卸下装备"));
                                }
                            } else {
                                run.show(crate::i18n::t("先选中一座塔"));
                            }
                        } else {
                            run.show(crate::i18n::t("先选中一座塔"));
                        }
                    }
                    UiAction::UnequipSlot(slot) => {
                        if let Some(e) = sel.selected {
                            if let Ok((_, mut t)) = towers.get_mut(e) {
                                if let Some(item) =
                                    unequip_slot_to_inventory(&mut inv, &mut t, *slot)
                                {
                                    if t.hero {
                                        crate::hero::apply_loadout_to_tower(
                                            &confirm_state.2,
                                            &mut t,
                                        );
                                    }
                                    run.show(crate::i18n::tf("卸下 {}", &[&crate::i18n::t(item.def().name)]));
                                } else {
                                    run.show(crate::i18n::t("该装备槽为空"));
                                }
                            } else {
                                run.show(crate::i18n::t("先选中一座塔"));
                            }
                        } else {
                            run.show(crate::i18n::t("先选中一座塔"));
                        }
                    }
                    UiAction::Sell => {
                        if let Some(e) = sel.selected {
                            if let Ok((ent, t)) = towers.get(e) {
                                if t.hero {
                                    run.show(crate::i18n::t("英雄不能出售；阵亡后会自动进入重生冷却"));
                                    continue;
                                }
                                let refund = t.refund();
                                let returned = return_equipment_to_inventory(&mut inv, t);
                                run.gold += refund;
                                vfx.write(crate::vfx::VfxEvent::Burst {
                                    pos: t.center(),
                                    radius: TILE_SIZE * t.footprint as f32 * 0.6,
                                    color: Color::srgb(0.95, 0.78, 0.3),
                                });
                                commands.entity(ent).despawn();
                                sel.selected = None;
                                sfx.write(crate::audio::SfxEvent(Sound::Sell));
                                if returned > 0 {
                                    run.show(crate::i18n::tf("出售 +{}，返还装备 {} 件", &[&refund.to_string(), &returned.to_string()]));
                                }
                            }
                        }
                    }
                    UiAction::Fullscreen => {
                        if let Ok(mut win) = windows.single_mut() {
                            win.mode = match win.mode {
                                WindowMode::Windowed => {
                                    WindowMode::BorderlessFullscreen(MonitorSelection::Current)
                                }
                                _ => WindowMode::Windowed,
                            };
                        }
                    }
                    UiAction::TalentDamage => {
                        let cost = talent_cost(talents.dmg_lvl);
                        if run.gold >= cost {
                            run.gold -= cost;
                            talents.dmg_lvl += 1;
                            talents.damage_mult *= 1.15;
                            for (_, mut t) in &mut towers {
                                t.base_damage = (t.base_damage * 1.15).floor();
                                t.damage = t.base_damage;
                            }
                            run.show(crate::i18n::t("全体攻击强化！"));
                        } else {
                            run.show(crate::i18n::t("金币不足"));
                        }
                    }
                    UiAction::TalentRange => {
                        let cost = talent_cost(talents.rng_lvl);
                        if run.gold >= cost {
                            run.gold -= cost;
                            talents.rng_lvl += 1;
                            talents.range_mult *= 1.12;
                            for (_, mut t) in &mut towers {
                                t.range = (t.range * 1.12).floor();
                            }
                            run.show(crate::i18n::t("全体射程强化！"));
                        } else {
                            run.show(crate::i18n::t("金币不足"));
                        }
                    }
                    UiAction::TalentSpeed => {
                        let cost = talent_cost(talents.spd_lvl);
                        if run.gold >= cost {
                            run.gold -= cost;
                            talents.spd_lvl += 1;
                            talents.firerate_mult *= 0.9;
                            for (_, mut t) in &mut towers {
                                t.cooldown = (t.cooldown * 0.9).max(0.03);
                            }
                            run.show(crate::i18n::t("全体攻速强化！"));
                        } else {
                            run.show(crate::i18n::t("金币不足"));
                        }
                    }
                    UiAction::Cast(a) => {
                        abilities.pending = Some(*a);
                    }
                    UiAction::Equip(item) => {
                        if let Some(e) = sel.selected {
                            if let Ok((_, mut t)) = towers.get_mut(e) {
                                let def = item.def();
                                if t.equipment_count() >= 3 {
                                    run.show(crate::i18n::t("装备槽已满"));
                                } else if inv.take(*item) {
                                    crate::equipment::equip_into(&mut t, *item);
                                    run.show(crate::i18n::tf("装配 {}！", &[&crate::i18n::t(def.name)]));
                                } else {
                                    run.show(crate::i18n::tf("没有{}", &[&crate::i18n::t(def.name)]));
                                }
                            } else {
                                run.show(crate::i18n::t("先选中一座塔"));
                            }
                        } else {
                            run.show(crate::i18n::t("先选中一座塔"));
                        }
                    }
                    _ => {}
                }
            }
            Interaction::Hovered => bg.0 = bg.0.with_alpha(1.0),
            Interaction::None => bg.0 = bg.0.with_alpha(0.85),
        }
    }
}

/// Show a tooltip with full stats when hovering a build button.
/// Keeps a tapped tooltip on screen for a few seconds on touch devices (which have
/// no hover). Counts down in `tooltip_system`.
#[derive(Resource, Default)]
pub struct TooltipHold(pub f32);

pub fn tooltip_system(
    time: Res<Time>,
    buttons: Query<(&Interaction, &UiAction)>,
    talents: Res<Talents>,
    abilities: Res<Abilities>,
    hero: Res<HeroLoadout>,
    levels: Res<Levels>,
    mut hold: ResMut<TooltipHold>,
    mut box_q: Query<&mut Node, With<TooltipBox>>,
    mut txt_q: Query<&mut Text, With<TooltipText>>,
) {
    let pressed = buttons
        .iter()
        .find(|(i, _)| **i == Interaction::Pressed)
        .map(|(_, a)| a.clone());
    let hovered = buttons
        .iter()
        .find(|(i, _)| **i == Interaction::Hovered)
        .map(|(_, a)| a.clone());
    let (Ok(mut node), Ok(mut text)) = (box_q.single_mut(), txt_q.single_mut()) else {
        return;
    };

    // A tap (Pressed) pins the tooltip for a few seconds so touch users can read
    // it; hover (desktop) shows it instantly and dismisses as soon as it ends.
    if let Some(s) = pressed
        .as_ref()
        .and_then(|a| tooltip_text(a, &talents, &abilities, &hero, &levels))
    {
        text.0 = s;
        node.display = Display::Flex;
        hold.0 = 3.5;
        return;
    }
    if let Some(s) = hovered
        .as_ref()
        .and_then(|a| tooltip_text(a, &talents, &abilities, &hero, &levels))
    {
        text.0 = s;
        node.display = Display::Flex;
        hold.0 = 0.0;
        return;
    }
    if hold.0 > 0.0 {
        hold.0 = (hold.0 - time.delta_secs()).max(0.0);
        node.display = Display::Flex;
    } else {
        node.display = Display::None;
    }
}

/// Build the tooltip text for a hovered action (towers, abilities, talents, equipment).
fn tooltip_text(
    a: &UiAction,
    talents: &Talents,
    abilities: &Abilities,
    hero: &HeroLoadout,
    levels: &Levels,
) -> Option<String> {
    // Info icons carry their tooltip text directly.
    if let UiAction::Info(s) = a {
        return Some(crate::i18n::t(s));
    }
    // For an ability, append its live cooldown status.
    if let UiAction::Cast(ab) = a {
        let cd = abilities.cd(*ab);
        let (name, base) = match ab {
            Ability::Meteor => (
                "陨石",
                crate::i18n::tf(
                    "花费{}金 · 冷却{}回合\n轰炸血量最高的敌人及周围",
                    &[&Abilities::METEOR_COST.to_string(), &Abilities::METEOR_MAX.to_string()],
                ),
            ),
            Ability::Freeze => (
                "冰封",
                crate::i18n::tf(
                    "花费{}金 · 冷却{}回合\n全场敌人冻结 2.5 秒",
                    &[&Abilities::FREEZE_COST.to_string(), &Abilities::FREEZE_MAX.to_string()],
                ),
            ),
            Ability::GoldRush => (
                "金币潮",
                crate::i18n::tf(
                    "献祭1生命 · 冷却{}回合\n立即获得 120 金币",
                    &[&Abilities::GOLD_MAX.to_string()],
                ),
            ),
        };
        let status = if cd > 0 {
            crate::i18n::tf("\n[冷却中] 还需 {} 回合", &[&cd.to_string()])
        } else {
            crate::i18n::t("\n[就绪] 可释放")
        };
        return Some(crate::i18n::tf("{} · {}{}", &[&crate::i18n::t(name), &base, &status]));
    }
    Some(match a {
        UiAction::Build(kind) => {
            let d = kind.def();
            let fp = if d.footprint > 1 {
                crate::i18n::tf("  [{} 大型]", &[&format!("{0}×{0}", d.footprint)])
            } else {
                String::new()
            };
            crate::i18n::tf(
                "{}  花费 {}{}\n伤害 {}   射程 {}\n攻速 {}/秒\n{}\n{}",
                &[
                    &crate::i18n::t(d.name),
                    &d.cost.to_string(),
                    &fp,
                    &(d.damage as i32).to_string(),
                    &(d.range as i32).to_string(),
                    &format!("{:.2}", 1000.0 / d.cooldown_ms.max(1.0)),
                    &crate::i18n::t(d.desc),
                    &element_matchup_text(d.element),
                ],
            )
        }
        UiAction::TalentDamage => crate::i18n::tf(
            "攻击强化 · 花费{}金\n全体防御塔 +15% 伤害（永久，含新建）",
            &[&talent_cost(talents.dmg_lvl).to_string()],
        ),
        UiAction::TalentRange => crate::i18n::tf(
            "射程强化 · 花费{}金\n全体防御塔 +12% 射程（永久）",
            &[&talent_cost(talents.rng_lvl).to_string()],
        ),
        UiAction::TalentSpeed => crate::i18n::tf(
            "攻速强化 · 花费{}金\n全体防御塔 -10% 冷却（永久）",
            &[&talent_cost(talents.spd_lvl).to_string()],
        ),
        UiAction::Equip(item) => {
            let d = item.def();
            let element = d
                .element
                .map(|e| crate::i18n::tf(
                    "\n属性转化：{}\n{}",
                    &[&crate::i18n::t(e.name()), &element_matchup_text(e)],
                ))
                .unwrap_or_default();
            crate::i18n::tf(
                "{} · {}\n{}\n{}\n伤害×{} 射程×{} 冷却×{}\n穿甲+{} HP×{} 护甲+{}{}\n{}\n{}\n每塔最多 3 件，需有库存",
                &[
                    &d.rarity.label(),
                    &crate::i18n::t(d.name),
                    &crate::i18n::t(d.desc),
                    &crate::i18n::t(equipment_visual_line(*item)),
                    &format!("{:.2}", d.damage_mult),
                    &format!("{:.2}", d.range_mult),
                    &format!("{:.2}", d.cooldown_mult),
                    &format!("{:.0}", d.armor_pierce),
                    &format!("{:.2}", d.hp_mult),
                    &format!("{:.0}", d.armor_add),
                    &element,
                    &crate::equipment::recommend_text(d),
                    &crate::i18n::t(drop_source_hint(d.rarity)),
                ],
            )
        }
        UiAction::Upgrade => crate::i18n::t("升级选中的防御塔（U键）：提升伤害/射程，加快攻速"),
        UiAction::Repair => crate::i18n::t("修理选中的防御塔（R键）：花费金币恢复到满血"),
        UiAction::CycleTargetPriority => {
            crate::i18n::t("切换选中防御塔的目标优先级（T键）：近身/前锋/强者/残血/威胁")
        }
        UiAction::Unequip => crate::i18n::t("卸下选中防御塔的全部装备（Z键）：装备返还库存并移除加成"),
        UiAction::UnequipSlot(_) => crate::i18n::t("卸下该槽位装备：返还库存并重新计算属性与共鸣"),
        UiAction::Sell => crate::i18n::t("出售选中的防御塔（X键），返还部分金币和已装配装备"),
        UiAction::StartWave => crate::i18n::t("开始下一波敌人（空格键）"),
        UiAction::ToggleAutoWave => crate::i18n::t("自动下一波（A键）：波间保留短暂准备倒计时"),
        UiAction::CycleQuality => {
            crate::i18n::t("切换画质：流畅(无抗锯齿)/标准(2×)/精细(4×)。分辨率自适应，手机卡顿就调低画质")
        }
        UiAction::TogglePause => crate::i18n::t("暂停 / 继续（P键）"),
        UiAction::CycleSpeed => crate::i18n::t("切换游戏速度 1× / 2× / 3×（F键）"),
        UiAction::Fullscreen => crate::i18n::t("切换全屏显示"),
        UiAction::OpenBestiary => crate::i18n::t("打开怪物图鉴"),
        UiAction::SummonHero => {
            crate::i18n::tf(
                "英雄开局自动登场（免费）。此键可在阵亡后立即重生。\n{}·{} Lv{}：{}\n左键选中英雄，右键命令它移动（触屏点地面移动）",
                &[
                    &crate::i18n::t(hero.race.name()),
                    &crate::i18n::t(hero.class.name()),
                    &hero.level.to_string(),
                    &crate::i18n::t(hero.class.blurb()),
                ],
            )
        }
        UiAction::HeroSkill => {
            let cd = if hero.skill_cd > 0 {
                crate::i18n::tf("冷却中，还需 {} 波", &[&hero.skill_cd.to_string()])
            } else {
                crate::i18n::tf("就绪，释放后冷却 {} 波", &[&hero.skill_cooldown_max().to_string()])
            };
            crate::i18n::tf(
                "{} · {}\n{}\n{}",
                &[
                    &crate::i18n::t(hero.class.name()),
                    &crate::i18n::t(hero.class.skill_name()),
                    &crate::i18n::t(hero.class.skill_desc()),
                    &cd,
                ],
            )
        }
        UiAction::HeroTalent(index) => {
            let rank = hero.talent_rank(*index);
            crate::i18n::tf(
                "{} {}/{}\n{}\n可用天赋点：{}",
                &[
                    &crate::i18n::t(hero.class.talent_name(*index)),
                    &rank.to_string(),
                    &crate::hero::HeroLoadout::TALENT_MAX_RANK.to_string(),
                    &crate::i18n::t(hero.class.talent_desc(*index)),
                    &hero.talent_points.to_string(),
                ],
            )
        }
        UiAction::ResetHeroTalents => crate::i18n::tf(
            "重置{}天赋\n返还当前职业已投入的 {} 点，不影响英雄等级和其他职业",
            &[&crate::i18n::t(hero.class.name()), &hero.spent_in_current_class().to_string()],
        ),
        UiAction::SetDifficulty(d) => match d {
            Difficulty::Easy => crate::i18n::t("难度：简单（出怪少、金币多）"),
            Difficulty::Normal => crate::i18n::t("难度：普通（标准平衡）"),
            Difficulty::Hard => crate::i18n::t("难度：噩梦（出怪强、奖励高）"),
        },
        UiAction::SelectHeroClass(c) => {
            let doc = c.doctrine();
            crate::i18n::tf(
                "{} · {}\n◆ 天赋【{}】{}\n◆ 技能·{}：{}",
                &[
                    &crate::i18n::t(c.name()),
                    &crate::i18n::t(c.role()),
                    &crate::i18n::t(doc.name),
                    &crate::i18n::t(doc.desc),
                    &crate::i18n::t(c.skill_name()),
                    &crate::i18n::t(c.skill_desc()),
                ],
            )
        }
        UiAction::SelectHeroRace(r) => crate::i18n::tf(
            "种族 · {}\n{}\n三族属性不同，出击前可更换",
            &[&crate::i18n::t(r.name()), &crate::i18n::t(r.blurb())],
        ),
        // --- navigation / screen buttons ---
        UiAction::PlayLevel(i) => {
            let lore = LEVEL_LORE.get(*i).copied().unwrap_or("");
            match levels.0.get(*i) {
                Some(lvl) => crate::i18n::tf(
                    "第{}关 · {}\n{}波\n{}\n点击查看简报并出击",
                    &[
                        &format!("{:02}", i + 1),
                        &crate::i18n::t(lvl.name),
                        &lvl.waves.to_string(),
                        &crate::i18n::t(lore),
                    ],
                ),
                None => crate::i18n::t("查看关卡简报、选择英雄职业与种族后出击"),
            }
        }
        UiAction::PlayEndless => crate::i18n::t("无尽模式：无限波次，每 5 波一个首领，刷装备的核心模式"),
        UiAction::BeginMission => crate::i18n::t("出击！进入这一关开始战斗"),
        UiAction::Restart => crate::i18n::t("重新开始本关（金币/防御塔/进度重置）"),
        UiAction::NextLevel => crate::i18n::t("进入下一关"),
        UiAction::ToMenu => crate::i18n::t("返回主菜单（战术指挥室）"),
        UiAction::OpenArmory => crate::i18n::t("打开装备库：查看已获得的装备与套装"),
        UiAction::OpenTowerArchive => crate::i18n::t("打开防御塔档案：查看全部塔的属性与机制"),
        UiAction::OpenMilestones => crate::i18n::t("查看封印成就与解锁进度"),
        UiAction::OpenCampaignDossier => crate::i18n::t("查看战役档案：剧情与首领情报"),
        UiAction::OpenHeroCodex => crate::i18n::t("打开英雄图鉴：浏览职业×种族，查看天赋/技能/终极并选择出战英雄"),
        UiAction::RefineEquipment(_) => crate::i18n::t("精炼：消耗重复装备，合成更高品质"),
        UiAction::ToggleDock => crate::i18n::t("打开 / 收起英雄面板（属性 · 装备 · 天赋）"),
        UiAction::ToggleSettings => {
            crate::i18n::t("打开 / 收起设置（画质 · 全屏 · 难度 · 重新开始 · 返回主页）")
        }
        _ => return None,
    })
}

fn ability_color(a: Ability) -> Color {
    match a {
        Ability::Meteor => Color::srgb(0.6, 0.3, 0.2),
        Ability::Freeze => Color::srgb(0.2, 0.45, 0.6),
        Ability::GoldRush => Color::srgb(0.5, 0.45, 0.15),
    }
}

/// Grey out ability buttons while on cooldown; restore their color when ready.
pub fn update_ability_buttons(
    abilities: Res<Abilities>,
    hero: Res<HeroLoadout>,
    mut q: Query<(&UiAction, &mut BackgroundColor)>,
) {
    for (a, mut bg) in &mut q {
        if let UiAction::Cast(ab) = a {
            bg.0 = if abilities.cd(*ab) > 0 {
                Color::srgb(0.16, 0.16, 0.18)
            } else {
                ability_color(*ab)
            };
        } else if matches!(a, UiAction::HeroSkill) {
            bg.0 = if hero.skill_cd > 0 {
                Color::srgb(0.16, 0.16, 0.18)
            } else {
                hero.class.skill_color()
            };
        }
    }
}

/// Mouse-wheel scrolling for the HUD panel (so the whole palette is reachable).
pub fn scroll_panel(
    mut wheel: MessageReader<bevy::input::mouse::MouseWheel>,
    // Every scrollable panel (in-game right rail AND menu/briefing columns). Menus
    // and the HUD never coexist, so scrolling all present panels is unambiguous.
    mut panels: Query<&mut ScrollPosition>,
) {
    use bevy::input::mouse::MouseScrollUnit;
    let mut dy = 0.0;
    for ev in wheel.read() {
        dy += match ev.unit {
            MouseScrollUnit::Line => ev.y * 26.0,
            MouseScrollUnit::Pixel => ev.y,
        };
    }
    if dy != 0.0 {
        for mut s in &mut panels {
            s.0.y = (s.0.y - dy).max(0.0);
        }
    }
}

/// Touch-drag scrolling (mobile): there is no scroll wheel on a phone, so a finger
/// drag must scroll the panels. Two cases:
///
/// * **In-game** (`HudRoot`): only scroll when the finger is over the right panel
///   region, otherwise a drag on the board (placing a tower) would scroll the
///   palette. Also skipped while a tower-placement drag is active (`sel.dragging`),
///   so pulling a tower out of the palette toward the board never scrolls the list.
/// * **Menus** (every other `ScrollPosition`): no board sits behind them, so any
///   vertical drag scrolls the visible panel.
///
/// Finger deltas are window-logical px; `ScrollPosition` is pre-`UiScale` UI px, so
/// divide by the scale for a 1:1 feel.
pub fn touch_scroll_panel(
    touches: Res<Touches>,
    windows: Query<&Window>,
    ui_scale: Res<UiScale>,
    sel: Res<Selection>,
    mut hud: Query<&mut ScrollPosition, With<HudRoot>>,
    mut menus: Query<&mut ScrollPosition, Without<HudRoot>>,
) {
    let Ok(win) = windows.single() else {
        return;
    };
    let scale = ui_scale.0.max(0.0001);
    let panel_left = win.width() - PANEL_W_UI * scale;

    let mut panel_dy = 0.0; // finger movement over the in-game panel region
    let mut any_dy = 0.0; // finger movement anywhere (menus)
    for t in touches.iter() {
        let d = t.delta().y;
        any_dy += d;
        if t.position().x >= panel_left {
            panel_dy += d;
        }
    }

    // In-game palette: scroll only when not mid-placement (so grabbing a tower and
    // dragging it onto the board doesn't fight the scroll).
    if panel_dy != 0.0 && !sel.dragging {
        for mut s in &mut hud {
            s.0.y = (s.0.y - panel_dy / scale).max(0.0);
        }
    }
    if any_dy != 0.0 {
        for mut s in &mut menus {
            s.0.y = (s.0.y - any_dy / scale).max(0.0);
        }
    }
}

// ============================ Menu ============================

fn reveal(local: f32, delay: f32, duration: f32) -> f32 {
    let x = ((local - delay) / duration.max(0.001)).clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

fn briefing_text(
    parent: &mut ChildSpawnerCommands,
    f: &Handle<Font>,
    text: impl Into<String>,
    size: f32,
    color: Color,
    alpha: f32,
    delay: f32,
) {
    parent.spawn((
        Text::new(text),
        text_font(f, size),
        TextColor(color.with_alpha(0.0)),
        BriefingTextFade {
            delay,
            duration: 0.7,
            color,
            alpha,
        },
    ));
}

fn briefing_panel_fade(
    color: Color,
    alpha: f32,
    delay: f32,
) -> (BackgroundColor, BriefingPanelFade) {
    (
        BackgroundColor(color.with_alpha(0.0)),
        BriefingPanelFade {
            delay,
            duration: 0.75,
            color,
            alpha,
        },
    )
}

/// Who is speaking a story dialogue line. Drives which side popup shows.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum Speaker {
    /// Scene narration (center box).
    Narrator,
    /// 守护者·艾琳 — left popup.
    Guardian,
    /// 虚空统帅 — right popup.
    Warlord,
}

/// The opening dialogue script + cursor (visual-novel style, advanced by tap).
#[derive(Resource, Default)]
pub struct StoryDialogue {
    pub lines: Vec<(Speaker, String)>,
    pub idx: usize,
    /// Characters of the current line revealed so far (typewriter effect).
    pub revealed: f32,
    /// Decaying intensity of the void-warlord entrance flash (0 = none).
    pub flash: f32,
    /// Player is currently picking 艾琳's reply at `choice_at` (advance is paused).
    pub choosing: bool,
    pub choice_made: bool,
    /// Line index at which the player chooses 艾琳's reply (usize::MAX = no choice).
    pub choice_at: usize,
    /// (button label, resulting Guardian line) options for the choice.
    pub choices: Vec<(String, String)>,
}

/// Full-screen flash overlay for the warlord's dramatic entrance.
#[derive(Component)]
pub struct StoryFlash;
/// A player dialogue-choice button (index into `StoryDialogue::choices`).
#[derive(Component)]
pub struct StoryChoice(pub usize);
/// The row container holding the choice buttons (shown only while choosing).
#[derive(Component)]
pub struct StoryChoiceRow;

/// A dialogue popup container, keyed by which speaker it belongs to.
#[derive(Component)]
pub struct DlgBox(pub Speaker);
/// The text node inside a dialogue popup.
#[derive(Component)]
pub struct DlgText(pub Speaker);

pub fn spawn_story_scene(
    mut commands: Commands,
    fonts: Res<UiFont>,
    assets: Res<AssetServer>,
    levels: Res<Levels>,
    current: Res<CurrentLevel>,
    mode: Res<GameMode>,
    time: Res<Time>,
    mut timeline: ResMut<StoryTimeline>,
    mut atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    // Animated opening backdrop: a 24-frame WAN i2v clip of the seal garden, packed
    // as a 6×4 grid atlas (426×240 cells); cycled in `update_story_animation`.
    let intro_layout = atlas_layouts.add(TextureAtlasLayout::from_grid(
        UVec2::new(426, 240),
        6,
        4,
        None,
        None,
    ));
    // Living-portrait atlas for the two story characters (4×4 grid, 160×240 cells).
    let char_layout = atlas_layouts.add(TextureAtlasLayout::from_grid(
        UVec2::new(160, 240),
        4,
        4,
        None,
        None,
    ));
    timeline.start = time.elapsed_secs();
    timeline.played_mask = 0;
    let f = &fonts.0;
    let level = &levels.0[current.0];
    let endless = mode.0.is_endless();
    let (kicker, title, line_a, line_b, line_c, guardian_line, warlord_line) = if endless {
        (
            "无尽裂隙",
            "它还在发甜味",
            "封印没有被拔出来，但虚空已经学会从裂缝里绕路。",
            "敌潮不再按战役规则排队，它们会一波比一波更硬。",
            "这不是通关挑战，是给世界多争一晚的时间。",
            "艾琳：把塔阵接到核心上，能撑几波就撑几波。",
            "虚空统帅：甜味会散尽，守夜人也会。",
        )
    } else if current.0 == 0 {
        (
            "序章",
            "最后的萝卜封印",
            "月雾落下时，边境塔阵一座接一座熄灭。",
            "古园中心只剩最后一枚萝卜核心还在发光。",
            "它不是粮食，也不是宝物；它是把虚空挡在地下三百年的封印。",
            "艾琳：你们想拔萝卜，也得先过我的塔阵。",
            "虚空统帅：把它拔出来，世界就会安静。",
        )
    } else {
        (
            "战役插曲",
            "新的防线正在成形",
            "前一处战场留下的裂纹，已经蔓延到下一座萝卜园。",
            "敌人开始针对塔阵转角和终点前沿集结。",
            "进入作战简报前，先确认本关路径和首领威胁。",
            "守护者艾琳：别让核心暴露在第二轮冲击里。",
            "虚空统帅：每一段路，都会变成你们的缺口。",
        )
    };

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Relative,
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::BLACK),
            GlobalZIndex(92),
            StoryRoot,
        ))
        .with_children(|root| {
            root.spawn((
                ImageNode {
                    image: assets.load("story/intro_seal.webp"),
                    texture_atlas: Some(TextureAtlas {
                        layout: intro_layout.clone(),
                        index: 0,
                    }),
                    color: Color::WHITE.with_alpha(0.0),
                    ..default()
                },
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(-4.0),
                    top: Val::Percent(-4.0),
                    width: Val::Percent(108.0),
                    height: Val::Percent(108.0),
                    ..default()
                },
                StoryBackdrop {
                    delay: 0.0,
                    duration: 1.4,
                    alpha: 1.0,
                },
            ));

            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.18)),
            ));

            root.spawn((
                ImageNode {
                    image: assets.load("story/guardian_anim.webp"),
                    texture_atlas: Some(TextureAtlas {
                        layout: char_layout.clone(),
                        index: 0,
                    }),
                    color: Color::WHITE.with_alpha(0.0),
                    ..default()
                },
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(-8.0),
                    bottom: Val::Percent(-5.0),
                    width: Val::Percent(31.0),
                    height: Val::Percent(92.0),
                    ..default()
                },
                StoryImageMotion {
                    from_left: -8.0,
                    to_left: 2.2,
                    from_bottom: -6.0,
                    to_bottom: -2.5,
                    delay: 0.45,
                    duration: 1.05,
                    tint: Color::WHITE,
                    alpha: 1.0,
                    float_amp: 0.45,
                    float_speed: 1.35,
                    speaker: Some(Speaker::Guardian),
                },
            ));

            root.spawn((
                ImageNode {
                    image: assets.load("story/warlord_anim.webp"),
                    texture_atlas: Some(TextureAtlas {
                        layout: char_layout.clone(),
                        index: 0,
                    }),
                    color: Color::WHITE.with_alpha(0.0),
                    ..default()
                },
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(79.0),
                    bottom: Val::Percent(-6.0),
                    width: Val::Percent(34.0),
                    height: Val::Percent(94.0),
                    ..default()
                },
                StoryImageMotion {
                    from_left: 79.0,
                    to_left: 62.5,
                    from_bottom: -7.0,
                    to_bottom: -3.0,
                    delay: 0.9,
                    duration: 1.2,
                    tint: Color::WHITE,
                    alpha: 0.96,
                    float_amp: 0.35,
                    float_speed: 1.0,
                    speaker: Some(Speaker::Warlord),
                },
            ));

            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(28.0),
                    top: Val::Px(24.0),
                    width: Val::Px(560.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(16.0)),
                    row_gap: Val::Px(8.0),
                    ..default()
                },
                briefing_panel_fade(Color::srgba(0.015, 0.020, 0.024, 1.0), 0.62, 0.25),
            ))
            .with_children(|panel| {
                briefing_text(panel, f, crate::i18n::t(kicker), 13.0, UI_ACCENT_TEAL, 1.0, 0.35);
                briefing_text(panel, f, crate::i18n::t(title), 34.0, UI_ACCENT_GOLD, 1.0, 0.55);
            });

            // --- visual-novel dialogue popups: left (Guardian) / right (Warlord) /
            // center (Narrator). Only the active speaker's box is shown; advanced by
            // tap in `advance_story_dialogue`. ---
            let left = Color::srgb(0.80, 0.94, 0.86);
            let right = Color::srgb(0.92, 0.74, 1.0);
            let nar = Color::srgb(0.86, 0.90, 0.94);
            // Guardian popup (left).
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(3.0),
                    bottom: Val::Percent(20.0),
                    width: Val::Percent(42.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::axes(Val::Px(16.0), Val::Px(12.0)),
                    row_gap: Val::Px(5.0),
                    display: Display::None,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.04, 0.10, 0.07, 0.92)),
                GlobalZIndex(96),
                DlgBox(Speaker::Guardian),
            ))
            .with_children(|b| {
                b.spawn((Text::new(crate::i18n::t("守护者·艾琳")), text_font(f, 15.0), TextColor(left)));
                b.spawn((Text::new(""), text_font(f, 16.0), TextColor(Color::WHITE), DlgText(Speaker::Guardian)));
            });
            // Warlord popup (right).
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Percent(3.0),
                    bottom: Val::Percent(20.0),
                    width: Val::Percent(42.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::FlexEnd,
                    padding: UiRect::axes(Val::Px(16.0), Val::Px(12.0)),
                    row_gap: Val::Px(5.0),
                    display: Display::None,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.10, 0.04, 0.12, 0.92)),
                GlobalZIndex(96),
                DlgBox(Speaker::Warlord),
            ))
            .with_children(|b| {
                b.spawn((Text::new(crate::i18n::t("虚空统帅")), text_font(f, 15.0), TextColor(right)));
                b.spawn((Text::new(""), text_font(f, 16.0), TextColor(Color::WHITE), DlgText(Speaker::Warlord)));
            });
            // Narrator box (center bottom).
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(16.0),
                    right: Val::Percent(16.0),
                    bottom: Val::Px(40.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::axes(Val::Px(18.0), Val::Px(14.0)),
                    display: Display::None,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.015, 0.02, 0.024, 0.9)),
                GlobalZIndex(96),
                DlgBox(Speaker::Narrator),
            ))
            .with_children(|b| {
                b.spawn((Text::new(""), text_font(f, 16.0), TextColor(nar), DlgText(Speaker::Narrator)));
            });
            // Tap-to-continue hint (always visible).
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(16.0),
                    bottom: Val::Px(10.0),
                    ..default()
                },
                GlobalZIndex(97),
                Text::new(crate::i18n::t("▶ 点击 / 空格 继续   (Esc 返回)")),
                text_font(f, 12.0),
                TextColor(Color::srgba(1.0, 1.0, 1.0, 0.6)),
            ));

            // Void-warlord entrance flash overlay (alpha driven by StoryDialogue.flash).
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.45, 0.12, 0.75, 0.0)),
                GlobalZIndex(98),
                StoryFlash,
            ));

            // Player dialogue-choice buttons (hidden until the choice point). Labels
            // mirror the StoryDialogue.choices set below.
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(16.0),
                    right: Val::Percent(16.0),
                    bottom: Val::Percent(40.0),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::Center,
                    column_gap: Val::Px(16.0),
                    display: Display::None,
                    ..default()
                },
                GlobalZIndex(99),
                StoryChoiceRow,
            ))
            .with_children(|row| {
                for (i, label) in ["死守萝卜", "诱敌深入"].into_iter().enumerate() {
                    row.spawn((
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(16.0), Val::Px(10.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.20, 0.40, 0.30)),
                        StoryChoice(i),
                    ))
                    .with_children(|b| {
                        b.spawn((Text::new(crate::i18n::t(label)), text_font(f, 16.0), TextColor(UI_TEXT)));
                    });
                }
            });
        });

    // The dialogue script: narration framing + a Guardian↔Warlord exchange. Reuses the
    // per-scenario localized lines so prologue / interlude / endless all read correctly.
    use Speaker::{Guardian, Narrator, Warlord};
    // Per-level narration: each of the 20 levels has its own opening lore line.
    let level_lore = LEVEL_LORE.get(current.0).copied().unwrap_or(line_a);
    commands.insert_resource(StoryDialogue {
        lines: vec![
            (Narrator, crate::i18n::t(level_lore)),
            (Narrator, crate::i18n::t(line_b)),
            (Warlord, crate::i18n::t(warlord_line)),
            (Guardian, crate::i18n::t(guardian_line)),
            (Narrator, crate::i18n::tf("{}：{}", &[&crate::i18n::t(level.name), &crate::i18n::t(line_c)])),
        ],
        idx: 0,
        revealed: 0.0,
        flash: 0.0,
        choosing: false,
        choice_made: false,
        // Before 艾琳's line (idx 3) the player chooses her reply.
        choice_at: 3,
        choices: vec![
            (crate::i18n::t("死守萝卜"), crate::i18n::t("只要我还站着，核心就不会熄。")),
            (crate::i18n::t("诱敌深入"), crate::i18n::t("让它们进来——塔阵的每一寸都是坟墓。")),
        ],
    });
}

pub fn update_story_animation(
    time: Res<Time>,
    timeline: Res<StoryTimeline>,
    dlg: Res<StoryDialogue>,
    mut backdrops: Query<(&StoryBackdrop, &mut ImageNode, &mut Node)>,
    mut images: Query<(&StoryImageMotion, &mut ImageNode, &mut Node), Without<StoryBackdrop>>,
    mut texts: Query<(&BriefingTextFade, &mut TextColor)>,
    mut panels: Query<(&BriefingPanelFade, &mut BackgroundColor)>,
) {
    let active = dlg.lines.get(dlg.idx).map(|l| l.0);
    let local = time.elapsed_secs() - timeline.start;
    for (fade, mut image, mut node) in &mut backdrops {
        let t = reveal(local, fade.delay, fade.duration);
        image.color = Color::WHITE.with_alpha(fade.alpha * t);
        // Play the 24-frame WAN clip over ~2.4s, then hold the last frame.
        if let Some(atlas) = &mut image.texture_atlas {
            atlas.index = ((local * 10.0) as usize).min(23);
        }
        let zoom = 108.0 - 3.0 * t;
        node.width = Val::Percent(zoom);
        node.height = Val::Percent(zoom);
        node.left = Val::Percent(-4.0 + 1.5 * t);
        node.top = Val::Percent(-4.0 + 1.5 * t);
    }
    for (motion, mut image, mut node) in &mut images {
        let t = reveal(local, motion.delay, motion.duration);
        // VN highlight: the speaking character is full-bright, the other dims.
        let dim = match motion.speaker {
            Some(s) => {
                if active == Some(s) {
                    1.0
                } else {
                    0.45
                }
            }
            None => 1.0,
        };
        let drift = (local * motion.float_speed).sin() * motion.float_amp * t;
        node.left = Val::Percent(motion.from_left + (motion.to_left - motion.from_left) * t);
        node.bottom =
            Val::Percent(motion.from_bottom + (motion.to_bottom - motion.from_bottom) * t + drift);
        image.color = motion.tint.with_alpha(motion.alpha * t * dim);
        // Loop the 16-frame living-portrait clip.
        if let Some(atlas) = &mut image.texture_atlas {
            atlas.index = ((local * 12.0) as usize) % 16;
        }
    }
    for (fade, mut color) in &mut texts {
        color.0 = fade
            .color
            .with_alpha(fade.alpha * reveal(local, fade.delay, fade.duration));
    }
    for (fade, mut bg) in &mut panels {
        bg.0 = fade
            .color
            .with_alpha(fade.alpha * reveal(local, fade.delay, fade.duration));
    }
}

pub fn play_story_voiceover(
    mut commands: Commands,
    assets: Res<AssetServer>,
    time: Res<Time>,
    mode: Res<GameMode>,
    current: Res<CurrentLevel>,
    mut timeline: ResMut<StoryTimeline>,
) {
    let local = time.elapsed_secs() - timeline.start;
    let prefix = if mode.0.is_endless() {
        "endless"
    } else if current.0 == 0 {
        "prologue"
    } else {
        return;
    };
    let cues = if prefix == "prologue" {
        [
            (0.9_f32, "narrator"),
            (10.4_f32, "guardian"),
            (13.6_f32, "warlord"),
        ]
    } else {
        [
            (0.9_f32, "narrator"),
            (8.2_f32, "guardian"),
            (11.4_f32, "warlord"),
        ]
    };
    for (idx, (delay, speaker)) in cues.into_iter().enumerate() {
        let bit = 1_u8 << idx;
        if timeline.played_mask & bit != 0 || local < delay {
            continue;
        }
        timeline.played_mask |= bit;
        commands.spawn((
            AudioPlayer(
                assets.load::<AudioSource>(format!("audio/story/{}_{}.mp3", prefix, speaker)),
            ),
            PlaybackSettings {
                mode: PlaybackMode::Despawn,
                volume: Volume::Linear(0.82),
                ..default()
            },
            StoryRoot,
        ));
    }
}

/// Visual-novel dialogue driver for the opening: tap/space advances a line; the active
/// speaker's side popup is shown; after the last line it continues to the briefing.
pub fn advance_story_dialogue(
    time: Res<Time>,
    timeline: Res<StoryTimeline>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    mut dlg: ResMut<StoryDialogue>,
    mut next: ResMut<NextState<GameState>>,
    mut boxes: Query<(&DlgBox, &mut Node), Without<StoryChoiceRow>>,
    mut texts: Query<(&DlgText, &mut Text)>,
    mut flash_q: Query<&mut BackgroundColor, With<StoryFlash>>,
    mut choice_row: Query<&mut Node, (With<StoryChoiceRow>, Without<DlgBox>)>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        next.set(GameState::Menu);
        return;
    }
    if dlg.lines.is_empty() {
        return;
    }
    let dt = time.delta_secs();
    // Decay + apply the void-warlord entrance flash.
    dlg.flash = (dlg.flash - dt * 2.2).max(0.0);
    let flash = dlg.flash;
    for mut bg in &mut flash_q {
        bg.0 = bg.0.with_alpha(flash * 0.5);
    }
    // Show choice buttons only while the player is choosing.
    for mut node in &mut choice_row {
        let want = if dlg.choosing {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != want {
            node.display = want;
        }
    }

    // Small lead-in so the click that opened this scene can't instantly advance.
    let local = time.elapsed_secs() - timeline.start;
    let pressed = local > 0.3
        && (keys.just_pressed(KeyCode::Enter)
            || keys.just_pressed(KeyCode::Space)
            || mouse.just_pressed(MouseButton::Left)
            || touches.iter_just_pressed().next().is_some());

    let cur_len = dlg.lines[dlg.idx].1.chars().count();
    if pressed && !dlg.choosing {
        if (dlg.revealed as usize) < cur_len {
            dlg.revealed = cur_len as f32; // first tap completes the line instantly
        } else {
            let next_idx = dlg.idx + 1;
            if next_idx == dlg.choice_at && !dlg.choice_made {
                dlg.choosing = true; // pause for the player to pick 艾琳's reply
            } else if next_idx >= dlg.lines.len() {
                next.set(GameState::Briefing);
                return;
            } else {
                dlg.idx = next_idx;
                dlg.revealed = 0.0;
                if dlg.lines[dlg.idx].0 == Speaker::Warlord {
                    dlg.flash = 1.0; // dramatic entrance
                }
            }
        }
    } else if !dlg.choosing {
        // Typewriter: reveal ~38 chars/sec.
        dlg.revealed = (dlg.revealed + dt * 38.0).min(cur_len as f32);
    }

    // Render: show the active speaker's popup with the revealed substring.
    let sp = dlg.lines[dlg.idx].0;
    let shown: String = dlg.lines[dlg.idx]
        .1
        .chars()
        .take(dlg.revealed as usize)
        .collect();
    for (b, mut node) in &mut boxes {
        let want = if b.0 == sp {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != want {
            node.display = want;
        }
    }
    for (t, mut text) in &mut texts {
        if t.0 == sp && text.0 != shown {
            text.0 = shown.clone();
        }
    }
}

/// Handle the player's dialogue choice: set 艾琳's reply and resume the dialogue.
pub fn story_choice_buttons(
    interactions: Query<(&Interaction, &StoryChoice), Changed<Interaction>>,
    mut dlg: ResMut<StoryDialogue>,
) {
    if !dlg.choosing {
        return;
    }
    for (interaction, choice) in &interactions {
        if *interaction == Interaction::Pressed {
            if let Some((_, reply)) = dlg.choices.get(choice.0).cloned() {
                let at = dlg.choice_at;
                if let Some(line) = dlg.lines.get_mut(at) {
                    line.1 = reply;
                }
                dlg.idx = at;
                dlg.revealed = 0.0;
                dlg.choosing = false;
                dlg.choice_made = true;
            }
        }
    }
}

// ===================== Hero deploy cutscene (HeroIntro) =====================

#[derive(Component)]
pub struct HeroIntroRoot;
/// Marks the animated portrait so `update_hero_intro` can fade/zoom it in.
#[derive(Component)]
pub struct HeroIntroPortrait;
/// Marks cutscene texts so they fade in (carries their base color).
#[derive(Component)]
pub struct HeroIntroText(pub Color);

/// The chosen class×race animated "living portrait" atlas under
/// `assets/story/combo_anim/` (4×4 grid, 16 frames, WAN i2v generated).
fn combo_anim_path(hero: &HeroLoadout) -> String {
    let race = match hero.race {
        Race::Human => "human",
        Race::Elf => "elf",
        Race::Orc => "orc",
    };
    format!("story/combo_anim/{}_{}.webp", hero.class.sprite_name(), race)
}

pub fn spawn_hero_intro(
    mut commands: Commands,
    fonts: Res<UiFont>,
    assets: Res<AssetServer>,
    hero: Res<HeroLoadout>,
    time: Res<Time>,
    mut timeline: ResMut<StoryTimeline>,
    mut atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    timeline.start = time.elapsed_secs();
    // 4×4 grid atlas of the 16-frame living-portrait clip for this combo.
    let anim_layout = atlas_layouts.add(TextureAtlasLayout::from_grid(
        UVec2::splat(192),
        4,
        4,
        None,
        None,
    ));
    let f = &fonts.0;
    let doc = hero.class.doctrine();
    let name = format!("{}·{}", hero.race.name(), hero.class.name());
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(6.0),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::srgb(0.02, 0.03, 0.04)),
            GlobalZIndex(95),
            HeroIntroRoot,
        ))
        .with_children(|root| {
            root.spawn((
                ImageNode {
                    image: assets.load(combo_anim_path(&hero)),
                    texture_atlas: Some(TextureAtlas {
                        layout: anim_layout.clone(),
                        index: 0,
                    }),
                    color: Color::WHITE.with_alpha(0.0),
                    ..default()
                },
                Node {
                    height: Val::Percent(74.0),
                    aspect_ratio: Some(1.0),
                    ..default()
                },
                HeroIntroPortrait,
            ));
            let gold = Color::srgb(1.0, 0.86, 0.4);
            let teal = Color::srgb(0.6, 0.92, 1.0);
            root.spawn((
                Text::new(name),
                text_font(f, 30.0),
                TextColor(gold.with_alpha(0.0)),
                HeroIntroText(gold),
            ));
            root.spawn((
                Text::new(format!("【{}】{}", doc.name, doc.desc)),
                text_font(f, 14.0),
                TextColor(teal.with_alpha(0.0)),
                Node {
                    max_width: Val::Px(640.0),
                    ..default()
                },
                HeroIntroText(teal),
            ));
            root.spawn((
                Text::new(crate::i18n::t("出击！  (点击/空格 跳过)")),
                text_font(f, 13.0),
                TextColor(Color::srgba(1.0, 1.0, 1.0, 0.0)),
                HeroIntroText(Color::srgb(0.85, 0.9, 0.95)),
                Node {
                    margin: UiRect::top(Val::Px(8.0)),
                    ..default()
                },
            ));
        });
}

/// Animate the deploy cutscene (fade/zoom the portrait, fade texts) and advance to
/// the level after ~3.4s or on a tap/click/key.
pub fn update_hero_intro(
    time: Res<Time>,
    timeline: Res<StoryTimeline>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    mut next: ResMut<NextState<GameState>>,
    mut portrait: Query<(&mut ImageNode, &mut Node), With<HeroIntroPortrait>>,
    mut texts: Query<(&HeroIntroText, &mut TextColor)>,
) {
    let local = time.elapsed_secs() - timeline.start;
    let app = reveal(local, 0.1, 0.7);
    for (mut img, mut node) in &mut portrait {
        img.color = Color::WHITE.with_alpha(app);
        node.height = Val::Percent(70.0 + 8.0 * app);
        // Loop the 16-frame living-portrait clip at ~12 fps.
        if let Some(atlas) = &mut img.texture_atlas {
            atlas.index = ((local * 12.0) as usize) % 16;
        }
    }
    let t = reveal(local, 0.55, 0.7);
    for (base, mut color) in &mut texts {
        color.0 = base.0.with_alpha(t);
    }
    let skip = keys.just_pressed(KeyCode::Space)
        || keys.just_pressed(KeyCode::Enter)
        || mouse.just_pressed(MouseButton::Left)
        || touches.iter_just_pressed().next().is_some();
    if local > 3.4 || (skip && local > 0.4) {
        next.set(GameState::Playing);
    }
}

/// Spawn a bottom-left hover/tap tooltip overlay (TooltipBox + TooltipText) for a
/// selection screen, so `tooltip_system` can show info there too (race/class/etc).
/// `root` tags it for that screen's `despawn_with` cleanup.
fn spawn_tooltip_box(commands: &mut Commands, f: &Handle<Font>, root: impl Bundle) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(12.0),
                bottom: Val::Px(12.0),
                max_width: Val::Px(420.0),
                padding: UiRect::all(Val::Px(9.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(UI_PANEL_DARK),
            GlobalZIndex(80),
            TooltipBox,
            root,
        ))
        .with_children(|t| {
            t.spawn((
                Text::new(""),
                text_font(f, 13.0),
                TextColor(Color::srgb(0.95, 0.95, 0.85)),
                TooltipText,
            ));
        });
}

pub fn spawn_level_briefing(
    mut commands: Commands,
    fonts: Res<UiFont>,
    levels: Res<Levels>,
    current: Res<CurrentLevel>,
    mode: Res<GameMode>,
    diff: Res<GameDifficulty>,
    hero: Res<HeroLoadout>,
    time: Res<Time>,
    mut timeline: ResMut<BriefingTimeline>,
) {
    timeline.start = time.elapsed_secs();
    let f = &fonts.0;
    let level = &levels.0[current.0];
    let theme = LEVEL_THEMES
        .get(current.0)
        .copied()
        .unwrap_or(LEVEL_THEMES[0]);
    let endless = mode.0.is_endless();
    let title = if endless {
        crate::i18n::tf("无尽模式 · {}", &[&crate::i18n::t(level.name)])
    } else {
        crate::i18n::tf("{}. {}", &[&format!("{:02}", current.0 + 1), &crate::i18n::t(level.name)])
    };
    let lore = if endless {
        crate::i18n::t("终章封印被反复冲刷，敌潮不再遵守战役节奏。每一轮部署都只是为下一次崩坏争取时间。")
    } else {
        crate::i18n::t(LEVEL_LORE.get(current.0).copied().unwrap_or("档案缺失。"))
    };
    let objective = if endless {
        crate::i18n::tf(
            "作战目标：无限坚持\n初始金币 {}  生命 {}  首领周期 {} 波\n难度：{}  英雄：{}·{}",
            &[
                &(((level.gold + 250) as f32 * diff.0.gold_mult()) as i32).to_string(),
                &(16 + diff.0.lives_bonus()).max(1).to_string(),
                &BOSS_WAVE_INTERVAL.to_string(),
                &crate::i18n::t(diff.0.name()),
                &crate::i18n::t(hero.race.name()),
                &crate::i18n::t(hero.class.name()),
            ],
        )
    } else {
        crate::i18n::tf(
            "作战目标：守住 {} 波\n初始金币 {}  生命 {}  出怪基数 {}\n难度：{}  英雄：{}·{}",
            &[
                &level.waves.to_string(),
                &((level.gold as f32 * diff.0.gold_mult()) as i32).to_string(),
                &(level.lives + diff.0.lives_bonus()).max(1).to_string(),
                &level.enemies.count.to_string(),
                &crate::i18n::t(diff.0.name()),
                &crate::i18n::t(hero.race.name()),
                &crate::i18n::t(hero.class.name()),
            ],
        )
    };
    let boss_line = if endless {
        crate::i18n::tf(
            "首领协议：每 {} 波出现一次，生命和技能强度随波次增长",
            &[&BOSS_WAVE_INTERVAL.to_string()],
        )
    } else {
        campaign_boss_line(current.0, level)
    };
    let recommend = if endless {
        crate::i18n::t("建议：优先做成套装备与元素覆盖，后期需要反隐、攻城防护和范围清场")
    } else {
        campaign_recommendation(current.0, level)
    };
    let affix = elite_affix_intel(level.waves, current.0);
    let map_w = 488.0;
    let map_h = 304.0;
    let cell_pos = |col: i32, row: i32| {
        Vec2::new(
            ((col as f32 + 0.5) / COLS as f32) * map_w,
            ((row as f32 + 0.5) / ROWS as f32) * map_h,
        )
    };

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Stretch,
                padding: UiRect::all(Val::Px(18.0)),
                column_gap: Val::Px(14.0),
                ..default()
            },
            BackgroundColor(UI_BG),
            BriefingRoot,
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Px(360.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(16.0)),
                    row_gap: Val::Px(10.0),
                    // Scrollable: the hero class/race picker makes this column tall.
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
                briefing_panel_fade(UI_PANEL_DARK, 0.96, 0.0),
                ScrollPosition::default(),
            ))
            .with_children(|left| {
                briefing_text(left, f, crate::i18n::t("作战简报"), 14.0, UI_ACCENT_TEAL, 1.0, 0.10);
                briefing_text(left, f, title, 28.0, UI_ACCENT_GOLD, 1.0, 0.25);
                briefing_text(left, f, lore, 13.0, UI_TEXT, 0.92, 0.55);

                left.spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(10.0)),
                        row_gap: Val::Px(4.0),
                        ..default()
                    },
                    briefing_panel_fade(UI_CARD, 0.9, 0.9),
                ))
                .with_children(|panel| {
                    briefing_text(panel, f, crate::i18n::t("部署参数"), 12.0, UI_ACCENT_GOLD, 1.0, 1.0);
                    briefing_text(panel, f, objective, 11.0, UI_TEXT, 0.95, 1.12);
                });

                left.spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(10.0)),
                        row_gap: Val::Px(4.0),
                        ..default()
                    },
                    briefing_panel_fade(theme.accent, 0.18, 1.25),
                ))
                .with_children(|panel| {
                    briefing_text(
                        panel,
                        f,
                        crate::i18n::t("敌情"),
                        12.0,
                        theme.accent.mix(&Color::WHITE, 0.35),
                        1.0,
                        1.35,
                    );
                    briefing_text(panel, f, boss_line, 10.5, UI_TEXT, 0.95, 1.48);
                    briefing_text(
                        panel,
                        f,
                        affix,
                        10.5,
                        Color::srgb(0.76, 0.70, 0.94),
                        0.95,
                        1.65,
                    );
                    briefing_text(
                        panel,
                        f,
                        recommend,
                        10.5,
                        Color::srgb(0.70, 0.88, 0.72),
                        0.95,
                        1.82,
                    );
                });

                left.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(8.0),
                        margin: UiRect::top(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.08)),
                    briefing_panel_fade(Color::WHITE, 0.08, 2.0).1,
                ))
                .with_children(|bar| {
                    bar.spawn((
                        Node {
                            width: Val::Percent(0.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        BackgroundColor(UI_ACCENT_GOLD),
                        BriefingMeter {
                            delay: 2.0,
                            duration: 2.3,
                        },
                    ));
                });

                // --- hero selection: pick class + race before deploying ---
                left.spawn((
                    Text::new(crate::i18n::t("选择英雄职业（出击前可更换）")),
                    text_font(f, 14.0),
                    TextColor(UI_ACCENT_GOLD),
                    Node {
                        margin: UiRect::top(Val::Px(8.0)),
                        ..default()
                    },
                ));
                left.spawn((
                    Text::new(""),
                    text_font(f, 12.0),
                    TextColor(Color::srgb(0.85, 0.9, 1.0)),
                    HeroLabel,
                ));
                left.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                })
                .with_children(|row| {
                    for class in Class::ALL {
                        let col = if class == hero.class {
                            Color::srgb(0.30, 0.52, 0.32)
                        } else {
                            BTN_BG
                        };
                        dock_button(row, f, &crate::i18n::t(class.name()), UiAction::SelectHeroClass(class), col);
                    }
                });
                left.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                })
                .with_children(|row| {
                    for race in Race::ALL {
                        let col = if race == hero.race {
                            Color::srgb(0.28, 0.40, 0.52)
                        } else {
                            BTN_BG
                        };
                        dock_button(row, f, &crate::i18n::t(race.name()), UiAction::SelectHeroRace(race), col);
                    }
                });

                left.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    margin: UiRect::top(Val::Px(4.0)),
                    ..default()
                })
                .with_children(|row| {
                    button(
                        row,
                        f,
                        &crate::i18n::t("开始部署"),
                        UiAction::BeginMission,
                        Color::srgb(0.23, 0.50, 0.30),
                    );
                    button(row, f, &crate::i18n::t("返回战情室"), UiAction::ToMenu, BTN_BG);
                });
            });

            root.spawn((
                Node {
                    flex_grow: 1.0,
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(16.0)),
                    row_gap: Val::Px(10.0),
                    overflow: Overflow::clip(),
                    ..default()
                },
                briefing_panel_fade(UI_PANEL, 0.94, 0.12),
            ))
            .with_children(|right| {
                right
                    .spawn(Node {
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|head| {
                        briefing_text(head, f, crate::i18n::t("战场投影"), 22.0, UI_TEXT, 1.0, 0.35);
                        briefing_text(
                            head,
                            f,
                            crate::i18n::t("扫描路线 / 确认火力窗口"),
                            11.0,
                            UI_TEXT_DIM,
                            0.9,
                            0.55,
                        );
                    });

                right
                    .spawn((
                        Node {
                            width: Val::Px(map_w),
                            height: Val::Px(map_h),
                            position_type: PositionType::Relative,
                            align_self: AlignSelf::Center,
                            overflow: Overflow::clip(),
                            ..default()
                        },
                        briefing_panel_fade(theme.backdrop, 0.46, 0.42),
                    ))
                    .with_children(|map| {
                        map.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(-90.0),
                                top: Val::Px(0.0),
                                width: Val::Px(82.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(UI_ACCENT_TEAL.with_alpha(0.0)),
                            BriefingSweep {
                                base_left: -90.0,
                                span: map_w + 180.0,
                                speed: 94.0,
                                width: 82.0,
                                color: UI_ACCENT_TEAL,
                                alpha: 0.18,
                            },
                        ));

                        for x in 0..=COLS {
                            let px = x as f32 / COLS as f32 * map_w;
                            map.spawn((
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: Val::Px(px),
                                    top: Val::Px(0.0),
                                    width: Val::Px(1.0),
                                    height: Val::Percent(100.0),
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.035)),
                            ));
                        }
                        for y in 0..=ROWS {
                            let py = y as f32 / ROWS as f32 * map_h;
                            map.spawn((
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: Val::Px(0.0),
                                    top: Val::Px(py),
                                    width: Val::Percent(100.0),
                                    height: Val::Px(1.0),
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.028)),
                            ));
                        }

                        for (idx, pair) in level.path.windows(2).enumerate() {
                            let a = cell_pos(pair[0].0, pair[0].1);
                            let b = cell_pos(pair[1].0, pair[1].1);
                            let left = a.x.min(b.x) - 3.0;
                            let top = a.y.min(b.y) - 3.0;
                            let horizontal = (a.x - b.x).abs() >= (a.y - b.y).abs();
                            let width = if horizontal {
                                (a.x - b.x).abs() + 6.0
                            } else {
                                6.0
                            };
                            let height = if horizontal {
                                6.0
                            } else {
                                (a.y - b.y).abs() + 6.0
                            };
                            map.spawn((
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: Val::Px(left),
                                    top: Val::Px(top),
                                    width: Val::Px(width.max(6.0)),
                                    height: Val::Px(height.max(6.0)),
                                    ..default()
                                },
                                briefing_panel_fade(
                                    theme.path_edge,
                                    0.72,
                                    0.75 + idx as f32 * 0.055,
                                ),
                            ));
                        }

                        for (idx, point) in level.path.iter().enumerate() {
                            let p = cell_pos(point.0, point.1);
                            let terminal = idx == 0 || idx + 1 == level.path.len();
                            let size = if terminal { 16.0 } else { 9.0 };
                            let color = if idx == 0 {
                                UI_ACCENT_RED
                            } else if idx + 1 == level.path.len() {
                                theme.seal
                            } else {
                                theme.accent
                            };
                            map.spawn((
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: Val::Px(p.x - size / 2.0),
                                    top: Val::Px(p.y - size / 2.0),
                                    width: Val::Px(size),
                                    height: Val::Px(size),
                                    ..default()
                                },
                                briefing_panel_fade(
                                    color,
                                    if terminal { 0.9 } else { 0.58 },
                                    0.9 + idx as f32 * 0.04,
                                ),
                            ));
                        }
                    });

                right
                    .spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(8.0),
                        ..default()
                    })
                    .with_children(|row| {
                        row.spawn((
                            Node {
                                flex_grow: 1.0,
                                flex_direction: FlexDirection::Column,
                                padding: UiRect::all(Val::Px(10.0)),
                                row_gap: Val::Px(3.0),
                                ..default()
                            },
                            briefing_panel_fade(UI_CARD_SOFT, 0.88, 1.95),
                        ))
                        .with_children(|card| {
                            briefing_text(card, f, crate::i18n::t("塔防窗口"), 12.0, UI_ACCENT_GOLD, 1.0, 2.05);
                            briefing_text(
                                card,
                                f,
                                crate::i18n::tf(
                                    "路径节点 {}  建议先覆盖转角和终点前两格",
                                    &[&level.path.len().to_string()],
                                ),
                                10.0,
                                UI_TEXT,
                                0.9,
                                2.2,
                            );
                        });
                        row.spawn((
                            Node {
                                flex_grow: 1.0,
                                flex_direction: FlexDirection::Column,
                                padding: UiRect::all(Val::Px(10.0)),
                                row_gap: Val::Px(3.0),
                                ..default()
                            },
                            briefing_panel_fade(UI_CARD_SOFT, 0.88, 2.15),
                        ))
                        .with_children(|card| {
                            briefing_text(card, f, crate::i18n::t("作战节奏"), 12.0, UI_ACCENT_TEAL, 1.0, 2.25);
                            briefing_text(
                                card,
                                f,
                                if endless {
                                    crate::i18n::t("优先成型经济与装备共鸣，5波节奏检查反隐和攻城防护")
                                } else {
                                    crate::i18n::tf(
                                        "第 {} 波前建立主输出，第 {} 波前补控制",
                                        &[
                                            &(level.waves / 3).max(1).to_string(),
                                            &(level.waves * 2 / 3).max(2).to_string(),
                                        ],
                                    )
                                },
                                10.0,
                                UI_TEXT,
                                0.9,
                                2.4,
                            );
                        });
                    });
            });
        });
    // Hover/tap tooltips for the class/race pickers on this screen.
    spawn_tooltip_box(&mut commands, f, BriefingRoot);
}

pub fn update_briefing_animation(
    time: Res<Time>,
    timeline: Res<BriefingTimeline>,
    mut texts: Query<(&BriefingTextFade, &mut TextColor)>,
    // `panels` and `sweeps` both touch BackgroundColor; `sweeps` and `meters` both
    // touch Node. Distinct markers aren't enough for Bevy — add Without<> so the
    // queries are provably disjoint (else B0001 at runtime).
    mut panels: Query<(&BriefingPanelFade, &mut BackgroundColor), Without<BriefingSweep>>,
    mut sweeps: Query<(&BriefingSweep, &mut Node, &mut BackgroundColor), Without<BriefingMeter>>,
    mut meters: Query<(&BriefingMeter, &mut Node), Without<BriefingSweep>>,
) {
    let local = time.elapsed_secs() - timeline.start;
    for (fade, mut color) in &mut texts {
        color.0 = fade
            .color
            .with_alpha(fade.alpha * reveal(local, fade.delay, fade.duration));
    }
    for (fade, mut bg) in &mut panels {
        bg.0 = fade
            .color
            .with_alpha(fade.alpha * reveal(local, fade.delay, fade.duration));
    }
    for (sweep, mut node, mut bg) in &mut sweeps {
        let x = (local * sweep.speed + sweep.base_left).rem_euclid(sweep.span) - sweep.width;
        node.left = Val::Px(x);
        let shimmer = 0.55 + 0.45 * (local * 4.2).sin().abs();
        bg.0 = sweep.color.with_alpha(sweep.alpha * shimmer);
    }
    for (meter, mut node) in &mut meters {
        node.width = Val::Percent(reveal(local, meter.delay, meter.duration) * 100.0);
    }
}

pub fn briefing_buttons(
    keys: Res<ButtonInput<KeyCode>>,
    interactions: Query<(&Interaction, &UiAction), Changed<Interaction>>,
    mut next: ResMut<NextState<GameState>>,
    mut hero: ResMut<HeroLoadout>,
) {
    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space) {
        next.set(GameState::HeroIntro);
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        next.set(GameState::Menu);
        return;
    }
    for (interaction, action) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action {
            UiAction::BeginMission => next.set(GameState::HeroIntro),
            UiAction::ToMenu => next.set(GameState::Menu),
            UiAction::SelectHeroClass(c) => hero.set_class(*c),
            UiAction::SelectHeroRace(r) => hero.set_race(*r),
            _ => {}
        }
    }
}

/// Highlight the currently-selected hero class/race buttons on the briefing screen.
pub fn update_hero_select_buttons(
    hero: Res<HeroLoadout>,
    mut q: Query<(&UiAction, &mut BackgroundColor)>,
) {
    for (action, mut bg) in &mut q {
        match action {
            UiAction::SelectHeroClass(c) => {
                bg.0 = if *c == hero.class {
                    Color::srgb(0.30, 0.52, 0.32)
                } else {
                    BTN_BG
                };
            }
            UiAction::SelectHeroRace(r) => {
                bg.0 = if *r == hero.race {
                    Color::srgb(0.28, 0.40, 0.52)
                } else {
                    BTN_BG
                };
            }
            _ => {}
        }
    }
}

pub fn spawn_menu(
    mut commands: Commands,
    levels: Res<Levels>,
    progress: Res<Progress>,
    inv: Res<EquipmentInventory>,
    bestiary: Res<Bestiary>,
    fonts: Res<UiFont>,
    sprites: Res<Sprites>,
    assets: Res<AssetServer>,
    lang: Res<Language>,
    audio: Res<AudioSettings>,
    mut dirty: ResMut<MenuDirty>,
) {
    dirty.0 = false;
    let lang = lang.lang;
    let tr = |s: &str| tr(lang, s);
    let f = &fonts.0;
    let unlocked_count = progress.unlocked.min(levels.0.len());
    let discovered = MONSTER_SPECIES
        .iter()
        .filter(|species| bestiary.count(species.id) > 0)
        .count();
    let total_kills: u32 = MONSTER_SPECIES
        .iter()
        .map(|species| bestiary.count(species.id))
        .sum();
    let equipment_kinds = Equipment::ALL
        .iter()
        .filter(|item| inv.counts[item.idx()] > 0)
        .count();
    let total_rating: u32 = progress
        .stars
        .iter()
        .take(levels.0.len())
        .map(|stars| *stars as u32)
        .sum();
    let milestones = milestone_rows(&levels, &progress, &inv, &bestiary);
    let milestone_done = milestones.iter().filter(|row| row.complete()).count();
    let max_rating = levels.0.len() as u32 * 3;
    let next_front = if unlocked_count < levels.0.len() {
        crate::i18n::tf("下一封印：{}", &[&tr(levels.0[unlocked_count].name)])
    } else {
        tr("全部封印战线已开启")
    };
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Stretch,
                padding: UiRect::all(Val::Px(14.0)),
                column_gap: Val::Px(12.0),
                ..default()
            },
            BackgroundColor(UI_BG),
            MenuRoot,
        ))
        .with_children(|p| {
            // 右上角悬浮设置齿轮 → 打开设置弹窗（画质/音量/语言/全屏）。
            p.spawn((
                Button,
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(10.0),
                    right: Val::Px(10.0),
                    width: Val::Px(40.0),
                    height: Val::Px(40.0),
                    padding: UiRect::all(Val::Px(5.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.16, 0.22, 0.30, 0.92)),
                GlobalZIndex(60),
                UiAction::ToggleMenuSettings,
            ))
            .with_children(|b| {
                b.spawn((
                    ImageNode::new(sprites.ui["ic_gear"].clone()),
                    Node {
                        width: Val::Px(28.0),
                        height: Val::Px(28.0),
                        ..default()
                    },
                ));
            });
            // The settings popup itself (hidden until the gear is tapped).
            p.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(56.0),
                    right: Val::Px(10.0),
                    width: Val::Px(248.0),
                    display: Display::None,
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(12.0)),
                    row_gap: Val::Px(8.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.07, 0.08, 0.09, 0.99)),
                GlobalZIndex(61),
                MenuSettingsRoot,
            ))
            .with_children(|s| {
                s.spawn((
                    Text::new(tr("设置")),
                    text_font(f, 18.0),
                    TextColor(Color::srgb(0.9, 0.95, 1.0)),
                ));
                // 画质
                s.spawn((
                    Text::new(format!("{}：{}", tr("画质"), tr("标准"))),
                    text_font(f, 13.0),
                    TextColor(UI_ACCENT_TEAL),
                    QualityLabel,
                ));
                s.spawn(row_node()).with_children(|row| {
                    button(row, f, &tr("切换画质"), UiAction::CycleQuality, BTN_BG);
                    button(row, f, &tr("全屏"), UiAction::Fullscreen, BTN_BG);
                });
                // 音量
                s.spawn((
                    Text::new(format!("{}：{}%", tr("音量"), audio.percent())),
                    text_font(f, 13.0),
                    TextColor(UI_ACCENT_TEAL),
                    VolumeLabel,
                ));
                s.spawn(row_node()).with_children(|row| {
                    button(row, f, &format!("{} +/-", tr("音量")), UiAction::CycleVolume, BTN_BG);
                });
                // 语言
                s.spawn((
                    Text::new(format!("{}：{}", tr("语言"), lang.label())),
                    text_font(f, 13.0),
                    TextColor(UI_ACCENT_TEAL),
                    LanguageLabel,
                ));
                s.spawn(row_node()).with_children(|row| {
                    button(row, f, lang.next().label(), UiAction::CycleLanguage, BTN_BG);
                });
                s.spawn(row_node()).with_children(|row| {
                    button(row, f, &tr("关闭"), UiAction::ToggleMenuSettings, BTN_BG);
                });
            });
            p.spawn((
                Node {
                    width: Val::Px(318.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(12.0)),
                    row_gap: Val::Px(6.0),
                    // Scroll so the briefing/difficulty controls stay reachable even
                    // when the column is taller than a short (wide-aspect) window.
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
                BackgroundColor(UI_PANEL_DARK),
                ScrollPosition::default(),
            ))
            .with_children(|left| {
                left.spawn((
                    Text::new(tr("保卫萝卜")),
                    text_font(f, 36.0),
                    TextColor(UI_ACCENT_GOLD),
                ));
                left.spawn((
                    Text::new(tr("战术指挥室")),
                    text_font(f, 15.0),
                    TextColor(UI_ACCENT_TEAL),
                ));
                left.spawn((
                    Text::new(tr(PROLOGUE)),
                    text_font(f, 12.0),
                    TextColor(UI_TEXT_DIM),
                    Node {
                        margin: UiRect::bottom(Val::Px(4.0)),
                        ..default()
                    },
                ));
                left.spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(10.0)),
                        row_gap: Val::Px(5.0),
                        ..default()
                    },
                    BackgroundColor(UI_CARD),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Text::new(tr("战役进度")),
                        text_font(f, 13.0),
                        TextColor(UI_ACCENT_GOLD),
                    ));
                    // Icon stat rows instead of a text block (flag/star/bestiary/skull/
                    // equipment/trophy + number). Two rows of three. Each is a hoverable
                    // button carrying an Info tooltip explaining what the stat means.
                    let prog = [
                        (
                            "ic_flag",
                            format!("{}/{}", unlocked_count, levels.0.len()),
                            "战役进度\n已解锁战线 / 总战线",
                        ),
                        (
                            "ic_star",
                            format!("{}/{}", total_rating, max_rating),
                            "评级印章\n各关累计星级 / 满星",
                        ),
                        (
                            "nav_bestiary",
                            format!("{}/{}", discovered, MONSTER_SPECIES.len()),
                            "怪物图鉴\n已发现物种 / 总物种",
                        ),
                        (
                            "ic_skull",
                            format!("{}", total_kills),
                            "累计击杀\n历战消灭的敌人总数",
                        ),
                        (
                            "nav_armory",
                            format!("{}/{}", inv.total(), equipment_kinds),
                            "装备库存\n持有装备件数 / 装备种类",
                        ),
                        (
                            "nav_milestones",
                            format!("{}/{}", milestone_done, milestones.len()),
                            "成就里程碑\n已完成 / 总数",
                        ),
                    ];
                    for chunk in prog.chunks(3) {
                        panel
                            .spawn(Node {
                                flex_direction: FlexDirection::Row,
                                column_gap: Val::Px(10.0),
                                ..default()
                            })
                            .with_children(|row| {
                                for (icon, val, tip) in chunk {
                                    row.spawn((
                                        Button,
                                        Node {
                                            flex_direction: FlexDirection::Row,
                                            align_items: AlignItems::Center,
                                            column_gap: Val::Px(3.0),
                                            ..default()
                                        },
                                        UiAction::Info(*tip),
                                    ))
                                    .with_children(|c| {
                                        c.spawn((
                                            ImageNode::new(sprites.ui[*icon].clone()),
                                            Node {
                                                width: Val::Px(18.0),
                                                height: Val::Px(18.0),
                                                ..default()
                                            },
                                        ));
                                        c.spawn((
                                            Text::new(val),
                                            text_font(f, 15.0),
                                            TextColor(UI_TEXT),
                                        ));
                                    });
                                }
                            });
                    }
                    panel.spawn((
                        Text::new(next_front.clone()),
                        text_font(f, 11.0),
                        TextColor(UI_TEXT_DIM),
                    ));
                });

                left.spawn((
                    Text::new(tr("难度：普通")),
                    text_font(f, 14.0),
                    TextColor(UI_ACCENT_GOLD),
                    DiffLabel,
                ));
                left.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                })
                .with_children(|row| {
                    for difficulty in Difficulty::ALL {
                        icon_button(
                            row,
                            sprites.ui[difficulty_icon_key(difficulty)].clone(),
                            UiAction::SetDifficulty(difficulty),
                            difficulty_color(difficulty),
                            (),
                        );
                    }
                });

            });

            p.spawn((
                Node {
                    flex_grow: 1.0,
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(12.0)),
                    row_gap: Val::Px(9.0),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(UI_PANEL),
            ))
            .with_children(|right| {
                right
                    .spawn(Node {
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|head| {
                        head.spawn((
                            Text::new(tr("战线选择")),
                            text_font(f, 24.0),
                            TextColor(UI_TEXT),
                        ));
                        head.spawn((
                            Text::new(next_front),
                            text_font(f, 12.0),
                            TextColor(UI_TEXT_DIM),
                        ));
                    });

                right
                    .spawn(Node {
                        flex_direction: FlexDirection::Row,
                        flex_wrap: FlexWrap::Wrap,
                        justify_content: JustifyContent::Center,
                        row_gap: Val::Px(5.0),
                        column_gap: Val::Px(5.0),
                        ..default()
                    })
                    .with_children(|grid| {
                        for (i, _lvl) in levels.0.iter().enumerate() {
                            let open = i < progress.unlocked;
                            let stars = progress.stars.get(i).copied().unwrap_or(0).min(3);
                            // Each level is a map THUMBNAIL tile; the name / wave count /
                            // rating live in the hover tooltip (text → tooltip). Locked
                            // levels are darkened. Number + stars overlay on the art.
                            let tint = if open {
                                Color::WHITE
                            } else {
                                Color::srgb(0.20, 0.20, 0.24)
                            };
                            let mut entity = if open {
                                grid.spawn((Button, UiAction::PlayLevel(i)))
                            } else {
                                grid.spawn_empty()
                            };
                            entity
                                .insert((
                                    Node {
                                        width: Val::Px(120.0),
                                        height: Val::Px(70.0),
                                        flex_direction: FlexDirection::Column,
                                        justify_content: JustifyContent::SpaceBetween,
                                        padding: UiRect::all(Val::Px(4.0)),
                                        overflow: Overflow::clip(),
                                        ..default()
                                    },
                                    ImageNode {
                                        image: assets
                                            .load(format!("sprites/levels/lvl_{:02}.webp", i)),
                                        color: tint,
                                        ..default()
                                    },
                                ))
                                .with_children(|card| {
                                    // Level number chip (top-left).
                                    card.spawn((
                                        Node {
                                            align_self: AlignSelf::FlexStart,
                                            padding: UiRect::axes(Val::Px(5.0), Val::Px(1.0)),
                                            ..default()
                                        },
                                        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                                    ))
                                    .with_children(|chip| {
                                        chip.spawn((
                                            Text::new(format!("{:02}", i + 1)),
                                            text_font(f, 13.0),
                                            TextColor(UI_ACCENT_GOLD),
                                        ));
                                    });
                                    // Stars (open) or lock (closed) chip — bottom-left.
                                    card.spawn((
                                        Node {
                                            align_self: AlignSelf::FlexStart,
                                            padding: UiRect::axes(Val::Px(5.0), Val::Px(1.0)),
                                            ..default()
                                        },
                                        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                                    ))
                                    .with_children(|chip| {
                                        if open {
                                            let s = format!(
                                                "{}{}",
                                                "★".repeat(stars as usize),
                                                "☆".repeat((3 - stars) as usize)
                                            );
                                            chip.spawn((
                                                Text::new(s),
                                                text_font(f, 11.0),
                                                TextColor(Color::srgb(1.0, 0.85, 0.3)),
                                            ));
                                        } else {
                                            chip.spawn((
                                                ImageNode::new(
                                                    sprites.ui["ic_lock"].clone(),
                                                ),
                                                Node {
                                                    width: Val::Px(12.0),
                                                    height: Val::Px(12.0),
                                                    ..default()
                                                },
                                            ));
                                        }
                                    });
                                });
                        }
                    });

                right
                    .spawn((
                        Button,
                        UiAction::PlayEndless,
                        Node {
                            width: Val::Percent(100.0),
                            min_height: Val::Px(50.0),
                            flex_direction: FlexDirection::Column,
                            justify_content: JustifyContent::Center,
                            padding: UiRect::axes(Val::Px(12.0), Val::Px(7.0)),
                            row_gap: Val::Px(2.0),
                            ..default()
                        },
                        BackgroundColor(UI_ACCENT_RED.with_alpha(0.24)),
                    ))
                    .with_children(|card| {
                        card.spawn((
                            Text::new(tr("无尽模式")),
                            text_font(f, 16.0),
                            TextColor(Color::srgb(1.0, 0.62, 0.48)),
                        ));
                        card.spawn((
                            Text::new(tr("终章战场 · 无限波次 · 每5波首领 · 装备刷取核心模式")),
                            text_font(f, 11.0),
                            TextColor(Color::srgb(0.86, 0.74, 0.66)),
                        ));
                    });

                right
                    .spawn(Node {
                        flex_direction: FlexDirection::Row,
                        flex_wrap: FlexWrap::Wrap,
                        justify_content: JustifyContent::Center,
                        margin: UiRect::top(Val::Px(2.0)),
                        ..default()
                    })
                    .with_children(|row| {
                        // Icon nav (hover/tap shows the name via tooltip). 英雄图鉴 lives
                        // here alongside the other codices instead of on the left column.
                        for (key, action, bg) in [
                            ("nav_hero", UiAction::OpenHeroCodex, Color::srgb(0.30, 0.26, 0.14)),
                            ("nav_bestiary", UiAction::OpenBestiary, Color::srgb(0.26, 0.22, 0.34)),
                            ("nav_towers", UiAction::OpenTowerArchive, Color::srgb(0.18, 0.34, 0.34)),
                            ("nav_armory", UiAction::OpenArmory, Color::srgb(0.35, 0.29, 0.16)),
                            ("nav_milestones", UiAction::OpenMilestones, Color::srgb(0.34, 0.22, 0.36)),
                            ("nav_dossier", UiAction::OpenCampaignDossier, Color::srgb(0.22, 0.28, 0.40)),
                        ] {
                            icon_button(row, sprites.ui[key].clone(), action, bg, ());
                        }
                    });
            });
        });
    // Hover/tap tooltips for difficulty + the hero card on the menu.
    spawn_tooltip_box(&mut commands, f, MenuRoot);
}

/// Keep the menu difficulty label in sync with the selected difficulty.
pub fn update_menu_diff(
    diff: Res<GameDifficulty>,
    lang: Res<Language>,
    mut q: Query<&mut Text, With<DiffLabel>>,
) {
    let l = lang.lang;
    if let Ok(mut t) = q.single_mut() {
        t.0 = format!("{}：{}", tr(l, "难度"), tr(l, diff.0.name()));
    }
}

/// Full-screen flash: red when the carrot loses a life, gold on a kill-combo
/// milestone (x5/x10/…). Tracks previous life/combo counts in `Local`s to fire once
/// per event, then fades out.
pub fn update_screen_flash(
    time: Res<Time>,
    run: Res<RunState>,
    mut prev_lives: Local<Option<i32>>,
    mut prev_combo: Local<i32>,
    mut tint: Local<Option<Color>>,
    mut flash: Query<(&mut ScreenFlash, &mut BackgroundColor)>,
) {
    let dropped = matches!(*prev_lives, Some(prev) if run.lives < prev);
    *prev_lives = Some(run.lives);
    let combo = run.kill_combo;
    let milestone = combo >= 5 && combo % 5 == 0 && combo != *prev_combo;
    *prev_combo = combo;

    let dt = time.delta_secs();
    for (mut f, mut bg) in &mut flash {
        // Life loss (red) takes priority over a combo milestone (gold).
        if dropped {
            let max = run.start_lives.max(1);
            f.level = if run.lives * 3 <= max { 0.5 } else { 0.34 };
            *tint = Some(Color::srgb(1.0, 0.1, 0.08));
        } else if milestone {
            f.level = f.level.max(0.3);
            *tint = Some(Color::srgb(1.0, 0.82, 0.3));
        }
        f.level = (f.level - dt * 1.6).max(0.0);
        let mut col = tint.unwrap_or(Color::srgb(1.0, 0.1, 0.08));
        col.set_alpha(f.level);
        bg.0 = col;
    }
}

/// Drive the on-screen movement joystick from touch input. Claims the touch that
/// starts within the stick, tracks it, and publishes a normalized direction in
/// [`JoystickState`] (consumed by `hero_move`). Also slides the knob to follow.
pub fn hero_joystick(
    mode: Res<TouchMode>,
    touches: Res<Touches>,
    windows: Query<&Window>,
    mut state: ResMut<JoystickState>,
    mut base: Query<&mut Node, (With<JoystickBase>, Without<JoystickKnob>)>,
    mut knob: Query<&mut Node, With<JoystickKnob>>,
) {
    // The floating joystick is TOUCH-ONLY. `TouchMode` is the single global flag for
    // PC-vs-mobile; in PC mode the joystick must never engage (mouse uses click-select
    // / right-click-move), so touch input can't leak into desktop behaviour.
    if !mode.0 {
        if let Ok(mut node) = base.single_mut() {
            if node.display != Display::None {
                node.display = Display::None;
            }
        }
        if state.touch.is_some() || state.dir != Vec2::ZERO {
            state.touch = None;
            state.dir = Vec2::ZERO;
        }
        return;
    }
    let Ok(win) = windows.single() else {
        return;
    };

    // Floating joystick: a fresh touch anywhere in the left movement zone spawns the
    // stick at that point. The zone excludes the left control strip (x<56), the top
    // status row (y<96), and the right ~40% (build panel / tower placement).
    if state.touch.is_none() {
        for t in touches.iter_just_pressed() {
            let p = t.position();
            if p.x >= 56.0 && p.x <= win.width() * 0.60 && p.y >= 96.0 {
                state.touch = Some(t.id());
                state.origin = p;
                break;
            }
        }
    }

    let mut offset = Vec2::ZERO;
    let mut active = false;
    if let Some(id) = state.touch {
        if let Some(t) = touches.iter().find(|t| t.id() == id) {
            active = true;
            let raw = t.position() - state.origin;
            offset = raw.clamp_length_max(JOY_RADIUS);
            // Screen y grows downward; world y grows up → invert y for the dir.
            state.dir = Vec2::new(offset.x, -offset.y) / JOY_RADIUS;
        } else {
            // Finger lifted — hide and stop.
            state.touch = None;
            state.dir = Vec2::ZERO;
        }
    } else {
        state.dir = Vec2::ZERO;
    }

    // Show the base at the touch origin while active; hide it otherwise.
    if let Ok(mut node) = base.single_mut() {
        if active {
            node.display = Display::Flex;
            node.left = Val::Px(state.origin.x - JOY_RADIUS);
            node.top = Val::Px(state.origin.y - JOY_RADIUS);
        } else if node.display != Display::None {
            node.display = Display::None;
        }
    }
    if let Ok(mut node) = knob.single_mut() {
        node.left = Val::Px(JOY_RADIUS - 24.0 + offset.x);
        node.top = Val::Px(JOY_RADIUS - 24.0 + offset.y);
    }
}

/// True if the given screen point lies within the joystick (so board-tap systems
/// can ignore joystick gestures).
pub fn in_joystick(screen: Vec2, win: &Window) -> bool {
    // The left movement zone (matches `hero_joystick`): taps here drive the floating
    // joystick, so they must not also command a tap-to-move.
    screen.x >= 56.0 && screen.x <= win.width() * 0.60 && screen.y >= 96.0
}

/// Keep the menu hero label in sync with the chosen race + class.
pub fn update_hero_label(hero: Res<HeroLoadout>, mut q: Query<&mut Text, With<HeroLabel>>) {
    for mut t in &mut q {
        // Compact: portraits/icons carry the visuals; talents are managed in-game.
        t.0 = format!(
            "{}·{} Lv{}\n{}",
            hero.race.name(),
            hero.class.name(),
            hero.level,
            hero.class.role(),
        );
    }
}

/// In-game summon button for the unique hero (kept out of `hud_buttons` to stay
/// within the system-param limit).
pub fn hero_buttons(
    mut commands: Commands,
    interactions: Query<(&Interaction, &UiAction), Changed<Interaction>>,
    mut run: ResMut<RunState>,
    mut loadout: ResMut<HeroLoadout>,
    mut towers: Query<(Entity, &mut crate::tower::Tower)>,
    enemies: Query<(Entity, &Enemy, &Transform)>,
    sprites: Res<Sprites>,
    walks: Res<crate::build::HeroWalks>,
    mut sfx: MessageWriter<crate::audio::SfxEvent>,
    mut dmg: MessageWriter<Damage>,
    mut status: MessageWriter<Status>,
    mut buff: MessageWriter<BuffTower>,
    mut vfx: MessageWriter<crate::vfx::VfxEvent>,
) {
    for (interaction, action) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action {
            UiAction::SummonHero => {
                // The hero is free and auto-present; this button just force-respawns
                // it instantly if it has died (no gold, no wait).
                if towers.iter().any(|(_, t)| t.hero) {
                    run.show(crate::i18n::t("英雄已在场上"));
                    continue;
                }
                loadout.respawn_waves = 0;
                let pos = crate::hero::hero_spawn_pos();
                let tower = crate::hero::make_hero_tower(&loadout, pos);
                crate::build::spawn_hero(&mut commands, tower, &sprites, &walks, loadout.class);
                loadout.alive = true;
                run.show(crate::i18n::tf(
                    "英雄降临：{}·{}（点击英雄选中，再点地面移动）",
                    &[&crate::i18n::t(loadout.race.name()), &crate::i18n::t(loadout.class.name())],
                ));
                sfx.write(crate::audio::SfxEvent(crate::audio::Sound::Raise));
                vfx.write(crate::vfx::VfxEvent::Burst {
                    pos,
                    radius: 64.0,
                    color: loadout.race.color(),
                });
            }
            UiAction::HeroTalent(index) => match loadout.add_talent(*index) {
                Ok(()) => {
                    let mut applied = false;
                    for (_, mut tower) in &mut towers {
                        if tower.hero {
                            crate::hero::apply_loadout_to_tower(&loadout, &mut tower);
                            applied = true;
                        }
                    }
                    run.show(crate::i18n::tf(
                        "{} +1（{}/{}）",
                        &[
                            &crate::i18n::t(loadout.class.talent_name(*index)),
                            &loadout.talent_rank(*index).to_string(),
                            &crate::hero::HeroLoadout::TALENT_MAX_RANK.to_string(),
                        ],
                    ));
                    if applied {
                        sfx.write(crate::audio::SfxEvent(crate::audio::Sound::Upgrade));
                    }
                }
                Err(msg) => run.show(crate::i18n::t(msg)),
            },
            UiAction::ResetHeroTalents => {
                let refunded = loadout.respec_current_class();
                if refunded > 0 {
                    for (_, mut tower) in &mut towers {
                        if tower.hero {
                            crate::hero::apply_loadout_to_tower(&loadout, &mut tower);
                        }
                    }
                    run.show(crate::i18n::tf(
                        "重置{}天赋，返还 {} 点",
                        &[&crate::i18n::t(loadout.class.name()), &refunded.to_string()],
                    ));
                } else {
                    run.show(crate::i18n::t("当前职业没有已投入天赋"));
                }
            }
            UiAction::HeroSkill => {
                if loadout.skill_cd > 0 {
                    run.show(crate::i18n::tf("英雄技能冷却中，还需 {} 波", &[&loadout.skill_cd.to_string()]));
                    continue;
                }
                let hero_source =
                    towers
                        .iter_mut()
                        .find(|(_, tower)| tower.hero)
                        .map(|(hero_entity, hero)| {
                            (
                                hero_entity,
                                HeroSkillSource {
                                    pos: hero.center(),
                                    damage: hero.damage,
                                    element: hero.element,
                                    max_hp: hero.max_hp,
                                },
                            )
                        });
                let Some((hero_entity, source)) = hero_source else {
                    run.show(crate::i18n::t("先召唤英雄"));
                    continue;
                };
                if cast_hero_skill(
                    hero_entity,
                    source,
                    &mut loadout,
                    &mut towers,
                    &enemies,
                    &mut dmg,
                    &mut status,
                    &mut buff,
                    &mut vfx,
                    &mut run,
                ) {
                    loadout.skill_cd = loadout.skill_cooldown_max();
                    sfx.write(crate::audio::SfxEvent(match loadout.class {
                        crate::hero::Class::Warrior => crate::audio::Sound::Boss,
                        crate::hero::Class::Mage => crate::audio::Sound::Meteor,
                        crate::hero::Class::Ranger => crate::audio::Sound::Chain,
                        crate::hero::Class::Guardian => crate::audio::Sound::Raise,
                        crate::hero::Class::Stormcaller => crate::audio::Sound::Chain,
                        crate::hero::Class::Warden => crate::audio::Sound::Upgrade,
                        crate::hero::Class::Assassin => crate::audio::Sound::Chain,
                        crate::hero::Class::Priest => crate::audio::Sound::Raise,
                        crate::hero::Class::Engineer => crate::audio::Sound::Upgrade,
                    }));
                }
            }
            _ => {}
        }
    }
}

#[derive(Clone, Copy)]
struct HeroSkillSource {
    pos: Vec2,
    damage: f32,
    element: Element,
    max_hp: f32,
}

fn cast_hero_skill(
    hero_entity: Entity,
    source: HeroSkillSource,
    loadout: &mut HeroLoadout,
    towers: &mut Query<(Entity, &mut crate::tower::Tower)>,
    enemies: &Query<(Entity, &Enemy, &Transform)>,
    dmg: &mut MessageWriter<Damage>,
    status: &mut MessageWriter<Status>,
    buff: &mut MessageWriter<BuffTower>,
    vfx: &mut MessageWriter<crate::vfx::VfxEvent>,
    run: &mut RunState,
) -> bool {
    let hero_pos = source.pos;
    let mult = loadout.skill_damage_mult();
    match loadout.class {
        crate::hero::Class::Warrior => {
            let radius = 108.0 + loadout.talent_rank(0) as f32 * 10.0;
            let amount = (190.0 + source.damage * 1.25) * mult;
            let mut hits = 0;
            for (enemy, _, tf) in enemies {
                if tf.translation.truncate().distance(hero_pos) <= radius {
                    hits += 1;
                    dmg.write(Damage {
                        source_tower: Some(hero_entity),
                        target: enemy,
                        amount,
                        magic: false,
                        element: source.element,
                        armor_pierce: 18.0 + loadout.talent_rank(0) as f32 * 5.0,
                    });
                    status.write(Status {
                        source_tower: Some(hero_entity),
                        target: enemy,
                        kind: StatusKind::Freeze { duration: 0.55 },
                    });
                }
            }
            if hits == 0 {
                run.show(crate::i18n::t("战旗冲锋没有命中目标"));
                return false;
            }
            if let Ok((_, mut hero)) = towers.get_mut(hero_entity) {
                hero.hp = (hero.hp + hero.max_hp * (0.18 + loadout.talent_rank(1) as f32 * 0.035))
                    .min(hero.max_hp);
            }
            vfx.write(crate::vfx::VfxEvent::Burst {
                pos: hero_pos,
                radius,
                color: loadout.class.skill_color(),
            });
            run.show(crate::i18n::tf(
                "{}命中 {} 个敌人",
                &[&crate::i18n::t(loadout.class.skill_name()), &hits.to_string()],
            ));
            true
        }
        crate::hero::Class::Mage => {
            let target = enemies
                .iter()
                .max_by(|a, b| a.1.hp.total_cmp(&b.1.hp))
                .map(|(_, _, tf)| tf.translation.truncate());
            let Some(center) = target else {
                run.show(crate::i18n::t("星火风暴没有可锁定目标"));
                return false;
            };
            let radius = 112.0 + loadout.talent_rank(1) as f32 * 16.0;
            let amount = 300.0 * mult;
            let freeze = 0.75 + loadout.talent_rank(2) as f32 * 0.18;
            let mut hits = 0;
            for (enemy, _, tf) in enemies {
                if tf.translation.truncate().distance(center) <= radius {
                    hits += 1;
                    dmg.write(Damage {
                        source_tower: Some(hero_entity),
                        target: enemy,
                        amount,
                        magic: true,
                        element: Element::Arcane,
                        armor_pierce: 0.0,
                    });
                    status.write(Status {
                        source_tower: Some(hero_entity),
                        target: enemy,
                        kind: StatusKind::Freeze { duration: freeze },
                    });
                }
            }
            vfx.write(crate::vfx::VfxEvent::Explosion {
                pos: center,
                radius,
                color: loadout.class.skill_color(),
            });
            run.show(crate::i18n::tf(
                "{}席卷 {} 个敌人",
                &[&crate::i18n::t(loadout.class.skill_name()), &hits.to_string()],
            ));
            true
        }
        crate::hero::Class::Ranger => {
            let mut targets = enemies
                .iter()
                .map(|(entity, enemy, tf)| (entity, enemy.path_index, tf.translation.truncate()))
                .collect::<Vec<_>>();
            targets.sort_by(|a, b| b.1.cmp(&a.1));
            let shots = 4 + ((loadout.talent_rank(2) + loadout.talent_rank(3)) / 2) as usize;
            let selected = targets.into_iter().take(shots).collect::<Vec<_>>();
            if selected.is_empty() {
                run.show(crate::i18n::t("猎影齐射没有目标"));
                return false;
            }
            let amount = 210.0 * mult;
            let poison = 34.0 + loadout.talent_rank(2) as f32 * 13.0;
            for (enemy, _, pos) in selected.iter().copied() {
                dmg.write(Damage {
                    source_tower: Some(hero_entity),
                    target: enemy,
                    amount,
                    magic: false,
                    element: Element::Physical,
                    armor_pierce: 34.0 + loadout.talent_rank(0) as f32 * 6.0,
                });
                status.write(Status {
                    source_tower: Some(hero_entity),
                    target: enemy,
                    kind: StatusKind::Poison {
                        dmg: poison,
                        duration: 4.0 + loadout.talent_rank(2) as f32 * 0.45,
                    },
                });
                status.write(Status {
                    source_tower: Some(hero_entity),
                    target: enemy,
                    kind: StatusKind::Slow { duration: 1.4 },
                });
                vfx.write(crate::vfx::VfxEvent::Muzzle {
                    pos: hero_pos,
                    dir: (pos - hero_pos).normalize_or_zero(),
                    color: loadout.class.skill_color(),
                });
            }
            run.show(crate::i18n::tf(
                "{}锁定 {} 个目标",
                &[&crate::i18n::t(loadout.class.skill_name()), &selected.len().to_string()],
            ));
            true
        }
        crate::hero::Class::Guardian => {
            let radius =
                132.0 + loadout.talent_rank(1) as f32 * 18.0 + loadout.talent_rank(5) as f32 * 10.0;
            let repaired = repair_and_buff_towers(
                hero_entity,
                hero_pos,
                radius,
                0.08 + loadout.talent_rank(3) as f32 * 0.025,
                2 + (loadout.talent_rank(1) / 2) as usize,
                towers,
                buff,
                vfx,
                loadout.class.skill_color(),
            );
            let hero_healed = heal_hero(
                hero_entity,
                towers,
                source.max_hp * (0.16 + loadout.talent_rank(0) as f32 * 0.02),
            );
            let mut hits = 0;
            let amount = (115.0 + source.damage * 0.75) * mult;
            for (enemy, _, tf) in enemies {
                if tf.translation.truncate().distance(hero_pos) <= radius {
                    hits += 1;
                    dmg.write(Damage {
                        source_tower: Some(hero_entity),
                        target: enemy,
                        amount,
                        magic: false,
                        element: source.element,
                        armor_pierce: 18.0,
                    });
                    status.write(Status {
                        source_tower: Some(hero_entity),
                        target: enemy,
                        kind: StatusKind::Freeze {
                            duration: 0.42 + loadout.talent_rank(4) as f32 * 0.12,
                        },
                    });
                }
            }
            if hits == 0 && repaired == 0 && !hero_healed {
                run.show(crate::i18n::t("守护壁垒没有覆盖目标"));
                return false;
            }
            vfx.write(crate::vfx::VfxEvent::Burst {
                pos: hero_pos,
                radius,
                color: loadout.class.skill_color(),
            });
            run.show(crate::i18n::tf(
                "{}鼓舞 {} 座塔，压制 {} 个敌人",
                &[
                    &crate::i18n::t(loadout.class.skill_name()),
                    &repaired.to_string(),
                    &hits.to_string(),
                ],
            ));
            true
        }
        crate::hero::Class::Stormcaller => {
            let tower_hits = if loadout.talent_rank(4) > 0 {
                repair_and_buff_towers(
                    hero_entity,
                    hero_pos,
                    118.0 + loadout.talent_rank(4) as f32 * 16.0,
                    0.0,
                    1 + (loadout.talent_rank(4) / 2) as usize,
                    towers,
                    buff,
                    vfx,
                    loadout.class.skill_color(),
                )
            } else {
                0
            };
            let target = enemies
                .iter()
                .max_by_key(|(_, enemy, _)| enemy.path_index)
                .map(|(_, _, tf)| tf.translation.truncate());
            let Some(center) = target else {
                if tower_hits > 0 {
                    run.show(crate::i18n::tf(
                        "{}超频 {} 座塔",
                        &[&crate::i18n::t(loadout.class.skill_name()), &tower_hits.to_string()],
                    ));
                    return true;
                }
                run.show(crate::i18n::t("雷云审判没有可锁定目标"));
                return false;
            };
            let radius = 118.0 + loadout.talent_rank(5) as f32 * 14.0;
            let amount = 245.0 * mult;
            let mut hits = 0;
            for (enemy, _, tf) in enemies {
                if tf.translation.truncate().distance(center) <= radius {
                    hits += 1;
                    dmg.write(Damage {
                        source_tower: Some(hero_entity),
                        target: enemy,
                        amount,
                        magic: true,
                        element: Element::Storm,
                        armor_pierce: 0.0,
                    });
                    status.write(Status {
                        source_tower: Some(hero_entity),
                        target: enemy,
                        kind: StatusKind::Slow {
                            duration: 1.0 + loadout.talent_rank(3) as f32 * 0.22,
                        },
                    });
                }
            }
            vfx.write(crate::vfx::VfxEvent::Explosion {
                pos: center,
                radius,
                color: loadout.class.skill_color(),
            });
            run.show(crate::i18n::tf(
                "{}轰击 {} 个敌人，超频 {} 座塔",
                &[
                    &crate::i18n::t(loadout.class.skill_name()),
                    &hits.to_string(),
                    &tower_hits.to_string(),
                ],
            ));
            true
        }
        crate::hero::Class::Warden => {
            let radius =
                140.0 + loadout.talent_rank(1) as f32 * 18.0 + loadout.talent_rank(5) as f32 * 10.0;
            let tower_hits = repair_and_buff_towers(
                hero_entity,
                hero_pos,
                radius,
                loadout.talent_rank(4) as f32 * 0.018,
                2 + (loadout.talent_rank(1) / 2) as usize,
                towers,
                buff,
                vfx,
                loadout.class.skill_color(),
            );
            let mut hits = 0;
            for (enemy, _, tf) in enemies {
                if tf.translation.truncate().distance(hero_pos) <= radius {
                    hits += 1;
                    dmg.write(Damage {
                        source_tower: Some(hero_entity),
                        target: enemy,
                        amount: (72.0 + source.damage * 0.45) * mult,
                        magic: true,
                        element: Element::Frost,
                        armor_pierce: 0.0,
                    });
                    status.write(Status {
                        source_tower: Some(hero_entity),
                        target: enemy,
                        kind: StatusKind::Slow {
                            duration: 1.4 + loadout.talent_rank(2) as f32 * 0.28,
                        },
                    });
                    status.write(Status {
                        source_tower: Some(hero_entity),
                        target: enemy,
                        kind: StatusKind::Curse {
                            reduce: 8.0 + loadout.talent_rank(2) as f32 * 3.0,
                            duration: 2.2 + loadout.talent_rank(5) as f32 * 0.25,
                        },
                    });
                }
            }
            if tower_hits == 0 && hits == 0 {
                run.show(crate::i18n::t("哨戒结界没有覆盖目标"));
                return false;
            }
            vfx.write(crate::vfx::VfxEvent::Burst {
                pos: hero_pos,
                radius,
                color: loadout.class.skill_color(),
            });
            run.show(crate::i18n::tf(
                "{}强化 {} 座塔，缠绕 {} 个敌人",
                &[
                    &crate::i18n::t(loadout.class.skill_name()),
                    &tower_hits.to_string(),
                    &hits.to_string(),
                ],
            ));
            true
        }
        crate::hero::Class::Assassin => {
            let mut targets = enemies
                .iter()
                .map(|(entity, enemy, tf)| {
                    (
                        entity,
                        enemy.path_index,
                        enemy.hp / enemy.max_hp.max(1.0),
                        tf.translation.truncate(),
                    )
                })
                .collect::<Vec<_>>();
            targets.sort_by(|a, b| b.1.cmp(&a.1));
            let marks = 3 + ((loadout.talent_rank(2) + loadout.talent_rank(5)) / 2) as usize;
            let selected = targets.into_iter().take(marks).collect::<Vec<_>>();
            if selected.is_empty() {
                run.show(crate::i18n::t("死印爆发没有目标"));
                return false;
            }
            for (enemy, _, hp_frac, pos) in selected.iter().copied() {
                let execute = 1.0 + (1.0 - hp_frac).clamp(0.0, 0.75) * 0.9;
                dmg.write(Damage {
                    source_tower: Some(hero_entity),
                    target: enemy,
                    amount: 230.0 * mult * execute,
                    magic: true,
                    element: Element::Shadow,
                    armor_pierce: 24.0 + loadout.talent_rank(0) as f32 * 6.0,
                });
                status.write(Status {
                    source_tower: Some(hero_entity),
                    target: enemy,
                    kind: StatusKind::Poison {
                        dmg: 42.0 + loadout.talent_rank(1) as f32 * 14.0,
                        duration: 4.2 + loadout.talent_rank(1) as f32 * 0.45,
                    },
                });
                status.write(Status {
                    source_tower: Some(hero_entity),
                    target: enemy,
                    kind: StatusKind::Curse {
                        reduce: 12.0 + loadout.talent_rank(4) as f32 * 5.0,
                        duration: 2.4 + loadout.talent_rank(4) as f32 * 0.3,
                    },
                });
                vfx.write(crate::vfx::VfxEvent::Muzzle {
                    pos: hero_pos,
                    dir: (pos - hero_pos).normalize_or_zero(),
                    color: loadout.class.skill_color(),
                });
            }
            run.show(crate::i18n::tf(
                "{}标记 {} 个目标",
                &[&crate::i18n::t(loadout.class.skill_name()), &selected.len().to_string()],
            ));
            true
        }
        crate::hero::Class::Priest => {
            let radius =
                145.0 + loadout.talent_rank(1) as f32 * 20.0 + loadout.talent_rank(4) as f32 * 10.0;
            let tower_hits = repair_and_buff_towers(
                hero_entity,
                hero_pos,
                radius,
                0.07 + loadout.talent_rank(4) as f32 * 0.025,
                1 + (loadout.talent_rank(1) / 2) as usize + (loadout.talent_rank(5) / 3) as usize,
                towers,
                buff,
                vfx,
                loadout.class.skill_color(),
            );
            let hero_healed = heal_hero(
                hero_entity,
                towers,
                source.max_hp * (0.12 + loadout.talent_rank(0) as f32 * 0.025),
            );
            let mut hits = 0;
            for (enemy, _, tf) in enemies {
                if tf.translation.truncate().distance(hero_pos) <= radius {
                    hits += 1;
                    dmg.write(Damage {
                        source_tower: Some(hero_entity),
                        target: enemy,
                        amount: (92.0 + source.damage * 0.45) * mult,
                        magic: true,
                        element: Element::Arcane,
                        armor_pierce: 0.0,
                    });
                    status.write(Status {
                        source_tower: Some(hero_entity),
                        target: enemy,
                        kind: StatusKind::Curse {
                            reduce: 12.0 + loadout.talent_rank(3) as f32 * 4.0,
                            duration: 2.4 + loadout.talent_rank(3) as f32 * 0.28,
                        },
                    });
                    status.write(Status {
                        source_tower: Some(hero_entity),
                        target: enemy,
                        kind: StatusKind::Slow { duration: 1.0 },
                    });
                }
            }
            if tower_hits == 0 && hits == 0 && !hero_healed {
                run.show(crate::i18n::t("圣辉祷言没有覆盖目标"));
                return false;
            }
            vfx.write(crate::vfx::VfxEvent::Burst {
                pos: hero_pos,
                radius,
                color: loadout.class.skill_color(),
            });
            run.show(crate::i18n::tf(
                "{}祝福 {} 座塔，虚弱 {} 个敌人",
                &[
                    &crate::i18n::t(loadout.class.skill_name()),
                    &tower_hits.to_string(),
                    &hits.to_string(),
                ],
            ));
            true
        }
        crate::hero::Class::Engineer => {
            let radius =
                135.0 + loadout.talent_rank(2) as f32 * 18.0 + loadout.talent_rank(5) as f32 * 10.0;
            let tower_hits = repair_and_buff_towers(
                hero_entity,
                hero_pos,
                radius,
                0.04 + loadout.talent_rank(3) as f32 * 0.02,
                2 + (loadout.talent_rank(1) / 2) as usize + (loadout.talent_rank(5) / 3) as usize,
                towers,
                buff,
                vfx,
                loadout.class.skill_color(),
            );
            let mut hits = 0;
            for (enemy, _, tf) in enemies {
                if tf.translation.truncate().distance(hero_pos) <= radius {
                    hits += 1;
                    dmg.write(Damage {
                        source_tower: Some(hero_entity),
                        target: enemy,
                        amount: (110.0 + source.damage * 0.55) * mult,
                        magic: true,
                        element: Element::Storm,
                        armor_pierce: loadout.talent_rank(4) as f32 * 4.0,
                    });
                    status.write(Status {
                        source_tower: Some(hero_entity),
                        target: enemy,
                        kind: StatusKind::Slow {
                            duration: 1.0 + loadout.talent_rank(4) as f32 * 0.22,
                        },
                    });
                }
            }
            if tower_hits == 0 && hits == 0 {
                run.show(crate::i18n::t("过载装置没有覆盖目标"));
                return false;
            }
            vfx.write(crate::vfx::VfxEvent::Burst {
                pos: hero_pos,
                radius,
                color: loadout.class.skill_color(),
            });
            run.show(crate::i18n::tf(
                "{}超频 {} 座塔，脉冲 {} 个敌人",
                &[
                    &crate::i18n::t(loadout.class.skill_name()),
                    &tower_hits.to_string(),
                    &hits.to_string(),
                ],
            ));
            true
        }
    }
}

fn heal_hero(
    hero_entity: Entity,
    towers: &mut Query<(Entity, &mut crate::tower::Tower)>,
    amount: f32,
) -> bool {
    let Ok((_, mut hero)) = towers.get_mut(hero_entity) else {
        return false;
    };
    if amount <= 0.0 || hero.hp >= hero.max_hp {
        return false;
    }
    hero.hp = (hero.hp + amount).min(hero.max_hp);
    true
}

fn repair_and_buff_towers(
    hero_entity: Entity,
    center: Vec2,
    radius: f32,
    repair_frac: f32,
    buff_stacks: usize,
    towers: &mut Query<(Entity, &mut crate::tower::Tower)>,
    buff: &mut MessageWriter<BuffTower>,
    vfx: &mut MessageWriter<crate::vfx::VfxEvent>,
    color: Color,
) -> usize {
    let mut affected = 0;
    for (entity, mut tower) in towers.iter_mut() {
        if entity == hero_entity || tower.center().distance(center) > radius {
            continue;
        }
        affected += 1;
        if repair_frac > 0.0 && tower.hp < tower.max_hp {
            tower.hp = (tower.hp + tower.max_hp * repair_frac).min(tower.max_hp);
            vfx.write(crate::vfx::VfxEvent::Heal {
                pos: tower.center(),
            });
        } else {
            vfx.write(crate::vfx::VfxEvent::ElementPulse {
                pos: tower.center(),
                color,
                strong: false,
            });
        }
        for _ in 0..buff_stacks {
            buff.write(BuffTower { target: entity });
        }
    }
    affected
}

/// Keep every graphics-quality label (menu + in-game panel) in sync. Runs every
/// frame (only a couple of labels) so freshly-spawned ones show the right tier.
pub fn update_quality_label(
    quality: Res<GraphicsQuality>,
    lang: Res<Language>,
    mut q: Query<&mut Text, With<QualityLabel>>,
) {
    let l = lang.lang;
    for mut t in &mut q {
        t.0 = format!("{}：{}", tr(l, "画质"), tr(l, quality.level.name()));
    }
}

pub fn update_volume_label(
    audio: Res<AudioSettings>,
    lang: Res<Language>,
    mut q: Query<&mut Text, With<VolumeLabel>>,
) {
    let l = lang.lang;
    for mut t in &mut q {
        t.0 = format!("{}：{}%", tr(l, "音量"), audio.percent());
    }
}

pub fn update_language_label(
    lang: Res<Language>,
    mut q: Query<&mut Text, With<LanguageLabel>>,
) {
    let l = lang.lang;
    for mut t in &mut q {
        t.0 = format!("{}：{}", tr(l, "语言"), l.label());
    }
}

pub fn menu_buttons(
    interactions: Query<(&Interaction, &UiAction), Changed<Interaction>>,
    mut current: ResMut<CurrentLevel>,
    mut diff: ResMut<GameDifficulty>,
    mut quality: ResMut<GraphicsQuality>,
    mut audio: ResMut<AudioSettings>,
    mut lang: ResMut<Language>,
    mut dirty: ResMut<MenuDirty>,
    mut hero: ResMut<HeroLoadout>,
    levels: Res<Levels>,
    mut mode: ResMut<GameMode>,
    mut next: ResMut<NextState<GameState>>,
    mut settings: Query<&mut Node, With<MenuSettingsRoot>>,
) {
    for (interaction, action) in &interactions {
        if *interaction == Interaction::Pressed {
            match action {
                UiAction::CycleQuality => quality.cycle(),
                UiAction::CycleVolume => audio.cycle(),
                UiAction::CycleLanguage => {
                    lang.cycle();
                    // Update the global immediately so the rebuild below (and any
                    // crate::i18n::t() calls during it) use the new language this frame.
                    crate::i18n::set_current_lang(lang.lang);
                    // Rebuild the whole menu so every translated string re-renders.
                    dirty.0 = true;
                }
                UiAction::ToggleMenuSettings => {
                    for mut node in &mut settings {
                        node.display = if node.display == Display::None {
                            Display::Flex
                        } else {
                            Display::None
                        };
                    }
                }
                UiAction::SelectHeroRace(r) => hero.set_race(*r),
                UiAction::SelectHeroClass(c) => hero.set_class(*c),
                UiAction::PlayLevel(i) => {
                    mode.0 = RunMode::Campaign;
                    current.0 = *i;
                    next.set(if *i == 0 {
                        GameState::Story
                    } else {
                        GameState::Briefing
                    });
                }
                UiAction::PlayEndless => {
                    mode.0 = RunMode::Endless;
                    current.0 = levels.0.len().saturating_sub(1);
                    next.set(GameState::Story);
                }
                UiAction::OpenBestiary => next.set(GameState::Bestiary),
                UiAction::OpenArmory => next.set(GameState::Armory),
                UiAction::OpenTowerArchive => next.set(GameState::TowerArchive),
                UiAction::OpenMilestones => next.set(GameState::Milestones),
                UiAction::OpenCampaignDossier => next.set(GameState::CampaignDossier),
                UiAction::OpenHeroCodex => next.set(GameState::HeroCodex),
                UiAction::SetDifficulty(d) => diff.0 = *d,
                _ => {}
            }
        }
    }
}

// ============================ Bestiary ============================

pub fn spawn_bestiary(
    mut commands: Commands,
    fonts: Res<UiFont>,
    sprites: Res<Sprites>,
    bestiary: Res<Bestiary>,
) {
    let f = &fonts.0;
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(12.0)),
                row_gap: Val::Px(8.0),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            BackgroundColor(UI_BG),
            ScrollPosition::default(),
            BestiaryRoot,
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(crate::i18n::t("怪物图鉴")),
                text_font(f, 30.0),
                TextColor(Color::srgb(0.85, 0.5, 0.9)),
            ));
            let total: u32 = MONSTER_SPECIES.iter().map(|s| bestiary.count(s.id)).sum();
            let found = MONSTER_SPECIES
                .iter()
                .filter(|s| bestiary.count(s.id) > 0)
                .count();
            p.spawn((
                Text::new(crate::i18n::tf(
                    "已发现 {}/{} 种   总击杀 {}",
                    &[&found.to_string(), &MONSTER_SPECIES.len().to_string(), &total.to_string()],
                )),
                text_font(f, 14.0),
                TextColor(Color::srgb(0.7, 0.7, 0.8)),
            ));
            p.spawn(Node {
                width: Val::Px(720.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                justify_content: JustifyContent::Center,
                ..default()
            })
            .with_children(|grid| {
                for species in MONSTER_SPECIES {
                    let n = bestiary.count(species.id);
                    grid.spawn((
                        Node {
                            width: Val::Px(165.0),
                            height: Val::Px(160.0),
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            row_gap: Val::Px(2.0),
                            margin: UiRect::all(Val::Px(4.0)),
                            padding: UiRect::all(Val::Px(6.0)),
                            ..default()
                        },
                        BackgroundColor(UI_CARD),
                    ))
                    .with_children(|cell| {
                        if n > 0 {
                            cell.spawn((
                                ImageNode {
                                    image: sprites.species[&species.id].clone(),
                                    ..default()
                                },
                                Node {
                                    width: Val::Px(52.0),
                                    height: Val::Px(52.0),
                                    ..default()
                                },
                            ));
                            cell.spawn((
                                Text::new(crate::i18n::t(species.name)),
                                text_font(f, 14.0),
                                TextColor(Color::WHITE),
                            ));
                            // 品级徽章：按威胁度分级（普通/精英/稀有/史诗/首领），颜色区分。
                            let grade = species.grade();
                            cell.spawn((
                                Node {
                                    padding: UiRect::axes(Val::Px(6.0), Val::Px(1.0)),
                                    margin: UiRect::top(Val::Px(1.0)),
                                    ..default()
                                },
                                BackgroundColor(grade.color().with_alpha(0.22)),
                            ))
                            .with_children(|badge| {
                                badge.spawn((
                                    Text::new(crate::i18n::t(grade.name())),
                                    text_font(f, 11.0),
                                    TextColor(grade.color()),
                                ));
                            });
                            cell.spawn((
                                Text::new(crate::i18n::tf("击杀 {}", &[&n.to_string()])),
                                text_font(f, 12.0),
                                TextColor(Color::srgb(1.0, 0.84, 0.2)),
                            ));
                            cell.spawn((
                                Text::new(brief(species)),
                                text_font(f, 10.0),
                                TextColor(Color::srgb(0.7, 0.8, 0.7)),
                            ));
                        } else {
                            cell.spawn((
                                Text::new("？？？"),
                                text_font(f, 24.0),
                                TextColor(Color::srgb(0.35, 0.35, 0.4)),
                            ));
                            cell.spawn((
                                Text::new(crate::i18n::t("未发现")),
                                text_font(f, 12.0),
                                TextColor(Color::srgb(0.4, 0.4, 0.45)),
                            ));
                        }
                    });
                }
            });

            // ---- 怪物技能图鉴 (skill codex) ----
            p.spawn((
                Text::new(crate::i18n::t("怪物技能图鉴")),
                text_font(f, 22.0),
                TextColor(Color::srgb(0.95, 0.78, 0.42)),
                Node {
                    margin: UiRect::top(Val::Px(14.0)),
                    ..default()
                },
            ));
            p.spawn((
                Text::new(crate::i18n::t("每种技能都有 普通 / 中级 / 高级 三个级别，由怪物品级决定")),
                text_font(f, 13.0),
                TextColor(Color::srgb(0.7, 0.7, 0.8)),
            ));
            p.spawn(Node {
                width: Val::Px(720.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                margin: UiRect::top(Val::Px(6.0)),
                ..default()
            })
            .with_children(|list| {
                for sk in crate::monster::skill_codex() {
                    list.spawn((
                        Node {
                            width: Val::Percent(100.0),
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(10.0),
                            padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                            ..default()
                        },
                        BackgroundColor(UI_CARD),
                    ))
                    .with_children(|card| {
                        card.spawn((
                            Text::new(sk.icon),
                            text_font(f, 24.0),
                            TextColor(Color::WHITE),
                            Node {
                                width: Val::Px(34.0),
                                ..default()
                            },
                        ));
                        card.spawn(Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(1.0),
                            ..default()
                        })
                        .with_children(|txt| {
                            txt.spawn((
                                Text::new(crate::i18n::t(sk.name)),
                                text_font(f, 15.0),
                                TextColor(Color::srgb(1.0, 0.86, 0.4)),
                            ));
                            txt.spawn((
                                Text::new(crate::i18n::t(sk.desc)),
                                text_font(f, 12.0),
                                TextColor(Color::srgb(0.82, 0.85, 0.8)),
                            ));
                            txt.spawn((
                                Text::new(crate::i18n::t(sk.tiers)),
                                text_font(f, 12.0),
                                TextColor(Color::srgb(0.55, 0.82, 0.95)),
                            ));
                        });
                    });
                }
            });

            p.spawn(Node {
                margin: UiRect::top(Val::Px(10.0)),
                ..default()
            })
            .with_children(|row| {
                button(row, f, &crate::i18n::t("返回"), UiAction::ToMenu, BTN_BG);
            });
        });
}

pub fn bestiary_buttons(
    interactions: Query<(&Interaction, &UiAction), Changed<Interaction>>,
    mut next: ResMut<NextState<GameState>>,
) {
    for (interaction, action) in &interactions {
        if *interaction == Interaction::Pressed && matches!(action, UiAction::ToMenu) {
            next.set(GameState::Menu);
        }
    }
}

// ============================ Tower Archive ============================

pub fn spawn_tower_archive(mut commands: Commands, fonts: Res<UiFont>, sprites: Res<Sprites>) {
    let f = &fonts.0;
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(12.0)),
                row_gap: Val::Px(8.0),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            BackgroundColor(UI_BG),
            ScrollPosition::default(),
            TowerArchiveRoot,
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(crate::i18n::t("防御塔档案")),
                text_font(f, 30.0),
                TextColor(Color::srgb(0.52, 0.88, 0.95)),
            ));
            p.spawn((
                Text::new(crate::i18n::tf(
                    "已收录 {} 座防御塔 · 元素、行为、占地、耐久和反制信息",
                    &[&TowerKind::ALL.len().to_string()],
                )),
                text_font(f, 14.0),
                TextColor(Color::srgb(0.76, 0.86, 0.84)),
            ));
            p.spawn(Node {
                width: Val::Px(780.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                justify_content: JustifyContent::Center,
                ..default()
            })
            .with_children(|grid| {
                for kind in TowerKind::ALL {
                    let d = kind.def();
                    grid.spawn((
                        Node {
                            width: Val::Px(248.0),
                            height: Val::Px(236.0),
                            flex_direction: FlexDirection::Column,
                            justify_content: JustifyContent::Center,
                            row_gap: Val::Px(3.0),
                            margin: UiRect::all(Val::Px(4.0)),
                            padding: UiRect::all(Val::Px(8.0)),
                            ..default()
                        },
                        BackgroundColor(d.color.with_alpha(0.18)),
                    ))
                    .with_children(|cell| {
                        cell.spawn((
                            ImageNode {
                                image: sprites.towers[&kind].clone(),
                                ..default()
                            },
                            Node {
                                width: Val::Px(50.0),
                                height: Val::Px(50.0),
                                align_self: AlignSelf::Center,
                                ..default()
                            },
                        ));
                        cell.spawn((
                            Text::new(crate::i18n::tf(
                                "{} · {} · {} [{}]",
                                &[
                                    &crate::i18n::t(d.category.name()),
                                    &crate::i18n::t(d.name),
                                    &crate::i18n::t(d.element.name()),
                                    &crate::i18n::t(element_marker(d.element)),
                                ],
                            )),
                            text_font(f, 13.0),
                            TextColor(d.color.mix(&Color::WHITE, 0.32)),
                        ));
                        cell.spawn((
                            Text::new(tower_stat_line(d)),
                            text_font(f, 10.0),
                            TextColor(Color::srgb(0.82, 0.86, 0.78)),
                        ));
                        cell.spawn((
                            Text::new(tower_behavior_line(d)),
                            text_font(f, 10.0),
                            TextColor(Color::srgb(0.72, 0.84, 0.82)),
                        ));
                        cell.spawn((
                            Text::new(crate::i18n::t(d.desc)),
                            text_font(f, 10.0),
                            TextColor(Color::srgb(0.74, 0.76, 0.70)),
                        ));
                        cell.spawn((
                            Text::new(tower_counter_line(d.element)),
                            text_font(f, 9.0),
                            TextColor(Color::srgb(0.62, 0.72, 0.64)),
                        ));
                    });
                }
            });
            p.spawn(Node {
                margin: UiRect::top(Val::Px(10.0)),
                ..default()
            })
            .with_children(|row| {
                button(row, f, &crate::i18n::t("返回"), UiAction::ToMenu, BTN_BG);
            });
        });
}

pub fn tower_archive_buttons(
    interactions: Query<(&Interaction, &UiAction), Changed<Interaction>>,
    mut next: ResMut<NextState<GameState>>,
) {
    for (interaction, action) in &interactions {
        if *interaction == Interaction::Pressed && matches!(action, UiAction::ToMenu) {
            next.set(GameState::Menu);
        }
    }
}

// ============================ Hero Codex ============================

#[derive(Component)]
pub struct HeroCodexRoot;
#[derive(Component)]
pub struct HeroCodexInfo;

/// The hero codex: browse every class × race, pick the deploy hero, and read its
/// role / doctrine / skill / ultimate. Replaces the cluttered menu hero card.
pub fn spawn_hero_codex(mut commands: Commands, fonts: Res<UiFont>, sprites: Res<Sprites>) {
    let f = &fonts.0;
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(12.0)),
                row_gap: Val::Px(8.0),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            BackgroundColor(UI_BG),
            ScrollPosition::default(),
            HeroCodexRoot,
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(crate::i18n::t("英雄图鉴")),
                text_font(f, 30.0),
                TextColor(UI_ACCENT_GOLD),
            ));
            p.spawn((
                Text::new(crate::i18n::t("选择出战英雄 · 种族 × 职业（悬停查看天赋与技能；三族属性不同）")),
                text_font(f, 13.0),
                TextColor(UI_ACCENT_TEAL),
            ));
            // Race row.
            p.spawn(Node {
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                justify_content: JustifyContent::Center,
                ..default()
            })
            .with_children(|row| {
                for race in Race::ALL {
                    icon_button(
                        row,
                        sprites.races[&race].clone(),
                        UiAction::SelectHeroRace(race),
                        BTN_BG,
                        (),
                    );
                }
            });
            // Class portrait grid.
            p.spawn(Node {
                width: Val::Px(660.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                justify_content: JustifyContent::Center,
                ..default()
            })
            .with_children(|grid| {
                for class in Class::ALL {
                    icon_button(
                        grid,
                        sprites.heroes[&class].clone(),
                        UiAction::SelectHeroClass(class),
                        BTN_BG,
                        (),
                    );
                }
            });
            // Selected-hero detail panel.
            p.spawn((
                Node {
                    width: Val::Px(660.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(14.0)),
                    margin: UiRect::top(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(UI_CARD),
            ))
            .with_children(|card| {
                card.spawn((
                    Text::new(""),
                    text_font(f, 14.0),
                    TextColor(Color::srgb(0.92, 0.95, 0.88)),
                    HeroCodexInfo,
                ));
            });
            p.spawn(Node {
                margin: UiRect::top(Val::Px(8.0)),
                ..default()
            })
            .with_children(|row| {
                button(row, f, &crate::i18n::t("返回"), UiAction::ToMenu, BTN_BG);
            });
        });
    // Hover/tap tooltips for the class/race buttons.
    spawn_tooltip_box(&mut commands, f, HeroCodexRoot);
}

pub fn hero_codex_buttons(
    interactions: Query<(&Interaction, &UiAction), Changed<Interaction>>,
    mut hero: ResMut<HeroLoadout>,
    mut next: ResMut<NextState<GameState>>,
) {
    for (interaction, action) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action {
            UiAction::SelectHeroClass(c) => hero.set_class(*c),
            UiAction::SelectHeroRace(r) => hero.set_race(*r),
            UiAction::ToMenu => next.set(GameState::Menu),
            _ => {}
        }
    }
}

pub fn update_hero_codex_info(hero: Res<HeroLoadout>, mut q: Query<&mut Text, With<HeroCodexInfo>>) {
    if let Ok(mut t) = q.single_mut() {
        let doc = hero.class.doctrine();
        t.0 = crate::i18n::tf(
            "{}·{}  Lv{}  ·  {}\n天赋【{}】{}\n技能·{}：{}\n终极·{}：{}",
            &[
                &crate::i18n::t(hero.race.name()),
                &crate::i18n::t(hero.class.name()),
                &hero.level.to_string(),
                &crate::i18n::t(hero.class.role()),
                &crate::i18n::t(doc.name),
                &crate::i18n::t(doc.desc),
                &crate::i18n::t(hero.class.skill_name()),
                &crate::i18n::t(hero.class.skill_desc()),
                &crate::i18n::t(hero.class.ultimate_name()),
                &crate::i18n::t(hero.class.ultimate_desc()),
            ],
        );
    }
}

// ============================ Campaign Dossier ============================

pub fn spawn_campaign_dossier(
    mut commands: Commands,
    fonts: Res<UiFont>,
    levels: Res<Levels>,
    progress: Res<Progress>,
) {
    let f = &fonts.0;
    let unlocked = progress.unlocked.min(levels.0.len());
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(12.0)),
                row_gap: Val::Px(8.0),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            BackgroundColor(UI_BG),
            ScrollPosition::default(),
            CampaignDossierRoot,
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(crate::i18n::t("战役档案")),
                text_font(f, 30.0),
                TextColor(Color::srgb(0.66, 0.82, 1.0)),
            ));
            p.spawn((
                Text::new(crate::i18n::tf(
                    "已开放 {}/{} 条战线 · 档案汇总关卡情报、旧日低语、首领波与推荐元素",
                    &[&unlocked.to_string(), &levels.0.len().to_string()],
                )),
                text_font(f, 14.0),
                TextColor(Color::srgb(0.74, 0.80, 0.88)),
            ));
            p.spawn(Node {
                width: Val::Px(820.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                justify_content: JustifyContent::Center,
                ..default()
            })
            .with_children(|grid| {
                for (i, level) in levels.0.iter().enumerate() {
                    let theme = LEVEL_THEMES.get(i).copied().unwrap_or(LEVEL_THEMES[0]);
                    let open = i < unlocked;
                    let stars = progress.stars.get(i).copied().unwrap_or(0);
                    let lore = LEVEL_LORE.get(i).copied().unwrap_or("档案缺失。");
                    let status = if open { crate::i18n::t(rating_label(stars)) } else { crate::i18n::t("封存") };
                    let accent = if open {
                        theme.accent
                    } else {
                        Color::srgb(0.42, 0.44, 0.50)
                    };
                    grid.spawn((
                        Node {
                            width: Val::Px(262.0),
                            height: Val::Px(246.0),
                            flex_direction: FlexDirection::Column,
                            justify_content: JustifyContent::Center,
                            row_gap: Val::Px(4.0),
                            margin: UiRect::all(Val::Px(4.0)),
                            padding: UiRect::all(Val::Px(9.0)),
                            ..default()
                        },
                        BackgroundColor(accent.with_alpha(if open { 0.18 } else { 0.07 })),
                    ))
                    .with_children(|cell| {
                        cell.spawn((
                            Text::new(crate::i18n::tf(
                                "{}. {} · {}",
                                &[&format!("{:02}", i + 1), &crate::i18n::t(level.name), &status],
                            )),
                            text_font(f, 15.0),
                            TextColor(accent.mix(&Color::WHITE, 0.24)),
                        ));
                        cell.spawn((
                            Text::new(campaign_level_stats(level)),
                            text_font(f, 9.0),
                            TextColor(Color::srgb(0.78, 0.82, 0.78)),
                        ));
                        cell.spawn((
                            Text::new(crate::i18n::t(lore)),
                            text_font(f, 10.0),
                            TextColor(if open {
                                Color::srgb(0.78, 0.80, 0.74)
                            } else {
                                Color::srgb(0.48, 0.50, 0.52)
                            }),
                        ));
                        cell.spawn((
                            Text::new(campaign_boss_line(i, level)),
                            text_font(f, 9.0),
                            TextColor(Color::srgb(0.88, 0.70, 0.56)),
                        ));
                        cell.spawn((
                            Text::new(campaign_recommendation(i, level)),
                            text_font(f, 9.0),
                            TextColor(Color::srgb(0.70, 0.86, 0.72)),
                        ));
                        cell.spawn((
                            Text::new(elite_affix_intel(level.waves, i)),
                            text_font(f, 9.0),
                            TextColor(Color::srgb(0.74, 0.70, 0.92)),
                        ));
                    });
                }
            });
            p.spawn(Node {
                margin: UiRect::top(Val::Px(10.0)),
                ..default()
            })
            .with_children(|row| {
                button(row, f, &crate::i18n::t("返回"), UiAction::ToMenu, BTN_BG);
            });
        });
}

pub fn campaign_dossier_buttons(
    interactions: Query<(&Interaction, &UiAction), Changed<Interaction>>,
    mut next: ResMut<NextState<GameState>>,
) {
    for (interaction, action) in &interactions {
        if *interaction == Interaction::Pressed && matches!(action, UiAction::ToMenu) {
            next.set(GameState::Menu);
        }
    }
}

// ============================ Milestones ============================

pub fn spawn_milestones(
    mut commands: Commands,
    fonts: Res<UiFont>,
    levels: Res<Levels>,
    progress: Res<Progress>,
    inv: Res<EquipmentInventory>,
    bestiary: Res<Bestiary>,
) {
    let f = &fonts.0;
    let rows = milestone_rows(&levels, &progress, &inv, &bestiary);
    let done = rows.iter().filter(|row| row.complete()).count();
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(12.0)),
                row_gap: Val::Px(8.0),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            BackgroundColor(UI_BG),
            ScrollPosition::default(),
            MilestonesRoot,
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(crate::i18n::t("封印成就")),
                text_font(f, 30.0),
                TextColor(Color::srgb(0.92, 0.72, 1.0)),
            ));
            p.spawn((
                Text::new(crate::i18n::tf(
                    "完成 {}/{} 项 · 成就进度来自关卡评级、图鉴、首领击杀和装备库存",
                    &[&done.to_string(), &rows.len().to_string()],
                )),
                text_font(f, 14.0),
                TextColor(Color::srgb(0.78, 0.74, 0.84)),
            ));
            p.spawn(Node {
                width: Val::Px(780.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                justify_content: JustifyContent::Center,
                ..default()
            })
            .with_children(|grid| {
                for row in rows {
                    let complete = row.complete();
                    let display_current = row.current.min(row.target);
                    let status = if complete {
                        crate::i18n::t("完成")
                    } else {
                        format!("{}/{}", display_current, row.target)
                    };
                    grid.spawn((
                        Node {
                            width: Val::Px(240.0),
                            height: Val::Px(132.0),
                            flex_direction: FlexDirection::Column,
                            justify_content: JustifyContent::Center,
                            row_gap: Val::Px(4.0),
                            margin: UiRect::all(Val::Px(4.0)),
                            padding: UiRect::all(Val::Px(9.0)),
                            ..default()
                        },
                        BackgroundColor(row.color.with_alpha(if complete { 0.22 } else { 0.10 })),
                    ))
                    .with_children(|cell| {
                        cell.spawn((
                            Text::new(crate::i18n::t(row.category)),
                            text_font(f, 10.0),
                            TextColor(row.color.mix(&Color::WHITE, 0.36)),
                        ));
                        cell.spawn((
                            Text::new(crate::i18n::t(row.title)),
                            text_font(f, 16.0),
                            TextColor(if complete {
                                row.color
                            } else {
                                Color::srgb(0.86, 0.84, 0.88)
                            }),
                        ));
                        cell.spawn((
                            Text::new(crate::i18n::t(&row.detail)),
                            text_font(f, 10.0),
                            TextColor(Color::srgb(0.70, 0.72, 0.70)),
                        ));
                        cell.spawn((
                            Text::new(format!("[{}] {}", ascii_bar(row.fraction(), 18), status)),
                            text_font(f, 11.0),
                            TextColor(if complete {
                                Color::srgb(1.0, 0.88, 0.42)
                            } else {
                                Color::srgb(0.70, 0.72, 0.76)
                            }),
                        ));
                    });
                }
            });
            p.spawn(Node {
                margin: UiRect::top(Val::Px(10.0)),
                ..default()
            })
            .with_children(|row| {
                button(row, f, &crate::i18n::t("返回"), UiAction::ToMenu, BTN_BG);
            });
        });
}

pub fn milestone_buttons(
    interactions: Query<(&Interaction, &UiAction), Changed<Interaction>>,
    mut next: ResMut<NextState<GameState>>,
) {
    for (interaction, action) in &interactions {
        if *interaction == Interaction::Pressed && matches!(action, UiAction::ToMenu) {
            next.set(GameState::Menu);
        }
    }
}

// ============================ Armory ============================

pub fn spawn_armory(
    mut commands: Commands,
    fonts: Res<UiFont>,
    sprites: Res<Sprites>,
    inv: Res<EquipmentInventory>,
) {
    let f = &fonts.0;
    spawn_armory_contents(&mut commands, f, &sprites, &inv, None);
}

fn spawn_armory_contents(
    commands: &mut Commands,
    f: &Handle<Font>,
    sprites: &Sprites,
    inv: &EquipmentInventory,
    notice: Option<String>,
) {
    let owned_kinds = Equipment::ALL
        .iter()
        .filter(|item| inv.counts[item.idx()] > 0)
        .count();
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(12.0)),
                row_gap: Val::Px(8.0),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            BackgroundColor(UI_BG),
            ScrollPosition::default(),
            ArmoryRoot,
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(crate::i18n::t("装备库")),
                text_font(f, 30.0),
                TextColor(Color::srgb(1.0, 0.74, 0.32)),
            ));
            p.spawn((
                Text::new(crate::i18n::tf(
                    "库存 {} 件   已获得 {}/{} 种   每座塔最多装配 3 件",
                    &[&inv.total().to_string(), &owned_kinds.to_string(), &Equipment::ALL.len().to_string()],
                )),
                text_font(f, 14.0),
                TextColor(Color::srgb(0.82, 0.78, 0.68)),
            ));
            if let Some(notice) = notice {
                p.spawn((
                    Text::new(notice),
                    text_font(f, 13.0),
                    TextColor(Color::srgb(1.0, 0.86, 0.38)),
                ));
            }
            p.spawn((
                Text::new(crate::i18n::t(
                    "套装规则：同属性转化2件触发元素共鸣+10%伤害，3件为+18%并护甲+3；任意3件有整备奖励，高阶遗物成组提供更强火力与抗攻城护甲。精炼：3件同名遗物→1件下一品级随机遗物。",
                )),
                text_font(f, 12.0),
                TextColor(Color::srgb(0.68, 0.74, 0.64)),
                Node {
                    max_width: Val::Px(720.0),
                    ..default()
                },
            ));
            p.spawn(Node {
                width: Val::Px(760.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                justify_content: JustifyContent::Center,
                ..default()
            })
            .with_children(|grid| {
                for item in Equipment::ALL {
                    let d = item.def();
                    let count = inv.counts[item.idx()];
                    let owned = count > 0;
                    let panel_color = if owned {
                        d.rarity.color().with_alpha(0.18)
                    } else {
                        Color::srgba(1.0, 1.0, 1.0, 0.04)
                    };
                    let name_color = if owned {
                        d.rarity.color()
                    } else {
                        Color::srgb(0.38, 0.36, 0.34)
                    };
                    let body_color = if owned {
                        Color::srgb(0.82, 0.82, 0.74)
                    } else {
                        Color::srgb(0.46, 0.45, 0.42)
                    };
                    grid.spawn((
                        Node {
                            width: Val::Px(240.0),
                            height: Val::Px(250.0),
                            flex_direction: FlexDirection::Column,
                            justify_content: JustifyContent::Center,
                            row_gap: Val::Px(3.0),
                            margin: UiRect::all(Val::Px(4.0)),
                            padding: UiRect::all(Val::Px(8.0)),
                            ..default()
                        },
                        BackgroundColor(panel_color),
                    ))
                    .with_children(|cell| {
                        cell.spawn((
                            ImageNode {
                                image: sprites.equipment[&item].clone(),
                                color: if owned {
                                    Color::WHITE
                                } else {
                                    Color::srgba(0.42, 0.42, 0.42, 0.55)
                                },
                                ..default()
                            },
                            Node {
                                width: Val::Px(42.0),
                                height: Val::Px(42.0),
                                align_self: AlignSelf::Center,
                                ..default()
                            },
                        ));
                        cell.spawn((
                            Text::new(crate::i18n::tf(
                                "{} · {}",
                                &[&d.rarity.label(), &crate::i18n::t(d.name)],
                            )),
                            text_font(f, 14.0),
                            TextColor(name_color),
                        ));
                        cell.spawn((
                            Text::new(if owned {
                                crate::i18n::tf("库存 {}   短名 {}", &[&count.to_string(), &crate::i18n::t(d.short)])
                            } else {
                                crate::i18n::tf("未获得   短名 {}", &[&crate::i18n::t(d.short)])
                            }),
                            text_font(f, 11.0),
                            TextColor(if owned {
                                Color::srgb(1.0, 0.9, 0.45)
                            } else {
                                Color::srgb(0.42, 0.40, 0.36)
                            }),
                        ));
                        cell.spawn((
                            Text::new(equipment_stat_line(d)),
                            text_font(f, 10.0),
                            TextColor(body_color),
                        ));
                        cell.spawn((
                            Text::new(crate::i18n::t(equipment_visual_line(item))),
                            text_font(f, 9.0),
                            TextColor(if owned {
                                Color::srgb(0.68, 0.78, 0.86)
                            } else {
                                Color::srgb(0.42, 0.44, 0.45)
                            }),
                        ));
                        cell.spawn((
                            Text::new(crate::i18n::t(d.desc)),
                            text_font(f, 10.0),
                            TextColor(body_color),
                        ));
                        cell.spawn((
                            Text::new(crate::equipment::recommend_text(d)),
                            text_font(f, 9.0),
                            TextColor(Color::srgb(0.78, 0.82, 0.6)),
                        ));
                        cell.spawn((
                            Text::new(crate::i18n::t(drop_source_hint(d.rarity))),
                            text_font(f, 9.0),
                            TextColor(Color::srgb(0.62, 0.66, 0.58)),
                        ));
                        if d.rarity == crate::equipment::Rarity::Mythic {
                            cell.spawn((
                                Text::new(crate::i18n::t("已满阶")),
                                text_font(f, 10.0),
                                TextColor(Color::srgb(0.72, 0.66, 0.62)),
                            ));
                        } else {
                            let refine_label = if count >= 3 {
                                crate::i18n::t("精炼×3")
                            } else {
                                crate::i18n::tf("精炼 {}/3", &[&count.min(3).to_string()])
                            };
                            button(
                                cell,
                                f,
                                &refine_label,
                                UiAction::RefineEquipment(item),
                                if count >= 3 {
                                    d.rarity.color().with_alpha(0.82)
                                } else {
                                    Color::srgb(0.18, 0.18, 0.17)
                                },
                            );
                        }
                    });
                }
            });
            p.spawn(Node {
                margin: UiRect::top(Val::Px(10.0)),
                ..default()
            })
            .with_children(|row| {
                button(row, f, &crate::i18n::t("返回"), UiAction::ToMenu, BTN_BG);
            });
        });
}

pub fn armory_buttons(
    mut commands: Commands,
    interactions: Query<(&Interaction, &UiAction), Changed<Interaction>>,
    roots: Query<Entity, With<ArmoryRoot>>,
    fonts: Res<UiFont>,
    sprites: Res<Sprites>,
    mut inv: ResMut<EquipmentInventory>,
    mut rng: ResMut<Rng>,
    mut next: ResMut<NextState<GameState>>,
) {
    for (interaction, action) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action {
            UiAction::ToMenu => next.set(GameState::Menu),
            UiAction::RefineEquipment(item) => {
                let notice = match refine_equipment(&mut inv, &mut rng, *item) {
                    Ok(reward) => crate::i18n::tf(
                        "精炼成功：消耗3件{}，获得{}·{}",
                        &[
                            &crate::i18n::t(item.def().name),
                            &reward.def().rarity.label(),
                            &crate::i18n::t(reward.def().name),
                        ],
                    ),
                    Err(reason) => crate::i18n::tf("精炼失败：{}", &[&crate::i18n::t(reason)]),
                };
                for root in &roots {
                    commands.entity(root).despawn();
                }
                spawn_armory_contents(&mut commands, &fonts.0, &sprites, &inv, Some(notice));
            }
            _ => {}
        }
    }
}

// ============================ Overlays ============================

fn recover_run_equipment(
    inv: &mut EquipmentInventory,
    towers: &mut Query<&mut crate::tower::Tower>,
) -> usize {
    let mut returned = 0;
    for mut tower in towers.iter_mut() {
        returned += unequip_all_to_inventory(inv, &mut tower);
    }
    returned
}

pub fn spawn_gameover(
    mut commands: Commands,
    current: Res<CurrentLevel>,
    levels: Res<Levels>,
    run: Res<RunState>,
    diff: Res<GameDifficulty>,
    fonts: Res<UiFont>,
    sfx: Res<crate::audio::Sfx>,
    audio: Res<crate::audio::AudioSettings>,
    mut inv: ResMut<EquipmentInventory>,
    mut towers: Query<&mut crate::tower::Tower>,
) {
    crate::audio::play_oneshot(&mut commands, &sfx, crate::audio::Sound::Defeat, audio.master);
    let returned = recover_run_equipment(&mut inv, &mut towers);
    let mut subtitle = settlement_summary(&crate::i18n::t("本局结算"), current.0, &levels, &run, diff.0);
    if returned > 0 {
        subtitle.push_str(&crate::i18n::tf("\n已回收本局装备 {} 件", &[&returned.to_string()]));
    }
    overlay(
        &mut commands,
        &fonts.0,
        &crate::i18n::t("失败！萝卜被吃掉了"),
        Some(subtitle),
        Color::srgb(0.8, 0.2, 0.2),
        false,
        None,
    );
}

pub fn spawn_victory(
    mut commands: Commands,
    current: Res<CurrentLevel>,
    levels: Res<Levels>,
    run: Res<RunState>,
    diff: Res<GameDifficulty>,
    mut progress: ResMut<Progress>,
    fonts: Res<UiFont>,
    sprites: Res<Sprites>,
    sfx: Res<crate::audio::Sfx>,
    audio: Res<crate::audio::AudioSettings>,
    mut inv: ResMut<EquipmentInventory>,
    mut rng: ResMut<Rng>,
    mut towers: Query<&mut crate::tower::Tower>,
) {
    crate::audio::play_oneshot(&mut commands, &sfx, crate::audio::Sound::Victory, audio.master);
    let returned = recover_run_equipment(&mut inv, &mut towers);
    let level = &levels.0[current.0];
    let start_lives = (level.lives + diff.0.lives_bonus()).max(1);
    let stars = victory_rating(run.lives.max(0), start_lives);
    let old_stars = progress.stars[current.0];
    if stars > old_stars {
        progress.stars[current.0] = stars;
        save_progress_stars(&progress.stars);
    }
    // Unlock the next level.
    let next_idx = current.0 + 1;
    let has_next = next_idx < levels.0.len();
    let newly_unlocked = has_next && progress.unlocked < next_idx + 1;
    if newly_unlocked {
        progress.unlocked = next_idx + 1;
        save_progress_unlocked(progress.unlocked);
    }
    let best_stars = progress.stars[current.0];
    let rewards = roll_clear_rewards(&mut rng, stars, clear_reward_bonus(diff.0), current.0);
    for item in &rewards {
        inv.add(*item);
    }
    let reward_cards = rewards
        .iter()
        .map(|item| {
            let d = item.def();
            RewardCard {
                image: sprites.equipment[item].clone(),
                color: d.rarity.color(),
                title: crate::i18n::t(d.name),
                subtitle: d.rarity.label(),
            }
        })
        .collect::<Vec<_>>();
    let mut subtitle = if has_next {
        if newly_unlocked {
            crate::i18n::tf("新关卡开启：{}", &[&crate::i18n::t(levels.0[next_idx].name)])
        } else {
            crate::i18n::tf("下一处封印：{}", &[&crate::i18n::t(levels.0[next_idx].name)])
        }
    } else {
        crate::i18n::t("终章完成：现实暂时稳住了，但群星仍在转动。")
    };
    subtitle.push_str(&format!(
        "\n{}",
        settlement_summary(&crate::i18n::t("本局结算"), current.0, &levels, &run, diff.0)
    ));
    subtitle.push_str(&crate::i18n::tf(
        "\n本次评级：{}  历史最佳：{}",
        &[&crate::i18n::t(rating_label(stars)), &crate::i18n::t(rating_label(best_stars))],
    ));
    subtitle.push_str(&crate::i18n::tf(
        "\n封印宝箱：{}",
        &[&equipment_reward_summary(&rewards)],
    ));
    if returned > 0 {
        subtitle.push_str(&crate::i18n::tf("\n已回收本局装备 {} 件", &[&returned.to_string()]));
    }
    overlay(
        &mut commands,
        &fonts.0,
        &crate::i18n::t("胜利！萝卜守住了"),
        Some(subtitle),
        Color::srgb(0.2, 0.7, 0.3),
        has_next,
        Some(&reward_cards),
    );
}

fn overlay(
    commands: &mut Commands,
    f: &Handle<Font>,
    title: &str,
    subtitle: Option<String>,
    color: Color,
    show_next: bool,
    rewards: Option<&[RewardCard]>,
) {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::all(Val::Px(24.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.74)),
            OverlayRoot,
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Px(620.0),
                    max_height: Val::Percent(92.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(18.0)),
                    row_gap: Val::Px(10.0),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(UI_PANEL_DARK),
            ))
            .with_children(|p| {
                p.spawn((Text::new(title), text_font(f, 34.0), TextColor(color)));
                if let Some(subtitle) = subtitle {
                    p.spawn((
                        Text::new(subtitle),
                        text_font(f, 14.0),
                        TextColor(UI_TEXT),
                        Node {
                            width: Val::Percent(100.0),
                            margin: UiRect::bottom(Val::Px(2.0)),
                            ..default()
                        },
                    ));
                }
                if let Some(rewards) = rewards {
                    if !rewards.is_empty() {
                        p.spawn((
                            Text::new(crate::i18n::t("封印宝箱")),
                            text_font(f, 15.0),
                            TextColor(UI_ACCENT_GOLD),
                        ));
                        p.spawn(Node {
                            flex_direction: FlexDirection::Row,
                            flex_wrap: FlexWrap::Wrap,
                            justify_content: JustifyContent::Center,
                            max_width: Val::Px(560.0),
                            column_gap: Val::Px(6.0),
                            row_gap: Val::Px(6.0),
                            margin: UiRect::bottom(Val::Px(4.0)),
                            ..default()
                        })
                        .with_children(|row| {
                            for reward in rewards {
                                row.spawn((
                                    Node {
                                        width: Val::Px(124.0),
                                        height: Val::Px(96.0),
                                        flex_direction: FlexDirection::Column,
                                        align_items: AlignItems::Center,
                                        justify_content: JustifyContent::Center,
                                        row_gap: Val::Px(2.0),
                                        padding: UiRect::all(Val::Px(6.0)),
                                        ..default()
                                    },
                                    BackgroundColor(reward.color.with_alpha(0.20)),
                                ))
                                .with_children(|card| {
                                    card.spawn((
                                        ImageNode {
                                            image: reward.image.clone(),
                                            ..default()
                                        },
                                        Node {
                                            width: Val::Px(42.0),
                                            height: Val::Px(42.0),
                                            ..default()
                                        },
                                    ));
                                    card.spawn((
                                        Text::new(&reward.title),
                                        text_font(f, 11.0),
                                        TextColor(reward.color.mix(&Color::WHITE, 0.35)),
                                    ));
                                    card.spawn((
                                        Text::new(&reward.subtitle),
                                        text_font(f, 10.0),
                                        TextColor(UI_TEXT),
                                    ));
                                });
                            }
                        });
                    }
                }
                p.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    justify_content: JustifyContent::Center,
                    ..default()
                })
                .with_children(|row| {
                    if show_next {
                        button(
                            row,
                            f,
                            &crate::i18n::t("下一关"),
                            UiAction::NextLevel,
                            Color::srgb(0.2, 0.5, 0.2),
                        );
                    }
                    button(row, f, &crate::i18n::t("重玩"), UiAction::Restart, BTN_BG);
                    button(row, f, &crate::i18n::t("关卡选择"), UiAction::ToMenu, BTN_BG);
                });
            });
        });
}

/// Level navigation from the in-game settings panel (重新开始 / 返回主页).
pub fn settings_nav_buttons(
    interactions: Query<(&Interaction, &UiAction), Changed<Interaction>>,
    mut next: ResMut<NextState<GameState>>,
) {
    for (interaction, action) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action {
            UiAction::Restart => next.set(GameState::Briefing),
            UiAction::ToMenu => next.set(GameState::Menu),
            _ => {}
        }
    }
}

pub fn overlay_buttons(
    interactions: Query<(&Interaction, &UiAction), Changed<Interaction>>,
    mut current: ResMut<CurrentLevel>,
    levels: Res<Levels>,
    mut next: ResMut<NextState<GameState>>,
) {
    for (interaction, action) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action {
            UiAction::Restart => next.set(GameState::Briefing),
            UiAction::NextLevel => {
                if current.0 + 1 < levels.0.len() {
                    current.0 += 1;
                }
                next.set(GameState::Briefing);
            }
            UiAction::ToMenu => next.set(GameState::Menu),
            _ => {}
        }
    }
}
