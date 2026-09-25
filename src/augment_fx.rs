//! 海克斯强化（构筑天赋三选一）的即时反馈。
//!
//! 割草类游戏里每一次拾取强化都必须"当场有感"，这里把它拆成四层，同一帧
//! 全部触发：
//! - 音效：卡牌专属音色 + 通用"强化到手"重音，双层叠加。
//! - 打击感：镜头震动 + 卡牌色全屏闪光 + 中央横幅从 1.7 倍"砸"下来回弹。
//! - 动画：英雄身上爆发光环与飘字；被这张卡强化的每座塔按离英雄远近依次
//!   亮起（涟漪扩散），让玩家一眼看到"这张卡改了谁"。
//! - 卡牌本身：三选一出现时依次弹入，悬停放大并带点击音，按下回压。
//!
//! 纯表现层：不改任何数值，数值由 `RogueliteRun::pick` 负责。

use bevy::prelude::*;

use crate::audio::{SfxEvent, Sound};
use crate::data::TowerKind;
use crate::hero::HeroLoadout;
use crate::roguelite::{RogueliteRun, RogueliteTalent};
use crate::tower::Tower;
use crate::ui::{RogueliteChoiceButton, UiFont};
use crate::vfx::{ScreenShake, VfxEvent, VfxTimeline};

/// 横幅"砸入"段（从大到略小）。
const SLAM: f32 = 0.10;
/// 回弹落定段。
const SETTLE: f32 = 0.12;
const HOLD: f32 = 1.8;
const FADE: f32 = 0.6;
const FLASH_TIME: f32 = 0.35;
/// 涟漪传播速度（世界像素/秒）。
const RIPPLE_SPEED: f32 = 700.0;
/// 塔身飘字最多几座，塔多时不刷屏。
const MAX_TOWER_TAGS: usize = 8;

/// 玩家刚拿到一张海克斯强化。由 UI 拾取处写出。
#[derive(Message, Clone, Copy)]
pub struct AugmentPicked {
    pub talent: RogueliteTalent,
    pub wave: i32,
}

#[derive(Component)]
pub struct AugmentBanner {
    t: f32,
}

/// 横幅内文字：记住基色，淡入淡出按基色缩放 alpha。
#[derive(Component)]
struct AugmentBannerText {
    base: Color,
}

#[derive(Component)]
struct AugmentFlash {
    t: f32,
    color: Color,
}

pub struct AugmentFxPlugin;

impl Plugin for AugmentFxPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<AugmentPicked>().add_systems(
            Update,
            (
                on_augment_picked,
                animate_augment_banner,
                animate_augment_flash,
                augment_card_juice,
            ),
        );
    }
}

fn on_augment_picked(
    mut commands: Commands,
    mut picks: MessageReader<AugmentPicked>,
    loadout: Res<HeroLoadout>,
    fonts: Res<UiFont>,
    towers: Query<&Tower>,
    old_banners: Query<Entity, With<AugmentBanner>>,
    mut shake: ResMut<ScreenShake>,
    mut vfx: MessageWriter<VfxEvent>,
    mut sfx: MessageWriter<SfxEvent>,
) {
    for pick in picks.read() {
        let talent = pick.talent;
        let (color, sound) = talent.feedback();

        // ---- 音效：专属音色 + 通用重音（撞音时换一个，保证两层可辨）。
        sfx.write(SfxEvent(sound));
        sfx.write(SfxEvent(if matches!(sound, Sound::Upgrade) {
            Sound::Combo
        } else {
            Sound::Upgrade
        }));

        // ---- 打击感：震屏 + 全屏闪 + 横幅（新横幅顶掉旧的，连点也不叠罗汉）。
        shake.add(0.32);
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(color.with_alpha(0.0)),
            GlobalZIndex(185),
            Pickable::IGNORE,
            AugmentFlash { t: 0.0, color },
        ));
        for e in &old_banners {
            commands.entity(e).despawn();
        }
        spawn_banner(&mut commands, &fonts.0, &loadout, talent, pick.wave, color);

        // ---- 动画：英雄爆发 + 飘字。
        let hero_pos = towers
            .iter()
            .find(|t| t.hero)
            .map(|t| t.center())
            .unwrap_or(Vec2::ZERO);
        vfx.write(VfxEvent::Burst {
            pos: hero_pos,
            radius: 96.0,
            color,
        });
        vfx.write(VfxEvent::ElementPulse {
            pos: hero_pos,
            color,
            strong: true,
        });
        vfx.write(VfxEvent::Text {
            pos: hero_pos + Vec2::new(0.0, 42.0),
            text: format!("+{}", talent.name(&loadout)),
            color,
            size: 16.0,
            life: 1.4,
        });
        match talent {
            RogueliteTalent::HumanLogistics | RogueliteTalent::CarrotDividend => {
                vfx.write(VfxEvent::GoldExplosion { center: hero_pos });
            }
            RogueliteTalent::Frostbound => {
                vfx.write(VfxEvent::FrostNova { center: hero_pos });
            }
            _ => {}
        }

        // ---- 涟漪：受这张卡影响的塔按距离依次亮起。
        let mut hit: Vec<(f32, &Tower)> = towers
            .iter()
            .filter(|t| !t.hero && talent.affects_tower(t))
            .map(|t| (t.center().distance(hero_pos), t))
            .collect();
        hit.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut events: Vec<(f32, VfxEvent)> = Vec::new();
        for (i, (dist, tower)) in hit.iter().enumerate() {
            let at = 0.12 + dist / RIPPLE_SPEED;
            let pos = tower.center();
            events.push((at, tower_accent(talent, tower.kind, pos, color)));
            if i < MAX_TOWER_TAGS {
                events.push((
                    at,
                    VfxEvent::Text {
                        pos: pos + Vec2::new(0.0, 24.0),
                        text: crate::i18n::t("强化"),
                        color,
                        size: 12.0,
                        life: 0.9,
                    },
                ));
            }
        }
        if !events.is_empty() {
            events.sort_by(|a, b| a.0.total_cmp(&b.0));
            commands.spawn(VfxTimeline { t: 0.0, events });
        }
    }
}

/// 每座被强化的塔的"应答"特效，按卡牌主题挑一个最贴切的现成特效。
fn tower_accent(talent: RogueliteTalent, kind: TowerKind, pos: Vec2, color: Color) -> VfxEvent {
    match (talent, kind) {
        (RogueliteTalent::CannonShockwave, TowerKind::Cannon) => VfxEvent::Explosion {
            pos,
            radius: 34.0,
            color,
        },
        (RogueliteTalent::ArrowVolley, TowerKind::Arrow) => VfxEvent::Muzzle {
            pos,
            dir: Vec2::Y,
            color,
        },
        _ => VfxEvent::ElementPulse {
            pos,
            color,
            strong: false,
        },
    }
}

fn spawn_banner(
    commands: &mut Commands,
    font: &Handle<Font>,
    loadout: &HeroLoadout,
    talent: RogueliteTalent,
    wave: i32,
    color: Color,
) {
    let text = |content: String, size: f32, base: Color| {
        (
            Text::new(content),
            TextFont {
                font: font.clone().into(),
                font_size: bevy::text::FontSize::Px(size),
                ..default()
            },
            TextColor(base.with_alpha(0.0)),
            AugmentBannerText { base },
        )
    };
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                top: Val::Percent(24.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            GlobalZIndex(190),
            Pickable::IGNORE,
        ))
        .with_children(|row| {
            row.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(4.0),
                    padding: UiRect::axes(Val::Px(28.0), Val::Px(12.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.03, 0.04, 0.06, 0.0)),
                BorderColor::all(color.with_alpha(0.0)),
                UiTransform::from_scale(Vec2::splat(1.7)),
                Pickable::IGNORE,
                AugmentBanner { t: 0.0 },
            ))
            .with_children(|b| {
                b.spawn(text(
                    crate::i18n::tf("{} · 海克斯强化", &[&crate::i18n::t(talent.pool().label())]),
                    13.0,
                    color,
                ));
                b.spawn(text(
                    talent.name(loadout),
                    36.0,
                    Color::srgb(1.0, 0.97, 0.9),
                ));
                b.spawn(text(
                    talent.desc(loadout, wave),
                    13.0,
                    Color::srgb(0.84, 0.90, 0.84),
                ));
            });
        });
}

fn ease_out(x: f32) -> f32 {
    1.0 - (1.0 - x).powi(3)
}

/// 横幅时间轴：砸入（1.7→0.92）→ 回弹（0.92→1.0）→ 停留 → 淡出后整行销毁。
fn animate_augment_banner(
    mut commands: Commands,
    time: Res<Time>,
    mut banners: Query<(
        Entity,
        &ChildOf,
        &mut AugmentBanner,
        &mut UiTransform,
        &mut BackgroundColor,
        &mut BorderColor,
        &Children,
    )>,
    mut texts: Query<(&AugmentBannerText, &mut TextColor)>,
) {
    for (_, parent, mut banner, mut ui_tf, mut bg, mut border, children) in &mut banners {
        banner.t += time.delta_secs();
        let t = banner.t;
        if t >= SLAM + SETTLE + HOLD + FADE {
            commands.entity(parent.parent()).despawn();
            continue;
        }
        let scale = if t < SLAM {
            1.7 + (0.92 - 1.7) * ease_out(t / SLAM)
        } else if t < SLAM + SETTLE {
            0.92 + 0.08 * ease_out((t - SLAM) / SETTLE)
        } else {
            1.0
        };
        ui_tf.scale = Vec2::splat(scale);
        let alpha = if t < SLAM {
            t / SLAM
        } else if t < SLAM + SETTLE + HOLD {
            1.0
        } else {
            1.0 - (t - SLAM - SETTLE - HOLD) / FADE
        };
        bg.0.set_alpha(0.86 * alpha);
        let edge = border.top.with_alpha(alpha);
        *border = BorderColor::all(edge);
        for child in children.iter() {
            if let Ok((base, mut color)) = texts.get_mut(child) {
                color.0 = base.base.with_alpha(base.base.alpha() * alpha);
            }
        }
    }
}

fn animate_augment_flash(
    mut commands: Commands,
    time: Res<Time>,
    mut flashes: Query<(Entity, &mut AugmentFlash, &mut BackgroundColor)>,
) {
    for (entity, mut flash, mut bg) in &mut flashes {
        flash.t += time.delta_secs();
        if flash.t >= FLASH_TIME {
            commands.entity(entity).despawn();
            continue;
        }
        let k = 1.0 - flash.t / FLASH_TIME;
        bg.0 = flash.color.with_alpha(0.30 * k * k);
    }
}

/// 三选一卡牌的手感：出现时依次弹入（0.55→1，带回弹），悬停放大 6% 并响一声
/// 点击音，按下回压到 95%。
fn augment_card_juice(
    time: Res<Time>,
    roguelite: Res<RogueliteRun>,
    mut cards: Query<(&RogueliteChoiceButton, Ref<Interaction>, &mut UiTransform)>,
    mut sfx: MessageWriter<SfxEvent>,
    mut reveal: Local<Option<f32>>,
    mut had_draft: Local<bool>,
    mut hover: Local<[f32; 3]>,
) {
    let dt = time.delta_secs();
    let has_draft = roguelite.draft.is_some();
    if has_draft && !*had_draft {
        *reveal = Some(0.0);
        sfx.write(SfxEvent(Sound::Raise));
    }
    *had_draft = has_draft;
    if let Some(t) = reveal.as_mut() {
        *t += dt;
    }

    for (card, interaction, mut ui_tf) in &mut cards {
        let i = card.index.min(2);
        if interaction.is_changed() && *interaction == Interaction::Hovered {
            sfx.write(SfxEvent(Sound::Click));
        }
        let target = match *interaction {
            Interaction::Pressed => 0.95,
            Interaction::Hovered => 1.06,
            Interaction::None => 1.0,
        };
        if hover[i] == 0.0 {
            hover[i] = 1.0;
        }
        hover[i] += (target - hover[i]) * (dt * 18.0).min(1.0);
        let intro = match *reveal {
            Some(t) => {
                let x = ((t - i as f32 * 0.08) / 0.25).clamp(0.0, 1.0);
                // ease-out-back：冲过 1 再回落，弹入感。
                let c = 1.7;
                let y = 1.0 + (c + 1.0) * (x - 1.0).powi(3) + c * (x - 1.0).powi(2);
                0.55 + 0.45 * y
            }
            None => 1.0,
        };
        ui_tf.scale = Vec2::splat(intro * hover[i]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cannon_card_makes_cannons_explode() {
        let ev = tower_accent(
            RogueliteTalent::CannonShockwave,
            TowerKind::Cannon,
            Vec2::ZERO,
            Color::WHITE,
        );
        assert!(matches!(ev, VfxEvent::Explosion { .. }));
    }

    #[test]
    fn generic_card_pulses_every_tower() {
        let ev = tower_accent(
            RogueliteTalent::TowerOverclock,
            TowerKind::Ice,
            Vec2::ZERO,
            Color::WHITE,
        );
        assert!(matches!(ev, VfxEvent::ElementPulse { .. }));
    }
}
