//! Render-only contact holds and directional recoil. Offsets are removed before
//! simulation and reapplied after animation, so collision/pathfinding never see
//! a presentation displacement. UI, input, projectiles and clocks keep running.

use crate::{combat_feel::ImpactTier, game::Paused, vfx::VfxEvent};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::sprite::Anchor;

#[derive(Component)]
pub struct ImpactReaction {
    pub tier: ImpactTier,
    pub elapsed: f32,
    direction: Vec2,
    attacker: bool,
    base_anchor: Option<Anchor>,
    base_scale: Vec3,
    held_scale: Option<Vec3>,
    held_frame: Option<usize>,
    held_position: Option<Vec2>,
}

impl ImpactReaction {
    fn new(tier: ImpactTier, direction: Vec2, attacker: bool) -> Self {
        Self {
            tier,
            elapsed: 0.0,
            direction,
            attacker,
            base_anchor: None,
            base_scale: Vec3::ONE,
            held_scale: None,
            held_frame: None,
            held_position: None,
        }
    }

    pub fn holding(&self) -> bool {
        self.elapsed < self.tier.hold()
    }

    fn duration(&self) -> f32 {
        self.tier.hold() + 0.16
    }
}

pub struct CombatFeedbackPlugin;

impl Plugin for CombatFeedbackPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, clear_visual_offsets)
            .add_systems(
                PostUpdate,
                (receive_impacts, animate_reactions)
                    .chain()
                    .after(bevy_spritesheet_animation::plugin::AnimationSystemSet)
                    .after(crate::hero_cast_visuals::pose_casts),
            );
    }
}

pub fn clear_visual_offsets(mut bodies: Query<(&mut Transform, &mut Anchor, &mut ImpactReaction)>) {
    for (mut transform, mut anchor, reaction) in &mut bodies {
        transform.scale = reaction.base_scale;
        if let Some(base) = reaction.base_anchor {
            *anchor = base;
        }
    }
}

pub fn receive_impacts(
    mut commands: Commands,
    mut events: MessageReader<VfxEvent>,
    bodies: Query<(), With<Sprite>>,
    sources: Query<&crate::tower::Tower>,
    reactions: Query<&ImpactReaction>,
) {
    let mut pending: HashMap<Entity, (ImpactTier, Vec2, bool)> = HashMap::default();
    for event in events.read() {
        let VfxEvent::Impact {
            source,
            target,
            pos,
            direction,
            tier,
            hero,
            ..
        } = event
        else {
            continue;
        };
        let mut register = |entity, direction, attacker| {
            if !bodies.contains(entity) {
                return;
            }
            let entry = pending
                .entry(entity)
                .or_insert((*tier, direction, attacker));
            if *tier > entry.0 {
                *entry = (*tier, direction, attacker);
            }
        };
        register(*target, *direction, false);
        if *hero && *tier > ImpactTier::Normal {
            if let Some(source) = source.filter(|source| {
                sources.get(*source).is_ok_and(|tower| {
                    tower.hero_attack_timer > 0.0
                        && matches!(
                            tower.hero_weapon,
                            Some(
                                crate::hero::HeroWeapon::BannerSword
                                    | crate::hero::HeroWeapon::OathShield
                                    | crate::hero::HeroWeapon::NightDagger
                                    | crate::hero::HeroWeapon::ForgeHammer
                            )
                        )
                        && tower.center().distance(*pos) <= crate::data::TILE_SIZE * 2.5
                })
            }) {
                register(source, -*direction, true);
            }
        }
    }
    for (entity, (tier, direction, attacker)) in pending {
        // Repeated AoE/DoT cannot perpetually extend a hold. A stronger impact
        // may upgrade it, but an equal one gets a short local refractory window.
        if reactions
            .get(entity)
            .is_ok_and(|old| old.elapsed < 0.12 && tier <= old.tier)
        {
            continue;
        }
        // Clear an existing offset before replacing the component; otherwise
        // losing the stored offset would contaminate the next simulation frame.
        if let Ok(old) = reactions.get(entity) {
            let scale = old.base_scale;
            commands
                .entity(entity)
                .entry::<Transform>()
                .and_modify(move |mut tf| {
                    tf.scale = scale;
                });
            if let Some(anchor) = old.base_anchor {
                commands.entity(entity).insert(anchor);
            }
        }
        commands
            .entity(entity)
            .insert(ImpactReaction::new(tier, direction, attacker));
    }
}

pub fn animate_reactions(
    mut commands: Commands,
    time: Res<Time>,
    paused: Res<Paused>,
    mut bodies: Query<(
        Entity,
        &mut Transform,
        &mut Sprite,
        &mut Anchor,
        &mut ImpactReaction,
        Option<&crate::tower::Tower>,
    )>,
) {
    for (entity, mut tf, mut sprite, mut anchor, mut reaction, tower) in &mut bodies {
        if reaction.elapsed >= reaction.duration() {
            commands.entity(entity).remove::<ImpactReaction>();
            continue;
        }
        reaction.base_scale = tf.scale;
        reaction.base_anchor = Some(*anchor);
        let scale = *reaction.held_scale.get_or_insert(tf.scale);
        if reaction.holding() {
            if reaction.held_frame.is_none() {
                reaction.held_frame = sprite.texture_atlas.as_ref().map(|atlas| atlas.index);
            }
            if let (Some(frame), Some(atlas)) = (reaction.held_frame, sprite.texture_atlas.as_mut())
            {
                atlas.index = frame;
            }
        }
        let recovery = ((reaction.elapsed - reaction.tier.hold()) / 0.16).clamp(0.0, 1.0);
        let envelope = (1.0 - recovery).powi(2);
        let strength = reaction.tier.scale();
        let squeeze = if reaction.attacker { 0.04 } else { 0.10 } * strength * envelope;
        let base = if reaction.holding() { scale } else { tf.scale };
        tf.scale = Vec3::new(base.x * (1.0 - squeeze), base.y * (1.0 + squeeze), base.z);
        let distance = if reaction.attacker { 1.5 } else { 3.0 };
        let mut offset = reaction.direction * (distance * strength * envelope);
        if reaction.holding() {
            // Freeze the rendered contact position as well as the atlas. The
            // hero still follows live movement input; only its lunge is held.
            let origin = tower.map_or(Vec2::ZERO, |tower| tower.center());
            let current = tf.translation.truncate() - origin;
            let held = *reaction.held_position.get_or_insert(current);
            offset += held - current;
        }
        let local = tf.rotation.inverse() * offset.extend(0.0);
        let size = sprite
            .custom_size
            .unwrap_or(Vec2::splat(crate::data::TILE_SIZE))
            .max(Vec2::ONE);
        let scale = Vec2::new(
            if tf.scale.x.abs() > 0.001 {
                tf.scale.x
            } else {
                1.0
            },
            if tf.scale.y.abs() > 0.001 {
                tf.scale.y
            } else {
                1.0
            },
        );
        anchor.0 -= local.truncate() / (size * scale);
        if !paused.0 {
            reaction.elapsed += time.delta_secs();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use std::time::Duration;

    fn setup(tier: ImpactTier, hero: bool) -> (App, Entity, Entity) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Paused>()
            .add_message::<VfxEvent>();
        let mut spawn = |position| {
            app.world_mut()
                .spawn((
                    Sprite {
                        custom_size: Some(Vec2::splat(64.0)),
                        texture_atlas: Some(TextureAtlas {
                            layout: default(),
                            index: 4,
                        }),
                        ..default()
                    },
                    Transform::from_translation(position),
                ))
                .id()
        };
        let source = spawn(Vec3::ZERO);
        let target = spawn(Vec3::new(30.0, 0.0, 0.0));
        let mut source_tower =
            crate::tower::Tower::from_def(crate::data::TowerKind::Arrow.def(), 0, 0);
        source_tower.hero = true;
        source_tower.hero_pos = Vec2::ZERO;
        source_tower.hero_weapon = Some(crate::hero::HeroWeapon::ForgeHammer);
        source_tower.hero_attack_timer = 0.2;
        app.world_mut().entity_mut(source).insert(source_tower);
        app.world_mut().write_message(VfxEvent::Impact {
            source: Some(source),
            target,
            pos: Vec2::new(30.0, 0.0),
            direction: Vec2::X,
            tier,
            hero,
            color: Color::WHITE,
            element: crate::data::Element::Physical,
        });
        app.world_mut().run_system_once(receive_impacts).unwrap();
        app.world_mut().resource_mut::<Messages<VfxEvent>>().clear();
        (app, source, target)
    }

    fn render_step(app: &mut App, dt: f32) {
        app.world_mut()
            .run_system_once(clear_visual_offsets)
            .unwrap();
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f32(dt));
        app.world_mut().run_system_once(animate_reactions).unwrap();
    }

    #[test]
    fn confirmed_heavy_hit_holds_pose_without_moving_simulation_positions() {
        let (mut app, source, target) = setup(ImpactTier::Heavy, true);
        render_step(&mut app, 1.0 / 60.0);
        let rendered_center = |world: &World| {
            let tf = world.get::<Transform>(target).unwrap();
            let anchor = world.get::<Anchor>(target).unwrap();
            tf.translation.truncate() - anchor.0 * Vec2::splat(64.0) * tf.scale.truncate()
        };
        let contact_center = rendered_center(app.world());
        for entity in [source, target] {
            assert!(app.world().get::<ImpactReaction>(entity).unwrap().holding());
            assert_ne!(app.world().get::<Anchor>(entity).unwrap().0, Vec2::ZERO);
        }
        let actual = Vec3::new(35.0, 2.0, 0.0);
        app.world_mut()
            .get_mut::<Transform>(target)
            .unwrap()
            .translation = actual;
        app.world_mut()
            .get_mut::<Sprite>(target)
            .unwrap()
            .texture_atlas
            .as_mut()
            .unwrap()
            .index = 9;
        render_step(&mut app, 1.0 / 60.0);
        assert_eq!(
            app.world().get::<Transform>(target).unwrap().translation,
            actual
        );
        assert!(rendered_center(app.world()).distance(contact_center) < 0.01);
        assert_eq!(
            app.world()
                .get::<Sprite>(target)
                .unwrap()
                .texture_atlas
                .as_ref()
                .unwrap()
                .index,
            4
        );
        for _ in 0..30 {
            render_step(&mut app, 1.0 / 60.0);
        }
        assert!(app.world().get::<ImpactReaction>(target).is_none());
        assert_eq!(app.world().get::<Anchor>(target).unwrap().0, Vec2::ZERO);
        assert_eq!(
            app.world().get::<Transform>(target).unwrap().scale,
            Vec3::ONE
        );
        assert_eq!(
            app.world().get::<Transform>(target).unwrap().translation,
            actual
        );
    }

    #[test]
    fn ordinary_tower_fire_never_holds_the_attacker_or_victim_pose() {
        let (mut app, source, target) = setup(ImpactTier::Normal, false);
        assert!(app.world().get::<ImpactReaction>(source).is_none());
        render_step(&mut app, 0.01);
        app.world_mut()
            .get_mut::<Sprite>(target)
            .unwrap()
            .texture_atlas
            .as_mut()
            .unwrap()
            .index = 7;
        render_step(&mut app, 0.01);
        assert_eq!(
            app.world()
                .get::<Sprite>(target)
                .unwrap()
                .texture_atlas
                .as_ref()
                .unwrap()
                .index,
            7
        );
    }

    #[test]
    fn pause_freezes_feedback_and_repeated_hits_cannot_perpetually_refresh_hold() {
        let (mut app, source, target) = setup(ImpactTier::Heavy, true);
        render_step(&mut app, 0.02);
        let before = app.world().get::<ImpactReaction>(target).unwrap().elapsed;
        app.world_mut().resource_mut::<Paused>().0 = true;
        for _ in 0..10 {
            render_step(&mut app, 0.1);
        }
        assert_eq!(
            app.world().get::<ImpactReaction>(target).unwrap().elapsed,
            before
        );
        app.world_mut().resource_mut::<Paused>().0 = false;
        for _ in 0..3 {
            app.world_mut().write_message(VfxEvent::Impact {
                source: Some(source),
                target,
                pos: Vec2::ZERO,
                direction: Vec2::X,
                tier: ImpactTier::Heavy,
                hero: true,
                color: Color::WHITE,
                element: crate::data::Element::Physical,
            });
            app.world_mut().run_system_once(receive_impacts).unwrap();
            app.world_mut().resource_mut::<Messages<VfxEvent>>().clear();
            render_step(&mut app, 0.02);
        }
        assert!(!app.world().get::<ImpactReaction>(target).unwrap().holding());
    }

    #[test]
    fn local_hold_duration_is_bounded_at_thirty_and_sixty_fps() {
        let mut durations = Vec::new();
        for fps in [30.0, 60.0] {
            let (mut app, _, target) = setup(ImpactTier::Finisher, true);
            let mut frames = 0;
            while app.world().get::<ImpactReaction>(target).unwrap().holding() {
                render_step(&mut app, 1.0 / fps);
                frames += 1;
            }
            let duration = frames as f32 / fps;
            assert!(duration >= ImpactTier::Finisher.hold());
            assert!(duration <= ImpactTier::Finisher.hold() + 1.0 / fps);
            durations.push(duration);
        }
        assert!((durations[0] - durations[1]).abs() <= 1.0 / 30.0);
    }
}
