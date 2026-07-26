//! Per-run state (gold/lives/wave), level loading, and wave control.

use crate::Levels;
use crate::board::Board;
use crate::components::{Carrot, CarrotSealBar, LevelEntity};
use crate::data::{
    BOARD_H, BOARD_W, BOSS_WAVE_INTERVAL, COLS, LEVEL_THEMES, ROWS, TILE_SIZE, cell_center,
};
use crate::equipment::{EquipmentInventory, unequip_all_to_inventory};
use crate::monster::{boss_skill, is_boss_wave, next_boss_wave, pick_boss};
use bevy::prelude::*;
use bevy::sprite::Anchor;
use std::collections::{HashSet, VecDeque};

const MESSAGE_QUEUE_LIMIT: usize = 5;
pub const KILL_COMBO_WINDOW: f32 = 3.6;
pub const AUTO_WAVE_DELAY: f32 = 3.0;

/// Which level is loaded / will be loaded on entering `Playing`.
#[derive(Resource, Default)]
pub struct CurrentLevel(pub usize);

/// Pause flag (true = simulation frozen). Separate from `GameState` on purpose.
#[derive(Resource, Default)]
pub struct Paused(pub bool);

/// High-level run mode selected from the menu.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RunMode {
    Campaign,
    Endless,
}

impl RunMode {
    pub fn is_endless(self) -> bool {
        matches!(self, RunMode::Endless)
    }
}

#[derive(Resource)]
pub struct GameMode(pub RunMode);

impl Default for GameMode {
    fn default() -> Self {
        GameMode(RunMode::Campaign)
    }
}

/// Difficulty tier, chosen on the menu; scales enemy stats and economy.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Difficulty {
    Easy,
    Normal,
    Hard,
}

impl Difficulty {
    pub const ALL: [Difficulty; 3] = [Difficulty::Easy, Difficulty::Normal, Difficulty::Hard];
    pub fn name(self) -> &'static str {
        match self {
            Difficulty::Easy => "简单",
            Difficulty::Normal => "普通",
            Difficulty::Hard => "噩梦",
        }
    }
    pub fn hp_mult(self) -> f32 {
        match self {
            Difficulty::Easy => 0.7,
            Difficulty::Normal => 1.0,
            Difficulty::Hard => 1.6,
        }
    }
    pub fn reward_mult(self) -> f32 {
        match self {
            Difficulty::Easy => 1.3,
            Difficulty::Normal => 1.0,
            Difficulty::Hard => 0.9,
        }
    }
    pub fn gold_mult(self) -> f32 {
        match self {
            Difficulty::Easy => 1.3,
            Difficulty::Normal => 1.0,
            Difficulty::Hard => 0.85,
        }
    }
    pub fn lives_bonus(self) -> i32 {
        match self {
            Difficulty::Easy => 6,
            Difficulty::Normal => 0,
            Difficulty::Hard => -3,
        }
    }
    pub fn elite_mult(self) -> f32 {
        match self {
            Difficulty::Easy => 0.5,
            Difficulty::Normal => 1.0,
            Difficulty::Hard => 2.2,
        }
    }
}

/// Currently selected difficulty.
#[derive(Resource)]
pub struct GameDifficulty(pub Difficulty);
impl Default for GameDifficulty {
    fn default() -> Self {
        GameDifficulty(Difficulty::Normal)
    }
}

/// Run condition: simulation systems only tick when not paused.
pub fn not_paused(p: Res<Paused>) -> bool {
    !p.0
}

/// Mutable per-run state, mirroring the original global game variables.
#[derive(Resource)]
pub struct RunState {
    pub gold: i32,
    pub lives: i32,
    pub start_lives: i32,
    pub wave: i32,
    pub total_waves: i32,
    pub kills: i32,
    pub wave_in_progress: bool,
    pub wave_start_lives: i32,
    pub wave_perfect: bool,
    pub spawned: i32,
    pub spawn_target: i32,
    /// Seconds since last spawn (already scaled by game speed).
    pub spawn_timer: f32,
    pub spawn_interval: f32, // seconds
    /// Per-level base enemy count (`level.enemies.count`); wave target derives from it.
    pub base_count: i32,
    pub game_speed: f32,
    /// Bonus gold fraction per kill from the hero's doctrine (赏金猎手 etc), refreshed
    /// each frame by `hero::hero_doctrine`. 0 when the hero is dead.
    pub hero_gold_bonus: f32,
    pub auto_wave: bool,
    pub auto_wave_timer: f32,
    pub pending_boss_species: Option<usize>,
    /// Species already announced during this run; does not affect persistent kill counts.
    pub encountered_species: HashSet<usize>,
    pub kill_combo: i32,
    pub kill_combo_timer: f32,
    pub kill_combo_window: f32,
    pub best_combo: i32,
    pub message: String,
    pub message_timer: f32,
    pub message_queue: VecDeque<(String, f32)>,
}

impl Default for RunState {
    fn default() -> Self {
        RunState {
            gold: 0,
            lives: 0,
            start_lives: 0,
            wave: 0,
            total_waves: 0,
            kills: 0,
            wave_in_progress: false,
            wave_start_lives: 0,
            wave_perfect: false,
            spawned: 0,
            spawn_target: 0,
            spawn_timer: 0.0,
            spawn_interval: 1.0,
            base_count: 0,
            game_speed: 1.0,
            hero_gold_bonus: 0.0,
            auto_wave: false,
            auto_wave_timer: 0.0,
            pending_boss_species: None,
            encountered_species: HashSet::new(),
            kill_combo: 0,
            kill_combo_timer: 0.0,
            kill_combo_window: 0.0,
            best_combo: 0,
            message: String::new(),
            message_timer: 0.0,
            message_queue: VecDeque::new(),
        }
    }
}

impl RunState {
    pub fn is_endless(&self) -> bool {
        self.total_waves <= 0
    }

    pub fn can_start_next_wave(&self) -> bool {
        !self.wave_in_progress && (self.is_endless() || self.wave < self.total_waves)
    }

    pub fn is_boss_wave_number(&self, wave: i32) -> bool {
        if self.is_endless() {
            wave > 0 && wave % BOSS_WAVE_INTERVAL == 0
        } else {
            is_boss_wave(wave, self.total_waves)
        }
    }

    pub fn next_boss_wave_after(&self, current_wave: i32) -> Option<i32> {
        if self.is_endless() {
            let next = ((current_wave / BOSS_WAVE_INTERVAL) + 1) * BOSS_WAVE_INTERVAL;
            Some(next.max(BOSS_WAVE_INTERVAL))
        } else {
            next_boss_wave(current_wave, self.total_waves)
        }
    }

    pub fn boss_pick_total_waves(&self) -> i32 {
        if self.is_endless() {
            (self.wave + BOSS_WAVE_INTERVAL).max(20)
        } else {
            self.total_waves
        }
    }

    pub fn show(&mut self, msg: impl Into<String>) {
        self.show_for(msg, 2.0);
    }

    pub fn show_for(&mut self, msg: impl Into<String>, seconds: f32) {
        let message = msg.into();
        let duration = seconds.max(0.25);
        if self.message_timer <= 0.0 || self.message.is_empty() {
            self.message = message;
            self.message_timer = duration;
            return;
        }
        if self.message_queue.len() >= MESSAGE_QUEUE_LIMIT {
            self.message_queue.pop_front();
        }
        self.message_queue.push_back((message, duration));
    }
}

pub fn toggle_auto_wave(run: &mut RunState) {
    run.auto_wave = !run.auto_wave;
    if run.auto_wave {
        if run.can_start_next_wave() {
            run.auto_wave_timer = AUTO_WAVE_DELAY;
        }
        run.show_for(crate::i18n::t("自动波次开启"), 1.6);
    } else {
        run.auto_wave_timer = 0.0;
        run.show_for(crate::i18n::t("自动波次关闭"), 1.6);
    }
}

/// Tiny deterministic RNG (xorshift64*). Avoids pulling in `rand`/`getrandom`,
/// which keeps the wasm build simple. Determinism is fine for a tower-defense.
#[derive(Resource)]
pub struct Rng(pub u64);

impl Default for Rng {
    fn default() -> Self {
        Rng(0x9E3779B97F4A7C15)
    }
}

impl Rng {
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }
    /// Uniform f32 in [0, 1).
    pub fn frac(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }
    pub fn range(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u64() % n as u64) as usize
        }
    }
}

/// Deterministic stream reserved for encounter composition: regular species,
/// bosses, elite affixes, and between-wave draft offers. Combat/VFX systems use
/// [`Rng`] instead, so changing a weapon's hit count cannot silently reroll the
/// enemies faced by another build under the same seed.
#[derive(Resource)]
pub struct EncounterRng(pub Rng);

impl EncounterRng {
    pub fn seeded(seed: u64) -> Self {
        let mixed = seed ^ 0xD1B5_4A32_D192_ED03;
        Self(Rng(if mixed == 0 {
            0xA24B_AED4_963E_E407
        } else {
            mixed
        }))
    }
}

impl Default for EncounterRng {
    fn default() -> Self {
        Self::seeded(0x9E37_79B9_7F4A_7C15)
    }
}

#[cfg(test)]
mod rng_tests {
    use super::*;

    #[test]
    fn encounter_stream_is_independent_from_combat_randomness() {
        let seed = 0x1234_5678_9ABC_DEF0;
        let mut combat = Rng(seed);
        let mut expected = EncounterRng::seeded(seed);
        let mut actual = EncounterRng::seeded(seed);

        for _ in 0..128 {
            combat.next_u64();
        }

        for _ in 0..16 {
            assert_eq!(actual.0.next_u64(), expected.0.next_u64());
        }
    }

    #[test]
    fn encounter_seed_never_uses_the_xorshift_zero_state() {
        let encounter = EncounterRng::seeded(0xD1B5_4A32_D192_ED03);
        assert_ne!(encounter.0.0, 0);
    }
}

/// `OnEnter(Playing)`: clear any previous level, build the board, reset run state,
/// and draw the static board visuals.
pub fn load_level(
    mut commands: Commands,
    levels: Res<Levels>,
    current: Res<CurrentLevel>,
    old: Query<Entity, With<LevelEntity>>,
    mut old_towers: Query<&mut crate::tower::Tower>,
    mut inv: ResMut<EquipmentInventory>,
    mut paused: ResMut<Paused>,
    diff: Res<GameDifficulty>,
    mode: Res<GameMode>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    assets: Res<AssetServer>,
    sprites: Res<crate::sprites::Sprites>,
    lighting: Res<crate::lighting::LightingSettings>,
) {
    for mut tower in &mut old_towers {
        unequip_all_to_inventory(&mut inv, &mut tower);
    }
    for e in &old {
        commands.entity(e).despawn();
    }
    paused.0 = false;

    let level = &levels.0[current.0];
    let board = Board::from_level(current.0, level);

    let mut run = RunState::default();
    if mode.0.is_endless() {
        run.gold = ((level.gold + 250) as f32 * diff.0.gold_mult()) as i32;
        run.start_lives = (16 + diff.0.lives_bonus()).max(1);
        run.lives = run.start_lives;
        run.total_waves = 0;
        run.spawn_interval = (level.spawn_interval_ms / 1000.0 * 0.78).clamp(0.38, 0.9);
        run.base_count = (level.enemies.count + 4).max(10);
        run.message = crate::i18n::tf(
            "无尽模式：{}\n怪物每波增强；每 {} 波首领来袭",
            &[&crate::i18n::t(level.name), &BOSS_WAVE_INTERVAL.to_string()],
        );
        run.message_timer = 8.0;
    } else {
        run.gold = (level.gold as f32 * diff.0.gold_mult()) as i32;
        run.start_lives = (level.lives + diff.0.lives_bonus()).max(1);
        run.lives = run.start_lives;
        run.total_waves = level.waves;
        run.spawn_interval = level.spawn_interval_ms / 1000.0;
        run.base_count = level.enemies.count;
        // Cthulhu-flavored level lore, shown a little longer than a normal message.
        // 第一章逐关独立，第 2-5 章共用章节 lore。
        let lore = crate::data::lore_for_level(current.0);
        run.message = crate::i18n::tf(
            "{}\n{}",
            &[&crate::i18n::t(level.name), &crate::i18n::t(lore)],
        );
        // 章节战场机制提示（第 2-5 章各有专属规则）。
        if let Some(desc) = crate::mutators::mechanic_desc(crate::data::episode_of(current.0)) {
            run.message.push_str(&crate::i18n::tf(
                "\n【{}】{}",
                &[
                    &crate::i18n::t(
                        crate::mutators::mechanic_name(crate::data::episode_of(current.0))
                            .unwrap_or(""),
                    ),
                    &crate::i18n::t(desc),
                ],
            ));
        }
        run.message_timer = 8.0;
    }

    // 每关的 AI 手绘地图背景：20 张美术按章节循环复用，叠加章节色调
    // （沙漠暖黄/冰原冷蓝/深渊血红/虚空紫）区分五章氛围。
    let bg = assets.load(format!(
        "sprites/levels/lvl_{:02}.webp",
        current.0 % crate::data::EPISODE_LEN
    ));
    let episode_tint = crate::data::EPISODES[crate::data::episode_of(current.0)].tint;
    draw_board(
        &mut commands,
        &board,
        run.start_lives,
        &mut meshes,
        &mut materials,
        bg,
        episode_tint,
        &sprites,
        &lighting,
    );

    commands.insert_resource(board);
    commands.insert_resource(run);
}

/// Draw grid tiles + carrot (static visuals for the level).
fn draw_board(
    commands: &mut Commands,
    board: &Board,
    start_lives: i32,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    bg: Handle<Image>,
    episode_tint: Color,
    sprites: &crate::sprites::Sprites,
    lighting: &crate::lighting::LightingSettings,
) {
    let theme = LEVEL_THEMES
        .get(board.level_index % LEVEL_THEMES.len())
        .copied()
        .unwrap_or(LEVEL_THEMES[0]);

    // Solid color fill (fallback / border) behind everything.
    commands.spawn((
        Sprite {
            color: theme.backdrop,
            custom_size: Some(Vec2::new(
                BOARD_W + TILE_SIZE * 2.0,
                BOARD_H + TILE_SIZE * 2.0,
            )),
            ..default()
        },
        Transform::from_translation(Vec3::new(0.0, 0.0, -4.0)),
        LevelEntity,
    ));
    // 每关的 AI 手绘地图背景：接近全亮显示——这是画面的主角，棋盘格只做
    // 极淡的引导覆盖（见下）。
    let bg_srgba = episode_tint.to_srgba();
    commands.spawn((
        Sprite {
            image: bg,
            color: Color::srgb(
                0.94 * bg_srgba.red,
                0.94 * bg_srgba.green,
                0.94 * bg_srgba.blue,
            ),
            custom_size: Some(Vec2::new(BOARD_W, BOARD_H)),
            ..default()
        },
        Transform::from_translation(Vec3::new(0.0, 0.0, -3.5)),
        LevelEntity,
    ));

    for x in 0..COLS {
        for y in 0..ROWS {
            let buildable = board.buildable.contains(&(x, y));
            let variation = (x * 17 + y * 31 + board.level_index as i32 * 7).rem_euclid(5);
            let color = if buildable {
                if variation == 0 {
                    theme.buildable_alt
                } else {
                    theme.buildable
                }
            } else if variation <= 1 {
                theme.path_edge
            } else {
                theme.path
            };
            // 可建造格只留极淡的提示（建造模式有幽灵高亮兜底）；行进路径要
            // 明显强于美术图里的装饰性道路——真正的路由这层标出，不能让玩家
            // 误把画出来的路当行进路线。
            let alpha = if buildable { 0.12 } else { 0.66 };
            commands.spawn((
                Sprite {
                    color: color.with_alpha(alpha),
                    custom_size: Some(Vec2::splat(TILE_SIZE - 2.0)),
                    ..default()
                },
                Transform::from_translation(cell_center(x as f32, y as f32).extend(0.0)),
                LevelEntity,
            ));
        }
    }

    for (i, pos) in board.path_world.iter().enumerate() {
        let radius = if i == 0 || i + 1 == board.path_world.len() {
            TILE_SIZE * 0.16
        } else {
            TILE_SIZE * 0.09
        };
        commands.spawn((
            Mesh2d(meshes.add(Circle::new(radius))),
            MeshMaterial2d(materials.add(theme.accent.with_alpha(0.42))),
            Transform::from_translation(pos.extend(1.6)),
            LevelEntity,
        ));
    }

    draw_map_decoration(commands, board, theme, meshes, materials, lighting);

    let spawn = board.spawn_pos();
    commands.spawn((
        Sprite {
            image: sprites.spawn_portal.clone(),
            custom_size: Some(Vec2::splat(TILE_SIZE * 1.5)),
            ..default()
        },
        Transform::from_translation(spawn.extend(1.8)),
        crate::components::SpawnPortal,
        LevelEntity,
    ));

    let carrot = board.carrot_pos();
    // Soft seal ring behind the carrot (kept as a subtle glow), then the carrot sprite.
    commands.spawn((
        Mesh2d(meshes.add(Annulus::new(TILE_SIZE * 0.54, TILE_SIZE * 0.64))),
        MeshMaterial2d(materials.add(theme.seal.with_alpha(0.55))),
        Transform::from_translation(carrot.extend(1.7)),
        LevelEntity,
    ));
    commands.spawn((
        Sprite {
            image: sprites.carrot.clone(),
            custom_size: Some(Vec2::splat(TILE_SIZE * 1.5)),
            ..default()
        },
        Transform::from_translation(carrot.extend(2.0)),
        Carrot {
            pulse_timer: 0.0,
            last_lives: start_lives,
        },
        LevelEntity,
    ));
    let bar_pos = carrot + Vec2::new(-TILE_SIZE * 0.46, -TILE_SIZE * 0.54);
    let bar_width = TILE_SIZE * 0.92;
    commands.spawn((
        Sprite {
            color: Color::srgba(0.02, 0.02, 0.03, 0.84),
            custom_size: Some(Vec2::new(bar_width, 5.0)),
            ..default()
        },
        Transform::from_translation((bar_pos + Vec2::new(bar_width * 0.5, 0.0)).extend(3.2)),
        LevelEntity,
    ));
    commands.spawn((
        Sprite {
            color: Color::srgb(0.38, 1.0, 0.48),
            custom_size: Some(Vec2::new(bar_width, 5.0)),
            ..default()
        },
        Anchor::CENTER_LEFT,
        Transform::from_translation(bar_pos.extend(3.3)),
        CarrotSealBar { width: bar_width },
        LevelEntity,
    ));

    crate::lighting::spawn_level_lighting(commands, board, theme, lighting);
}

fn draw_map_decoration(
    commands: &mut Commands,
    board: &Board,
    theme: crate::data::LevelTheme,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    lighting: &crate::lighting::LightingSettings,
) {
    let path_cells = expanded_path_cells(board);
    let mut torches = 0;
    let torch_limit = 3 + (board.level_index % 2) as i32;
    for y in 0..ROWS {
        for x in 0..COLS {
            if !board.buildable.contains(&(x, y)) {
                continue;
            }
            if edge_cell(x, y) {
                continue;
            }
            let dist = distance_to_path((x, y), &path_cells);
            let hash = decor_hash(board.level_index, x, y, 0x9e37_79b9);
            if dist == 1 && torches < torch_limit && hash % 17 == 0 {
                draw_torch(commands, x, y, board, theme, meshes, materials, lighting);
                torches += 1;
                continue;
            }

            if dist > 3 {
                continue;
            }
            match hash % 17 {
                0 | 1 => draw_rock(commands, x, y, board, theme, meshes, materials, hash),
                2 => draw_roots(commands, x, y, board, theme, hash),
                3 if dist <= 2 => draw_ground_crack(commands, x, y, board, theme, hash),
                _ => {}
            }
        }
    }
}

fn expanded_path_cells(board: &Board) -> HashSet<(i32, i32)> {
    let mut cells = HashSet::new();
    for x in 0..COLS {
        for y in 0..ROWS {
            if !board.buildable.contains(&(x, y)) {
                cells.insert((x, y));
            }
        }
    }
    cells
}

fn distance_to_path(cell: (i32, i32), path_cells: &HashSet<(i32, i32)>) -> i32 {
    path_cells
        .iter()
        .map(|&(px, py)| (cell.0 - px).abs() + (cell.1 - py).abs())
        .min()
        .unwrap_or(i32::MAX)
}

fn edge_cell(x: i32, y: i32) -> bool {
    x <= 1 || y <= 1 || x >= COLS - 2 || y >= ROWS - 2
}

fn decor_hash(level: usize, x: i32, y: i32, salt: u32) -> u32 {
    let mut v = salt ^ ((level as u32 + 1).wrapping_mul(0x85eb_ca6b));
    v ^= (x as u32).wrapping_mul(0xc2b2_ae35);
    v = v.rotate_left(13);
    v ^= (y as u32).wrapping_mul(0x27d4_eb2f);
    v ^ (v >> 16)
}

fn cell_jitter(level: usize, x: i32, y: i32, salt: u32, amount: f32) -> Vec2 {
    let h = decor_hash(level, x, y, salt);
    let jx = ((h & 0xff) as f32 / 255.0 - 0.5) * amount;
    let jy = (((h >> 8) & 0xff) as f32 / 255.0 - 0.5) * amount;
    Vec2::new(jx, jy)
}

fn draw_torch(
    commands: &mut Commands,
    x: i32,
    y: i32,
    board: &Board,
    theme: crate::data::LevelTheme,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    lighting: &crate::lighting::LightingSettings,
) {
    let base = cell_center(x as f32, y as f32)
        + cell_jitter(board.level_index, x, y, 0x51f1_5e12, TILE_SIZE * 0.22);
    let rot = ((decor_hash(board.level_index, x, y, 0xbadc_0ffe) % 9) as f32 - 4.0) * 0.025;
    commands.spawn((
        Sprite {
            color: Color::srgb(0.24, 0.15, 0.08).with_alpha(0.82),
            custom_size: Some(Vec2::new(5.0, TILE_SIZE * 0.42)),
            ..default()
        },
        Transform::from_translation((base + Vec2::new(0.0, -3.0)).extend(1.82))
            .with_rotation(Quat::from_rotation_z(rot)),
        LevelEntity,
    ));
    commands.spawn((
        Mesh2d(meshes.add(Ellipse::new(8.0, 11.0))),
        MeshMaterial2d(materials.add(Color::srgb(1.0, 0.36, 0.08).with_alpha(0.82))),
        Transform::from_translation((base + Vec2::new(0.0, 10.0)).extend(1.92)),
        LevelEntity,
    ));
    commands.spawn((
        Mesh2d(meshes.add(Ellipse::new(4.6, 7.0))),
        MeshMaterial2d(materials.add(Color::srgb(1.0, 0.88, 0.28).with_alpha(0.9))),
        Transform::from_translation((base + Vec2::new(0.0, 12.0)).extend(1.94)),
        LevelEntity,
    ));
    commands.spawn((
        Mesh2d(meshes.add(Ellipse::new(12.0, 4.2))),
        MeshMaterial2d(materials.add(theme.path_edge.mix(&Color::BLACK, 0.24).with_alpha(0.42))),
        Transform::from_translation((base + Vec2::new(5.0, -13.0)).extend(1.24)),
        LevelEntity,
    ));
    crate::lighting::spawn_scene_light(
        commands,
        base + Vec2::new(0.0, 10.0),
        Color::srgb(1.0, 0.48, 0.14),
        TILE_SIZE * 4.2,
        2.35,
        true,
        48.0,
        lighting,
    );
}

fn draw_rock(
    commands: &mut Commands,
    x: i32,
    y: i32,
    board: &Board,
    theme: crate::data::LevelTheme,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    hash: u32,
) {
    let pos = cell_center(x as f32, y as f32)
        + cell_jitter(board.level_index, x, y, 0x7255_a11a, TILE_SIZE * 0.34);
    let w = 7.0 + (hash & 0x7) as f32;
    let h = 4.5 + ((hash >> 4) & 0x7) as f32;
    commands.spawn((
        Mesh2d(meshes.add(Ellipse::new(w, h))),
        MeshMaterial2d(
            materials.add(
                theme
                    .path_edge
                    .mix(&Color::srgb(0.12, 0.12, 0.13), 0.40)
                    .with_alpha(0.54),
            ),
        ),
        Transform::from_translation(pos.extend(1.16))
            .with_rotation(Quat::from_rotation_z((hash as f32).sin() * 0.28)),
        LevelEntity,
    ));
}

fn draw_roots(
    commands: &mut Commands,
    x: i32,
    y: i32,
    board: &Board,
    theme: crate::data::LevelTheme,
    hash: u32,
) {
    let pos = cell_center(x as f32, y as f32)
        + cell_jitter(board.level_index, x, y, 0x9191_f00d, TILE_SIZE * 0.30);
    for i in 0..2 {
        let angle = ((hash.rotate_left(i * 7) % 360) as f32).to_radians();
        commands.spawn((
            Sprite {
                color: theme
                    .backdrop
                    .mix(&Color::srgb(0.08, 0.045, 0.025), 0.68)
                    .with_alpha(0.36),
                custom_size: Some(Vec2::new(TILE_SIZE * 0.42, 3.0)),
                ..default()
            },
            Transform::from_translation(
                (pos + Vec2::from_angle(angle) * (i as f32 * 4.0)).extend(1.12),
            )
            .with_rotation(Quat::from_rotation_z(angle)),
            LevelEntity,
        ));
    }
}

fn draw_ground_crack(
    commands: &mut Commands,
    x: i32,
    y: i32,
    board: &Board,
    theme: crate::data::LevelTheme,
    hash: u32,
) {
    let pos = cell_center(x as f32, y as f32)
        + cell_jitter(board.level_index, x, y, 0x6d2b_79f5, TILE_SIZE * 0.24);
    let angle = ((hash % 180) as f32).to_radians();
    commands.spawn((
        Sprite {
            color: theme
                .path
                .mix(&Color::srgb(0.02, 0.02, 0.025), 0.58)
                .with_alpha(0.32),
            custom_size: Some(Vec2::new(TILE_SIZE * 0.46, 2.0)),
            ..default()
        },
        Transform::from_translation(pos.extend(1.13)).with_rotation(Quat::from_rotation_z(angle)),
        LevelEntity,
    ));
}

pub fn update_carrot_seal(
    time: Res<Time>,
    run: Res<RunState>,
    sprites: Res<crate::sprites::Sprites>,
    mut carrots: Query<(&mut Carrot, &mut Transform, &mut Sprite), Without<CarrotSealBar>>,
    mut bars: Query<(&CarrotSealBar, &mut Transform, &mut Sprite), Without<Carrot>>,
) {
    let max_lives = run.start_lives.max(1);
    let life_frac = (run.lives.max(0) as f32 / max_lives as f32).clamp(0.0, 1.0);
    let dt = time.delta_secs();
    let t = time.elapsed_secs();
    for (mut carrot, mut tf, mut sprite) in &mut carrots {
        if carrot.last_lives != run.lives {
            carrot.last_lives = run.lives;
            carrot.pulse_timer = 0.36;
            // Swap to a more battered carrot as lives fall.
            sprite.image = if life_frac > 0.5 {
                sprites.carrot.clone()
            } else if life_frac > 0.25 {
                sprites.carrot_hurt.clone()
            } else {
                sprites.carrot_crit.clone()
            };
        }
        carrot.pulse_timer = (carrot.pulse_timer - dt).max(0.0);
        let danger_pulse = if life_frac <= 0.33 {
            (t * 9.0).sin().max(0.0) * 0.055
        } else {
            0.0
        };
        let hit_pulse = if carrot.pulse_timer > 0.0 {
            carrot.pulse_timer / 0.36 * 0.20
        } else {
            0.0
        };
        tf.scale = Vec3::splat(1.0 + danger_pulse + hit_pulse);
    }
    for (bar, mut tf, mut sprite) in &mut bars {
        tf.scale.x = life_frac;
        sprite.color = if life_frac <= 0.25 {
            Color::srgb(1.0, 0.15, 0.10)
        } else if life_frac <= 0.50 {
            Color::srgb(1.0, 0.68, 0.14)
        } else {
            Color::srgb(0.38, 1.0, 0.48)
        };
        if let Some(size) = sprite.custom_size.as_mut() {
            size.x = bar.width;
        }
    }
}

/// The enemy spawn portal (abyss) swells as the run's wave count climbs.
pub fn grow_portal(
    run: Res<RunState>,
    mut portals: Query<&mut Transform, With<crate::components::SpawnPortal>>,
) {
    let scale = (1.0 + run.wave.max(0) as f32 * 0.08).min(2.8);
    for mut tf in &mut portals {
        if (tf.scale.x - scale).abs() > 0.001 {
            tf.scale = Vec3::splat(scale);
        }
    }
}

/// Keyboard shortcuts: Space = start next wave, P = pause toggle, F = cycle speed.
pub fn keyboard_controls(
    keys: Res<ButtonInput<KeyCode>>,
    mut run: ResMut<RunState>,
    mut paused: ResMut<Paused>,
    current: Res<CurrentLevel>,
    mut rng: ResMut<EncounterRng>,
    roguelite: Res<crate::roguelite::RogueliteRun>,
) {
    if keys.just_pressed(KeyCode::KeyP) {
        paused.0 = !paused.0;
    }
    if keys.just_pressed(KeyCode::KeyF) {
        run.game_speed = match run.game_speed as i32 {
            1 => 2.0,
            2 => 4.0,
            4 => 8.0,
            _ => 1.0,
        };
    }
    if keys.just_pressed(KeyCode::KeyA) {
        toggle_auto_wave(&mut run);
    }
    if keys.just_pressed(KeyCode::Space) {
        if roguelite.is_waiting() {
            run.show(crate::i18n::t("先选择一个本波构筑天赋"));
        } else {
            start_wave(&mut run, current.0, &mut rng);
        }
    }
}

/// Begin the next wave (original `startWave`).
pub fn start_wave(run: &mut RunState, level_index: usize, rng: &mut EncounterRng) {
    if !run.can_start_next_wave() {
        return;
    }
    run.wave += 1;
    run.wave_in_progress = true;
    run.wave_start_lives = run.lives;
    run.wave_perfect = true;
    run.auto_wave_timer = 0.0;
    run.spawned = 0;
    // base count comes from the level; we store it via spawn_target seed below.
    run.spawn_timer = run.spawn_interval; // spawn first enemy promptly
    let mut target = if run.is_endless() {
        let wave = run.wave as f32;
        (run.base_count as f32 + wave * 2.2 + (wave / 5.0).floor() * 2.0).round() as i32
    } else {
        run.base_count + (run.wave as f32 * 1.95) as i32 + ((run.wave - 1) / 8).max(0)
    };
    if run.is_endless() {
        target = target.clamp(10, 96);
    }
    let boss_wave = run.is_boss_wave_number(run.wave);
    if boss_wave {
        target = if run.is_endless() {
            ((target as f32) * 0.72).round() as i32 + 2
        } else {
            target + 2
        };
    }
    run.spawn_target = target;
    if boss_wave {
        let boss = pick_boss(
            run.wave,
            run.boss_pick_total_waves(),
            level_index,
            &mut rng.0,
        );
        run.pending_boss_species = Some(boss.id);
        let skill = boss_skill(boss.id);
        let prefix = if run.is_endless() {
            crate::i18n::tf("无尽第 {} 波！", &[&run.wave.to_string()])
        } else {
            crate::i18n::tf("第 {} 波！", &[&run.wave.to_string()])
        };
        if skill.name().is_empty() {
            run.show_for(
                crate::i18n::tf("{}首领来袭：{}", &[&prefix, &crate::i18n::t(boss.name)]),
                4.0,
            );
        } else {
            run.show_for(
                crate::i18n::tf(
                    "{}首领来袭：{}\n技能：{}",
                    &[
                        &prefix,
                        &crate::i18n::t(boss.name),
                        &crate::i18n::t(skill.name()),
                    ],
                ),
                4.0,
            );
        }
    } else if let Some(next_boss) = run.next_boss_wave_after(run.wave) {
        run.pending_boss_species = None;
        if run.is_endless() {
            run.show(crate::i18n::tf(
                "无尽第 {} 波！怪物强度继续上升\n距离首领 {} 波",
                &[&run.wave.to_string(), &(next_boss - run.wave).to_string()],
            ));
        } else {
            run.show(crate::i18n::tf(
                "第 {} 波！距离首领 {} 波",
                &[&run.wave.to_string(), &(next_boss - run.wave).to_string()],
            ));
        }
    } else {
        run.pending_boss_species = None;
        run.show(crate::i18n::tf("第 {} 波！", &[&run.wave.to_string()]));
    }
    // 章节机制波次播报：沙暴/血潮来袭时提前告知玩家应对策略。
    if crate::mutators::is_sandstorm_wave(level_index, run.wave) {
        run.show_for(
            crate::i18n::t("沙暴来袭：敌人移速+30%！备好减速与控制"),
            3.2,
        );
    }
    if crate::mutators::is_bloodtide_wave(level_index, run.wave) {
        run.show_for(
            crate::i18n::t("血潮涌动：敌人自带血护盾！持续输出破盾"),
            3.2,
        );
    }
}

pub fn tick_auto_wave(
    time: Res<Time>,
    mut run: ResMut<RunState>,
    current: Res<CurrentLevel>,
    mut rng: ResMut<EncounterRng>,
    roguelite: Res<crate::roguelite::RogueliteRun>,
) {
    if roguelite.is_waiting() || !run.auto_wave || !run.can_start_next_wave() {
        return;
    }
    if run.auto_wave_timer <= 0.0 {
        run.auto_wave_timer = AUTO_WAVE_DELAY;
    }
    run.auto_wave_timer -= time.delta_secs();
    if run.auto_wave_timer <= 0.0 {
        run.auto_wave_timer = 0.0;
        start_wave(&mut run, current.0, &mut rng);
    }
}

/// Tick the on-screen message timer down.
pub fn tick_message(time: Res<Time>, mut run: ResMut<RunState>) {
    if run.message_timer > 0.0 {
        run.message_timer -= time.delta_secs();
    }
    if run.message_timer <= 0.0 {
        if let Some((message, duration)) = run.message_queue.pop_front() {
            run.message = message;
            run.message_timer = duration;
        } else {
            run.message.clear();
            run.message_timer = 0.0;
        }
    }
}
