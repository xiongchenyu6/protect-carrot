//! 保卫萝卜 (Protect the Carrot) — Bevy port.
//!
//! Learning roadmap (see IMPLEMENTATION_PLAN.md):
//!   Stage 1  foundation + playfield   [done]
//!   Stage 2  enemies + waves          [done]
//!   Stage 3  towers + projectiles     [done]
//!   Stage 4  economy + UI + flow      <-- you are here
//!   Stage 5  WebGPU / wasm export
//!
//! Bevy concepts: `App` + plugins, `Resource` shared state, `Component`s on
//! entities, `States` for flow, `Message`s for decoupled combat, `bevy_ui` Nodes
//! for HUD/menus, `.run_if(..)` gating, and 2D rendering with `Sprite`/`Mesh2d`.

use bevy::camera::ScalingMode;
use bevy::prelude::*;
use bevy::window::{MonitorSelection, WindowMode};

use protect_carrot::{
    Levels, audio, bestiary, build, creatures, data, enemy, equipment, game, hero, i18n, meta,
    quality, sprites, states, tower, tutorial, ui, vfx,
};

// Web-only: a retrying HTTP asset reader, installed before AssetPlugin so a
// transient fetch failure retries instead of panicking the game.
#[cfg(target_arch = "wasm32")]
mod asset_io;

use build::Selection;
use data::{BOARD_H, BOARD_W, hex, levels};
use game::{
    CurrentLevel, Paused, Rng, RunState, keyboard_controls, load_level, not_paused, tick_auto_wave,
    tick_message,
};
use sprites::build_sprites;
use states::GameState;
use tower::{BuffTower, Damage, HealCarrot, Snapshot, Status};
use ui::{
    Progress, UiFont, despawn_with, hud_buttons, menu_buttons, overlay_buttons, spawn_gameover,
    spawn_hud, spawn_menu, spawn_victory, update_hud,
};

/// Virtual design size (board 800 + HUD panel 240, height 600). The camera and
/// `UiScale` scale this to fill the window so the game can run fullscreen.
const VIRTUAL_W: f32 = BOARD_W + PANEL_W;
const VIRTUAL_H: f32 = BOARD_H;

/// Width of the right-hand HUD/build panel (screen space).
// Width of the right HUD rail in world units. MUST match `ui::PANEL_W_UI` so the
// rail exactly covers the reserved strip and never overlaps the board.
const PANEL_W: f32 = 256.0;

/// World units reserved on the LEFT for the mobile control/status strip (touch mode
/// only). Because UI and the camera share `UiScale`, a fixed-screen-px strip maps to
/// a constant world width — so reserving it in the projection keeps the board fully
/// in the free center and never under the strip. See `fit_camera_mode`.
const LEFT_RESERVE: f32 = 80.0;

// Tell the HTML loading screen the game is actually ready (first frames rendered +
// menu sprites loaded), so it doesn't fade out into a blank screen.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(inline_js = r#"
export function carrot_game_ready() { try { if (window.__carrotReady) window.__carrotReady(); } catch (_) {} }
"#)]
extern "C" {
    fn carrot_game_ready();
}
#[cfg(not(target_arch = "wasm32"))]
fn carrot_game_ready() {}

/// Signal the JS loading overlay to fade once the menu has rendered a few frames and
/// the key menu sprites (hero portraits + carrot) have finished loading.
fn signal_game_ready(
    assets: Res<AssetServer>,
    sprites: Res<sprites::Sprites>,
    mut frames: Local<u32>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }
    *frames += 1;
    let sprites_ready = sprites.heroes.values().all(|h| assets.is_loaded(h.id()))
        && assets.is_loaded(sprites.carrot.id());
    // Signal once the menu's sprites are loaded; fall back after ~4s so the "enter"
    // button always appears even if asset load-state never reports ready.
    if (*frames > 3 && sprites_ready) || *frames > 240 {
        *done = true;
        carrot_game_ready();
    }
}

fn main() {
    // Resolution is adaptive: native device-pixel-ratio + `fit_canvas_to_parent`
    // let the canvas track the screen. The player-adjustable graphics quality
    // (流畅/标准/精细) controls anti-aliasing via `quality::apply_quality`, not
    // resolution. Load it here only to persist/seed the resource.
    let quality = quality::GraphicsQuality::load();
    let resolution: bevy::window::WindowResolution =
        ((BOARD_W + PANEL_W) as u32, BOARD_H as u32).into();

    let mut app = App::new();

    // Web: install our retrying HTTP asset reader as the default asset source.
    // MUST run before `AssetPlugin` (added via `DefaultPlugins`) builds the
    // sources. Bevy's stock wasm reader unwraps on a body-read rejection, so a
    // single HTTP/2 stream reset while bulk-loading sprites panics the game;
    // ours retries transient failures with backoff instead.
    #[cfg(target_arch = "wasm32")]
    {
        use bevy::asset::AssetApp;
        use bevy::asset::io::{AssetSourceBuilder, AssetSourceId, ErasedAssetReader};
        app.register_asset_source(
            AssetSourceId::Default,
            AssetSourceBuilder::new(|| {
                Box::new(asset_io::RobustHttpAssetReader::new("assets"))
                    as Box<dyn ErasedAssetReader>
            }),
        );
    }

    app.add_plugins(
        DefaultPlugins
            // Bevy 0.19's text backend (parley→icu_segmenter) has no bundled CJK
            // dictionary and logs "ICU4X data error: No segmentation model for
            // language: ja" on every layout via `icu_provider`'s `log::warn!`. It's
            // non-fatal (CJK still renders). Bevy prepends its `level` to this filter
            // (`format!("{level},{filter}")`), so the leading bare `error` is no longer
            // a reliable global default — silence the icu crates EXPLICITLY by target
            // so they're dropped regardless of how the default resolves.
            .set(bevy::log::LogPlugin {
                filter: "error,protect_carrot=info,bevy=warn,wgpu=error,naga=warn,\
                         icu_provider=off,icu_segmenter=off,icu_locale=off,\
                         icu_properties=off,icu_normalizer=off,icu_collections=off"
                    .into(),
                level: bevy::log::Level::INFO,
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "保卫萝卜 — Bevy".into(),
                    resolution,
                    // Web: resize the canvas to fill the page (responsive / mobile).
                    fit_canvas_to_parent: true,
                    prevent_default_event_handling: true,
                    ..default()
                }),
                ..default()
            })
            // Assets ship without `.meta` sidecars; skip the probe so the web build
            // doesn't fire a 404 for every texture/audio file.
            .set(AssetPlugin {
                meta_check: bevy::asset::AssetMetaCheck::Never,
                ..default()
            }),
    )
    .init_state::<GameState>()
    .insert_resource(ClearColor(hex(0x1e2a1e)))
    .insert_resource(quality)
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
    .init_resource::<Progress>()
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
            setup,
            creatures::load_creatures,
            build::load_hero_walks,
            audio::load_sfx,
            audio::start_bgm,
        ),
    )
    .init_resource::<build::HeroWalks>()
    .init_resource::<ui::StoryDialogue>()
    .add_systems(
        Update,
        (
            fit_ui_scale,
            fit_camera_mode,
            ui::cjk_linebreak,
            signal_game_ready,
            toggle_fullscreen,
            audio::play_sfx,
            creatures::animate_creatures,
            vfx::update_camera_shake,
        ),
    )
    // ---- menu ----
    .add_systems(OnEnter(GameState::Menu), spawn_menu)
    .add_systems(OnExit(GameState::Menu), despawn_with::<ui::MenuRoot>)
    .add_systems(
        Update,
        (
            menu_buttons,
            ui::update_menu_diff,
            ui::update_quality_label,
            ui::update_volume_label,
            ui::update_language_label,
            ui::update_hero_label,
            ui::update_hero_select_buttons,
        )
            .run_if(in_state(GameState::Menu)),
    )
    // Language change flips `MenuDirty`; rebuild the menu so translations apply.
    .add_systems(
        Update,
        (despawn_with::<ui::MenuRoot>, spawn_menu)
            .chain()
            .run_if(in_state(GameState::Menu))
            .run_if(|d: Res<ui::MenuDirty>| d.0),
    )
    // ---- opening story scene ----
    .add_systems(OnEnter(GameState::Story), ui::spawn_story_scene)
    .add_systems(OnExit(GameState::Story), despawn_with::<ui::StoryRoot>)
    .add_systems(
        Update,
        (
            ui::update_story_animation,
            ui::play_story_voiceover,
            ui::advance_story_dialogue,
            ui::story_choice_buttons,
        )
            .run_if(in_state(GameState::Story)),
    )
    // ---- pre-level briefing / animated transition ----
    .add_systems(OnEnter(GameState::Briefing), ui::spawn_level_briefing)
    .add_systems(
        OnExit(GameState::Briefing),
        despawn_with::<ui::BriefingRoot>,
    )
    .add_systems(
        Update,
        (
            ui::update_briefing_animation,
            ui::briefing_buttons,
            ui::update_hero_label,
            ui::update_hero_select_buttons,
        )
            .run_if(in_state(GameState::Briefing)),
    )
    // ---- hero deploy cutscene ----
    .add_systems(OnEnter(GameState::HeroIntro), ui::spawn_hero_intro)
    .add_systems(
        OnExit(GameState::HeroIntro),
        despawn_with::<ui::HeroIntroRoot>,
    )
    .add_systems(
        Update,
        ui::update_hero_intro.run_if(in_state(GameState::HeroIntro)),
    )
    .add_systems(
        Update,
        (
            ui::update_quality_label,
            ui::update_screen_flash,
            ui::tick_talent_confirm,
            ui::hero_buttons,
            ui::settings_nav_buttons,
            ui::hero_joystick,
            build::hero_move,
            build::hero_control.after(build::mouse_build),
            build::hero_status,
            build::hero_respawn,
        )
            .run_if(in_state(GameState::Playing)),
    )
    // ---- level lifecycle ----
    .add_systems(
        OnEnter(GameState::Playing),
        (
            load_level,
            spawn_hud,
            build::auto_spawn_hero,
            tutorial::maybe_start_tutorial,
        )
            .chain(),
    )
    .add_systems(
        OnExit(GameState::Playing),
        (
            despawn_with::<ui::HudRoot>,
            despawn_with::<ui::MobileHudRoot>,
            despawn_with::<build::BuildGhost>,
            despawn_with::<tutorial::TutorialRoot>,
        ),
    )
    // simulation (frozen while paused)
    .add_systems(
        Update,
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
            tower::enemy_vs_ally,
            tower::enemy_vs_tower,
            enemy::boss_specials,
            tower::update_fire_grounds,
            enemy::spawn_enemies,
            enemy::update_enemies,
            tower::necromancer_raise,
            // 嵌套成二元组以绕开单个 add_systems 最多 20 个系统的上限。
            (enemy::heal_auras, enemy::incubation),
            tick_auto_wave,
            tick_message,
        )
            .chain()
            .run_if(in_state(GameState::Playing).and_then(not_paused)),
    )
    // input + HUD always tick while in Playing (so you can unpause / click UI)
    .add_systems(
        Update,
        (
            keyboard_controls,
            build::select_build_kind,
            build::mouse_build,
            build::upgrade_sell,
            build::draw_range_gizmos,
            build::update_build_ghost,
            enemy::draw_silence_auras,
            enemy::draw_heal_auras,
            enemy::draw_elite_auras,
            enemy::draw_boss_cast_telegraphs,
            tower::draw_tower_raider_threats,
            tower::draw_equipment_resonance,
            build::summon_god_tower,
            build::hero_afterimage,
            build::animate_hero_walk,
            build::rotate_towers,
            build::update_hero_race_badges,
            build::tint_silenced_towers,
            build::update_tower_hp_bars,
        )
            .run_if(in_state(GameState::Playing)),
    )
    .add_systems(
        Update,
        (
            tutorial::watch_tutorial,
            tutorial::tutorial_buttons,
            tutorial::refresh_panel,
            tutorial::draw_tutorial_hint,
        )
            .run_if(in_state(GameState::Playing)),
    )
    .add_systems(
        Update,
        (
            game::update_carrot_seal,
            game::grow_portal,
            tower::compute_synergy,
        )
            .run_if(in_state(GameState::Playing)),
    )
    .add_systems(
        Update,
        (
            enemy::update_hp_bars,
            tower::update_summon_hp_bars,
            meta::tick_cooldowns,
            meta::ability_keys,
            meta::cast_abilities,
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
            ui::detect_touch_mode,
            ui::update_mobile_controls,
            ui::update_panel_visibility,
            hud_buttons,
        )
            .run_if(in_state(GameState::Playing)),
    )
    .add_systems(
        Update,
        ui::update_boss_bar.run_if(in_state(GameState::Playing)),
    )
    // tooltip + panel scrolling
    .add_systems(
        Update,
        ui::update_ability_buttons.run_if(in_state(GameState::Playing)),
    )
    // tooltips also run on the selection screens (menu/briefing) so the race/class/
    // difficulty pickers show their info. Harmless where no TooltipBox exists.
    .add_systems(
        Update,
        ui::tooltip_system.run_if(
            in_state(GameState::Playing)
                .or_else(in_state(GameState::Menu))
                .or_else(in_state(GameState::Briefing)),
        ),
    )
    // wheel + touch-drag scrolling work in every state (in-game palette AND menus,
    // so the menu's tall side column stays reachable on short/wide windows)
    .add_systems(Update, ui::scroll_panel)
    .add_systems(Update, ui::touch_scroll_panel.after(build::mouse_build))
    // graphics-quality (render scale) applied whenever the player changes it
    .add_systems(
        Update,
        (
            quality::apply_quality,
            audio::apply_master_volume,
            i18n::sync_current_lang,
        ),
    )
    // ---- overlays ----
    .add_systems(OnEnter(GameState::GameOver), spawn_gameover)
    .add_systems(OnExit(GameState::GameOver), despawn_with::<ui::OverlayRoot>)
    .add_systems(OnEnter(GameState::Victory), spawn_victory)
    .add_systems(OnExit(GameState::Victory), despawn_with::<ui::OverlayRoot>)
    .add_systems(
        Update,
        overlay_buttons.run_if(in_state(GameState::GameOver).or_else(in_state(GameState::Victory))),
    )
    // ---- bestiary ----
    .add_systems(OnEnter(GameState::Bestiary), ui::spawn_bestiary)
    .add_systems(
        OnExit(GameState::Bestiary),
        despawn_with::<ui::BestiaryRoot>,
    )
    .add_systems(
        Update,
        ui::bestiary_buttons.run_if(in_state(GameState::Bestiary)),
    )
    // ---- armory ----
    .add_systems(OnEnter(GameState::Armory), ui::spawn_armory)
    .add_systems(OnExit(GameState::Armory), despawn_with::<ui::ArmoryRoot>)
    .add_systems(
        Update,
        ui::armory_buttons.run_if(in_state(GameState::Armory)),
    )
    // ---- tower archive ----
    .add_systems(OnEnter(GameState::TowerArchive), ui::spawn_tower_archive)
    .add_systems(
        OnExit(GameState::TowerArchive),
        despawn_with::<ui::TowerArchiveRoot>,
    )
    .add_systems(
        Update,
        ui::tower_archive_buttons.run_if(in_state(GameState::TowerArchive)),
    )
    // ---- hero codex ----
    .add_systems(OnEnter(GameState::HeroCodex), ui::spawn_hero_codex)
    .add_systems(
        OnExit(GameState::HeroCodex),
        despawn_with::<ui::HeroCodexRoot>,
    )
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
    // ---- milestones ----
    .add_systems(OnEnter(GameState::Milestones), ui::spawn_milestones)
    .add_systems(
        OnExit(GameState::Milestones),
        despawn_with::<ui::MilestonesRoot>,
    )
    .add_systems(
        Update,
        ui::milestone_buttons.run_if(in_state(GameState::Milestones)),
    )
    // ---- campaign dossier ----
    .add_systems(
        OnEnter(GameState::CampaignDossier),
        ui::spawn_campaign_dossier,
    )
    .add_systems(
        OnExit(GameState::CampaignDossier),
        despawn_with::<ui::CampaignDossierRoot>,
    )
    .add_systems(
        Update,
        ui::campaign_dossier_buttons.run_if(in_state(GameState::CampaignDossier)),
    );

    // Register the embedded CJK font BEFORE running, so the first `OnEnter` state
    // schedule (which spawns UI reading `UiFont`) is guaranteed to find it. Doing
    // this in a `Startup` system can lose a race with the initial state transition.
    {
        let bytes = include_bytes!("../assets/fonts/wqy-microhei.ttc");
        let font = Font::from_bytes(bytes.to_vec());
        let handle = app.world_mut().resource_mut::<Assets<Font>>().add(font);
        app.insert_resource(UiFont(handle));
    }

    // Same race fix for sprites: the menu (OnEnter Menu) reads `Sprites`, so build
    // the handle table before `run()` rather than in a Startup system.
    {
        let assets = app.world().resource::<AssetServer>().clone();
        let sprites = build_sprites(&assets);
        app.insert_resource(sprites);
    }

    app.run();
}

/// Spawn the 2D camera. `AutoMin` keeps the whole virtual area (VIRTUAL_W ×
/// VIRTUAL_H) visible and scales it up to fill the window; the camera is centered
/// on that area (board on the left, +x toward the HUD panel on the right).
fn setup(mut commands: Commands) {
    let mut projection = OrthographicProjection::default_2d();
    projection.scaling_mode = ScalingMode::AutoMin {
        min_width: VIRTUAL_W,
        min_height: VIRTUAL_H,
    };
    commands.spawn((
        Camera2d,
        // Present so `quality::apply_quality` can drive it; its real value is set
        // from the persisted graphics-quality tier on the first Update.
        Msaa::default(),
        Projection::Orthographic(projection),
        Transform::from_xyz(PANEL_W / 2.0, 0.0, 0.0),
        vfx::ShakeCamera {
            base: Vec3::new(PANEL_W / 2.0, 0.0, 0.0),
        },
    ));
}

/// Scale `bevy_ui` to match the camera's scaling so the HUD grows with the board.
/// The camera (AutoMin) scales the world by `window_height / VIRTUAL_H` on wide
/// windows; matching `UiScale` keeps the HUD panel aligned with the board.
fn fit_ui_scale(windows: Query<&Window>, mode: Res<ui::TouchMode>, mut ui_scale: ResMut<UiScale>) {
    if let Ok(win) = windows.single() {
        // In touch mode the virtual area also reserves the left strip, so UI + world
        // shrink together and stay aligned with the reserved regions.
        let vw = if mode.0 {
            VIRTUAL_W + LEFT_RESERVE
        } else {
            VIRTUAL_W
        };
        let s = (win.width() / vw).min(win.height() / VIRTUAL_H);
        if s.is_finite() && s > 0.0 {
            ui_scale.0 = s;
        }
    }
}

/// Keep the camera's reserved regions in sync with PC/mobile mode: the board is
/// projected into the center, with `PANEL_W` reserved on the right (always) and
/// `LEFT_RESERVE` on the left (touch only). Driven by the single global `TouchMode`,
/// so the board is never rendered under the side bars.
fn fit_camera_mode(
    mode: Res<ui::TouchMode>,
    mut q: Query<(&mut Projection, &mut vfx::ShakeCamera)>,
) {
    let (vw, center_x) = if mode.0 {
        (VIRTUAL_W + LEFT_RESERVE, (PANEL_W - LEFT_RESERVE) / 2.0)
    } else {
        (VIRTUAL_W, PANEL_W / 2.0)
    };
    for (mut proj, mut shake) in &mut q {
        if let Projection::Orthographic(o) = &mut *proj {
            let want = ScalingMode::AutoMin {
                min_width: vw,
                min_height: VIRTUAL_H,
            };
            // ScalingMode isn't PartialEq; set unconditionally (cheap).
            o.scaling_mode = want;
        }
        shake.base.x = center_x;
    }
}

/// F11 toggles borderless fullscreen. On the web this triggers the browser's
/// Fullscreen API (the keypress counts as the required user gesture).
fn toggle_fullscreen(keys: Res<ButtonInput<KeyCode>>, mut windows: Query<&mut Window>) {
    if !keys.just_pressed(KeyCode::F11) {
        return;
    }
    if let Ok(mut win) = windows.single_mut() {
        win.mode = match win.mode {
            WindowMode::Windowed => WindowMode::BorderlessFullscreen(MonitorSelection::Current),
            _ => WindowMode::Windowed,
        };
    }
}
