//! 新手引导 (first-play tutorial). A scripted, step-by-step overlay shown the very
//! first time a player enters level 1, guiding them through the core loop: arm a
//! tower, place it, start the wave, then hand off to free play. Persists a
//! `tutorial_done` flag so it only ever runs once (skippable any time).
//!
//! Kept fully self-contained: its own marker components, its own tiny text/color
//! helpers, and a private button handler — so it never touches the large
//! `UiAction`/`hud_buttons` machinery in `ui.rs`.

use bevy::prelude::*;

use crate::build::Selection;
use crate::data::cell_center;
use crate::game::{CurrentLevel, GameMode, RunMode, RunState};
use crate::tower::Tower;
use crate::ui::UiFont;

/// What the player must do to advance from a step.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Trigger {
    /// Wait for the player to press the 继续/完成 button.
    Continue,
    /// Wait until a tower kind is armed (`Selection.build_kind`).
    SelectTower,
    /// Wait until at least one tower has been placed.
    BuildTower,
    /// Wait until the first wave has been started.
    StartWave,
}

struct Step {
    /// Instruction text (Chinese key; translated via i18n at display time).
    zh: &'static str,
    trigger: Trigger,
    /// Highlight the suggested build cell with a pulsing ring.
    ring: bool,
}

/// The scripted sequence. Level 1 only, so the suggested build cell is fixed.
const STEPS: &[Step] = &[
    Step {
        zh: "欢迎来到保卫萝卜！🥕\n敌人会沿着道路前进，别让它们碰到萝卜。",
        trigger: Trigger::Continue,
        ring: false,
    },
    Step {
        zh: "先造一座防御塔。\n点击塔栏里的【箭塔】图标，选中它。",
        trigger: Trigger::SelectTower,
        ring: false,
    },
    Step {
        zh: "在高亮的格子上点一下，把箭塔造在路边。",
        trigger: Trigger::BuildTower,
        ring: true,
    },
    Step {
        zh: "干得漂亮！塔会自动攻击范围内的敌人。\n现在点击【开始】按钮，放出第一波敌人。",
        trigger: Trigger::StartWave,
        ring: false,
    },
    Step {
        zh: "击败敌人会掉落金币💰，\n用来建造更多塔，或点击已有的塔进行升级。",
        trigger: Trigger::Continue,
        ring: false,
    },
    Step {
        zh: "教程结束，守住萝卜，开启你的冒险吧！加油！",
        trigger: Trigger::Continue,
        ring: false,
    },
];

/// Suggested first build cell on level 1 (buildable, hugs the opening path corner).
const HINT_CELL: (i32, i32) = (2, 3);

/// Tutorial runtime state.
#[derive(Resource, Default)]
pub struct Tutorial {
    pub active: bool,
    step: usize,
    /// Set whenever `step`/`active` changes so the panel refreshes once.
    dirty: bool,
}

/// Root node of the instruction banner (despawned on tutorial end / level exit).
#[derive(Component)]
pub struct TutorialRoot;

#[derive(Component)]
pub struct TutorialText;

#[derive(Component)]
pub struct TutorialNextBtn;

#[derive(Component)]
pub enum TutorialBtn {
    Next,
    Skip,
}

fn tut_font(f: &Handle<Font>, size: f32) -> TextFont {
    TextFont {
        font: FontSource::Handle(f.clone()),
        font_size: FontSize::Px(size),
        ..default()
    }
}

const PANEL_BG: Color = Color::srgba(0.04, 0.06, 0.05, 0.95);
const ACCENT: Color = Color::srgb(0.96, 0.72, 0.28);
const TEXT: Color = Color::srgb(0.92, 0.94, 0.88);

/// On entering Playing: start the tutorial iff it's the first campaign run of
/// level 1 and the player hasn't completed/skipped it before.
pub fn maybe_start_tutorial(
    mut commands: Commands,
    mut tut: ResMut<Tutorial>,
    current: Res<CurrentLevel>,
    mode: Res<GameMode>,
    fonts: Res<UiFont>,
) {
    let first_level = current.0 == 0 && mode.0 == RunMode::Campaign;
    if !first_level || load_tutorial_done() {
        tut.active = false;
        return;
    }
    tut.active = true;
    tut.step = 0;
    tut.dirty = true;
    spawn_panel(&mut commands, &fonts.0);
}

fn spawn_panel(commands: &mut Commands, font: &Handle<Font>) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(14.0),
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                ..default()
            },
            GlobalZIndex(70),
            TutorialRoot,
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    max_width: Val::Px(560.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::axes(Val::Px(22.0), Val::Px(14.0)),
                    row_gap: Val::Px(10.0),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(PANEL_BG),
                BorderColor::all(ACCENT),
            ))
            .with_children(|p| {
                p.spawn((
                    Text::new(""),
                    tut_font(font, 19.0),
                    TextColor(TEXT),
                    TextLayout::justify(Justify::Center),
                    TutorialText,
                ));
                p.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(12.0),
                    ..default()
                })
                .with_children(|row| {
                    // 继续/完成 — only shown for Continue-type steps.
                    row.spawn((
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(20.0), Val::Px(9.0)),
                            border: UiRect::all(Val::Px(2.0)),
                            ..default()
                        },
                        BackgroundColor(ACCENT),
                        BorderColor::all(ACCENT),
                        TutorialBtn::Next,
                        TutorialNextBtn,
                    ))
                    .with_children(|b| {
                        b.spawn((
                            Text::new(crate::i18n::t("继续")),
                            tut_font(font, 17.0),
                            TextColor(Color::srgb(0.08, 0.1, 0.07)),
                        ));
                    });
                    // 跳过教程 — always available.
                    row.spawn((
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(16.0), Val::Px(9.0)),
                            border: UiRect::all(Val::Px(2.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
                        BorderColor::all(Color::srgba(0.6, 0.62, 0.58, 0.6)),
                        TutorialBtn::Skip,
                    ))
                    .with_children(|b| {
                        b.spawn((
                            Text::new(crate::i18n::t("跳过教程")),
                            tut_font(font, 15.0),
                            TextColor(Color::srgb(0.7, 0.72, 0.68)),
                        ));
                    });
                });
            });
        });
}

/// Auto-advance the current step when its (non-button) trigger condition is met.
pub fn watch_tutorial(
    mut tut: ResMut<Tutorial>,
    sel: Res<Selection>,
    run: Res<RunState>,
    towers: Query<(), With<Tower>>,
) {
    if !tut.active {
        return;
    }
    let Some(step) = STEPS.get(tut.step) else {
        return;
    };
    let met = match step.trigger {
        Trigger::Continue => false, // advanced by the 继续 button
        Trigger::SelectTower => sel.build_kind.is_some(),
        Trigger::BuildTower => towers.iter().next().is_some(),
        Trigger::StartWave => run.wave >= 1,
    };
    if met {
        advance(&mut tut);
    }
}

/// Handle the 继续/完成 and 跳过 buttons.
pub fn tutorial_buttons(
    mut commands: Commands,
    mut tut: ResMut<Tutorial>,
    interactions: Query<(&Interaction, &TutorialBtn), Changed<Interaction>>,
    root: Query<Entity, With<TutorialRoot>>,
) {
    for (interaction, btn) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match btn {
            TutorialBtn::Next => advance(&mut tut),
            TutorialBtn::Skip => finish(&mut tut),
        }
    }
    // If the tutorial just ended, tear down its panel.
    if !tut.active && tut.dirty {
        tut.dirty = false;
        for e in &root {
            commands.entity(e).despawn();
        }
    }
}

fn advance(tut: &mut Tutorial) {
    tut.step += 1;
    tut.dirty = true;
    if tut.step >= STEPS.len() {
        finish(tut);
    }
}

fn finish(tut: &mut Tutorial) {
    tut.active = false;
    tut.dirty = true; // signal the button system to despawn the panel
    save_tutorial_done(true);
}

/// Refresh the panel text + 继续 button visibility when the step changes.
pub fn refresh_panel(
    mut tut: ResMut<Tutorial>,
    mut text: Query<&mut Text, With<TutorialText>>,
    mut next_btn: Query<&mut Node, With<TutorialNextBtn>>,
) {
    if !tut.active || !tut.dirty {
        return;
    }
    let Some(step) = STEPS.get(tut.step) else {
        return;
    };
    let progress = format!("({}/{})", tut.step + 1, STEPS.len());
    if let Ok(mut t) = text.single_mut() {
        *t = Text::new(format!("{}\n{}", crate::i18n::t(step.zh), progress));
    }
    if let Ok(mut node) = next_btn.single_mut() {
        node.display = if step.trigger == Trigger::Continue {
            Display::Flex
        } else {
            Display::None
        };
    }
    tut.dirty = false;
}

/// Draw a pulsing ring over the suggested build cell during the placement step.
pub fn draw_tutorial_hint(tut: Res<Tutorial>, time: Res<Time>, mut gizmos: Gizmos) {
    if !tut.active {
        return;
    }
    let Some(step) = STEPS.get(tut.step) else {
        return;
    };
    if !step.ring {
        return;
    }
    let center = cell_center(HINT_CELL.0 as f32, HINT_CELL.1 as f32);
    let pulse = (time.elapsed_secs() * 4.0).sin() * 0.5 + 0.5; // 0..1
    let radius = 20.0 + pulse * 8.0;
    let alpha = 0.5 + pulse * 0.5;
    gizmos.circle_2d(center, radius, Color::srgba(0.96, 0.72, 0.28, alpha));
    gizmos.circle_2d(
        center,
        radius + 3.0,
        Color::srgba(0.96, 0.72, 0.28, alpha * 0.5),
    );
}

// ---- persistence (mirrors i18n::load_lang / save_lang) ----

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(inline_js = r#"
export function load_tutorial_done() {
  try { return globalThis.localStorage?.getItem('protect_carrot_tutorial_done') === '1'; }
  catch (_) { return false; }
}
export function save_tutorial_done(value) {
  try { globalThis.localStorage?.setItem('protect_carrot_tutorial_done', value ? '1' : '0'); }
  catch (_) {}
}
"#)]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = load_tutorial_done)]
    fn load_tutorial_done_js() -> bool;
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = save_tutorial_done)]
    fn save_tutorial_done_js(value: bool);
}

#[cfg(target_arch = "wasm32")]
fn load_tutorial_done() -> bool {
    load_tutorial_done_js()
}

#[cfg(target_arch = "wasm32")]
fn save_tutorial_done(value: bool) {
    save_tutorial_done_js(value);
}

#[cfg(not(target_arch = "wasm32"))]
fn load_tutorial_done() -> bool {
    std::fs::read_to_string("tmp/tutorial_done.txt")
        .map(|s| s.trim() == "1")
        .unwrap_or(false)
}

#[cfg(not(target_arch = "wasm32"))]
fn save_tutorial_done(value: bool) {
    let _ = std::fs::create_dir_all("tmp");
    let _ = std::fs::write("tmp/tutorial_done.txt", if value { "1" } else { "0" });
}
