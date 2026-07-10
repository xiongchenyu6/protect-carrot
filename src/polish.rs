//! 演出润色：章节标题过场卡 + 全局状态切换淡入。
//!
//! - 章节卡：进入每章第一关时，全屏渐显「第 N 章 · 章名 / 副标题」，
//!   按章节色调染色，约 3 秒后渐隐，不阻塞备战操作。
//! - 状态淡入：每次 GameState 切换（菜单/简报/战场/结算…）时从黑色
//!   快速淡入 0.35 秒，掩盖界面重建的突兀感。

use bevy::prelude::*;

use crate::data::{episode_of, EPISODES, EPISODE_LEN};
use crate::game::CurrentLevel;
use crate::states::GameState;
use crate::ui::UiFont;

const CARD_FADE_IN: f32 = 0.45;
const CARD_HOLD: f32 = 1.9;
const CARD_FADE_OUT: f32 = 0.85;
const STATE_FADE: f32 = 0.35;

pub struct PolishPlugin;

impl Plugin for PolishPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StateFade>()
            .add_systems(Startup, spawn_fade_overlay)
            .add_systems(OnEnter(GameState::Playing), spawn_episode_card)
            .add_systems(
                Update,
                (tick_episode_card, tick_state_fade, trigger_state_fade),
            );
    }
}

// ---------------- 章节标题卡 ----------------

#[derive(Component)]
pub struct EpisodeCard {
    timer: f32,
}

#[derive(Component)]
struct EpisodeCardText;

fn spawn_episode_card(
    mut commands: Commands,
    current: Res<CurrentLevel>,
    fonts: Res<UiFont>,
    old: Query<Entity, With<EpisodeCard>>,
) {
    for e in &old {
        commands.entity(e).despawn();
    }
    // 只在每章第一关打出全屏章节卡。
    if current.0 % EPISODE_LEN != 0 {
        return;
    }
    let ep = episode_of(current.0);
    let def = &EPISODES[ep];
    let tint = def.tint.to_srgba();
    let f = &fonts.0;
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.02, 0.04, 0.0)),
            GlobalZIndex(180),
            Pickable::IGNORE,
            EpisodeCard { timer: 0.0 },
        ))
        .with_children(|card| {
            card.spawn((
                Text::new(crate::i18n::tf(
                    "第 {} 章",
                    &[&(ep + 1).to_string()],
                )),
                TextFont {
                    font: f.clone().into(),
                    font_size: bevy::text::FontSize::Px(26.0),
                    ..default()
                },
                TextColor(Color::srgba(tint.red, tint.green, tint.blue, 0.0)),
                EpisodeCardText,
            ));
            card.spawn((
                Text::new(crate::i18n::t(def.name)),
                TextFont {
                    font: f.clone().into(),
                    font_size: bevy::text::FontSize::Px(54.0),
                    ..default()
                },
                TextColor(Color::srgba(1.0, 0.97, 0.9, 0.0)),
                EpisodeCardText,
            ));
            card.spawn((
                Text::new(crate::i18n::t(def.subtitle)),
                TextFont {
                    font: f.clone().into(),
                    font_size: bevy::text::FontSize::Px(18.0),
                    ..default()
                },
                TextColor(Color::srgba(tint.red, tint.green, tint.blue, 0.0)),
                EpisodeCardText,
            ));
        });
}

fn tick_episode_card(
    mut commands: Commands,
    time: Res<Time>,
    mut cards: Query<(Entity, &mut EpisodeCard, &mut BackgroundColor)>,
    mut texts: Query<&mut TextColor, With<EpisodeCardText>>,
) {
    let total = CARD_FADE_IN + CARD_HOLD + CARD_FADE_OUT;
    for (entity, mut card, mut bg) in &mut cards {
        card.timer += time.delta_secs();
        let t = card.timer;
        if t >= total {
            commands.entity(entity).despawn();
            continue;
        }
        // alpha 包络：淡入 → 保持 → 淡出。
        let alpha = if t < CARD_FADE_IN {
            t / CARD_FADE_IN
        } else if t < CARD_FADE_IN + CARD_HOLD {
            1.0
        } else {
            1.0 - (t - CARD_FADE_IN - CARD_HOLD) / CARD_FADE_OUT
        };
        bg.0.set_alpha(alpha * 0.66);
        for mut tc in &mut texts {
            let base = tc.0.to_srgba();
            tc.0 = Color::srgba(base.red, base.green, base.blue, alpha);
        }
    }
}

// ---------------- 全局状态切换淡入 ----------------

#[derive(Resource, Default)]
pub struct StateFade {
    remaining: f32,
}

#[derive(Component)]
struct FadeOverlay;

fn spawn_fade_overlay(mut commands: Commands) {
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
        GlobalZIndex(220),
        Pickable::IGNORE,
        FadeOverlay,
    ));
}

/// 状态一变就点亮遮罩，随后由 tick 系统淡出。
fn trigger_state_fade(
    state: Res<State<GameState>>,
    mut fade: ResMut<StateFade>,
    mut last: Local<Option<GameState>>,
) {
    let cur = *state.get();
    if *last != Some(cur) {
        if last.is_some() {
            fade.remaining = STATE_FADE;
        }
        *last = Some(cur);
    }
}

fn tick_state_fade(
    time: Res<Time>,
    mut fade: ResMut<StateFade>,
    mut overlays: Query<&mut BackgroundColor, With<FadeOverlay>>,
) {
    if fade.remaining <= 0.0 {
        return;
    }
    fade.remaining = (fade.remaining - time.delta_secs()).max(0.0);
    let alpha = (fade.remaining / STATE_FADE).clamp(0.0, 1.0);
    for mut bg in &mut overlays {
        bg.0.set_alpha(alpha * 0.9);
    }
}
