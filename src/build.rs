//! Player interaction: pick a tower type, place it, select/upgrade/sell.
//!
//! Stage 3 uses keyboard + mouse (number keys choose a type, click to build).
//! Stage 4 adds a proper clickable sidebar that drives the same `Selection`.

use crate::board::Board;
use crate::components::{LevelEntity, Particle, TowerHpBar};
use crate::data::Behavior;
use crate::data::{BOARD_H, BOARD_W, COLS, ROWS, TILE_SIZE, TowerKind, cell_center};
use crate::equipment::{
    EquipmentInventory, return_equipment_to_inventory, unequip_all_to_inventory,
};
use crate::game::RunState;
use crate::hero::{Class, HeroLoadout, Race, hero_move_speed};
use crate::meta::Talents;
use crate::sprites::Sprites;
use crate::tower::{GodTower, HERO_MELEE_ATTACK_TIME, Tower};
use crate::ui::UiAction;
use bevy::prelude::*;
use bevy::sprite::Anchor;

/// Current build/selection state, shared with the (future) UI.
#[derive(Resource, Default)]
pub struct Selection {
    /// Tower type queued for placement (None = not building).
    pub build_kind: Option<TowerKind>,
    /// Currently inspected placed tower.
    pub selected: Option<Entity>,
    /// Touch placement: the cell the finger is currently over (drives the ghost
    /// preview). Set while a touch drags over the board; the build commits on
    /// release. Unused for desktop (mouse builds on click), so it stays `None`.
    pub preview_cell: Option<(i32, i32)>,
    /// True when the current touch gesture began on a UI element, so its release
    /// must NOT build/select on the board behind it (e.g. the bottom touch bar).
    pub touch_from_ui: bool,
    /// A placement drag is in progress (finger held after grabbing a tower icon,
    /// or pressing the board with a tower armed). Drives the follow-the-finger ghost.
    pub dragging: bool,
    /// True when the current touch gesture began by grabbing a tower from the
    /// palette (drag-and-drop): such gestures place on release. A gesture that
    /// begins on the board instead uses tap-to-preview then tap-to-confirm.
    pub grabbed_from_palette: bool,
}

/// Convert the cursor position to a world coordinate.
fn cursor_world(
    windows: &Query<&Window>,
    camera: &Query<(&Camera, &GlobalTransform)>,
) -> Option<Vec2> {
    let window = windows.single().ok()?;
    let cursor = window.cursor_position()?;
    let (cam, cam_tf) = camera.single().ok()?;
    cam.viewport_to_world_2d(cam_tf, cursor).ok()
}

/// A translucent preview of the tower about to be placed. Far more visible on
/// touch than thin gizmo lines: it shows the actual sprite, tinted green/red.
#[derive(Component)]
pub struct BuildGhost;

/// Update the build-ghost sprite to follow the armed cell (touch: `preview_cell`;
/// desktop: the cursor cell), tinted by whether placement is valid. Hidden when
/// not building.
pub fn update_build_ghost(
    sel: Res<Selection>,
    board: Res<Board>,
    run: Res<RunState>,
    sprites: Res<Sprites>,
    towers: Query<&Tower>,
    windows: Query<&Window>,
    camera: Query<(&Camera, &GlobalTransform)>,
    mut ghost: Query<(&mut Sprite, &mut Transform, &mut Visibility), With<BuildGhost>>,
) {
    let Ok((mut sprite, mut tf, mut vis)) = ghost.single_mut() else {
        return;
    };
    let Some(kind) = sel.build_kind else {
        *vis = Visibility::Hidden;
        return;
    };
    let cell = sel
        .preview_cell
        .or_else(|| cursor_world(&windows, &camera).and_then(world_to_cell));
    let Some((col, row)) = cell else {
        *vis = Visibility::Hidden;
        return;
    };
    let def = kind.def();
    let fp = def.footprint.max(1);
    let off = (fp - 1) as f32 / 2.0;
    let can_build =
        footprint_buildable(&board, towers.iter(), kind, col, row) && run.gold >= def.cost;
    sprite.image = sprites.towers[&kind].clone();
    sprite.custom_size = Some(Vec2::splat(TILE_SIZE * fp as f32 * 0.95));
    sprite.color = if can_build {
        Color::srgba(0.5, 1.0, 0.5, 0.6)
    } else {
        Color::srgba(1.0, 0.4, 0.4, 0.6)
    };
    tf.translation = cell_center(col as f32 + off, row as f32 + off).extend(7.0);
    *vis = Visibility::Visible;
}

/// World position -> (col,row), or None if outside the board.
fn world_to_cell(world: Vec2) -> Option<(i32, i32)> {
    let col = ((world.x + BOARD_W / 2.0) / TILE_SIZE).floor() as i32;
    let row = ((BOARD_H / 2.0 - world.y) / TILE_SIZE).floor() as i32;
    if (0..COLS).contains(&col) && (0..ROWS).contains(&row) {
        Some((col, row))
    } else {
        None
    }
}

/// Number-row keys select a tower type to build (temporary; Stage 4 adds buttons).
pub fn select_build_kind(keys: Res<ButtonInput<KeyCode>>, mut sel: ResMut<Selection>) {
    const ROW: [(KeyCode, usize); 10] = [
        (KeyCode::Digit1, 0),
        (KeyCode::Digit2, 1),
        (KeyCode::Digit3, 2),
        (KeyCode::Digit4, 3),
        (KeyCode::Digit5, 4),
        (KeyCode::Digit6, 5),
        (KeyCode::Digit7, 6),
        (KeyCode::Digit8, 7),
        (KeyCode::Digit9, 8),
        (KeyCode::Digit0, 9),
    ];
    for (key, idx) in ROW {
        if keys.just_pressed(key) {
            sel.build_kind = Some(TowerKind::ALL[idx]);
            sel.selected = None;
            sel.preview_cell = None;
        }
    }
    if keys.just_pressed(KeyCode::Escape) {
        sel.build_kind = None;
        sel.selected = None;
        sel.preview_cell = None;
    }
}

/// Placement & selection — drag-and-drop from the build palette.
///
/// **Mobile**: press a tower icon → a ghost follows your finger → release over a
/// valid board cell to drop it (release elsewhere cancels). You can also tap the
/// icon, then tap the board. Tapping any non-build UI (波次/暂停/…) cancels build
/// mode, so those buttons always work.
///
/// **Desktop**: click a tower icon to arm, click the board to place (repeatable);
/// right-click cancels.
pub fn mouse_build(
    mut commands: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    windows: Query<&Window>,
    camera: Query<(&Camera, &GlobalTransform)>,
    board: Res<Board>,
    mut run: ResMut<RunState>,
    mut sel: ResMut<Selection>,
    towers: Query<(Entity, &Tower)>,
    sprites: Res<Sprites>,
    talents: Res<Talents>,
    mut sfx: MessageWriter<crate::audio::SfxEvent>,
    mut vfx: MessageWriter<crate::vfx::VfxEvent>,
    ui_buttons: Query<(&Interaction, &UiAction)>,
) {
    if buttons.just_pressed(MouseButton::Right) {
        // Right-click cancels an in-progress build/drag. With nothing armed it is a
        // hero move command (handled in `hero_control`), so keep the selection.
        if sel.build_kind.is_some() || sel.dragging || sel.grabbed_from_palette {
            sel.build_kind = None;
            sel.preview_cell = None;
            sel.dragging = false;
            sel.grabbed_from_palette = false;
        }
        return;
    }

    // Which UI element (if any) is currently pressed, and is the pointer over UI?
    let pressed_build = ui_buttons.iter().find_map(|(i, a)| match (i, a) {
        (Interaction::Pressed, UiAction::Build(k)) => Some(*k),
        _ => None,
    });
    let pressed_other_ui = ui_buttons
        .iter()
        .any(|(i, a)| *i == Interaction::Pressed && !matches!(a, UiAction::Build(_)));
    let over_ui = ui_buttons
        .iter()
        .any(|(i, _)| !matches!(*i, Interaction::None));

    let fresh_press =
        buttons.just_pressed(MouseButton::Left) || touches.iter_just_pressed().next().is_some();
    let is_touch_press = touches.iter_just_pressed().next().is_some();

    let Ok((cam, cam_tf)) = camera.single() else {
        return;
    };
    let to_cell = |screen: Vec2| {
        cam.viewport_to_world_2d(cam_tf, screen)
            .ok()
            .and_then(world_to_cell)
    };

    // --- Grabbing a tower from the palette (its Build icon was just pressed) ---
    if let Some(kind) = pressed_build {
        if fresh_press {
            if is_touch_press && sel.build_kind == Some(kind) {
                // Tapping the already-armed tower again cancels it (mobile has no
                // right-click to back out of build mode).
                sel.build_kind = None;
                sel.preview_cell = None;
                sel.dragging = false;
                sel.grabbed_from_palette = false;
            } else {
                sel.build_kind = Some(kind);
                sel.selected = None;
                sel.preview_cell = None;
                sel.dragging = is_touch_press; // touch: a drag begins from the icon
                sel.grabbed_from_palette = is_touch_press;
                sel.touch_from_ui = false; // this gesture is allowed to drop on the board
            }
        }
        return; // never place on the same press that grabbed the tower
    }

    // --- Pressing a *non-build* button cancels build mode (so 波次 etc. work) ---
    if pressed_other_ui && fresh_press {
        sel.build_kind = None;
        sel.preview_cell = None;
        sel.dragging = false;
        sel.grabbed_from_palette = false;
        sel.touch_from_ui = true;
        return;
    }

    // --- Touch press on the open board (not a palette grab) ---
    if touches.iter_just_pressed().next().is_some() {
        sel.touch_from_ui = false;
    }

    // --- Drag: the ghost follows the finger while a palette drag is active ---
    if sel.dragging && sel.grabbed_from_palette && sel.build_kind.is_some() {
        if let Some(t) = touches.iter().next() {
            sel.preview_cell = to_cell(t.position());
        }
    }

    // --- Commit: desktop left-click, or touch lift-off ---
    //
    // Touch has two placement gestures:
    //  * Drag a tower out of the palette → it drops on release (the ghost has been
    //    visible the whole drag, so the range is already previewed).
    //  * Tap an empty board cell → the first tap only *previews* the range there;
    //    a second tap on the same cell confirms and builds.
    let (col, row, building, was_touch) = if buttons.just_pressed(MouseButton::Left) {
        if over_ui {
            return;
        }
        match windows
            .single()
            .ok()
            .and_then(|w| w.cursor_position())
            .and_then(to_cell)
        {
            Some((c, r)) => (c, r, true, false),
            None => return,
        }
    } else if let Some(t) = touches.iter_just_released().next() {
        sel.dragging = false;
        if sel.touch_from_ui {
            return;
        }
        let grabbed = sel.grabbed_from_palette;
        sel.grabbed_from_palette = false;
        match to_cell(t.position()) {
            Some((c, r)) => {
                // Build now only if dragged from the palette, or this cell already
                // had its preview armed by a prior tap.
                let confirm = grabbed || sel.preview_cell == Some((c, r));
                (c, r, confirm, true)
            }
            None => {
                // Released off the board (e.g. back over the panel): cancel preview.
                sel.preview_cell = None;
                return;
            }
        }
    } else {
        return;
    };

    // Tapping/clicking a placed tower selects it (and cancels any pending build).
    if let Some((e, _)) = towers.iter().find(|(_, t)| t.covers(col, row)) {
        sel.selected = Some(e);
        sel.build_kind = None;
        sel.preview_cell = None;
        return;
    }

    let Some(kind) = sel.build_kind else {
        return;
    };

    // First tap on an empty cell: just arm the range preview, don't build yet.
    if !building {
        sel.preview_cell = Some((col, row));
        let affordable = run.gold >= kind.def().cost;
        let placeable = footprint_buildable(&board, towers.iter().map(|(_, t)| t), kind, col, row);
        if placeable && affordable {
            run.show(crate::i18n::t("再次点击该格子确认建造"));
        } else if !placeable {
            run.show(crate::i18n::t("此处放不下"));
        } else {
            run.show(crate::i18n::t("金币不足"));
        }
        return;
    }

    let def = kind.def();
    if !footprint_buildable(&board, towers.iter().map(|(_, t)| t), kind, col, row) {
        run.show(crate::i18n::t("此处放不下"));
        sel.preview_cell = None;
        return;
    }
    if run.gold < def.cost {
        run.show(crate::i18n::t("金币不足"));
        sel.preview_cell = None;
        sfx.write(crate::audio::SfxEvent(crate::audio::Sound::NoGold));
        return;
    }
    run.gold -= def.cost;
    spawn_tower(&mut commands, kind, col, row, &sprites, &talents);
    // Placement "poof": a dust ring + sparks sized to the footprint.
    let fp = def.footprint.max(1);
    let off = (fp - 1) as f32 / 2.0;
    let center = cell_center(col as f32 + off, row as f32 + off);
    vfx.write(crate::vfx::VfxEvent::Burst {
        pos: center,
        radius: TILE_SIZE * fp as f32 * 0.6,
        color: Color::srgb(0.7, 0.95, 0.6),
    });
    sel.preview_cell = None;
    // Touch is one-shot (re-tap the icon for another); desktop keeps the tower
    // armed for rapid repeat placement (right-click or another icon to change).
    if was_touch {
        sel.build_kind = None;
    }
    sfx.write(crate::audio::SfxEvent(crate::audio::Sound::Build));
}

/// True if a `kind` tower placed with top-left at `(col,row)` fits: every cell of
/// its footprint is buildable and not already covered by another tower.
pub fn footprint_buildable<'a>(
    board: &Board,
    towers: impl Iterator<Item = &'a Tower> + Clone,
    kind: TowerKind,
    col: i32,
    row: i32,
) -> bool {
    let fp = kind.def().footprint.max(1);
    for dx in 0..fp {
        for dy in 0..fp {
            let (cx, cy) = (col + dx, row + dy);
            if !board.buildable.contains(&(cx, cy)) {
                return false;
            }
            if towers.clone().any(|t| t.covers(cx, cy)) {
                return false;
            }
        }
    }
    true
}

/// Spawn a tower entity using its sprite. The sprite art faces +x; `rotate_towers`
/// turns it toward the current target.
pub fn spawn_tower(
    commands: &mut Commands,
    kind: TowerKind,
    col: i32,
    row: i32,
    sprites: &Sprites,
    talents: &Talents,
) {
    let def = kind.def();
    let fp = def.footprint.max(1);
    let off = (fp - 1) as f32 / 2.0;
    let pos = cell_center(col as f32 + off, row as f32 + off);
    // Apply current global talents to the new tower's base stats.
    let mut tower = Tower::from_def(def, col, row);
    tower.base_damage *= talents.damage_mult;
    tower.damage = tower.base_damage;
    tower.range *= talents.range_mult;
    tower.cooldown *= talents.firerate_mult;
    let tower_entity = commands
        .spawn((
            tower,
            Sprite {
                image: sprites.towers[&kind].clone(),
                custom_size: Some(Vec2::splat(TILE_SIZE * fp as f32 * 0.95)),
                ..default()
            },
            Transform::from_translation(pos.extend(4.0)),
            LevelEntity,
        ))
        .id();

    let bar_w = (TILE_SIZE * fp as f32 * 0.78).clamp(24.0, 92.0);
    let bar_h = 5.0;
    let bar_y = TILE_SIZE * fp as f32 * 0.52 + 5.0;
    commands.spawn((
        Sprite {
            color: Color::srgb(0.09, 0.04, 0.04),
            custom_size: Some(Vec2::new(bar_w, bar_h)),
            ..default()
        },
        Transform::from_translation((pos + Vec2::new(0.0, bar_y)).extend(6.2)),
        TowerHpBar {
            owner: tower_entity,
            width: bar_w,
            offset_y: bar_y,
            foreground: false,
        },
        LevelEntity,
    ));
    commands.spawn((
        Sprite {
            color: Color::srgb(0.35, 0.95, 0.38),
            custom_size: Some(Vec2::new(bar_w, bar_h)),
            ..default()
        },
        Anchor::CENTER_LEFT,
        Transform::from_translation((pos + Vec2::new(-bar_w / 2.0, bar_y)).extend(6.3)),
        TowerHpBar {
            owner: tower_entity,
            width: bar_w,
            offset_y: bar_y,
            foreground: true,
        },
        LevelEntity,
    ));
}

/// Engineer's level-30 ultimate: summon a single 神之塔 (god tower) — a stationary
/// tower fusing every attribute (huge AoE damage + chain + slow + poison + range).
/// Runs each frame; spawns at most one (guarded by the `GodTower` marker).
pub fn summon_god_tower(
    mut commands: Commands,
    loadout: Res<HeroLoadout>,
    sprites: Res<Sprites>,
    existing: Query<(), With<GodTower>>,
) {
    if loadout.class != Class::Engineer
        || loadout.level < HeroLoadout::MAX_LEVEL
        || !existing.is_empty()
    {
        return;
    }
    // Place it on a central upper-board cell so its huge range blankets the field.
    let (col, row) = (COLS / 2, ROWS / 3);
    let pos = cell_center(col as f32, row as f32);
    let mut t = Tower::from_def(TowerKind::Arrow.def(), col, row);
    t.behavior = Behavior::Aoe;
    t.base_damage = 520.0;
    t.damage = 520.0;
    t.range = 360.0;
    t.cooldown = 0.3;
    t.aoe_radius = 130.0;
    t.chain_count = 4;
    t.chain_range = 150.0;
    t.slow_duration = 1.0;
    t.freeze_duration = 0.4;
    t.dot_damage = 80.0;
    t.poison_duration = 3.0;
    t.knock_dist = 16.0;
    t.armor_reduce = 12.0;
    t.curse_duration = 2.0;
    t.max_hp = 6000.0;
    t.hp = 6000.0;
    t.armor = 50.0;
    t.color = Color::srgb(1.0, 0.88, 0.35);
    let tower_entity = commands
        .spawn((
            t,
            GodTower,
            Sprite {
                image: sprites.towers[&TowerKind::Fortress].clone(),
                color: Color::srgb(1.0, 0.88, 0.35),
                custom_size: Some(Vec2::splat(TILE_SIZE * 1.6)),
                ..default()
            },
            Transform::from_translation(pos.extend(4.5)),
            LevelEntity,
        ))
        .id();
    let bar_w = TILE_SIZE * 1.3;
    let bar_y = TILE_SIZE * 0.95;
    commands.spawn((
        Sprite {
            color: Color::srgb(0.09, 0.04, 0.04),
            custom_size: Some(Vec2::new(bar_w, 5.0)),
            ..default()
        },
        Transform::from_translation((pos + Vec2::new(0.0, bar_y)).extend(6.2)),
        TowerHpBar {
            owner: tower_entity,
            width: bar_w,
            offset_y: bar_y,
            foreground: false,
        },
        LevelEntity,
    ));
    commands.spawn((
        Sprite {
            color: Color::srgb(1.0, 0.86, 0.3),
            custom_size: Some(Vec2::new(bar_w, 5.0)),
            ..default()
        },
        Anchor::CENTER_LEFT,
        Transform::from_translation((pos + Vec2::new(-bar_w / 2.0, bar_y)).extend(6.3)),
        TowerHpBar {
            owner: tower_entity,
            width: bar_w,
            offset_y: bar_y,
            foreground: true,
        },
        LevelEntity,
    ));
}

/// Spawn the unique hero entity (a `Tower` with `hero = true`) plus its HP bars.
/// The sprite is chosen from dedicated hero class art, tinted by race color.
/// A class's walk-cycle sprite sheet: a horizontal strip of 128px square frames,
/// frame 0 = idle portrait, frames 1.. = walk poses (Flux-Kontext generated, see
/// `tools/comfy_kontext.py`). Plugged into Bevy's `TextureAtlas` like the creatures.
const HERO_WORLD_SIZE: f32 = TILE_SIZE * 1.18;
const HERO_GHOST_SIZE: f32 = TILE_SIZE * 1.04;

pub struct HeroWalkCfg {
    pub image: Handle<Image>,
    pub layout: Handle<TextureAtlasLayout>,
    pub frames: usize,
    pub size: f32,
}

#[derive(Resource, Default)]
pub struct HeroWalks {
    pub class_walks: std::collections::HashMap<Class, HeroWalkCfg>,
    pub race_worlds: std::collections::HashMap<(Class, Race), HeroWalkCfg>,
}

/// Per-hero walk animation cursor.
#[derive(Component)]
pub struct HeroWalkAnim {
    pub timer: Timer,
    pub frames: usize,
}

#[derive(Component)]
pub struct HeroRaceBadge {
    pub owner: Entity,
    pub offset: Vec2,
}

/// Classes that have a walk strip in `assets/heroes_walk/<name>.webp` (frame count
/// includes the idle frame 0). Add a class here once its strip is generated.
fn hero_walk_mapping() -> &'static [(Class, &'static str, usize)] {
    &[
        (Class::Warrior, "warrior", 7),
        (Class::Mage, "mage", 7),
        (Class::Ranger, "ranger", 7),
        (Class::Guardian, "guardian", 7),
        (Class::Stormcaller, "stormcaller", 7),
        (Class::Warden, "warden", 7),
        (Class::Assassin, "assassin", 7),
        (Class::Priest, "priest", 7),
        (Class::Engineer, "engineer", 7),
    ]
}

fn hero_race_file(race: Race) -> &'static str {
    match race {
        Race::Human => "human",
        Race::Elf => "elf",
        Race::Orc => "orc",
    }
}

/// Startup: load the per-class walk sheets into a `HeroWalks` resource.
pub fn load_hero_walks(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let mut class_walks = std::collections::HashMap::new();
    for (class, file, frames) in hero_walk_mapping() {
        let image = assets.load(format!("heroes_walk/{}.webp", file));
        let layout = layouts.add(TextureAtlasLayout::from_grid(
            UVec2::splat(128),
            *frames as u32,
            1,
            None,
            None,
        ));
        class_walks.insert(
            *class,
            HeroWalkCfg {
                image,
                layout,
                frames: *frames,
                size: HERO_WORLD_SIZE,
            },
        );
    }

    let mut race_worlds = std::collections::HashMap::new();
    for class in Class::ALL {
        for race in Race::ALL {
            let image = assets.load(format!(
                "heroes_world/{}_{}.webp",
                class.sprite_name(),
                hero_race_file(race)
            ));
            let layout = layouts.add(TextureAtlasLayout::from_grid(
                UVec2::splat(192),
                4,
                4,
                None,
                None,
            ));
            race_worlds.insert(
                (class, race),
                HeroWalkCfg {
                    image,
                    layout,
                    frames: 16,
                    size: TILE_SIZE * 1.12,
                },
            );
        }
    }

    commands.insert_resource(HeroWalks {
        class_walks,
        race_worlds,
    });
}

fn hero_world_cfg(walks: &HeroWalks, class: Class, race: Race) -> Option<&HeroWalkCfg> {
    walks
        .race_worlds
        .get(&(class, race))
        .or_else(|| walks.class_walks.get(&class))
}

/// Advance the hero's walk/attack cycle. Frame 0 is idle; movement cycles the walk
/// frames, while melee attacks briefly hold generated sword-out frames from the
/// same sheet so the hero body appears to swing.
pub fn animate_hero_walk(
    time: Res<Time>,
    mut q: Query<(&Tower, &mut HeroWalkAnim, &mut Sprite)>,
    mut last: Local<Vec2>,
) {
    for (t, mut a, mut sprite) in &mut q {
        if !t.hero {
            continue;
        }
        let pos = t.center();
        let moving = pos.distance(*last) > 0.4;
        *last = pos;
        let Some(atlas) = &mut sprite.texture_atlas else {
            continue;
        };
        if t.hero_attack_timer > 0.0 && a.frames > 2 {
            let progress = 1.0 - (t.hero_attack_timer / HERO_MELEE_ATTACK_TIME).clamp(0.0, 1.0);
            atlas.index = if progress < 0.42 {
                2.min(a.frames - 1)
            } else if progress < 0.82 {
                (a.frames - 1).max(1)
            } else {
                1.min(a.frames - 1)
            };
        } else if moving && a.frames > 1 {
            a.timer.tick(time.delta());
            if a.timer.just_finished() {
                // Cycle frames 1..=frames-1 (frame 0 is the idle pose).
                atlas.index = 1 + (atlas.index % (a.frames - 1));
            }
        } else {
            atlas.index = 0;
        }
    }
}

pub fn spawn_hero(
    commands: &mut Commands,
    tower: Tower,
    sprites: &Sprites,
    walks: &HeroWalks,
    class: Class,
    race: Race,
) {
    let pos = tower.hero_pos;
    let tint = tower.color;
    let mut ec = commands.spawn((
        tower,
        Transform::from_translation(pos.extend(5.0)),
        LevelEntity,
    ));
    if let Some(cfg) = hero_world_cfg(walks, class, race) {
        // Race-specific world atlas first; old class walk strips are only fallback.
        let mut sprite = Sprite::from_atlas_image(
            cfg.image.clone(),
            TextureAtlas {
                layout: cfg.layout.clone(),
                index: 0,
            },
        );
        sprite.color = tint;
        sprite.custom_size = Some(Vec2::splat(cfg.size));
        ec.insert((
            sprite,
            HeroWalkAnim {
                timer: Timer::from_seconds(0.10, TimerMode::Repeating),
                frames: cfg.frames,
            },
        ));
    } else {
        // Fallback: static portrait (classes without a walk sheet yet).
        ec.insert(Sprite {
            image: sprites.heroes[&class].clone(),
            color: tint,
            custom_size: Some(Vec2::splat(HERO_WORLD_SIZE)),
            ..default()
        });
    }
    let hero_entity = ec.id();

    commands.spawn((
        Sprite {
            image: sprites.races[&race].clone(),
            color: Color::WHITE,
            custom_size: Some(Vec2::splat(TILE_SIZE * 0.34)),
            ..default()
        },
        Transform::from_translation(
            (pos + Vec2::new(TILE_SIZE * 0.36, -TILE_SIZE * 0.34)).extend(7.1),
        ),
        HeroRaceBadge {
            owner: hero_entity,
            offset: Vec2::new(TILE_SIZE * 0.36, -TILE_SIZE * 0.34),
        },
        LevelEntity,
    ));

    let bar_w = TILE_SIZE * 0.9;
    let bar_h = 5.0;
    let bar_y = TILE_SIZE * 0.62 + 5.0;
    commands.spawn((
        Sprite {
            color: Color::srgb(0.09, 0.04, 0.04),
            custom_size: Some(Vec2::new(bar_w, bar_h)),
            ..default()
        },
        Transform::from_translation((pos + Vec2::new(0.0, bar_y)).extend(6.2)),
        TowerHpBar {
            owner: hero_entity,
            width: bar_w,
            offset_y: bar_y,
            foreground: false,
        },
        LevelEntity,
    ));
    commands.spawn((
        Sprite {
            color: Color::srgb(0.35, 0.95, 0.38),
            custom_size: Some(Vec2::new(bar_w, bar_h)),
            ..default()
        },
        Anchor::CENTER_LEFT,
        Transform::from_translation((pos + Vec2::new(-bar_w / 2.0, bar_y)).extend(6.3)),
        TowerHpBar {
            owner: hero_entity,
            width: bar_w,
            offset_y: bar_y,
            foreground: true,
        },
        LevelEntity,
    ));
}

/// Walk the hero toward its commanded `move_target` each frame.
pub fn hero_move(
    time: Res<Time>,
    run: Res<RunState>,
    loadout: Res<HeroLoadout>,
    joystick: Res<crate::ui::JoystickState>,
    mut q: Query<&mut Tower>,
) {
    let dt = time.delta_secs() * run.game_speed;
    let speed = hero_move_speed(&loadout);
    for mut t in &mut q {
        if !t.hero {
            continue;
        }
        if joystick.dir.length_squared() > 0.0025 {
            let step = joystick.dir.clamp_length_max(1.0) * speed * dt;
            t.hero_pos += step;
            t.hero_pos.x = t.hero_pos.x.clamp(-BOARD_W * 0.48, BOARD_W * 0.48);
            t.hero_pos.y = t.hero_pos.y.clamp(-BOARD_H * 0.48, BOARD_H * 0.48);
            t.move_target = None;
            t.angle = joystick.dir.to_angle();
            continue;
        }
        let Some(target) = t.move_target else {
            continue;
        };
        let delta = target - t.hero_pos;
        let dist = delta.length();
        let step = speed * dt;
        if dist <= step || dist < 1.0 {
            t.hero_pos = target;
            t.move_target = None;
        } else {
            t.hero_pos += delta / dist * step;
            t.angle = delta.to_angle();
        }
    }
}

/// Motion trail: while the hero moves, periodically spawn a fading, tinted copy of
/// its sprite (afterimage / 身影遁形). This sells movement — without it the hero looks
/// like it's sliding/floating — and reads as a dash especially for the assassin.
/// Reuses `Particle` (fades alpha by life/max_life, so starting life<max_life makes
/// the ghost start translucent) and `update_particles` for the fade + despawn.
pub fn hero_afterimage(
    time: Res<Time>,
    run: Res<RunState>,
    loadout: Res<HeroLoadout>,
    sprites: Res<Sprites>,
    walks: Res<HeroWalks>,
    heroes: Query<&Tower>,
    mut commands: Commands,
    mut last: Local<Vec2>,
    mut acc: Local<f32>,
) {
    let Some(t) = heroes.iter().find(|t| t.hero && t.hp > 0.0) else {
        return;
    };
    let pos = t.center();
    let moving = pos.distance(*last) > 0.6;
    *last = pos;
    if !moving {
        *acc = 0.0;
        return;
    }
    *acc += time.delta_secs() * run.game_speed;
    if *acc < 0.05 {
        return;
    }
    *acc = 0.0;
    // Match the hero's left/right facing so the ghost isn't mirrored wrong.
    let flip = if t.angle.cos() < 0.0 { -1.0 } else { 1.0 };
    let mut sprite = if let Some(cfg) = hero_world_cfg(&walks, loadout.class, loadout.race) {
        let mut sprite = Sprite::from_atlas_image(
            cfg.image.clone(),
            TextureAtlas {
                layout: cfg.layout.clone(),
                index: 0,
            },
        );
        sprite.custom_size = Some(Vec2::splat(cfg.size * (HERO_GHOST_SIZE / HERO_WORLD_SIZE)));
        sprite
    } else {
        Sprite {
            image: sprites.heroes[&loadout.class].clone(),
            custom_size: Some(Vec2::splat(HERO_GHOST_SIZE)),
            ..default()
        }
    };
    sprite.color = loadout
        .race
        .color()
        .mix(&loadout.class.skill_color(), 0.35)
        .with_alpha(0.58);
    commands.spawn((
        sprite,
        Transform::from_translation(pos.extend(4.6)).with_scale(Vec3::new(flip, 1.0, 1.0)),
        // life < max_life → starts at ~0.5 alpha, fades to 0 over `life` seconds.
        Particle {
            vel: Vec2::ZERO,
            life: 0.3,
            max_life: 0.6,
        },
        LevelEntity,
    ));
}

/// Tap/click handling for the hero: tapping near it selects it; with the hero
/// selected, tapping open ground commands it to walk there. Building takes
/// priority (handled in `mouse_build`), and taps over UI are ignored.
pub fn hero_control(
    mode: Res<crate::ui::TouchMode>,
    buttons: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    windows: Query<&Window>,
    camera: Query<(&Camera, &GlobalTransform)>,
    mut sel: ResMut<Selection>,
    ui_buttons: Query<&Interaction>,
    mut heroes: Query<(Entity, &mut Tower)>,
) {
    if sel.build_kind.is_some() {
        return; // a tower is armed — that gesture is for building
    }
    let over_ui = ui_buttons.iter().any(|i| !matches!(*i, Interaction::None));
    let Ok((cam, cam_tf)) = camera.single() else {
        return;
    };
    let Ok(win) = windows.single() else {
        return;
    };
    // Mouse → world (desktop has no on-screen joystick, so clicks are never gated).
    let raw = |screen: Vec2| -> Option<Vec2> { cam.viewport_to_world_2d(cam_tf, screen).ok() };
    // Touch → world, ignoring taps in the floating-joystick movement zone.
    let to_world_touch = |screen: Vec2| -> Option<Vec2> {
        if crate::ui::in_joystick(screen, win) {
            return None;
        }
        cam.viewport_to_world_2d(cam_tf, screen).ok()
    };
    let near_hero = |world: Vec2, hero: &Tower| world.distance(hero.hero_pos) <= TILE_SIZE * 0.7;

    let Some((hero_e, mut hero)) = heroes.iter_mut().find(|(_, t)| t.hero) else {
        return;
    };

    // Desktop, Warcraft-style: LEFT-click selects the hero, RIGHT-click commands a
    // move while it is selected. (Mouse is never blocked by the joystick zone.)
    if buttons.just_pressed(MouseButton::Left) && !over_ui {
        if let Some(world) = win.cursor_position().and_then(raw) {
            if near_hero(world, &hero) {
                sel.selected = Some(hero_e);
            }
        }
    }
    if buttons.just_pressed(MouseButton::Right) && !over_ui && sel.selected == Some(hero_e) {
        if let Some(world) = win.cursor_position().and_then(raw) {
            hero.move_target = Some(world);
        }
    }

    // Touch (no right button): tap the hero to select; once selected, tap open
    // ground to move there. Gated on TouchMode so it only applies in mobile mode.
    if mode.0 {
        if let Some(t) = touches.iter_just_released().next() {
            if let Some(world) = to_world_touch(t.position()) {
                if near_hero(world, &hero) {
                    sel.selected = Some(hero_e);
                } else if sel.selected == Some(hero_e) {
                    hero.move_target = Some(world);
                }
            }
        }
    }
}

/// Detect hero death (its entity vanished while marked alive) and tick the
/// respawn cooldown down once per wave.
pub fn hero_status(
    mut loadout: ResMut<HeroLoadout>,
    mut heroes: Query<&mut Tower>,
    mut run: ResMut<RunState>,
    mut vfx: MessageWriter<crate::vfx::VfxEvent>,
    mut last_wave: Local<i32>,
) {
    if run.wave > *last_wave {
        if loadout.respawn_waves > 0 {
            loadout.respawn_waves -= 1;
        }
        if loadout.alive && heroes.iter().any(|t| t.hero) {
            loadout.tick_wave_cooldowns();
            let xp = 14 + run.wave.max(1) * 3;
            let gained = loadout.gain_xp(xp);
            if gained > 0 {
                for mut hero in &mut heroes {
                    if hero.hero {
                        crate::hero::apply_loadout_to_tower(&loadout, &mut hero);
                        vfx.write(crate::vfx::VfxEvent::Burst {
                            pos: hero.center(),
                            radius: 72.0,
                            color: loadout.class.skill_color(),
                        });
                    }
                }
                run.show(crate::i18n::tf(
                    "英雄升级至 Lv{}，获得 {} 点天赋点",
                    &[&loadout.level.to_string(), &gained.to_string()],
                ));
            }
        }
    }
    *last_wave = run.wave;

    let exists = heroes.iter().any(|t| t.hero);
    if loadout.alive && !exists {
        loadout.alive = false;
        loadout.respawn_waves = 2;
        run.show(crate::i18n::t("英雄阵亡！2 回合后可再次召唤"));
    }
}

/// Reset hero run-state when (re)entering a level — the old hero entity is despawned
/// with the level, so allow a fresh summon.
/// Auto-spawn the hero at the start of a level (no summon, no gold cost). The hero
/// is free and always present now; its stats are buffed to compensate (see
/// `apply_loadout_to_tower`).
pub fn auto_spawn_hero(
    mut commands: Commands,
    mut loadout: ResMut<HeroLoadout>,
    sprites: Res<Sprites>,
    walks: Res<HeroWalks>,
) {
    loadout.skill_cd = 0;
    loadout.respawn_waves = 0;
    let pos = crate::hero::hero_spawn_pos();
    let tower = crate::hero::make_hero_tower(&loadout, pos);
    spawn_hero(
        &mut commands,
        tower,
        &sprites,
        &walks,
        loadout.class,
        loadout.race,
    );
    loadout.alive = true;
}

/// Auto-respawn the hero a couple of waves after it dies (no summon button needed).
pub fn hero_respawn(
    mut commands: Commands,
    mut loadout: ResMut<HeroLoadout>,
    sprites: Res<Sprites>,
    walks: Res<HeroWalks>,
    heroes: Query<&Tower>,
) {
    if loadout.alive || loadout.respawn_waves > 0 {
        return;
    }
    if heroes.iter().any(|t| t.hero) {
        return; // already on the field
    }
    let pos = crate::hero::hero_spawn_pos();
    let tower = crate::hero::make_hero_tower(&loadout, pos);
    spawn_hero(
        &mut commands,
        tower,
        &sprites,
        &walks,
        loadout.class,
        loadout.race,
    );
    loadout.alive = true;
}

pub fn update_tower_hp_bars(
    mut commands: Commands,
    towers: Query<&Tower>,
    mut bars: Query<(Entity, &TowerHpBar, &mut Transform, &mut Sprite)>,
) {
    for (entity, bar, mut tf, mut sprite) in &mut bars {
        let Ok(tower) = towers.get(bar.owner) else {
            commands.entity(entity).despawn();
            continue;
        };
        let c = tower.center();
        tf.translation.y = c.y + bar.offset_y;
        if bar.foreground {
            let frac = if tower.max_hp > 0.0 {
                (tower.hp / tower.max_hp).clamp(0.0, 1.0)
            } else {
                0.0
            };
            tf.translation.x = c.x - bar.width / 2.0;
            tf.translation.z = 6.3;
            tf.scale.x = frac;
            sprite.color = if frac <= 0.3 {
                Color::srgb(1.0, 0.16, 0.1)
            } else if frac <= 0.6 {
                Color::srgb(1.0, 0.74, 0.18)
            } else {
                Color::srgb(0.35, 0.95, 0.38)
            };
        } else {
            tf.translation.x = c.x;
            tf.translation.z = 6.2;
            tf.scale.x = 1.0;
        }
    }
}

/// Keep tower/hero sprites upright (no spinning) and animate the hero's walk.
/// Towers face their target via projectiles, not by rotating their sprite. The hero
/// flips left/right to face its travel direction and bobs while moving (a stand-in
/// for a full walk-cycle sprite sheet) instead of rotating.
pub fn rotate_towers(
    time: Res<Time>,
    mut towers: Query<(&mut Tower, &mut Transform)>,
    mut last_hero: Local<Vec2>,
) {
    let dt = time.delta_secs();
    for (mut t, mut tf) in &mut towers {
        tf.rotation = Quat::IDENTITY; // never spin
        if t.hero {
            let c = t.center();
            let moving = c.distance(*last_hero) > 0.4;
            *last_hero = c;
            // Face the travel direction by mirroring horizontally (no rotation).
            let facing = t.angle.cos();
            let current_sign = if tf.scale.x < 0.0 { -1.0 } else { 1.0 };
            let facing_sign = if facing.abs() > 0.05 {
                if facing < 0.0 { -1.0 } else { 1.0 }
            } else {
                current_sign
            };
            let attack_progress = if t.hero_attack_timer > 0.0 {
                1.0 - (t.hero_attack_timer / HERO_MELEE_ATTACK_TIME).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let attack_pop = if t.hero_attack_timer > 0.0 {
                (attack_progress * std::f32::consts::PI).sin().max(0.0)
            } else {
                0.0
            };
            tf.scale.x = facing_sign * (1.0 + attack_pop * 0.10);
            tf.scale.y = 1.0 - attack_pop * 0.035;
            // Bouncier walk bob while moving so the hero clearly strides, not floats.
            let bob = if moving {
                (time.elapsed_secs() * 16.0).sin().abs() * 5.5
            } else {
                0.0
            };
            let attack_step = Vec2::from_angle(t.angle) * (attack_pop * 5.0);
            t.recoil *= (1.0 - 16.0 * dt).clamp(0.0, 1.0);
            if t.recoil.length_squared() < 0.05 {
                t.recoil = Vec2::ZERO;
            }
            if t.hero_attack_timer > 0.0 {
                t.hero_attack_timer = (t.hero_attack_timer - dt).max(0.0);
            }
            tf.translation.x = c.x + t.recoil.x + attack_step.x;
            tf.translation.y = c.y + t.recoil.y + attack_step.y + bob;
        } else if t.recoil != Vec2::ZERO {
            // Grid towers move only via transient muzzle recoil.
            let c = t.center();
            tf.translation.x = c.x + t.recoil.x;
            tf.translation.y = c.y + t.recoil.y;
            t.recoil *= (1.0 - 16.0 * dt).clamp(0.0, 1.0);
            if t.recoil.length_squared() < 0.05 {
                t.recoil = Vec2::ZERO;
                tf.translation.x = c.x;
                tf.translation.y = c.y;
            }
        }
    }
}

pub fn update_hero_race_badges(
    mut commands: Commands,
    towers: Query<&Tower>,
    mut badges: Query<(Entity, &HeroRaceBadge, &mut Transform)>,
) {
    for (entity, badge, mut tf) in &mut badges {
        let Ok(hero) = towers.get(badge.owner) else {
            commands.entity(entity).despawn();
            continue;
        };
        if !hero.hero {
            commands.entity(entity).despawn();
            continue;
        }
        let pos = hero.center() + badge.offset;
        tf.translation.x = pos.x;
        tf.translation.y = pos.y;
    }
}

pub fn tint_silenced_towers(
    snap: Res<crate::tower::Snapshot>,
    mut towers: Query<(&Tower, &mut Sprite)>,
) {
    for (tower, mut sprite) in &mut towers {
        sprite.color = if snap.tower_silenced(tower.center()) {
            Color::srgb(0.78, 0.48, 1.0)
        } else {
            Color::WHITE
        };
    }
}

pub fn repair_tower(t: &mut Tower, run: &mut RunState) -> bool {
    let cost = t.repair_cost();
    if cost == 0 {
        run.show(crate::i18n::t("防御塔无需修理"));
        return false;
    }
    if run.gold < cost {
        run.show(crate::i18n::tf("修理需要 {} 金", &[&cost.to_string()]));
        return false;
    }
    run.gold -= cost;
    t.hp = t.max_hp;
    t.low_hp_warned = false;
    t.siege_vfx_timer = 0.0;
    run.show(crate::i18n::tf("修理完成 -{} 金", &[&cost.to_string()]));
    true
}

fn select_most_damaged_tower(
    sel: &mut Selection,
    run: &mut RunState,
    towers: &Query<(Entity, &mut Tower)>,
) -> bool {
    let target = towers
        .iter()
        .filter(|(_, tower)| tower.max_hp > 0.0 && tower.hp < tower.max_hp - 0.5)
        .min_by(|(_, a), (_, b)| {
            let af = a.hp / a.max_hp;
            let bf = b.hp / b.max_hp;
            af.total_cmp(&bf)
        })
        .map(|(entity, tower)| (entity, tower.kind.def().name, tower.repair_cost()));

    if let Some((entity, name, cost)) = target {
        sel.selected = Some(entity);
        sel.build_kind = None;
        run.show(crate::i18n::tf(
            "已选中受损防御塔：{}，修理 {}",
            &[&crate::i18n::t(name), &cost.to_string()],
        ));
        true
    } else {
        run.show(crate::i18n::t("没有受损防御塔"));
        false
    }
}

/// U = upgrade selected tower, R = repair it, T = target mode, Z = unequip, X = sell it, Tab = select damaged tower.
pub fn upgrade_sell(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut run: ResMut<RunState>,
    mut sel: ResMut<Selection>,
    mut inv: ResMut<EquipmentInventory>,
    hero: Res<crate::hero::HeroLoadout>,
    mut towers: Query<(Entity, &mut Tower)>,
) {
    if keys.just_pressed(KeyCode::Tab) {
        select_most_damaged_tower(&mut sel, &mut run, &towers);
        return;
    }

    let Some(entity) = sel.selected else {
        return;
    };

    if keys.just_pressed(KeyCode::KeyU) {
        if let Ok((_, mut t)) = towers.get_mut(entity) {
            if t.hero {
                run.show(crate::i18n::t("英雄通过经验升级；请使用英雄天赋点强化"));
                return;
            }
            let cost = t.upgrade_cost();
            if t.level < 3 && run.gold >= cost {
                run.gold -= cost;
                upgrade_tower(&mut t);
                if let Some(note) = upgrade_unlock_note(t.kind, t.level) {
                    run.show(crate::i18n::t(note));
                }
            }
        }
    }

    if keys.just_pressed(KeyCode::KeyR) {
        if let Ok((_, mut t)) = towers.get_mut(entity) {
            repair_tower(&mut t, &mut run);
        }
    }

    if keys.just_pressed(KeyCode::KeyT) {
        if let Ok((_, mut t)) = towers.get_mut(entity) {
            let priority = t.cycle_target_priority();
            run.show(crate::i18n::tf(
                "目标优先：{} - {}",
                &[
                    &crate::i18n::t(priority.label()),
                    &crate::i18n::t(priority.description()),
                ],
            ));
        }
    }

    if keys.just_pressed(KeyCode::KeyZ) {
        if let Ok((_, mut t)) = towers.get_mut(entity) {
            let returned = unequip_all_to_inventory(&mut inv, &mut t);
            if returned > 0 && t.hero {
                crate::hero::apply_loadout_to_tower(&hero, &mut t);
            }
            if returned > 0 {
                run.show(crate::i18n::tf("卸下装备 {} 件", &[&returned.to_string()]));
            } else {
                run.show(crate::i18n::t("没有可卸下装备"));
            }
        }
    }

    if keys.just_pressed(KeyCode::KeyX) {
        if let Ok((e, t)) = towers.get(entity) {
            if t.hero {
                run.show(crate::i18n::t("英雄不能出售；阵亡后会自动进入重生冷却"));
                return;
            }
            let refund = t.refund();
            let returned = return_equipment_to_inventory(&mut inv, t);
            run.gold += refund;
            commands.entity(e).despawn();
            sel.selected = None;
            if returned > 0 {
                run.show(crate::i18n::tf(
                    "出售 +{}，返还装备 {} 件",
                    &[&refund.to_string(), &returned.to_string()],
                ));
            }
        }
    }
}

/// Apply one upgrade level (original `doUpgradeTower` multipliers).
pub fn upgrade_tower(t: &mut Tower) {
    use crate::data::UpgradeMul as M;
    t.level += 1;
    t.base_damage = (t.base_damage * M::DAMAGE).floor();
    t.range = (t.range * M::RANGE).floor();
    t.cooldown = (t.cooldown * M::COOLDOWN).max(0.001);
    let old_hp = t.max_hp;
    t.max_hp = (t.max_hp * 1.22).floor();
    t.hp += t.max_hp - old_hp;
    if t.max_hp > 0.0 && t.hp / t.max_hp > 0.45 {
        t.low_hp_warned = false;
    }
    t.armor += 2.0;
    t.armor_pierce += 2.0;
    if t.aoe_radius > 0.0 {
        t.aoe_radius = (t.aoe_radius * M::AOE_RADIUS).floor();
    }
    if t.dot_damage > 0.0 {
        t.dot_damage = (t.dot_damage * M::DOT_DAMAGE).floor();
    }
    if t.heal_amount > 0.0 {
        t.heal_amount = (t.heal_amount * M::HEAL_AMOUNT).floor();
    }
    if t.summon_hp > 0.0 {
        t.summon_hp = (t.summon_hp * M::SUMMON_HP).floor();
    }
    if t.chain_count > 0 {
        t.chain_count += 1;
    }
    // Behavior-specific unlocks that escalate with level.
    if t.behavior == crate::data::Behavior::Summon {
        // Each level raises the summon cap; the minion *tier* is chosen at spawn
        // time from `level` in `update_towers` (skeleton → fireworm → mimic).
        t.max_summons += 1;
    }
    // Necromancer raise-count and minion strength are derived from `level` in
    // `necromancer_raise`, so leveling up alone unlocks "+1 revive".
}

/// One-line description of the new ability unlocked at `new_level` (for the HUD).
pub fn upgrade_unlock_note(kind: TowerKind, new_level: i32) -> Option<&'static str> {
    use crate::data::Behavior;
    match (kind.def().behavior, new_level) {
        (Behavior::Summon, 2) => Some("解锁：召唤冲锋骷髅，召唤上限 +1"),
        (Behavior::Summon, 3) => Some("解锁：召唤重装巨像，召唤上限 +1，攻击大增"),
        (Behavior::Necromancer, 2) => Some("解锁：每次可复活 2 个亡灵，亡灵更强"),
        (Behavior::Necromancer, 3) => Some("解锁：每次可复活 3 个亡灵，亡灵更强"),
        (Behavior::Chain, 2) | (Behavior::Chain, 3) => Some("解锁：闪电额外多弹射 1 个目标"),
        _ => None,
    }
}

/// Draw the selected tower's range, and — when building — a highlight square on
/// the hovered cell (green = can build here, red = blocked) plus the range preview.
pub fn draw_range_gizmos(
    mut gizmos: Gizmos,
    time: Res<Time>,
    sel: Res<Selection>,
    board: Res<Board>,
    run: Res<RunState>,
    towers: Query<&Tower>,
    windows: Query<&Window>,
    camera: Query<(&Camera, &GlobalTransform)>,
) {
    if let Some(e) = sel.selected {
        if let Ok(t) = towers.get(e) {
            // Show the effective range (includes a Warden hero's range aura).
            let r = t.range * (1.0 + t.aura_range);
            gizmos.circle_2d(t.center(), r, Color::WHITE.with_alpha(0.4));
            if t.aura_range > 0.0 {
                gizmos.circle_2d(t.center(), t.range, Color::WHITE.with_alpha(0.15));
            }
            // Pulsing footprint highlight so the selected tower is obvious.
            let pulse = (time.elapsed_secs() * 4.0).sin() * 0.5 + 0.5;
            let block = TILE_SIZE * t.footprint as f32;
            let col = Color::srgb(1.0, 0.92, 0.4).with_alpha(0.5 + 0.4 * pulse);
            gizmos.rect_2d(
                Isometry2d::from_translation(t.center()),
                Vec2::splat(block - 2.0),
                col,
            );
            gizmos.rect_2d(
                Isometry2d::from_translation(t.center()),
                Vec2::splat(block - 6.0 - 3.0 * pulse),
                col,
            );
        }
    }

    if let Some(kind) = sel.build_kind {
        // Touch arms a `preview_cell`; desktop tracks the live cursor cell.
        let Some((col, row)) = sel
            .preview_cell
            .or_else(|| cursor_world(&windows, &camera).and_then(world_to_cell))
        else {
            return;
        };
        let def = kind.def();
        let fp = def.footprint.max(1);
        let off = (fp - 1) as f32 / 2.0;
        let c = cell_center(col as f32 + off, row as f32 + off);
        let block = TILE_SIZE * fp as f32;

        let can_build =
            footprint_buildable(&board, towers.iter(), kind, col, row) && run.gold >= def.cost;
        let color = if can_build {
            Color::srgb(0.3, 1.0, 0.4)
        } else {
            Color::srgb(1.0, 0.3, 0.3)
        };

        // Bold-ish highlight over the whole footprint (two nested squares).
        gizmos.rect_2d(Isometry2d::from_translation(c), Vec2::splat(block), color);
        gizmos.rect_2d(
            Isometry2d::from_translation(c),
            Vec2::splat(block - 4.0),
            color,
        );
        // Range preview, tinted to match.
        gizmos.circle_2d(c, def.range, color.with_alpha(0.25));
    }
}
