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
    window::{ExitCondition, WindowPlugin},
    winit::WinitPlugin,
};

use protect_carrot::{
    Levels, audio, bestiary,
    board::Board,
    build::{self, Selection},
    creatures, data, enemy, equipment, game, hero, i18n, meta, quality, sprites,
    states::GameState,
    tower, tutorial, ui, vfx,
};

use data::{BOARD_H, BOARD_W, TowerKind, hex, levels};
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
struct CapturePrepared(bool);

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
    .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
        1.0 / FPS,
    )))
    .insert_state(GameState::Playing)
    .insert_resource(ClearColor(hex(0x1e2a1e)))
    .insert_resource(quality::GraphicsQuality {
        level: quality::QualityLevel::Balanced,
    })
    .init_resource::<audio::AudioSettings>()
    .init_resource::<i18n::Language>()
    .init_resource::<ui::MenuDirty>()
    .insert_resource(Levels(levels()))
    .init_resource::<CurrentLevel>()
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
    .init_resource::<ui::TooltipHold>()
    .init_resource::<meta::Talents>()
    .init_resource::<meta::Abilities>()
    .init_resource::<equipment::EquipmentInventory>()
    .init_resource::<bestiary::Bestiary>()
    .init_resource::<vfx::ScreenShake>()
    .init_resource::<build::HeroWalks>()
    .init_resource::<ui::StoryDialogue>()
    .init_resource::<CapturePrepared>()
    .insert_resource(job)
    .add_systems(Startup, ui::load_persistent_progress)
    .add_message::<Damage>()
    .add_message::<Status>()
    .add_message::<BuffTower>()
    .add_message::<HealCarrot>()
    .add_message::<vfx::VfxEvent>()
    .add_message::<audio::SfxEvent>()
    .add_message::<tower::EnemyDied>()
    .add_systems(
        Startup,
        (
            setup_capture_camera,
            creatures::load_creatures,
            build::load_hero_walks,
        ),
    )
    .add_systems(
        OnEnter(GameState::Playing),
        (load_level, spawn_hud, build::auto_spawn_hero).chain(),
    )
    .add_systems(PreUpdate, ui::cjk_linebreak)
    .add_systems(
        Update,
        (
            fit_capture_ui_scale,
            ui::cjk_linebreak,
            creatures::animate_creatures,
            vfx::update_camera_shake,
            quality::apply_quality,
            i18n::sync_current_lang,
        ),
    )
    .add_systems(
        Update,
        (
            prepare_capture_level,
            (
                tower::build_snapshot,
                hero::hero_doctrine,
                tower::update_towers,
                tower::update_projectiles,
                tower::update_shot_fx,
                tower::update_summons,
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
            ui::update_equipment_button_labels,
            ui::update_upgrade_button_label,
            ui::update_equipped_slot_icons,
            ui::update_boss_bar,
            ui::update_ability_buttons,
            ui::tooltip_system,
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

    app.run()
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
    ));
}

fn fit_capture_ui_scale(mut ui_scale: ResMut<UiScale>) {
    ui_scale.0 = (CAPTURE_W as f32 / VIRTUAL_W).min(CAPTURE_H as f32 / VIRTUAL_H);
}

fn prepare_capture_level(
    mut commands: Commands,
    board: Option<Res<Board>>,
    sprites: Res<sprites::Sprites>,
    talents: Res<meta::Talents>,
    current: Res<CurrentLevel>,
    mut rng: ResMut<Rng>,
    mut run: ResMut<RunState>,
    mut prepared: ResMut<CapturePrepared>,
) {
    if prepared.0 {
        return;
    }
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
    prepared.0 = true;
}

fn drive_capture(
    mut commands: Commands,
    target: Res<CaptureTarget>,
    mut job: ResMut<CaptureJob>,
    capturing: Query<Entity, With<Capturing>>,
    mut exit: MessageWriter<AppExit>,
) {
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
