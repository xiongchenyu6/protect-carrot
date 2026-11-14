//! Periodic, telegraphed reinforcements shared by every boss species.

use crate::{
    audio::{SfxEvent, Sound},
    components::Enemy,
    creatures::Creatures,
    data::TILE_SIZE,
    game::{Paused, RunState},
    vfx::VfxEvent,
};
use bevy::prelude::*;

#[derive(Component)]
pub struct BossSummoner {
    pub cooldown: f32,
    pub windup: Option<f32>,
}

impl Default for BossSummoner {
    fn default() -> Self {
        Self {
            cooldown: 6.0,
            windup: None,
        }
    }
}

#[derive(Component)]
pub struct BossReinforcement(pub Entity);

pub fn update(
    mut commands: Commands,
    time: Res<Time>,
    run: Res<RunState>,
    paused: Res<Paused>,
    creatures: Res<Creatures>,
    bosses: Query<(Entity, &Enemy, &Transform, Option<&mut BossSummoner>)>,
    adds: Query<&BossReinforcement>,
    enemies: Query<&Enemy>,
    mut vfx: MessageWriter<VfxEvent>,
    mut sfx: MessageWriter<SfxEvent>,
) {
    if paused.0 || !run.wave_in_progress || run.lives <= 0 {
        return;
    }
    let dt = time.delta_secs() * run.game_speed;
    if dt <= 0.0 {
        return;
    }
    let mut global_slots =
        192usize.saturating_sub(enemies.iter().filter(|enemy| enemy.hp > 0.0).count());
    for (owner, boss, tf, state) in bosses {
        if !boss.boss || boss.hp <= 0.0 {
            continue;
        }
        let Some(mut state) = state else {
            commands.entity(owner).insert(BossSummoner::default());
            continue;
        };
        // Enemy control can interrupt a summon before its release.
        if boss.frozen || boss.stun_timer > 0.0 {
            if state.windup.take().is_some() {
                state.cooldown = 3.0;
            }
            continue;
        }
        let slots = 12usize
            .saturating_sub(adds.iter().filter(|add| add.0 == owner).count())
            .min(global_slots);
        let pos = tf.translation.truncate();
        let color = Color::srgb(0.85, 0.35, 1.0);
        if let Some(remaining) = state.windup.as_mut() {
            *remaining -= dt;
            if *remaining > 0.0 {
                continue;
            }
            state.windup = None;
            state.cooldown = if boss.enraged { 8.0 } else { 12.0 };
            let count =
                (3 + (run.wave.max(0) as usize / 10).min(3) + usize::from(boss.enraged) * 2)
                    .min(slots);
            for index in 0..count {
                let offset =
                    Vec2::from_angle(index as f32 * std::f32::consts::TAU / count as f32) * 16.0;
                let hp = (boss.max_hp * 0.025).clamp(30.0, 400.0);
                let entity = crate::enemy::spawn_child(
                    &mut commands,
                    &creatures,
                    pos + offset,
                    boss.path_index,
                    hp,
                );
                commands.entity(entity).insert(BossReinforcement(owner));
            }
            global_slots -= count;
            if count > 0 {
                sfx.write(SfxEvent(Sound::Summon));
                vfx.write(VfxEvent::Burst {
                    pos,
                    radius: TILE_SIZE * 1.5,
                    color,
                });
                vfx.write(VfxEvent::Text {
                    pos: pos + Vec2::Y * 40.0,
                    text: crate::i18n::tf("援军 +{}", &[&count.to_string()]),
                    color,
                    size: 20.0,
                    life: 1.4,
                });
            }
        } else {
            state.cooldown = (state.cooldown - dt).max(0.0);
            if state.cooldown <= 0.0 && slots > 0 {
                state.windup = Some(1.0);
                sfx.write(SfxEvent(Sound::Boss));
                vfx.write(VfxEvent::BossCast {
                    pos,
                    radius: TILE_SIZE * 1.5,
                    color,
                    label: crate::i18n::t("首领召唤"),
                });
            }
        }
    }
}
