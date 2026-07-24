//! Headless capture entrypoint for screenshots and frame sequences.
//!
//! Usage:
//!   cargo run --bin capture
//!   cargo run --bin capture -- screenshot screenshots/capture/still.png
//!   cargo run --bin capture -- frames screenshots/capture 120

use std::{collections::HashSet, env, path::PathBuf, process, time::Duration};

use bevy::{
    app::{AppExit, ScheduleRunnerPlugin},
    camera::{RenderTarget, ScalingMode},
    prelude::*,
    render::{
        RenderPlugin,
        render_resource::TextureFormat,
        view::screenshot::{Capturing, Screenshot, save_to_disk},
    },
    time::TimeUpdateStrategy,
    window::{ExitCondition, WindowPlugin, WindowResolution},
    winit::WinitPlugin,
};
use bevy_fog_of_war::prelude::{FogMapSettings, FogOfWarCamera, VisionSource};
use bevy_paperdoll::PaperdollPlugin;
use bevy_spritesheet_animation::prelude::SpritesheetAnimationPlugin;

use protect_carrot::{
    Levels, audio, bestiary,
    board::Board,
    build::{self, Selection},
    components::{Enemy, FogHidden},
    creatures, data, enemy, equipment, game, hero, hero_gear, hero_paperdoll, i18n, lighting, meta,
    quality, roguelite, sprites,
    states::GameState,
    tower, tutorial, ui, vfx,
};

use data::{BOARD_H, BOARD_W, TILE_SIZE, TowerKind, cell_center, hex, levels};
use game::{
    CurrentLevel, Paused, Rng, RunState, load_level, not_paused, tick_auto_wave, tick_message,
};
use sprites::build_sprites;
use tower::{BuffTower, Damage, HealCarrot, Snapshot, Status};
use ui::{UiFont, spawn_hud, update_hud};

const CAPTURE_W: u32 = 1280;
const CAPTURE_H: u32 = 720;
const FPS: f64 = 30.0;
const SETTLE_FRAMES: u32 = 90;
const DRAIN_FRAMES: u32 = 45;
const DEFAULT_STILL: &str = "screenshots/capture/still.png";
const DEFAULT_FRAMES_DIR: &str = "screenshots/capture";
const DEFAULT_FRAME_COUNT: u32 = 120;

const PANEL_W: f32 = 256.0;
const VIRTUAL_W: f32 = BOARD_W + PANEL_W;
const VIRTUAL_H: f32 = BOARD_H;

#[derive(Resource, Clone)]
struct CaptureTarget(Handle<Image>);

#[derive(Resource, Default)]
struct CapturePrepared {
    level_ready: bool,
    scenario_ready: bool,
}

#[derive(Resource, Default)]
struct CaptureEscapeAfterScenario {
    pending: bool,
    fired: bool,
}

#[derive(Resource, Default)]
struct CaptureMouseInputSmoke {
    step: u8,
    build_cell: Option<(i32, i32)>,
    tower_count_before: usize,
    hero_entity: Option<Entity>,
    move_target: Option<Vec2>,
}

#[derive(Resource, Default)]
struct CaptureHeroSkillSmoke {
    step: u8,
    mythic_count: usize,
    guard_count: usize,
    guard_homes: Vec<Vec2>,
    moved_hero_pos: Option<Vec2>,
}

#[derive(Resource, Default)]
struct CaptureFogProbeSeeded(bool);

#[derive(Resource, Clone, Copy)]
enum CaptureScreen {
    Playing,
    HeroCodex,
    Briefing,
    Armory,
}

impl CaptureScreen {
    fn initial_state(self) -> GameState {
        match self {
            CaptureScreen::Playing => GameState::Playing,
            CaptureScreen::HeroCodex => GameState::HeroCodex,
            CaptureScreen::Briefing => GameState::Briefing,
            CaptureScreen::Armory => GameState::Armory,
        }
    }
}

#[derive(Resource, Clone, Copy, PartialEq, Eq)]
enum CaptureScenario {
    Default,
    TowerGems,
    HeroPaperdoll,
    HeroPaperdollEscClosed,
    InventoryVisibility,
    Fog,
    TowerSelected,
    HeroSelected,
    MouseInputSmoke,
    HeroSkillSmoke,
    RogueliteDraft,
    RoguelitePick,
}

#[derive(Clone)]
enum CaptureMode {
    Screenshot,
    Frames,
}

#[derive(Resource, Clone)]
struct CaptureJob {
    mode: CaptureMode,
    output: PathBuf,
    total_frames: u32,
    tick: u32,
    scheduled: u32,
    exit_after_tick: Option<u32>,
}

fn main() -> AppExit {
    let job = parse_args();
    prepare_output_path(&job);
    let capture_screen = capture_screen_from_env();
    let capture_scenario = capture_scenario_from_env();
    let capture_level = capture_level_from_env(capture_scenario);

    let mut app = App::new();

    app.add_plugins(
        DefaultPlugins
            .set(bevy::log::LogPlugin {
                filter: "error,protect_carrot=info,bevy=warn,wgpu=error,naga=warn,\
                         icu_provider=off,icu_segmenter=off,icu_locale=off,\
                         icu_properties=off,icu_normalizer=off,icu_collections=off"
                    .into(),
                level: bevy::log::Level::INFO,
                ..default()
            })
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..default()
            })
            .set(AssetPlugin {
                file_path: format!("{}/assets", env!("CARGO_MANIFEST_DIR")),
                meta_check: bevy::asset::AssetMetaCheck::Never,
                watch_for_changes_override: cfg!(feature = "dev").then_some(true),
                ..default()
            })
            .set(RenderPlugin {
                synchronous_pipeline_compilation: true,
                ..default()
            })
            .disable::<WinitPlugin>(),
    )
    .add_plugins(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
        1.0 / 120.0,
    )))
    .add_plugins(lighting::LightingPlugin)
    .add_plugins(PaperdollPlugin)
    .add_plugins(hero_paperdoll::HeroPaperdollPlugin)
    .add_plugins(protect_carrot::polish::PolishPlugin)
    .add_plugins(SpritesheetAnimationPlugin)
    .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
        1.0 / FPS,
    )))
    .insert_state(capture_screen.initial_state())
    .insert_resource(ClearColor(hex(0x1e2a1e)))
    .insert_resource(quality::GraphicsQuality {
        level: quality::QualityLevel::Balanced,
    })
    .insert_resource(lighting::LightingSettings::load())
    .init_resource::<audio::AudioSettings>()
    .init_resource::<i18n::Language>()
    .init_resource::<ui::MenuDirty>()
    .insert_resource(Levels(levels()))
    .insert_resource(CurrentLevel(capture_level))
    .init_resource::<Paused>()
    .init_resource::<game::GameMode>()
    .init_resource::<game::GameDifficulty>()
    .init_resource::<Rng>()
    .init_resource::<RunState>()
    .init_resource::<Selection>()
    .init_resource::<Snapshot>()
    .init_resource::<tutorial::Tutorial>()
    .init_resource::<ui::TouchMode>()
    .init_resource::<ui::HudPanels>()
    .init_resource::<ui::JoystickState>()
    .init_resource::<ui::TalentConfirm>()
    .init_resource::<ui::StoryTimeline>()
    .init_resource::<ui::BriefingTimeline>()
    .init_resource::<hero::HeroLoadout>()
    .init_resource::<hero_gear::HeroGearInventory>()
    .init_resource::<ui::TooltipHold>()
    .init_resource::<meta::Talents>()
    .init_resource::<meta::Abilities>()
    .init_resource::<roguelite::RogueliteRun>()
    .init_resource::<equipment::EquipmentInventory>()
    .init_resource::<bestiary::Bestiary>()
    .init_resource::<vfx::ScreenShake>()
    .init_resource::<build::HeroWalks>()
    .init_resource::<ui::StoryDialogue>()
    .init_resource::<CapturePrepared>()
    .init_resource::<CaptureEscapeAfterScenario>()
    .init_resource::<CaptureMouseInputSmoke>()
    .init_resource::<CaptureHeroSkillSmoke>()
    .init_resource::<CaptureFogProbeSeeded>()
    .insert_resource(capture_screen)
    .insert_resource(capture_scenario)
    .insert_resource(job)
    .add_systems(Startup, ui::load_persistent_progress)
    .add_message::<Damage>()
    .add_message::<Status>()
    .add_message::<BuffTower>()
    .add_message::<HealCarrot>()
    .add_message::<vfx::VfxEvent>()
    .add_message::<audio::SfxEvent>()
    .add_message::<tower::EnemyDied>()
    .add_message::<ui::UiActionActivated>()
    .add_observer(ui::widget_button_activated)
    .add_systems(
        Startup,
        (
            setup_capture_camera,
            setup_capture_window,
            creatures::load_creatures,
            build::load_hero_walks,
        ),
    )
    .add_systems(
        OnEnter(GameState::Playing),
        (
            load_level,
            roguelite::reset_run,
            spawn_hud,
            build::auto_spawn_hero,
        )
            .chain(),
    )
    .add_systems(OnEnter(GameState::HeroCodex), ui::spawn_hero_codex)
    .add_systems(OnEnter(GameState::Briefing), ui::spawn_level_briefing)
    .add_systems(OnEnter(GameState::Armory), ui::spawn_armory)
    .add_systems(
        Update,
        (
            ui::hero_codex_buttons,
            ui::update_hero_codex_info,
            ui::update_hero_select_buttons,
            ui::tooltip_system,
        )
            .run_if(in_state(GameState::HeroCodex)),
    )
    .add_systems(
        Update,
        (
            ui::update_briefing_animation,
            ui::briefing_buttons,
            ui::update_hero_label,
            ui::update_hero_select_buttons,
            ui::tooltip_system,
        )
            .run_if(in_state(GameState::Briefing)),
    )
    .add_systems(PreUpdate, ui::cjk_linebreak)
    .add_systems(
        Update,
        (
            fit_capture_ui_scale,
            ui::cjk_linebreak,
            hero::validate_hero_gear_inventory,
            creatures::animate_creatures,
            vfx::update_camera_shake,
            quality::apply_quality,
            i18n::sync_current_lang,
        ),
    )
    .add_systems(
        Update,
        (
            drive_mouse_input_smoke.before(build::mouse_build),
            build::mouse_build,
            build::hero_control.after(build::mouse_build),
            prepare_capture_level,
            drive_hero_skill_smoke,
            ui::hero_buttons,
            prepare_roguelite_draft_capture.after(prepare_capture_level),
            seed_fog_probe_enemy.after(prepare_capture_level),
            (
                tower::build_snapshot,
                hero::hero_doctrine,
                tower::update_towers,
                tower::update_projectiles,
                tower::update_shot_fx,
                tower::update_summons,
                tower::tick_attack_actions,
                tower::apply_buffs,
                tower::apply_heal,
                tower::apply_status,
                tower::apply_damage,
            )
                .chain(),
            (
                tower::enemy_vs_ally,
                tower::enemy_vs_tower,
                enemy::boss_specials,
                tower::update_fire_grounds,
                enemy::spawn_enemies,
                enemy::update_enemies,
                tower::necromancer_raise,
                enemy::heal_auras,
                enemy::incubation,
                tick_auto_wave,
                tick_message,
            )
                .chain(),
        )
            .chain()
            .run_if(in_state(GameState::Playing).and_then(not_paused)),
    )
    .add_systems(
        Update,
        (
            game::update_carrot_seal,
            game::grow_portal,
            tower::compute_synergy,
            build::animate_hero_walk,
            build::rotate_towers,
            build::update_hero_race_badges,
            build::tint_silenced_towers,
            build::update_tower_upgrade_visuals.after(build::tint_silenced_towers),
            build::update_tower_hp_bars,
            enemy::update_hp_bars,
            tower::update_summon_hp_bars,
            meta::tick_cooldowns,
            vfx::spawn_vfx,
            vfx::update_particles,
            vfx::animate_sword_swing,
            vfx::update_float_text,
            vfx::update_shockwaves,
            vfx::enemy_hit_pop,
            enemy::animate_enemy_sprites,
        )
            .run_if(in_state(GameState::Playing)),
    )
    .add_systems(
        Update,
        (
            update_hud,
            ui::update_unit_stats,
            ui::update_hero_info,
            ui::update_combo_meter,
            ui::update_equipment_button_labels.after(prepare_capture_level),
            ui::update_upgrade_button_label,
            ui::update_equipped_slot_icons,
            ui::update_boss_bar,
            ui::update_ability_buttons,
            ui::update_hero_select_buttons,
            ui::update_hero_paperdoll_panel.after(prepare_capture_level),
            ui::update_roguelite_draft_panel.after(prepare_roguelite_draft_capture),
            press_capture_escape_once.before(ui::close_hud_panels_with_escape),
            ui::close_hud_panels_with_escape,
            ui::update_panel_visibility.after(prepare_capture_level),
            ui::tooltip_system,
        )
            .run_if(in_state(GameState::Playing)),
    )
    .add_systems(
        Update,
        (
            verify_inventory_visibility
                .after(ui::update_equipment_button_labels)
                .after(ui::update_hero_paperdoll_panel)
                .after(ui::update_panel_visibility),
            verify_tower_gems.after(ui::update_equipped_slot_icons),
            verify_hero_paperdoll.after(ui::update_hero_paperdoll_panel),
            verify_roguelite_draft.after(ui::update_roguelite_draft_panel),
            verify_roguelite_pick.after(prepare_roguelite_draft_capture),
            verify_fog_of_war.after(prepare_capture_level),
        )
            .run_if(in_state(GameState::Playing)),
    )
    .add_systems(Update, drive_capture);

    let image =
        Image::new_target_texture(CAPTURE_W, CAPTURE_H, TextureFormat::Rgba8UnormSrgb, None);
    let target = app.world_mut().resource_mut::<Assets<Image>>().add(image);
    app.insert_resource(CaptureTarget(target));

    let bytes = include_bytes!("../../assets/fonts/wqy-microhei.ttc");
    let font = Font::from_bytes(bytes.to_vec());
    let handle = app.world_mut().resource_mut::<Assets<Font>>().add(font);
    app.insert_resource(UiFont(handle));

    let assets = app.world().resource::<AssetServer>().clone();
    app.insert_resource(build_sprites(&assets));

    if capture_scenario == CaptureScenario::InventoryVisibility {
        seed_owned_only_inventory(app.world_mut());
    }

    app.run()
}

fn seed_owned_only_inventory(world: &mut World) {
    {
        let mut inventory = world.resource_mut::<equipment::EquipmentInventory>();
        for item in equipment::Equipment::ALL {
            inventory.set_runtime_count(item, 0);
        }
        inventory.set_runtime_count(equipment::Equipment::PrismShard, 2);
    }

    {
        let mut gear_inventory = world.resource_mut::<hero_gear::HeroGearInventory>();
        for item in hero_gear::HeroGear::ALL {
            gear_inventory.set_runtime_count(item, 0);
        }
        gear_inventory.set_runtime_count(hero_gear::HeroGear::VowPlate, 1);
    }
}

fn parse_args() -> CaptureJob {
    let mut args = env::args().skip(1);
    let Some(mode) = args.next() else {
        return CaptureJob {
            mode: CaptureMode::Screenshot,
            output: PathBuf::from(DEFAULT_STILL),
            total_frames: 1,
            tick: 0,
            scheduled: 0,
            exit_after_tick: None,
        };
    };

    match mode.as_str() {
        "screenshot" => CaptureJob {
            mode: CaptureMode::Screenshot,
            output: PathBuf::from(args.next().unwrap_or_else(|| DEFAULT_STILL.into())),
            total_frames: 1,
            tick: 0,
            scheduled: 0,
            exit_after_tick: None,
        },
        "frames" => {
            let output = PathBuf::from(args.next().unwrap_or_else(|| DEFAULT_FRAMES_DIR.into()));
            let total_frames = args
                .next()
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(DEFAULT_FRAME_COUNT)
                .max(1);
            CaptureJob {
                mode: CaptureMode::Frames,
                output,
                total_frames,
                tick: 0,
                scheduled: 0,
                exit_after_tick: None,
            }
        }
        "-h" | "--help" | "help" => {
            print_usage();
            process::exit(0);
        }
        other => {
            eprintln!("[capture] unknown mode `{other}`");
            print_usage();
            process::exit(2);
        }
    }
}

fn print_usage() {
    eprintln!("Usage:");
    eprintln!("  cargo run --bin capture");
    eprintln!("  cargo run --bin capture -- screenshot screenshots/capture/still.png");
    eprintln!("  cargo run --bin capture -- frames screenshots/capture 120");
}

fn prepare_output_path(job: &CaptureJob) {
    match job.mode {
        CaptureMode::Screenshot => {
            if let Some(parent) = job.output.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
        }
        CaptureMode::Frames => {
            let _ = std::fs::create_dir_all(&job.output);
        }
    }
}

fn setup_capture_camera(mut commands: Commands, target: Res<CaptureTarget>) {
    let mut projection = OrthographicProjection::default_2d();
    projection.scaling_mode = ScalingMode::AutoMin {
        min_width: VIRTUAL_W,
        min_height: VIRTUAL_H,
    };

    commands.spawn((
        Camera2d,
        Camera::default(),
        RenderTarget::Image(target.0.clone().into()),
        IsDefaultUiCamera,
        Msaa::Off,
        Projection::Orthographic(projection),
        Transform::from_xyz(PANEL_W / 2.0, 0.0, 0.0),
        vfx::ShakeCamera {
            base: Vec3::new(PANEL_W / 2.0, 0.0, 0.0),
        },
        lighting::camera_config(0),
        FogOfWarCamera,
    ));
}

fn setup_capture_window(mut commands: Commands) {
    let mut window = Window {
        resolution: WindowResolution::new(CAPTURE_W, CAPTURE_H),
        ..default()
    };
    window.set_cursor_position(None);
    commands.spawn(window);
}

fn capture_level_from_env(scenario: CaptureScenario) -> usize {
    env::var("CARROT_CAPTURE_LEVEL")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .map(|level| level.saturating_sub(1))
        .unwrap_or_else(|| match scenario {
            // Fog starts at level index 1 (the second visible level). Make the
            // fog scenario self-contained so `CARROT_CAPTURE_SCENARIO=fog`
            // cannot accidentally validate the always-clear first level.
            CaptureScenario::Fog => 1,
            _ => 0,
        })
}

fn capture_screen_from_env() -> CaptureScreen {
    match env::var("CARROT_CAPTURE_SCREEN").ok().as_deref() {
        Some("hero_codex") | Some("hero") => CaptureScreen::HeroCodex,
        Some("briefing") | Some("brief") => CaptureScreen::Briefing,
        Some("armory") | Some("equipment") | Some("gems") => CaptureScreen::Armory,
        _ => CaptureScreen::Playing,
    }
}

fn capture_scenario_from_env() -> CaptureScenario {
    match env::var("CARROT_CAPTURE_SCENARIO").ok().as_deref() {
        Some("tower_gems") | Some("tower-gems") | Some("gems") => CaptureScenario::TowerGems,
        Some("hero_paperdoll") | Some("hero-paperdoll") | Some("paperdoll") => {
            CaptureScenario::HeroPaperdoll
        }
        Some("hero_paperdoll_esc") | Some("hero-paperdoll-esc") | Some("paperdoll-esc") => {
            CaptureScenario::HeroPaperdollEscClosed
        }
        Some("items_bag") | Some("items-bag") | Some("items") | Some("bag") => {
            CaptureScenario::TowerGems
        }
        Some("items_bag_esc") | Some("items-bag-esc") | Some("bag-esc") => {
            CaptureScenario::TowerGems
        }
        Some("inventory_visibility")
        | Some("inventory-visibility")
        | Some("owned_only")
        | Some("owned-only") => CaptureScenario::InventoryVisibility,
        Some("fog") | Some("fog_of_war") | Some("fog-of-war") => CaptureScenario::Fog,
        Some("tower_select") | Some("tower-selected") | Some("tower_selected") => {
            CaptureScenario::TowerSelected
        }
        Some("hero_select") | Some("hero-selected") | Some("hero_selected") => {
            CaptureScenario::HeroSelected
        }
        Some("mouse_input")
        | Some("mouse-input")
        | Some("mouse_input_smoke")
        | Some("mouse-input-smoke")
        | Some("input_smoke")
        | Some("input-smoke") => CaptureScenario::MouseInputSmoke,
        Some("hero_skill")
        | Some("hero-skill")
        | Some("hero_skill_smoke")
        | Some("hero-skill-smoke")
        | Some("skill_smoke")
        | Some("skill-smoke") => CaptureScenario::HeroSkillSmoke,
        Some("roguelite") | Some("roguelite_draft") | Some("roguelite-draft") => {
            CaptureScenario::RogueliteDraft
        }
        Some("roguelite_pick") | Some("roguelite-pick") => CaptureScenario::RoguelitePick,
        _ => CaptureScenario::Default,
    }
}

fn fit_capture_ui_scale(mut ui_scale: ResMut<UiScale>) {
    ui_scale.0 = (CAPTURE_W as f32 / VIRTUAL_W).min(CAPTURE_H as f32 / VIRTUAL_H);
}

fn press_capture_escape_once(
    mut escape: ResMut<CaptureEscapeAfterScenario>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
) {
    if escape.pending && !escape.fired {
        keys.press(KeyCode::Escape);
        escape.fired = true;
    }
}

fn drive_mouse_input_smoke(
    scenario: Res<CaptureScenario>,
    board: Option<Res<Board>>,
    mut state: ResMut<CaptureMouseInputSmoke>,
    mut prepared: ResMut<CapturePrepared>,
    mut windows: Query<&mut Window>,
    camera: Query<(&Camera, &GlobalTransform), (With<Camera2d>, With<vfx::ShakeCamera>)>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut selection: ResMut<Selection>,
    mut panels: ResMut<ui::HudPanels>,
    mut run: ResMut<RunState>,
    towers: Query<(Entity, &tower::Tower)>,
) {
    if *scenario != CaptureScenario::MouseInputSmoke
        || !prepared.level_ready
        || prepared.scenario_ready
    {
        return;
    }

    let Some(board) = board else {
        return;
    };
    let Ok(mut window) = windows.single_mut() else {
        panic!("[capture/input] expected exactly one virtual Window");
    };
    let Ok((camera, camera_tf)) = camera.single() else {
        panic!("[capture/input] expected exactly one capture camera");
    };
    let to_screen = |world: Vec2| {
        camera
            .world_to_viewport(camera_tf, world.extend(0.0))
            .unwrap_or_else(|err| panic!("[capture/input] world_to_viewport failed: {err:?}"))
    };

    match state.step {
        0 => {
            let hero_pos = towers
                .iter()
                .find_map(|(_, tower)| tower.hero.then_some(tower.hero_pos));
            let mut cells: Vec<(i32, i32)> = board.buildable.iter().copied().collect();
            cells.sort_by_key(|(col, row)| ((col - 10).abs() + (row - 8).abs(), *row, *col));
            let Some((col, row)) = cells.into_iter().find(|(col, row)| {
                let far_from_hero = hero_pos
                    .map(|pos| {
                        cell_center(*col as f32, *row as f32).distance(pos) > TILE_SIZE * 2.0
                    })
                    .unwrap_or(true);
                far_from_hero
                    && build::footprint_buildable(
                        &board,
                        towers.iter().map(|(_, tower)| tower),
                        TowerKind::Arrow,
                        *col,
                        *row,
                    )
            }) else {
                panic!("[capture/input] no free buildable cell for mouse build smoke");
            };

            state.build_cell = Some((col, row));
            state.tower_count_before = towers.iter().filter(|(_, tower)| !tower.hero).count();
            selection.build_kind = Some(TowerKind::Arrow);
            selection.selected = None;
            selection.preview_cell = None;
            panels.hero_open = false;
            panels.settings_open = false;
            window.set_cursor_position(Some(to_screen(cell_center(col as f32, row as f32))));
            mouse.press(MouseButton::Left);
            state.step = 1;
            println!("[capture/input] mouse down build at cell ({col},{row})");
        }
        1 => {
            mouse.release(MouseButton::Left);
            let tower_count_after = towers.iter().filter(|(_, tower)| !tower.hero).count();
            if tower_count_after <= state.tower_count_before {
                panic!(
                    "[capture/input] mouse_build failed: tower count stayed {} after clicking {:?}",
                    state.tower_count_before, state.build_cell
                );
            }
            selection.build_kind = None;
            selection.selected = None;
            selection.preview_cell = None;
            state.step = 2;
            println!(
                "[capture/input] mouse build succeeded: {} -> {} towers",
                state.tower_count_before, tower_count_after
            );
        }
        2 => {
            let Some((hero_entity, hero_pos)) = towers
                .iter()
                .find_map(|(entity, tower)| tower.hero.then_some((entity, tower.hero_pos)))
            else {
                panic!("[capture/input] no hero entity for mouse select smoke");
            };
            state.hero_entity = Some(hero_entity);
            window.set_cursor_position(Some(to_screen(hero_pos)));
            mouse.press(MouseButton::Left);
            state.step = 3;
            println!("[capture/input] mouse down hero at {hero_pos:?}");
        }
        3 => {
            mouse.release(MouseButton::Left);
            let Some(hero_entity) = state.hero_entity else {
                panic!("[capture/input] missing stored hero entity");
            };
            if selection.selected != Some(hero_entity) || panels.hero_open {
                panic!(
                    "[capture/input] hero select failed: selected={:?}, expected={hero_entity:?}, hero_open={} (battlefield click must not open paperdoll)",
                    selection.selected, panels.hero_open
                );
            }
            state.step = 4;
            println!("[capture/input] hero mouse select succeeded without opening paperdoll");
        }
        4 => {
            let Some(hero_entity) = state.hero_entity else {
                panic!("[capture/input] missing stored hero entity before right-click move");
            };
            let Ok((_, hero)) = towers.get(hero_entity) else {
                panic!("[capture/input] stored hero entity vanished before right-click move");
            };
            let target = hero.hero_pos + Vec2::new(TILE_SIZE * 2.0, 0.0);
            state.move_target = Some(target);
            window.set_cursor_position(Some(to_screen(target)));
            mouse.press(MouseButton::Right);
            state.step = 5;
            println!("[capture/input] mouse right-down move target {target:?}");
        }
        5 => {
            mouse.release(MouseButton::Right);
            let Some(hero_entity) = state.hero_entity else {
                panic!("[capture/input] missing stored hero entity after right-click move");
            };
            let Some(target) = state.move_target else {
                panic!("[capture/input] missing stored move target");
            };
            let Ok((_, hero)) = towers.get(hero_entity) else {
                panic!("[capture/input] stored hero entity vanished after right-click move");
            };
            let Some(actual) = hero.move_target else {
                panic!("[capture/input] right-click move failed: hero.move_target is None");
            };
            if actual.distance(target) > 0.5 {
                panic!(
                    "[capture/input] right-click move failed: target={actual:?}, expected={target:?}"
                );
            }
            run.show(crate::i18n::t("capture: 鼠标建塔、英雄选择与右键移动通过"));
            prepared.scenario_ready = true;
            state.step = 6;
            println!("[capture/input] hero right-click move succeeded");
        }
        _ => {}
    }
}

fn drive_hero_skill_smoke(
    scenario: Res<CaptureScenario>,
    mut state: ResMut<CaptureHeroSkillSmoke>,
    mut prepared: ResMut<CapturePrepared>,
    mut loadout: ResMut<hero::HeroLoadout>,
    mut run: ResMut<RunState>,
    mut selection: ResMut<Selection>,
    mut panels: ResMut<ui::HudPanels>,
    mut actions: MessageWriter<ui::UiActionActivated>,
    mut towers: Query<(Entity, &mut tower::Tower)>,
    summons: Query<
        (
            Entity,
            &Transform,
            Option<&tower::MythicSummonSprite>,
            Option<&tower::TemporaryGuard>,
            Option<&tower::FixedSummonHome>,
        ),
        With<tower::Summon>,
    >,
) {
    if *scenario != CaptureScenario::HeroSkillSmoke
        || !prepared.level_ready
        || prepared.scenario_ready
    {
        return;
    }

    let Some((hero_entity, _)) = towers
        .iter_mut()
        .find(|(_, tower)| tower.hero)
        .map(|(entity, tower)| (entity, tower.hero_pos))
    else {
        return;
    };

    match state.step {
        0 => {
            loadout.race = hero::Race::Elf;
            loadout.weapon = hero::HeroWeapon::SummonStaff;
            loadout.level = hero::HeroLoadout::MAX_LEVEL;
            loadout.skill_cd = 0;
            let weapon_index = loadout.weapon_index();
            loadout.weapon_talents[weapon_index] = [3, 0, 3, 3, 3, 3];
            for (_, mut tower) in &mut towers {
                if tower.hero {
                    hero::apply_loadout_to_tower(&loadout, &mut tower);
                    tower.hp = tower.max_hp;
                    tower.cooldown_timer = 0.0;
                }
            }
            selection.selected = Some(hero_entity);
            selection.build_kind = None;
            panels.dock_open = false;
            panels.hero_open = false;
            panels.settings_open = false;
            actions.write(ui::UiActionActivated {
                entity: hero_entity,
                action: ui::UiAction::HeroSkill,
            });
            state.step = 1;
            println!("[capture/skill] fired SummonStaff hero skill");
        }
        1 => {
            let mythic_count = summons
                .iter()
                .filter(|(_, _, mythic, _, _)| mythic.is_some())
                .count();
            if mythic_count == 0 {
                panic!("[capture/skill] SummonStaff skill spawned no MythicSummonSprite allies");
            }
            state.mythic_count = mythic_count;
            state.guard_count = summons
                .iter()
                .filter(|(_, _, _, guard, _)| guard.is_some())
                .count();

            loadout.weapon = hero::HeroWeapon::ForgeHammer;
            loadout.level = hero::HeroLoadout::MAX_LEVEL;
            loadout.skill_cd = 0;
            let weapon_index = loadout.weapon_index();
            loadout.weapon_talents[weapon_index] = [3, 3, 3, 3, 3, 3];
            for (_, mut tower) in &mut towers {
                if tower.hero {
                    hero::apply_loadout_to_tower(&loadout, &mut tower);
                    tower.hp = tower.max_hp;
                    tower.cooldown_timer = 0.0;
                }
            }
            actions.write(ui::UiActionActivated {
                entity: hero_entity,
                action: ui::UiAction::HeroSkill,
            });
            state.step = 2;
            println!(
                "[capture/skill] fired ForgeHammer hero skill after {mythic_count} mythic allies"
            );
        }
        2 => {
            let mythic_count = summons
                .iter()
                .filter(|(_, _, mythic, _, _)| mythic.is_some())
                .count();
            let guard_count = summons
                .iter()
                .filter(|(_, _, _, guard, _)| guard.is_some())
                .count();
            if mythic_count < state.mythic_count {
                panic!(
                    "[capture/skill] mythic ally count regressed: {} -> {}",
                    state.mythic_count, mythic_count
                );
            }
            if guard_count <= state.guard_count {
                panic!(
                    "[capture/skill] ForgeHammer skill spawned no temporary guards: {} -> {}",
                    state.guard_count, guard_count
                );
            }
            state.guard_homes = summons
                .iter()
                .filter_map(|(_, _, _, guard, home)| guard.and(home).map(|home| home.pos))
                .collect();
            if state.guard_homes.len() != guard_count {
                panic!(
                    "[capture/skill] temporary guards missing fixed homes: homes={} guards={guard_count}",
                    state.guard_homes.len()
                );
            }
            let moved = Vec2::new(BOARD_W * 0.34, -BOARD_H * 0.34);
            for (_, mut tower) in &mut towers {
                if tower.hero {
                    tower.hero_pos = moved;
                    tower.move_target = None;
                    tower.angle = 0.0;
                    break;
                }
            }
            state.moved_hero_pos = Some(moved);
            state.step = 3;
            println!("[capture/skill] moved hero away from {guard_count} fixed temporary guards");
        }
        3 => {
            let Some(hero_pos) = state.moved_hero_pos else {
                panic!("[capture/skill] missing moved hero position");
            };
            let guards = summons
                .iter()
                .filter_map(|(_, tf, _, guard, home)| {
                    guard
                        .and(home)
                        .map(|home| (tf.translation.truncate(), home.pos))
                })
                .collect::<Vec<_>>();
            if guards.len() != state.guard_homes.len() {
                panic!(
                    "[capture/skill] guard count changed after hero relocation: {} -> {}",
                    state.guard_homes.len(),
                    guards.len()
                );
            }
            for (pos, home) in guards.iter().copied() {
                let anchored = state
                    .guard_homes
                    .iter()
                    .any(|expected| home.distance(*expected) <= 0.5);
                if !anchored {
                    panic!(
                        "[capture/skill] guard home moved or was recreated after hero moved: {home:?}, known homes {:?}",
                        state.guard_homes
                    );
                }
                if home.distance(hero_pos) < TILE_SIZE * 2.0 {
                    panic!(
                        "[capture/skill] guard home is still tied to moved hero: home={home:?} hero={hero_pos:?}"
                    );
                }
                if pos.distance(hero_pos) < TILE_SIZE * 1.2 {
                    panic!(
                        "[capture/skill] guard body followed moved hero: guard={pos:?} hero={hero_pos:?}"
                    );
                }
            }
            run.show(crate::i18n::tf(
                "capture: 英雄技能召唤通过，神话{} 临时守卫{}",
                &[&state.mythic_count.to_string(), &guards.len().to_string()],
            ));
            prepared.scenario_ready = true;
            state.step = 4;
            println!(
                "[capture/skill] verified {} mythic allies and {} anchored temporary guards after hero moved",
                state.mythic_count,
                guards.len()
            );
        }
        _ => {}
    }
}

fn verify_inventory_visibility(
    scenario: Res<CaptureScenario>,
    prepared: Res<CapturePrepared>,
    inventory: Res<equipment::EquipmentInventory>,
    gear_inventory: Res<hero_gear::HeroGearInventory>,
    equipment_tiles: Query<(&ui::EquipmentBagTile, &Node)>,
    hero_tiles: Query<(&ui::HeroGearBagTile, &Node)>,
    mut done: Local<bool>,
) {
    if *done || *scenario != CaptureScenario::InventoryVisibility || !prepared.scenario_ready {
        return;
    }

    let mut visible_equipment = 0;
    let mut hidden_equipment = 0;
    for (tile, node) in &equipment_tiles {
        let should_show = inventory.owns(tile.item);
        let visible = node.display != Display::None;
        if should_show != visible {
            panic!(
                "[capture/inventory] equipment visibility mismatch for {:?}: count={}, visible={visible}",
                tile.item,
                inventory.count(tile.item)
            );
        }
        if visible {
            visible_equipment += 1;
        } else {
            hidden_equipment += 1;
        }
    }

    let mut visible_hero = 0;
    let mut hidden_hero = 0;
    for (tile, node) in &hero_tiles {
        let should_show = gear_inventory.owns(tile.item);
        let visible = node.display != Display::None;
        if should_show != visible {
            panic!(
                "[capture/inventory] hero gear visibility mismatch for {:?}: count={}, visible={visible}",
                tile.item,
                gear_inventory.count(tile.item)
            );
        }
        if visible {
            visible_hero += 1;
        } else {
            hidden_hero += 1;
        }
    }

    if visible_equipment == 0 || hidden_equipment == 0 || visible_hero == 0 || hidden_hero == 0 {
        panic!(
            "[capture/inventory] weak visibility coverage: equipment visible/hidden={visible_equipment}/{hidden_equipment}, hero visible/hidden={visible_hero}/{hidden_hero}"
        );
    }

    *done = true;
    println!(
        "[capture/inventory] owned-only visibility passed: equipment visible/hidden={visible_equipment}/{hidden_equipment}, hero visible/hidden={visible_hero}/{hidden_hero}"
    );
}

fn verify_tower_gems(
    scenario: Res<CaptureScenario>,
    prepared: Res<CapturePrepared>,
    selection: Res<Selection>,
    towers: Query<&tower::Tower>,
    mut done: Local<bool>,
) {
    if *done || *scenario != CaptureScenario::TowerGems || !prepared.scenario_ready {
        return;
    }

    let Some(selected) = selection.selected else {
        panic!("[capture/tower_gems] no selected tower");
    };
    let tower = towers
        .get(selected)
        .unwrap_or_else(|_| panic!("[capture/tower_gems] selected tower entity vanished"));
    if tower.hero {
        panic!("[capture/tower_gems] selected entity is hero, expected defensive tower");
    }
    let expected = [
        equipment::Equipment::PrismShard,
        equipment::Equipment::VoidCapacitor,
        equipment::Equipment::AzathothEye,
    ];
    for item in expected {
        if !tower.equipment.contains(&Some(item)) {
            panic!(
                "[capture/tower_gems] selected tower missing socketed gem {:?}: {:?}",
                item, tower.equipment
            );
        }
    }
    let resonance = equipment::equipment_set_bonus(&tower.equipment);
    if resonance.resonance_element != Some(data::Element::Arcane)
        || resonance.resonance_count != 3
        || resonance.range_mult <= 1.10
    {
        panic!(
            "[capture/tower_gems] expected full arcane resonance, got {:?}",
            resonance
        );
    }

    *done = true;
    println!(
        "[capture/tower_gems] selected {:?} tower has full arcane resonance, range x{:.2}",
        tower.kind, resonance.range_mult
    );
}

fn verify_hero_paperdoll(
    scenario: Res<CaptureScenario>,
    prepared: Res<CapturePrepared>,
    loadout: Res<hero::HeroLoadout>,
    runtime: Res<hero_paperdoll::HeroPaperdollRuntime>,
    heroes: Query<(&tower::Tower, &Sprite), With<hero_paperdoll::HeroPaperdollSprite>>,
    mut ticks: Local<u32>,
    mut done: Local<bool>,
) {
    if *done
        || !matches!(
            *scenario,
            CaptureScenario::HeroPaperdoll | CaptureScenario::HeroPaperdollEscClosed
        )
        || !prepared.scenario_ready
    {
        return;
    }

    *ticks += 1;
    let equipped_gear = loadout.gear.iter().flatten().count();
    if loadout.race != hero::Race::Elf || equipped_gear < 4 {
        panic!(
            "[capture/paperdoll] unexpected loadout: race={}, gear_count={equipped_gear}",
            loadout.race.name()
        );
    }

    let Some(image) = runtime.image() else {
        if *ticks < 150 {
            return;
        }
        panic!(
            "[capture/paperdoll] runtime image was not composed after {} ticks",
            *ticks
        );
    };
    let applied = heroes.iter().any(|(tower, sprite)| {
        tower.hero
            && sprite.image == image
            && sprite
                .custom_size
                .map(|size| {
                    (size.x - hero_paperdoll::HERO_PAPERDOLL_WORLD_SIZE).abs() < 0.5
                        && (size.y - hero_paperdoll::HERO_PAPERDOLL_WORLD_SIZE).abs() < 0.5
                })
                .unwrap_or(false)
    });
    if !applied {
        if *ticks < 150 {
            return;
        }
        panic!("[capture/paperdoll] composed paperdoll image was not applied to battlefield hero");
    }

    *done = true;
    println!(
        "[capture/paperdoll] {} hero composed and applied with {equipped_gear} gear pieces",
        loadout.race.name()
    );
}

fn verify_fog_of_war(
    scenario: Res<CaptureScenario>,
    mut prepared: ResMut<CapturePrepared>,
    current: Res<CurrentLevel>,
    run: Res<RunState>,
    fog: Res<FogMapSettings>,
    enemies: Query<Option<&FogHidden>, With<Enemy>>,
    mut ticks: Local<u32>,
    mut done: Local<bool>,
) {
    if *done
        || *scenario != CaptureScenario::Fog
        || !prepared.level_ready
        || prepared.scenario_ready
    {
        return;
    }

    *ticks += 1;
    // Let the level spawn enemies and let the lighting plugin run its fog pass
    // before treating the absence of hidden enemies as a failure.
    if *ticks < 30 {
        return;
    }

    if current.0 < 1 {
        panic!(
            "[capture/fog] expected fog scenario to run from level 2+, got level {}",
            current.0 + 1
        );
    }
    if !fog.enabled {
        panic!(
            "[capture/fog] FogMapSettings.enabled is false on level {}",
            current.0 + 1
        );
    }

    let total = enemies.iter().count();
    let hidden = enemies.iter().filter(|hidden| hidden.is_some()).count();
    if total == 0 {
        if *ticks < 180 {
            return;
        }
        panic!(
            "[capture/fog] no enemy coverage after {} ticks: wave={}, in_progress={}, spawned={}/{}, spawn_timer={:.2}, spawn_interval={:.2}",
            *ticks,
            run.wave,
            run.wave_in_progress,
            run.spawned,
            run.spawn_target,
            run.spawn_timer,
            run.spawn_interval
        );
    }
    if hidden == 0 {
        if *ticks < 240 || total < 2 {
            return;
        }
        panic!(
            "[capture/fog] weak coverage after {} ticks: total enemies={total}, hidden by fog={hidden}",
            *ticks
        );
    }

    *done = true;
    prepared.scenario_ready = true;
    println!(
        "[capture/fog] fog-of-war passed on level {}: hidden/total={hidden}/{total}",
        current.0 + 1
    );
}

fn prepare_roguelite_draft_capture(
    scenario: Res<CaptureScenario>,
    mut prepared: ResMut<CapturePrepared>,
    mut roguelite: ResMut<roguelite::RogueliteRun>,
    mut run: ResMut<RunState>,
    loadout: Res<hero::HeroLoadout>,
    mut rng: ResMut<Rng>,
    mut panels: ResMut<ui::HudPanels>,
) {
    if !matches!(
        *scenario,
        CaptureScenario::RogueliteDraft | CaptureScenario::RoguelitePick
    ) || !prepared.level_ready
        || prepared.scenario_ready
    {
        return;
    }
    run.wave = run.wave.max(1);
    run.wave_in_progress = false;
    run.auto_wave = true;
    run.auto_wave_timer = 0.0;
    panels.dock_open = false;
    panels.hero_open = false;
    panels.settings_open = false;
    if roguelite.offer_wave_draft(&loadout, run.wave, &mut rng) {
        prepared.scenario_ready = true;
        println!("[capture/roguelite] draft opened after wave {}", run.wave);
    }
}

fn verify_roguelite_draft(
    scenario: Res<CaptureScenario>,
    prepared: Res<CapturePrepared>,
    roguelite: Res<roguelite::RogueliteRun>,
    roots: Query<&Node, With<ui::RogueliteDraftRoot>>,
    titles: Query<(&ui::RogueliteChoiceTitle, &Text)>,
    mut done: Local<bool>,
) {
    if *done || *scenario != CaptureScenario::RogueliteDraft || !prepared.scenario_ready {
        return;
    }
    let Some(draft) = roguelite.draft.as_ref() else {
        eprintln!("[capture/roguelite] expected an active draft");
        process::exit(1);
    };
    let pools = draft
        .choices
        .iter()
        .map(|choice| choice.pool())
        .collect::<HashSet<_>>();
    for required in [
        roguelite::TalentPool::Race,
        roguelite::TalentPool::Weapon,
        roguelite::TalentPool::Common,
    ] {
        if !pools.contains(&required) {
            eprintln!("[capture/roguelite] missing required choice pool");
            process::exit(1);
        }
    }
    if !roots.iter().any(|node| node.display == Display::Flex) {
        eprintln!("[capture/roguelite] draft panel root is not visible");
        process::exit(1);
    }
    let visible_titles = titles
        .iter()
        .filter(|(_, text)| !text.0.trim().is_empty())
        .count();
    if visible_titles < 3 {
        eprintln!("[capture/roguelite] expected 3 rendered choice titles, got {visible_titles}");
        process::exit(1);
    }
    println!(
        "[capture/roguelite] draft verified: wave={} choices={}",
        draft.wave, visible_titles
    );
    *done = true;
}

fn verify_roguelite_pick(
    scenario: Res<CaptureScenario>,
    prepared: Res<CapturePrepared>,
    mut roguelite: ResMut<roguelite::RogueliteRun>,
    mut loadout: ResMut<hero::HeroLoadout>,
    mut talents: ResMut<meta::Talents>,
    mut run: ResMut<RunState>,
    mut towers: Query<(Entity, &mut tower::Tower)>,
    mut done: Local<bool>,
) {
    if *done || *scenario != CaptureScenario::RoguelitePick || !prepared.scenario_ready {
        return;
    }
    let before_damage = loadout.run_mods.damage_mult;
    let before_range = loadout.run_mods.range_mult;
    let before_cooldown = loadout.run_mods.cooldown_mult;
    let before_hp = loadout.run_mods.hp_mult;
    let before_move = loadout.run_mods.move_mult;
    let before_gold = run.gold;
    let before_tower_damage = talents.rogue_damage_mult;
    let before_tower_range = talents.rogue_range_mult;
    let before_tower_cooldown = talents.rogue_firerate_mult;
    let picked = roguelite.pick(0, &mut loadout, &mut talents, &mut run, &mut towers);
    let Some(picked) = picked else {
        eprintln!("[capture/roguelite] expected pick to succeed");
        process::exit(1);
    };
    if roguelite.draft.is_some() || roguelite.picked.len() != 1 {
        eprintln!("[capture/roguelite] pick did not close draft or record selection");
        process::exit(1);
    }
    let changed = (loadout.run_mods.damage_mult - before_damage).abs() > 0.001
        || (loadout.run_mods.range_mult - before_range).abs() > 0.001
        || (loadout.run_mods.cooldown_mult - before_cooldown).abs() > 0.001
        || (loadout.run_mods.hp_mult - before_hp).abs() > 0.001
        || (loadout.run_mods.move_mult - before_move).abs() > 0.001
        || run.gold != before_gold
        || (talents.rogue_damage_mult - before_tower_damage).abs() > 0.001
        || (talents.rogue_range_mult - before_tower_range).abs() > 0.001
        || (talents.rogue_firerate_mult - before_tower_cooldown).abs() > 0.001;
    if !changed {
        eprintln!("[capture/roguelite] pick produced no gameplay modifier");
        process::exit(1);
    }
    println!(
        "[capture/roguelite] pick verified: {}",
        picked.name(&loadout)
    );
    *done = true;
}

fn seed_fog_probe_enemy(
    mut commands: Commands,
    scenario: Res<CaptureScenario>,
    prepared: Res<CapturePrepared>,
    mut seeded: ResMut<CaptureFogProbeSeeded>,
    sources: Query<(&GlobalTransform, &VisionSource)>,
) {
    if *scenario != CaptureScenario::Fog || !prepared.level_ready || seeded.0 {
        return;
    }

    let candidates = [
        Vec2::new(-BOARD_W * 0.43, -BOARD_H * 0.43),
        Vec2::new(BOARD_W * 0.33, -BOARD_H * 0.42),
        Vec2::new(-BOARD_W * 0.12, BOARD_H * 0.40),
        Vec2::new(BOARD_W * 0.42, BOARD_H * 0.34),
        Vec2::new(0.0, -BOARD_H * 0.47),
    ];
    let pos = candidates
        .into_iter()
        .max_by(|a, b| {
            let score = |p: Vec2| {
                sources
                    .iter()
                    .map(|(tf, source)| p.distance(tf.translation().truncate()) - source.range)
                    .fold(f32::INFINITY, f32::min)
            };
            score(*a).total_cmp(&score(*b))
        })
        .unwrap_or(Vec2::new(0.0, -BOARD_H * 0.47));

    commands.spawn((
        capture_probe_enemy(),
        Sprite {
            color: Color::srgb(0.7, 0.9, 1.0),
            custom_size: Some(Vec2::splat(TILE_SIZE * 0.8)),
            ..default()
        },
        Transform::from_translation(pos.extend(5.0)),
        Visibility::Visible,
        protect_carrot::components::LevelEntity,
    ));
    seeded.0 = true;
    println!("[capture/fog] seeded dark probe enemy at {pos:?}");
}

fn capture_probe_enemy() -> Enemy {
    Enemy {
        kind: data::EnemyKind::Normal,
        species_id: 0,
        hp: 10_000.0,
        max_hp: 10_000.0,
        base_speed: 0.0,
        reward: 0,
        path_index: 0,
        armor: 0.0,
        magic_resist: 0.0,
        element_resist: data::ElementProfile::none(),
        flying: false,
        invisible: false,
        skill_mult: 1.0,
        stealth: 1.0,
        regen: 0.0,
        boss: false,
        size: 10.0,
        slow_timer: 0.0,
        stun_timer: 9_999.0,
        frozen: true,
        poison_timer: 0.0,
        poison_damage: 0.0,
        fire_timer: 0.0,
        fire_damage: 0.0,
        fire_element: data::Element::Fire,
        poison_source_tower: None,
        fire_source_tower: None,
        curse_timer: 0.0,
        armor_reduce: 0.0,
        shield: 0.0,
        max_shield: 0.0,
        splits: 0,
        heal_aura: 0.0,
        charger: false,
        charge_timer: 0.0,
        hit_flash: 0.0,
        last_hit_tower: None,
        blocked: true,
        melee: 0.0,
        elite: false,
        elite_affix: protect_carrot::monster::EliteAffix::None,
        boss_skill_timer: 0.0,
        enraged: false,
        phase_timer: 0.0,
        tower_raider: false,
        tower_dps: 0.0,
        silence_aura: 0.0,
        ranged_tower: false,
        ranged_range: 0.0,
        ranged_damage: 0.0,
        ranged_cooldown: 1.0,
        ranged_timer: 0.0,
        explosive: false,
        explode_damage: 0.0,
        explode_radius: 0.0,
        explode_sense: 0.0,
        explode_trigger: 0.0,
        moss_destroy: false,
        moss_destroyed: false,
        incubate: false,
        incubate_timer: 0.0,
        incubate_stacks: 0,
        facing: Vec2::X,
    }
}

fn prepare_capture_level(
    mut commands: Commands,
    board: Option<Res<Board>>,
    sprites: Res<sprites::Sprites>,
    talents: Res<meta::Talents>,
    current: Res<CurrentLevel>,
    scenario: Res<CaptureScenario>,
    mut rng: ResMut<Rng>,
    mut run: ResMut<RunState>,
    mut selection: ResMut<Selection>,
    mut panels: ResMut<ui::HudPanels>,
    mut inventory: ResMut<equipment::EquipmentInventory>,
    mut gear_inventory: ResMut<hero_gear::HeroGearInventory>,
    mut loadout: ResMut<hero::HeroLoadout>,
    mut escape: ResMut<CaptureEscapeAfterScenario>,
    mut towers: Query<(Entity, &mut tower::Tower, &mut Transform)>,
    mut prepared: ResMut<CapturePrepared>,
) {
    if !prepared.level_ready {
        let Some(board) = board else {
            return;
        };

        run.gold = 9_999;
        run.auto_wave = true;
        run.game_speed = 1.0;

        let mut occupied = HashSet::new();
        let mut cells: Vec<(i32, i32)> = board.buildable.iter().copied().collect();
        cells.sort_by_key(|cell| {
            let dist = board
                .path_cells
                .iter()
                .map(|p| (p.0 - cell.0).abs() + (p.1 - cell.1).abs())
                .min()
                .unwrap_or(99);
            (dist, cell.1, cell.0)
        });

        let kinds = [
            TowerKind::Arrow,
            TowerKind::Cannon,
            TowerKind::Magic,
            TowerKind::Ice,
            TowerKind::Thunder,
            TowerKind::Poison,
            TowerKind::Fire,
            TowerKind::Detection,
        ];

        let mut placed = 0usize;
        for kind in kinds {
            let fp = kind.def().footprint.max(1);
            let Some((col, row)) = cells.iter().copied().find(|(col, row)| {
                (0..fp).all(|dx| {
                    (0..fp).all(|dy| {
                        board.buildable.contains(&(*col + dx, *row + dy))
                            && !occupied.contains(&(*col + dx, *row + dy))
                    })
                })
            }) else {
                continue;
            };

            for dx in 0..fp {
                for dy in 0..fp {
                    occupied.insert((col + dx, row + dy));
                }
            }
            build::spawn_tower(&mut commands, kind, col, row, &sprites, &talents);
            placed += 1;
        }

        game::start_wave(&mut run, current.0, &mut rng);
        println!(
            "[capture] prepared level {} with {placed} towers",
            current.0 + 1
        );
        prepared.level_ready = true;
        return;
    }

    if prepared.scenario_ready {
        return;
    }

    if apply_capture_scenario(
        *scenario,
        &mut selection,
        &mut panels,
        &mut run,
        &mut inventory,
        &mut gear_inventory,
        &mut loadout,
        &mut escape,
        &mut towers,
    ) {
        prepared.scenario_ready = true;
        println!("[capture] scenario prepared");
    }
}

fn apply_capture_scenario(
    scenario: CaptureScenario,
    selection: &mut Selection,
    panels: &mut ui::HudPanels,
    run: &mut RunState,
    inventory: &mut equipment::EquipmentInventory,
    gear_inventory: &mut hero_gear::HeroGearInventory,
    loadout: &mut hero::HeroLoadout,
    escape: &mut CaptureEscapeAfterScenario,
    towers: &mut Query<(Entity, &mut tower::Tower, &mut Transform)>,
) -> bool {
    match scenario {
        CaptureScenario::Default => true,
        CaptureScenario::MouseInputSmoke => false,
        CaptureScenario::HeroSkillSmoke => false,
        CaptureScenario::RogueliteDraft => false,
        CaptureScenario::RoguelitePick => false,
        CaptureScenario::Fog => {
            panels.dock_open = false;
            panels.hero_open = false;
            panels.settings_open = false;
            false
        }
        CaptureScenario::TowerSelected => {
            let selected = towers
                .iter_mut()
                .find_map(|(entity, tower, _)| (!tower.hero).then_some(entity));
            let Some(selected) = selected else {
                return false;
            };

            selection.build_kind = None;
            selection.selected = Some(selected);
            selection.preview_cell = None;
            panels.dock_open = true;
            panels.hero_open = false;
            panels.settings_open = false;
            run.show(crate::i18n::t("capture: 防御塔已选中"));
            true
        }
        CaptureScenario::HeroSelected => {
            let selected = towers
                .iter_mut()
                .find_map(|(entity, tower, _)| tower.hero.then_some(entity));
            let Some(selected) = selected else {
                return false;
            };

            selection.build_kind = None;
            selection.selected = Some(selected);
            selection.preview_cell = None;
            panels.dock_open = false;
            panels.hero_open = true;
            panels.settings_open = false;
            run.show(crate::i18n::t("capture: 英雄已选中"));
            true
        }
        CaptureScenario::TowerGems => {
            let mut fallback = None;
            let mut preferred = None;
            for (entity, tower, _) in towers.iter_mut() {
                if tower.hero {
                    continue;
                }
                fallback.get_or_insert(entity);
                if matches!(tower.kind, TowerKind::Thunder | TowerKind::Magic) {
                    preferred = Some(entity);
                    break;
                }
            }

            let Some(selected) = preferred.or(fallback) else {
                return false;
            };

            let gems = [
                equipment::Equipment::PrismShard,
                equipment::Equipment::VoidCapacitor,
                equipment::Equipment::AzathothEye,
            ];
            for item in gems {
                inventory.counts[item.idx()] = inventory.counts[item.idx()].max(1);
            }
            if let Ok((_, mut tower, _)) = towers.get_mut(selected) {
                tower.equipment = [None; 3];
                for item in gems {
                    let _ = equipment::equip_into(&mut *tower, item);
                }
            }

            selection.build_kind = None;
            selection.selected = Some(selected);
            panels.dock_open = true;
            panels.hero_open = false;
            panels.settings_open = false;
            true
        }
        CaptureScenario::InventoryVisibility => {
            for item in equipment::Equipment::ALL {
                inventory.set_runtime_count(item, 0);
            }
            inventory.set_runtime_count(equipment::Equipment::PrismShard, 2);

            for item in hero_gear::HeroGear::ALL {
                gear_inventory.set_runtime_count(item, 0);
            }
            gear_inventory.set_runtime_count(hero_gear::HeroGear::VowPlate, 1);

            selection.build_kind = None;
            selection.selected = None;
            panels.dock_open = false;
            panels.hero_open = true;
            panels.settings_open = false;
            run.show(crate::i18n::t("capture: 只显示已拥有背包内容"));
            true
        }
        CaptureScenario::HeroPaperdoll | CaptureScenario::HeroPaperdollEscClosed => {
            let gear = [
                hero_gear::HeroGear::StarweaveRobe,
                hero_gear::HeroGear::SaintBell,
                hero_gear::HeroGear::CarrotHalo,
                hero_gear::HeroGear::CarrotWings,
            ];
            for item in gear {
                gear_inventory.ensure_runtime_owned(item);
            }
            loadout.race = hero::Race::Elf;
            loadout.weapon = hero::HeroWeapon::SummonStaff;
            loadout.level = 26;
            loadout.talent_points = 13;
            loadout.gear = [None; hero_gear::HeroGearSlot::COUNT];
            for item in gear {
                loadout.gear[item.def().slot.idx()] = Some(item);
            }

            for (entity, mut tower, _) in towers.iter_mut() {
                if !tower.hero {
                    continue;
                }
                tower.hero_pos = Vec2::new(0.0, -60.0);
                tower.move_target = None;
                tower.angle = 0.0;
                hero::apply_loadout_to_tower(loadout, &mut *tower);
                tower.hp = tower.max_hp;
                selection.build_kind = None;
                selection.selected = Some(entity);
                panels.dock_open = false;
                panels.hero_open = true;
                panels.settings_open = false;
                if scenario == CaptureScenario::HeroPaperdollEscClosed {
                    escape.pending = true;
                    escape.fired = false;
                }
                return true;
            }
            false
        }
    }
}

fn drive_capture(
    mut commands: Commands,
    screen: Res<CaptureScreen>,
    prepared: Res<CapturePrepared>,
    target: Res<CaptureTarget>,
    mut job: ResMut<CaptureJob>,
    capturing: Query<Entity, With<Capturing>>,
    mut exit: MessageWriter<AppExit>,
) {
    if matches!(*screen, CaptureScreen::Playing) && !prepared.scenario_ready {
        return;
    }

    job.tick += 1;
    if job.tick < SETTLE_FRAMES {
        return;
    }

    match job.mode {
        CaptureMode::Screenshot => {
            if job.scheduled == 0 {
                commands
                    .spawn(Screenshot::image(target.0.clone()))
                    .observe(save_to_disk(job.output.clone()));
                println!("[capture] scheduled {}", job.output.display());
                job.scheduled = 1;
                job.exit_after_tick = Some(job.tick + DRAIN_FRAMES);
            }
        }
        CaptureMode::Frames => {
            if job.scheduled < job.total_frames {
                let path = job.output.join(format!("frame{:05}.png", job.scheduled));
                commands
                    .spawn(Screenshot::image(target.0.clone()))
                    .observe(save_to_disk(path));
                job.scheduled += 1;
                if job.scheduled == 1 || job.scheduled == job.total_frames {
                    println!(
                        "[capture] scheduled frame {}/{}",
                        job.scheduled, job.total_frames
                    );
                }
            } else if job.exit_after_tick.is_none() {
                job.exit_after_tick = Some(job.tick + DRAIN_FRAMES);
                println!("[capture] all frames scheduled");
            }
        }
    }

    if let Some(exit_tick) = job.exit_after_tick {
        if job.tick >= exit_tick && capturing.is_empty() {
            println!("[capture] complete");
            exit.write(AppExit::Success);
        }
    }
}
