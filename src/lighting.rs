//! Firefly-backed 2D lighting: level weather lights, real occluder shadows, and
//! short-lived combat glows.

use crate::board::Board;
use crate::components::{Carrot, Enemy, FireGround, FogHidden, LevelEntity, SpawnPortal};
use crate::data::{
    BOARD_H, BOARD_W, LEVEL_THEMES, LEVEL_WEATHERS, LevelTheme, LevelWeather, TILE_SIZE, TowerKind,
};
use crate::game::CurrentLevel;
use crate::hero::HeroWeapon;
use crate::states::GameState;
use crate::tower::{HERO_MELEE_ATTACK_TIME, ProjKind, Projectile, ProjectileVisual, Summon, Tower};
use bevy::prelude::*;
use bevy_firefly::prelude::*;
use bevy_fog_of_war::prelude::{
    FogMapSettings, FogOfWarPlugin, ResetFogOfWar, VisionShape, VisionSource,
};

pub struct LightingPlugin;

impl Plugin for LightingPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(default_fog_settings())
            .add_plugins((FireflyPlugin, FogOfWarPlugin))
            .add_systems(
                Update,
                (
                    sync_firefly_camera,
                    apply_brightness_to_lights,
                    attach_shadow_casters,
                    spawn_tower_lights,
                    update_tower_lights,
                    spawn_hero_weapon_lights,
                    spawn_fire_ground_lights,
                    update_follow_lights,
                    update_hero_weapon_lights,
                    spawn_projectile_shadows,
                    spawn_contact_shadows,
                    update_projectile_shadows,
                    update_contact_shadows,
                    update_timed_lights,
                ),
            )
            .add_systems(
                Update,
                (
                    configure_fog_of_war,
                    sync_fog_source_enabled,
                    update_enemy_fog_visibility,
                )
                    .chain(),
            );
    }
}

const FOG_OF_WAR_START_LEVEL: usize = 1;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BrightnessLevel {
    Standard,
    Bright,
    High,
}

impl BrightnessLevel {
    pub fn name(self) -> &'static str {
        match self {
            BrightnessLevel::Standard => "标准",
            BrightnessLevel::Bright => "明亮",
            BrightnessLevel::High => "高亮",
        }
    }

    fn ambient_mult(self) -> f32 {
        match self {
            BrightnessLevel::Standard => 0.92,
            BrightnessLevel::Bright => 1.12,
            BrightnessLevel::High => 1.34,
        }
    }

    fn light_mult(self) -> f32 {
        match self {
            BrightnessLevel::Standard => 1.12,
            BrightnessLevel::Bright => 1.38,
            BrightnessLevel::High => 1.68,
        }
    }

    fn next(self) -> BrightnessLevel {
        match self {
            BrightnessLevel::Standard => BrightnessLevel::Bright,
            BrightnessLevel::Bright => BrightnessLevel::High,
            BrightnessLevel::High => BrightnessLevel::Standard,
        }
    }

    fn tag(self) -> &'static str {
        match self {
            BrightnessLevel::Standard => "standard",
            BrightnessLevel::Bright => "bright",
            BrightnessLevel::High => "high",
        }
    }

    fn from_tag(tag: &str) -> Option<BrightnessLevel> {
        match tag.trim() {
            "standard" => Some(BrightnessLevel::Standard),
            "bright" => Some(BrightnessLevel::Bright),
            "high" => Some(BrightnessLevel::High),
            _ => None,
        }
    }
}

#[derive(Resource)]
pub struct LightingSettings {
    pub brightness: BrightnessLevel,
}

impl Default for LightingSettings {
    fn default() -> Self {
        Self::load()
    }
}

impl LightingSettings {
    pub fn load() -> Self {
        let brightness =
            BrightnessLevel::from_tag(&load_brightness()).unwrap_or(BrightnessLevel::Bright);
        Self { brightness }
    }

    pub fn cycle_brightness(&mut self) {
        self.brightness = self.brightness.next();
        save_brightness(self.brightness.tag());
    }
}

#[derive(Component)]
pub struct TimedLight {
    life: f32,
    max_life: f32,
    base_intensity: f32,
    base_radius: f32,
}

#[derive(Component)]
struct BrightnessScaledLight {
    base_intensity: f32,
}

#[derive(Component)]
struct FogVisionSource;

#[derive(Component)]
struct ContactShadowCaster;

#[derive(Component)]
struct ContactShadow {
    owner: Entity,
    offset: Vec2,
}

#[derive(Component)]
struct TowerLightCaster;

#[derive(Component)]
struct TowerLight {
    owner: Entity,
}

#[derive(Component)]
struct FireGroundLightCaster;

#[derive(Component)]
struct FollowLight {
    owner: Entity,
    offset: Vec2,
}

#[derive(Component)]
struct HeroWeaponLightCaster;

#[derive(Component)]
struct HeroWeaponLight {
    owner: Entity,
}

#[derive(Component)]
struct ProjectileShadowCaster;

#[derive(Component)]
struct ProjectileShadow {
    owner: Entity,
    offset: Vec2,
}

fn theme_for_level(level_index: usize) -> LevelTheme {
    LEVEL_THEMES
        .get(level_index % LEVEL_THEMES.len())
        .copied()
        .unwrap_or(LEVEL_THEMES[0])
}

fn weather_for_level(level_index: usize) -> LevelWeather {
    LEVEL_WEATHERS
        .get(level_index % LEVEL_WEATHERS.len())
        .copied()
        .unwrap_or(LevelWeather::VerdantDusk)
}

fn default_fog_settings() -> FogMapSettings {
    FogMapSettings {
        enabled: false,
        // The board is 800x600, so smaller chunks keep reveal edges aligned with
        // tower light circles instead of opening giant rectangular blocks.
        chunk_size: UVec2::splat(96),
        texture_resolution_per_chunk: UVec2::splat(192),
        // 未探索区压暗但不全黑（带一点冷色氛围），已探索区只轻微降亮——
        // 精美的关卡美术图应保持可见，迷雾是氛围而非黑幕。
        fog_color_unexplored: Color::srgba(0.01, 0.02, 0.035, 0.58),
        fog_color_explored: Color::srgba(0.01, 0.018, 0.016, 0.30),
        vision_clear_color: Color::NONE,
        ..default()
    }
}

fn fog_of_war_level(level_index: usize) -> bool {
    level_index >= FOG_OF_WAR_START_LEVEL
}

fn fog_source(radius: f32) -> VisionSource {
    let mut source = VisionSource::circle((radius * 0.88).clamp(TILE_SIZE * 1.25, TILE_SIZE * 8.0));
    source.enabled = false;
    source.intensity = 1.0;
    source.transition_ratio = 0.34;
    source
}

fn weather_ambient(weather: LevelWeather, _theme: LevelTheme) -> (Color, f32, Option<f32>, f32) {
    // 环境光基准整体上调：AI 手绘关卡图是画面主角，白天关卡要清晰明快，
    // 夜晚/风暴关卡保留氛围压暗但绝不能糊成一团。
    match weather {
        LevelWeather::DeepNight => (Color::WHITE, 0.46, None, 0.94),
        LevelWeather::BloodRain => (Color::WHITE, 0.52, None, 0.90),
        LevelWeather::Starfall | LevelWeather::MoonlitGorge | LevelWeather::ThunderRift => {
            (Color::WHITE, 0.54, None, 0.92)
        }
        LevelWeather::Sandstorm | LevelWeather::Blizzard => (Color::WHITE, 0.62, None, 0.92),
        _ => (Color::WHITE, 0.66, None, 0.90),
    }
}

fn firefly_config(level_index: usize, brightness: BrightnessLevel) -> FireflyConfig {
    let theme = theme_for_level(level_index);
    let weather = weather_for_level(level_index);
    let (ambient_color, ambient_brightness, light_bands, normal_attenuation) =
        weather_ambient(weather, theme);
    FireflyConfig {
        ambient_color,
        ambient_brightness: (ambient_brightness * brightness.ambient_mult()).clamp(0.25, 0.96),
        light_bands,
        soft_shadows: true,
        z_sorting: true,
        z_sorting_error_margin: 0.06,
        normal_mode: NormalMode::None,
        normal_attenuation,
        combination_mode: CombinationMode::Multiply,
        lightmap_size: LightmapSize::Scaled(1.0),
        lightmap_filtering: true,
        enable_32bit_stencils: false,
    }
}

pub fn camera_config(level_index: usize) -> FireflyConfig {
    firefly_config(level_index, BrightnessLevel::Bright)
}

pub fn sync_firefly_camera(
    current: Res<CurrentLevel>,
    settings: Res<LightingSettings>,
    mut cameras: Query<&mut FireflyConfig, With<Camera2d>>,
    mut last_state: Local<Option<(usize, BrightnessLevel)>>,
) {
    let state = (current.0, settings.brightness);
    if *last_state == Some(state) && !current.is_changed() && !settings.is_changed() {
        return;
    }
    let config = firefly_config(current.0, settings.brightness);
    for mut camera in &mut cameras {
        *camera = config.clone();
    }
    *last_state = Some(state);
}

fn configure_fog_of_war(
    current: Res<CurrentLevel>,
    game_state: Res<State<GameState>>,
    mut fog: ResMut<FogMapSettings>,
    mut reset: MessageWriter<ResetFogOfWar>,
    mut last: Local<Option<(usize, bool)>>,
) {
    let enabled = matches!(game_state.get(), GameState::Playing) && fog_of_war_level(current.0);
    let state = (current.0, enabled);
    if *last == Some(state) && !current.is_changed() && !game_state.is_changed() {
        return;
    }

    if fog.enabled != enabled {
        fog.enabled = enabled;
    }
    if enabled && *last != Some(state) {
        reset.write(ResetFogOfWar);
    }
    *last = Some(state);
}

fn sync_fog_source_enabled(
    current: Res<CurrentLevel>,
    game_state: Res<State<GameState>>,
    mut sources: Query<&mut VisionSource, With<FogVisionSource>>,
) {
    let enabled = matches!(game_state.get(), GameState::Playing) && fog_of_war_level(current.0);
    for mut source in &mut sources {
        source.enabled = enabled;
    }
}

fn update_enemy_fog_visibility(
    mut commands: Commands,
    fog: Res<FogMapSettings>,
    game_state: Res<State<GameState>>,
    sources: Query<(&GlobalTransform, &VisionSource), With<FogVisionSource>>,
    mut enemies: Query<(Entity, &Transform, &mut Visibility, Option<&FogHidden>), With<Enemy>>,
) {
    let enabled = fog.enabled && matches!(game_state.get(), GameState::Playing);
    for (entity, enemy_tf, mut visibility, hidden) in &mut enemies {
        let lit = !enabled
            || sources.iter().any(|(source_tf, source)| {
                source.enabled
                    && vision_contains(source_tf, source, enemy_tf.translation.truncate())
            });

        if lit {
            if hidden.is_some() {
                commands.entity(entity).remove::<FogHidden>();
            }
            *visibility = Visibility::Visible;
        } else {
            if hidden.is_none() {
                commands.entity(entity).insert(FogHidden);
            }
            *visibility = Visibility::Hidden;
        }
    }
}

fn vision_contains(tf: &GlobalTransform, source: &VisionSource, pos: Vec2) -> bool {
    let origin = tf.translation().truncate();
    match source.shape {
        VisionShape::Circle => origin.distance_squared(pos) <= source.range * source.range,
        VisionShape::Square => {
            let half = source.range;
            (pos.x - origin.x).abs() <= half && (pos.y - origin.y).abs() <= half
        }
        VisionShape::Cone => {
            let delta = pos - origin;
            if delta.length_squared() > source.range * source.range {
                return false;
            }
            let forward = Vec2::from_angle(source.direction);
            let angle = forward.angle_to(delta.normalize_or_zero()).abs();
            angle <= source.angle * 0.5
        }
    }
}

pub fn spawn_level_lighting(
    commands: &mut Commands,
    board: &Board,
    theme: LevelTheme,
    settings: &LightingSettings,
) {
    let weather = weather_for_level(board.level_index);
    let (main_color, fill_color, main_pos, fill_pos, intensity) = weather_lights(weather, theme);
    let light_mult = settings.brightness.light_mult();

    spawn_weather_light(
        commands,
        main_pos,
        main_color,
        BOARD_W * 0.74,
        intensity,
        light_mult,
        true,
        175.0,
    );
    spawn_weather_light(
        commands,
        fill_pos,
        fill_color,
        BOARD_W * 0.56,
        intensity * 0.24,
        light_mult,
        false,
        120.0,
    );

    let spawn = board.spawn_pos();
    spawn_static_light(
        commands,
        spawn,
        Color::srgb(0.72, 0.22, 1.0),
        TILE_SIZE * 4.8,
        1.52,
        light_mult,
        true,
        55.0,
    );

    let carrot = board.carrot_pos();
    spawn_static_light(
        commands,
        carrot,
        theme.seal.mix(&Color::srgb(0.55, 1.0, 0.25), 0.44),
        TILE_SIZE * 4.4,
        1.24,
        light_mult,
        true,
        48.0,
    );
}

fn weather_lights(weather: LevelWeather, theme: LevelTheme) -> (Color, Color, Vec2, Vec2, f32) {
    match weather {
        LevelWeather::Sandstorm => (
            Color::srgb(1.0, 0.86, 0.52),
            Color::srgb(0.82, 0.90, 1.0),
            Vec2::new(-BOARD_W * 0.44, BOARD_H * 0.44),
            Vec2::new(BOARD_W * 0.30, -BOARD_H * 0.22),
            0.07,
        ),
        LevelWeather::Blizzard => (
            Color::srgb(0.82, 0.96, 1.0),
            Color::srgb(0.62, 0.78, 1.0),
            Vec2::new(BOARD_W * 0.32, BOARD_H * 0.46),
            Vec2::new(-BOARD_W * 0.40, -BOARD_H * 0.28),
            0.06,
        ),
        LevelWeather::BloodRain => (
            Color::srgb(1.0, 0.50, 0.44),
            Color::srgb(0.72, 0.46, 0.58),
            Vec2::new(-BOARD_W * 0.40, BOARD_H * 0.30),
            Vec2::new(BOARD_W * 0.42, -BOARD_H * 0.34),
            0.055,
        ),
        LevelWeather::ThunderRift => (
            Color::srgb(0.66, 0.94, 1.0),
            Color::srgb(0.70, 0.66, 1.0),
            Vec2::new(BOARD_W * 0.15, BOARD_H * 0.52),
            Vec2::new(-BOARD_W * 0.44, -BOARD_H * 0.08),
            0.075,
        ),
        LevelWeather::DeepNight => (
            Color::srgb(0.56, 0.64, 1.0),
            Color::srgb(0.62, 0.52, 0.90),
            Vec2::new(-BOARD_W * 0.18, BOARD_H * 0.50),
            Vec2::new(BOARD_W * 0.42, -BOARD_H * 0.24),
            0.045,
        ),
        _ => (
            theme.accent.mix(&Color::srgb(1.0, 0.95, 0.78), 0.70),
            theme.seal.mix(&Color::srgb(0.82, 0.90, 1.0), 0.76),
            Vec2::new(-BOARD_W * 0.42, BOARD_H * 0.42),
            Vec2::new(BOARD_W * 0.36, -BOARD_H * 0.26),
            0.055,
        ),
    }
}

fn point_light(color: Color, radius: f32, intensity: f32, cast_shadows: bool) -> PointLight2d {
    PointLight2d {
        color,
        intensity,
        radius,
        falloff: Falloff::linear(-0.38),
        core: LightCore::from_radius_boost((radius * 0.11).clamp(16.0, 54.0), 1.85),
        cast_shadows,
        ..default()
    }
}

fn weather_light(color: Color, radius: f32, intensity: f32, cast_shadows: bool) -> PointLight2d {
    PointLight2d {
        color,
        intensity,
        radius,
        falloff: Falloff::linear(0.18),
        core: LightCore::NONE,
        cast_shadows,
        ..default()
    }
}

fn spawn_static_light(
    commands: &mut Commands,
    pos: Vec2,
    color: Color,
    radius: f32,
    intensity: f32,
    light_mult: f32,
    cast_shadows: bool,
    height: f32,
) {
    commands.spawn((
        point_light(color, radius, intensity * light_mult, cast_shadows),
        BrightnessScaledLight {
            base_intensity: intensity,
        },
        FogVisionSource,
        fog_source(radius),
        LightHeight(height),
        Transform::from_translation(pos.extend(18.0)),
        LevelEntity,
    ));
}

fn spawn_weather_light(
    commands: &mut Commands,
    pos: Vec2,
    color: Color,
    radius: f32,
    intensity: f32,
    light_mult: f32,
    cast_shadows: bool,
    height: f32,
) {
    commands.spawn((
        weather_light(color, radius, intensity * light_mult, cast_shadows),
        BrightnessScaledLight {
            base_intensity: intensity,
        },
        LightHeight(height),
        Transform::from_translation(pos.extend(18.0)),
        LevelEntity,
    ));
}

pub fn spawn_scene_light(
    commands: &mut Commands,
    pos: Vec2,
    color: Color,
    radius: f32,
    intensity: f32,
    cast_shadows: bool,
    height: f32,
    settings: &LightingSettings,
) {
    spawn_static_light(
        commands,
        pos,
        color,
        radius,
        intensity,
        settings.brightness.light_mult(),
        cast_shadows,
        height,
    );
}

fn apply_brightness_to_lights(
    settings: Res<LightingSettings>,
    mut lights: Query<(&BrightnessScaledLight, &mut PointLight2d)>,
) {
    if !settings.is_changed() {
        return;
    }
    let mult = settings.brightness.light_mult();
    for (scaled, mut light) in &mut lights {
        light.intensity = scaled.base_intensity * mult;
    }
}

fn spawn_tower_lights(
    mut commands: Commands,
    settings: Res<LightingSettings>,
    towers: Query<(Entity, &Tower, &Transform), (Added<Tower>, Without<TowerLightCaster>)>,
) {
    let light_mult = settings.brightness.light_mult();
    for (entity, tower, tf) in &towers {
        let Some(spec) = tower_light_spec(tower) else {
            continue;
        };
        commands.entity(entity).insert(TowerLightCaster);
        let (radius, intensity) = tower_light_level_values(&spec, tower.level);
        commands.spawn((
            point_light(
                spec.color,
                radius,
                intensity * light_mult,
                spec.cast_shadows,
            ),
            BrightnessScaledLight {
                base_intensity: intensity,
            },
            FogVisionSource,
            fog_source(radius),
            LightHeight(spec.height),
            Transform::from_translation((tf.translation.truncate() + spec.offset).extend(17.6)),
            TowerLight { owner: entity },
            FollowLight {
                owner: entity,
                offset: spec.offset,
            },
            LevelEntity,
        ));
    }
}

struct TowerLightSpec {
    color: Color,
    radius: f32,
    intensity: f32,
    level_radius: f32,
    height: f32,
    offset: Vec2,
    cast_shadows: bool,
}

fn tower_light_level_values(spec: &TowerLightSpec, level: i32) -> (f32, f32) {
    let lvl = (level - 1).max(0) as f32;
    (
        spec.radius * (1.0 + lvl * spec.level_radius),
        spec.intensity * (1.0 + lvl * 0.24),
    )
}

fn tower_light_spec(tower: &Tower) -> Option<TowerLightSpec> {
    let body = TILE_SIZE * tower.footprint.max(1) as f32;
    let offset = Vec2::new(0.0, body * 0.12);
    let spec = match tower.kind {
        TowerKind::Fire => TowerLightSpec {
            color: Color::srgb(1.0, 0.46, 0.12),
            radius: TILE_SIZE * 4.2,
            intensity: 2.15,
            level_radius: 0.20,
            height: 70.0,
            offset,
            cast_shadows: true,
        },
        TowerKind::Thunder => TowerLightSpec {
            color: Color::srgb(0.52, 0.86, 1.0),
            radius: TILE_SIZE * 2.8,
            intensity: 0.72,
            level_radius: 0.18,
            height: 76.0,
            offset,
            cast_shadows: false,
        },
        TowerKind::Prism | TowerKind::Laser => TowerLightSpec {
            color: Color::srgb(0.54, 0.96, 1.0),
            radius: TILE_SIZE * 3.0,
            intensity: 0.88,
            level_radius: 0.22,
            height: 86.0,
            offset,
            cast_shadows: false,
        },
        TowerKind::Magic => TowerLightSpec {
            color: Color::srgb(0.72, 0.44, 1.0),
            radius: TILE_SIZE * 2.45,
            intensity: 0.58,
            level_radius: 0.16,
            height: 62.0,
            offset,
            cast_shadows: false,
        },
        _ => return None,
    };
    Some(spec)
}

fn update_tower_lights(
    mut commands: Commands,
    settings: Res<LightingSettings>,
    towers: Query<&Tower>,
    mut lights: Query<(
        Entity,
        &TowerLight,
        &mut PointLight2d,
        &mut BrightnessScaledLight,
        &mut VisionSource,
        &mut LightHeight,
        &mut FollowLight,
    )>,
) {
    let light_mult = settings.brightness.light_mult();
    for (entity, tower_light, mut light, mut scaled, mut vision, mut height, mut follow) in
        &mut lights
    {
        let Ok(tower) = towers.get(tower_light.owner) else {
            commands.entity(entity).despawn();
            continue;
        };
        let Some(spec) = tower_light_spec(tower) else {
            commands.entity(entity).despawn();
            continue;
        };
        let (radius, intensity) = tower_light_level_values(&spec, tower.level);
        light.color = spec.color;
        light.radius = radius;
        light.intensity = intensity * light_mult;
        light.cast_shadows = spec.cast_shadows;
        scaled.base_intensity = intensity;
        vision.range = fog_source(radius).range;
        vision.intensity = 1.0;
        vision.transition_ratio = 0.34;
        height.0 = spec.height;
        follow.owner = tower_light.owner;
        follow.offset = spec.offset;
    }
}

fn spawn_hero_weapon_lights(
    mut commands: Commands,
    settings: Res<LightingSettings>,
    heroes: Query<(Entity, &Tower), (Added<Tower>, Without<HeroWeaponLightCaster>)>,
) {
    let light_mult = settings.brightness.light_mult();
    for (entity, tower) in &heroes {
        if !tower.hero {
            continue;
        }
        commands.entity(entity).insert(HeroWeaponLightCaster);
        let color = hero_weapon_color(tower);
        let pos = hero_weapon_pos(tower);
        commands.spawn((
            point_light(color, TILE_SIZE * 1.75, 0.30 * light_mult, false),
            FogVisionSource,
            fog_source(TILE_SIZE * 1.75),
            LightHeight(58.0),
            Transform::from_translation(pos.extend(17.75)),
            HeroWeaponLight { owner: entity },
            LevelEntity,
        ));
    }
}

fn update_hero_weapon_lights(
    mut commands: Commands,
    settings: Res<LightingSettings>,
    towers: Query<&Tower>,
    mut lights: Query<(
        Entity,
        &HeroWeaponLight,
        &mut PointLight2d,
        &mut VisionSource,
        &mut Transform,
    )>,
) {
    let light_mult = settings.brightness.light_mult();
    for (entity, weapon, mut light, mut vision, mut tf) in &mut lights {
        let Ok(tower) = towers.get(weapon.owner) else {
            commands.entity(entity).despawn();
            continue;
        };
        if !tower.hero || tower.hp <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        let melee = (tower.hero_attack_timer / HERO_MELEE_ATTACK_TIME).clamp(0.0, 1.0);
        let shot = if tower.cooldown > 0.0 {
            ((tower.cooldown_timer - (tower.cooldown - 0.22)) / 0.22).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let pulse = melee.max(shot);
        let color = hero_weapon_color(tower);
        let pos = hero_weapon_pos(tower);
        light.color = color.mix(&Color::WHITE, 0.10 + pulse * 0.20);
        light.radius = TILE_SIZE * (1.70 + pulse * 1.55);
        light.intensity = (0.28 + pulse * 1.70) * light_mult;
        vision.range = fog_source(light.radius).range;
        tf.translation.x = pos.x;
        tf.translation.y = pos.y;
    }
}

fn hero_weapon_color(tower: &Tower) -> Color {
    match tower.hero_weapon {
        Some(HeroWeapon::BannerSword) => Color::srgb(1.0, 0.70, 0.30),
        Some(HeroWeapon::StarfireStaff) => Color::srgb(0.70, 0.42, 1.0),
        Some(HeroWeapon::ShadowBow) => Color::srgb(0.54, 1.0, 0.42),
        Some(HeroWeapon::OathShield) => Color::srgb(0.56, 0.84, 1.0),
        Some(HeroWeapon::StormOrb) => Color::srgb(0.46, 0.92, 1.0),
        Some(HeroWeapon::SentryCrossbow) => Color::srgb(0.48, 1.0, 0.70),
        Some(HeroWeapon::NightDagger) => Color::srgb(0.92, 0.30, 1.0),
        Some(HeroWeapon::SummonStaff) => Color::srgb(0.48, 1.0, 0.82),
        Some(HeroWeapon::ForgeHammer) => Color::srgb(1.0, 0.62, 0.24),
        None => tower.element.color().mix(&tower.color, 0.35),
    }
}

fn hero_weapon_pos(tower: &Tower) -> Vec2 {
    let dir = Vec2::from_angle(tower.angle);
    tower.center() + dir * (TILE_SIZE * 0.42) + Vec2::Y * (TILE_SIZE * 0.16)
}

fn spawn_fire_ground_lights(
    mut commands: Commands,
    settings: Res<LightingSettings>,
    fires: Query<
        (Entity, &FireGround, &Transform),
        (Added<FireGround>, Without<FireGroundLightCaster>),
    >,
) {
    let light_mult = settings.brightness.light_mult();
    for (entity, fire, tf) in &fires {
        if fire.dps <= 0.0 {
            continue;
        }
        commands.entity(entity).insert(FireGroundLightCaster);
        let radius = (fire.half_len * 0.82).clamp(TILE_SIZE * 2.0, TILE_SIZE * 4.6);
        let intensity = (0.58 + fire.dps * 0.004).clamp(0.60, 1.05);
        commands.spawn((
            point_light(
                fire.element
                    .color()
                    .mix(&Color::srgb(1.0, 0.42, 0.12), 0.44),
                radius,
                intensity * light_mult,
                false,
            ),
            BrightnessScaledLight {
                base_intensity: intensity,
            },
            FogVisionSource,
            fog_source(radius),
            LightHeight(44.0),
            Transform::from_translation(tf.translation.truncate().extend(17.3)),
            FollowLight {
                owner: entity,
                offset: Vec2::ZERO,
            },
            LevelEntity,
        ));
    }
}

fn update_follow_lights(
    mut commands: Commands,
    owners: Query<&Transform, Without<FollowLight>>,
    mut lights: Query<(Entity, &FollowLight, &mut Transform)>,
) {
    for (entity, follow, mut tf) in &mut lights {
        let Ok(owner_tf) = owners.get(follow.owner) else {
            commands.entity(entity).despawn();
            continue;
        };
        let pos = owner_tf.translation.truncate() + follow.offset;
        tf.translation.x = pos.x;
        tf.translation.y = pos.y;
    }
}

fn spawn_projectile_shadows(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    projectiles: Query<
        (Entity, &Projectile, Option<&ProjectileVisual>, &Transform),
        (Added<Projectile>, Without<ProjectileShadowCaster>),
    >,
) {
    for (entity, projectile, visual, tf) in &projectiles {
        let (size, alpha) = projectile_shadow_style(projectile, visual.map(|v| v.tower_kind));
        let offset = Vec2::new(6.0, -TILE_SIZE * 0.22);
        commands.entity(entity).insert(ProjectileShadowCaster);
        commands.spawn((
            Mesh2d(meshes.add(Ellipse::new(size.x * 0.5, size.y * 0.5))),
            MeshMaterial2d(materials.add(Color::srgba(0.0, 0.0, 0.0, alpha))),
            Transform::from_translation((tf.translation.truncate() + offset).extend(3.28))
                .with_rotation(tf.rotation),
            ProjectileShadow {
                owner: entity,
                offset,
            },
            LevelEntity,
        ));
    }
}

fn projectile_shadow_style(projectile: &Projectile, tower_kind: Option<TowerKind>) -> (Vec2, f32) {
    let base = match projectile.kind {
        ProjKind::Fireball => Vec2::new(34.0, 13.0),
        ProjKind::Missile => Vec2::new(30.0, 11.0),
        ProjKind::Slow | ProjKind::Poison | ProjKind::Curse => Vec2::new(24.0, 8.0),
        ProjKind::Knockback => Vec2::new(26.0, 9.0),
        ProjKind::Normal => Vec2::new(20.0, 7.0),
    };
    let scaled = match tower_kind {
        Some(TowerKind::Sniper | TowerKind::Arrow | TowerKind::Wind) => base * 0.82,
        Some(TowerKind::Missile | TowerKind::Magic) => base * 1.12,
        _ => base,
    };
    (scaled, 0.30)
}

fn update_projectile_shadows(
    mut commands: Commands,
    owners: Query<&Transform, Without<ProjectileShadow>>,
    mut shadows: Query<(Entity, &ProjectileShadow, &mut Transform)>,
) {
    for (entity, shadow, mut tf) in &mut shadows {
        let Ok(owner_tf) = owners.get(shadow.owner) else {
            commands.entity(entity).despawn();
            continue;
        };
        let pos = owner_tf.translation.truncate() + shadow.offset;
        tf.translation.x = pos.x;
        tf.translation.y = pos.y;
        tf.rotation = owner_tf.rotation;
    }
}

pub fn attach_shadow_casters(
    mut commands: Commands,
    enemies: Query<(Entity, &Enemy), (Added<Enemy>, Without<Occluder2d>)>,
    towers: Query<(Entity, &Tower), (Added<Tower>, Without<Occluder2d>)>,
    summons: Query<Entity, (Added<Summon>, Without<Occluder2d>)>,
    carrots: Query<Entity, (Added<Carrot>, Without<Occluder2d>)>,
    portals: Query<Entity, (Added<SpawnPortal>, Without<Occluder2d>)>,
) {
    for (entity, enemy) in &enemies {
        let radius = (enemy.size * if enemy.boss { 0.34 } else { 0.28 }).clamp(8.0, 34.0);
        commands.entity(entity).insert((
            Occluder2d::circle(radius)
                .with_opacity(if enemy.boss { 0.48 } else { 0.34 })
                .with_color(Color::srgb(0.02, 0.02, 0.025)),
            SpriteHeight(enemy.size * if enemy.boss { 0.62 } else { 0.36 }),
        ));
    }

    for (entity, tower) in &towers {
        let fp = tower.footprint.max(1) as f32;
        let body = TILE_SIZE * fp;
        let opacity = if tower.hero { 0.36 } else { 0.46 };
        commands.entity(entity).insert((
            Occluder2d::round_rectangle(body * 0.44, body * 0.34, body * 0.08)
                .with_opacity(opacity)
                .with_color(Color::srgb(0.025, 0.025, 0.03)),
            SpriteHeight(if tower.hero { 32.0 } else { body * 0.46 }),
        ));
    }

    for entity in &summons {
        commands.entity(entity).insert((
            Occluder2d::circle(TILE_SIZE * 0.20)
                .with_opacity(0.32)
                .with_color(Color::srgb(0.025, 0.025, 0.03)),
            SpriteHeight(TILE_SIZE * 0.34),
        ));
    }

    for entity in &carrots {
        commands.entity(entity).insert((
            Occluder2d::circle(TILE_SIZE * 0.36)
                .with_opacity(0.34)
                .with_color(Color::srgb(0.03, 0.02, 0.01)),
            SpriteHeight(TILE_SIZE * 0.56),
        ));
    }

    for entity in &portals {
        commands.entity(entity).insert((
            Occluder2d::circle(TILE_SIZE * 0.58)
                .with_opacity(0.36)
                .with_color(Color::srgb(0.02, 0.016, 0.03)),
            SpriteHeight(TILE_SIZE * 0.30),
        ));
    }
}

fn spawn_contact_shadows(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    enemies: Query<(Entity, &Enemy, &Transform), (Added<Enemy>, Without<ContactShadowCaster>)>,
    towers: Query<(Entity, &Tower, &Transform), (Added<Tower>, Without<ContactShadowCaster>)>,
    summons: Query<(Entity, &Transform), (Added<Summon>, Without<ContactShadowCaster>)>,
    carrots: Query<(Entity, &Transform), (Added<Carrot>, Without<ContactShadowCaster>)>,
    portals: Query<(Entity, &Transform), (Added<SpawnPortal>, Without<ContactShadowCaster>)>,
) {
    for (entity, enemy, tf) in &enemies {
        let alpha = if enemy.flying {
            0.16
        } else if enemy.boss {
            0.42
        } else {
            0.34
        };
        let width = enemy.size * if enemy.boss { 1.0 } else { 0.78 };
        let height = enemy.size * if enemy.boss { 0.34 } else { 0.24 };
        let offset = Vec2::new(5.0, -enemy.size * if enemy.flying { 0.42 } else { 0.30 });
        spawn_contact_shadow(
            &mut commands,
            &mut meshes,
            &mut materials,
            entity,
            tf.translation.truncate(),
            Vec2::new(width, height),
            offset,
            alpha,
            3.15,
        );
    }

    for (entity, tower, tf) in &towers {
        let body = TILE_SIZE * tower.footprint.max(1) as f32;
        let alpha = if tower.hero { 0.34 } else { 0.38 };
        spawn_contact_shadow(
            &mut commands,
            &mut meshes,
            &mut materials,
            entity,
            tf.translation.truncate(),
            Vec2::new(body * 0.76, body * 0.28),
            Vec2::new(5.0, -body * 0.30),
            alpha,
            3.05,
        );
    }

    for (entity, tf) in &summons {
        spawn_contact_shadow(
            &mut commands,
            &mut meshes,
            &mut materials,
            entity,
            tf.translation.truncate(),
            Vec2::new(TILE_SIZE * 0.38, TILE_SIZE * 0.12),
            Vec2::new(4.0, -TILE_SIZE * 0.18),
            0.28,
            3.20,
        );
    }

    for (entity, tf) in &carrots {
        spawn_contact_shadow(
            &mut commands,
            &mut meshes,
            &mut materials,
            entity,
            tf.translation.truncate(),
            Vec2::new(TILE_SIZE * 0.82, TILE_SIZE * 0.24),
            Vec2::new(5.0, -TILE_SIZE * 0.34),
            0.34,
            1.9,
        );
    }

    for (entity, tf) in &portals {
        spawn_contact_shadow(
            &mut commands,
            &mut meshes,
            &mut materials,
            entity,
            tf.translation.truncate(),
            Vec2::new(TILE_SIZE * 1.05, TILE_SIZE * 0.30),
            Vec2::new(6.0, -TILE_SIZE * 0.28),
            0.32,
            1.55,
        );
    }
}

fn spawn_contact_shadow(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    owner: Entity,
    owner_pos: Vec2,
    size: Vec2,
    offset: Vec2,
    alpha: f32,
    z: f32,
) {
    commands.entity(owner).insert(ContactShadowCaster);
    commands.spawn((
        Mesh2d(meshes.add(Ellipse::new(size.x * 0.5, size.y * 0.5))),
        MeshMaterial2d(materials.add(Color::srgba(0.0, 0.0, 0.0, alpha))),
        Transform::from_translation((owner_pos + offset).extend(z)),
        ContactShadow { owner, offset },
        LevelEntity,
    ));
}

fn update_contact_shadows(
    mut commands: Commands,
    owners: Query<&Transform, Without<ContactShadow>>,
    mut shadows: Query<(Entity, &ContactShadow, &mut Transform)>,
) {
    for (entity, shadow, mut tf) in &mut shadows {
        let Ok(owner_tf) = owners.get(shadow.owner) else {
            commands.entity(entity).despawn();
            continue;
        };
        let pos = owner_tf.translation.truncate() + shadow.offset;
        tf.translation.x = pos.x;
        tf.translation.y = pos.y;
    }
}

// ---- persistence ----

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(inline_js = r#"
export function load_brightness() {
  try { return globalThis.localStorage?.getItem('protect_carrot_brightness') || ''; }
  catch (_) { return ''; }
}
export function save_brightness(value) {
  try { globalThis.localStorage?.setItem('protect_carrot_brightness', value); }
  catch (_) {}
}
"#)]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = load_brightness)]
    fn load_brightness_js() -> String;
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = save_brightness)]
    fn save_brightness_js(value: &str);
}

#[cfg(target_arch = "wasm32")]
fn load_brightness() -> String {
    load_brightness_js()
}

#[cfg(target_arch = "wasm32")]
fn save_brightness(value: &str) {
    save_brightness_js(value);
}

#[cfg(not(target_arch = "wasm32"))]
fn load_brightness() -> String {
    std::fs::read_to_string("tmp/brightness.txt").unwrap_or_default()
}

#[cfg(not(target_arch = "wasm32"))]
fn save_brightness(value: &str) {
    let _ = std::fs::create_dir_all("tmp");
    let _ = std::fs::write("tmp/brightness.txt", value);
}

pub fn pulse_light(
    commands: &mut Commands,
    pos: Vec2,
    color: Color,
    radius: f32,
    intensity: f32,
    life: f32,
    cast_shadows: bool,
) {
    commands.spawn((
        point_light(color, radius, intensity, cast_shadows),
        LightHeight((radius * 0.22).clamp(34.0, 120.0)),
        TimedLight {
            life,
            max_life: life,
            base_intensity: intensity,
            base_radius: radius,
        },
        Transform::from_translation(pos.extend(19.0)),
        LevelEntity,
    ));
}

pub fn update_timed_lights(
    mut commands: Commands,
    time: Res<Time>,
    mut lights: Query<(Entity, &mut TimedLight, &mut PointLight2d)>,
) {
    let dt = time.delta_secs();
    for (entity, mut timed, mut light) in &mut lights {
        timed.life -= dt;
        if timed.life <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        let t = (timed.life / timed.max_life).clamp(0.0, 1.0);
        let ease = t * t;
        light.intensity = timed.base_intensity * ease;
        light.radius = timed.base_radius * (0.72 + 0.28 * t);
    }
}
