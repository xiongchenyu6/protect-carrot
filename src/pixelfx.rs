//! PixelFX sprite-sheet playback and combat event integration.
//! Source metadata and transparent pixels are exported by tools/import_pixelfx.mjs.

use std::collections::{HashMap, HashSet};

use bevy::{
    image::{ImageLoaderSettings, ImageSampler},
    prelude::*,
    sprite::Anchor,
};
use serde::Deserialize;

use crate::{
    components::LevelEntity,
    game::{Paused, RunState},
    hero::HeroWeapon,
    hero_passives::{CastPhase, HeroCast, HeroSkillCastEvent},
    states::GameState,
    tower::Tower,
    vfx::VfxEvent,
};

mod combat;
pub use combat::skill_effect;
use combat::{route_impacts, route_skills};

const MAX_EFFECTS: usize = 64;

#[derive(Clone, Deserialize)]
pub struct EffectDefinition {
    pub id: String,
    pub size: [u32; 2],
    pub pivot: [f32; 2],
    pub fps: f32,
    pub frames: usize,
    pub columns: u32,
    pub looping: bool,
    pub contact: f32,
}

impl EffectDefinition {
    fn anchor(&self) -> Anchor {
        Anchor(Vec2::new(
            self.pivot[0] / self.size[0] as f32 - 0.5,
            0.5 - self.pivot[1] / self.size[1] as f32,
        ))
    }

    fn frame(&self, seconds: f32) -> usize {
        let frame = (seconds.max(0.0) * self.fps).floor() as usize;
        if self.looping {
            frame % self.frames
        } else {
            frame.min(self.frames - 1)
        }
    }

    fn playback_rate(&self, start: f32, duration: f32) -> f32 {
        if self.looping {
            1.0
        } else {
            (self.frames as f32 / self.fps - start) / duration
        }
    }
}

#[derive(Deserialize)]
struct Catalog {
    effects: Vec<EffectDefinition>,
}

#[derive(Clone)]
struct Clip {
    definition: EffectDefinition,
    image: Handle<Image>,
    layout: Handle<TextureAtlasLayout>,
}

#[derive(Resource, Default)]
pub struct PixelFxLibrary {
    definitions: HashMap<String, EffectDefinition>,
    clips: HashMap<String, Clip>,
}

impl PixelFxLibrary {
    pub fn ready(&self, assets: &AssetServer) -> bool {
        !self.clips.is_empty()
            && self
                .clips
                .values()
                .all(|clip| assets.is_loaded(clip.image.id()))
    }

    fn load(
        &mut self,
        id: &str,
        assets: &AssetServer,
        layouts: &mut Assets<TextureAtlasLayout>,
    ) -> Option<Clip> {
        if let Some(clip) = self.clips.get(id) {
            return Some(clip.clone());
        }
        let definition = self.definitions.get(id)?.clone();
        let image = assets
            .load_builder()
            .with_settings(|settings: &mut ImageLoaderSettings| {
                settings.sampler = ImageSampler::nearest();
            })
            .load(format!("pixelfx/{id}.webp"));
        let rows = (definition.frames as u32).div_ceil(definition.columns);
        let layout = layouts.add(TextureAtlasLayout::from_grid(
            UVec2::from_array(definition.size),
            definition.columns,
            rows,
            None,
            None,
        ));
        let clip = Clip {
            definition,
            image,
            layout,
        };
        self.clips.insert(id.into(), clip.clone());
        Some(clip)
    }
}

/// Play any imported effect. Loops are bounded by `duration` as well.
#[derive(Message, Clone)]
pub struct PixelFxRequest {
    pub id: String,
    pub position: Vec2,
    pub angle: f32,
    pub scale: f32,
    pub duration: Option<f32>,
    pub follow: Option<Entity>,
    pub follow_rotation: bool,
    pub start_at_contact: bool,
    pub windup_only: bool,
    pub important: bool,
}

impl PixelFxRequest {
    pub fn new(id: &str, position: Vec2) -> Self {
        Self {
            id: id.into(),
            position,
            angle: 0.0,
            scale: 1.0,
            duration: None,
            follow: None,
            follow_rotation: false,
            start_at_contact: false,
            windup_only: false,
            important: false,
        }
    }
}

#[derive(Component)]
pub struct PixelFxAnimation {
    clip: EffectDefinition,
    image: Handle<Image>,
    elapsed: f32,
    start: f32,
    duration: f32,
    rate: f32,
    follow: Option<Entity>,
    follow_rotation: bool,
}

impl PixelFxAnimation {
    pub fn id(&self) -> &str {
        &self.clip.id
    }
}

#[derive(Component)]
struct WindupEffect;

pub struct PixelFxPlugin;

impl Plugin for PixelFxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PixelFxLibrary>()
            .add_message::<PixelFxRequest>()
            .add_systems(Startup, load_library)
            .add_systems(
                Update,
                (route_skills, route_impacts, play_requests, animate)
                    .chain()
                    .after(crate::hero_passives::update_hero_passives)
                    .after(crate::tower::apply_damage)
                    .run_if(in_state(GameState::Playing)),
            );
    }
}

fn load_library(
    mut library: ResMut<PixelFxLibrary>,
    assets: Res<AssetServer>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let catalog: Catalog = serde_json::from_str(include_str!("../assets/pixelfx/catalog.json"))
        .expect("invalid exported PixelFX catalog");
    library.definitions = catalog
        .effects
        .into_iter()
        .map(|effect| (effect.id.clone(), effect))
        .collect();
    for weapon in HeroWeapon::ALL {
        for slot in 0..3 {
            library.load(skill_effect(weapon, slot).0, &assets, &mut layouts);
        }
    }
    for id in [
        "arcane/spell-charge",
        "impacts/hit-spark",
        "impacts/blunt-hit",
        "impacts/crit-hit",
        "holy/smite",
        "melee/sword-slash",
        "melee/dagger-flurry",
        "melee/hammer-smash",
        "holy/heal",
        "lightning/zap-hit",
    ] {
        library.load(id, &assets, &mut layouts);
    }
}

#[allow(clippy::too_many_arguments)]
fn play_requests(
    mut commands: Commands,
    mut requests: MessageReader<PixelFxRequest>,
    mut library: ResMut<PixelFxLibrary>,
    assets: Res<AssetServer>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
    existing: Query<(), With<PixelFxAnimation>>,
) {
    let mut available = MAX_EFFECTS.saturating_sub(existing.iter().count());
    for request in requests.read() {
        if available == 0 || (!request.important && available <= 8) {
            continue;
        }
        let Some(clip) = library.load(&request.id, &assets, &mut layouts) else {
            warn!("Unknown PixelFX effect {}", request.id);
            continue;
        };
        available -= 1;
        let full = clip.definition.frames as f32 / clip.definition.fps;
        let start = if request.start_at_contact {
            clip.definition
                .contact
                .min(full - 1.0 / clip.definition.fps)
        } else {
            0.0
        };
        let duration = request.duration.unwrap_or(full).max(0.01);
        let rate = clip.definition.playback_rate(start, duration);
        let mut entity = commands.spawn((
            Sprite::from_atlas_image(
                clip.image.clone(),
                TextureAtlas {
                    layout: clip.layout,
                    index: clip.definition.frame(start),
                },
            ),
            clip.definition.anchor(),
            Transform::from_translation(request.position.extend(16.5))
                .with_rotation(Quat::from_rotation_z(request.angle))
                .with_scale(Vec3::splat(request.scale)),
            PixelFxAnimation {
                clip: clip.definition,
                image: clip.image,
                elapsed: 0.0,
                start,
                duration,
                rate,
                follow: request.follow,
                follow_rotation: request.follow_rotation,
            },
            LevelEntity,
        ));
        if request.windup_only {
            entity.insert(WindupEffect);
        }
    }
}

fn animate(
    mut commands: Commands,
    time: Res<Time>,
    paused: Res<Paused>,
    run: Res<RunState>,
    images: Res<Assets<Image>>,
    followers: Query<(&Transform, Option<&Tower>, Option<&HeroCast>), Without<PixelFxAnimation>>,
    mut effects: Query<(
        Entity,
        &mut PixelFxAnimation,
        &mut Sprite,
        &mut Transform,
        Option<&WindupEffect>,
    )>,
) {
    if paused.0 {
        return;
    }
    for (entity, mut animation, mut sprite, mut tf, windup) in &mut effects {
        if let Some(owner) = animation.follow {
            let Ok((position, tower, cast)) = followers.get(owner) else {
                commands.entity(entity).despawn();
                continue;
            };
            if windup.is_some() && cast.is_none_or(|cast| cast.phase != CastPhase::Windup) {
                commands.entity(entity).despawn();
                continue;
            }
            tf.translation = tower
                .map_or(position.translation.truncate(), Tower::center)
                .extend(tf.translation.z);
            if animation.follow_rotation {
                tf.rotation = position.rotation;
            }
        }
        if !images.contains(animation.image.id()) {
            continue;
        }
        if animation.elapsed >= animation.duration {
            commands.entity(entity).despawn();
            continue;
        }
        if let Some(atlas) = sprite.texture_atlas.as_mut() {
            atlas.index = animation
                .clip
                .frame(animation.start + animation.elapsed * animation.rate);
        }
        animation.elapsed += time.delta_secs() * run.game_speed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use std::time::Duration;

    fn catalog() -> Catalog {
        serde_json::from_str(include_str!("../assets/pixelfx/catalog.json")).unwrap()
    }

    #[test]
    fn all_class_skill_routes_resolve_to_exported_clips() {
        let catalog = catalog();
        assert_eq!(catalog.effects.len(), 244);
        for weapon in HeroWeapon::ALL {
            for slot in 0..3 {
                let id = skill_effect(weapon, slot).0;
                assert!(
                    catalog.effects.iter().any(|clip| clip.id == id),
                    "{weapon:?} {slot}: {id}"
                );
            }
        }
        for clip in &catalog.effects {
            assert!(clip.frames > 0 && clip.fps > 0.0 && clip.columns > 0);
            assert!(clip.pivot.iter().all(|value| value.is_finite()));
            let rows = (clip.frames as u32).div_ceil(clip.columns);
            assert!(
                clip.size[0] * clip.columns <= 8192 && clip.size[1] * rows <= 8192,
                "{} exceeds portable texture limits",
                clip.id
            );
        }
    }

    #[test]
    fn pivot_preserves_top_left_origin_and_animation_skips_frames_without_drift() {
        let mut clip = catalog()
            .effects
            .into_iter()
            .find(|clip| clip.id == "melee/sword-slash")
            .unwrap();
        let anchor = clip.anchor().0;
        assert!((anchor.x - (44.0 / 136.0 - 0.5)).abs() < 0.0001);
        assert!((anchor.y - (0.5 - 92.0 / 104.0)).abs() < 0.0001);
        assert_eq!(clip.frame(0.2), 6);
        assert_eq!(clip.frame(100.0), clip.frames - 1);
        assert!(
            (clip.playback_rate(0.0, clip.frames as f32 / clip.fps * 2.0) - 0.5).abs() < 0.0001
        );
        clip.looping = true;
        assert_eq!(clip.playback_rate(0.0, 10.0), 1.0);
        assert_eq!(clip.frame((clip.frames + 4) as f32 / clip.fps), 4);
    }

    #[test]
    fn playback_obeys_pause_speed_lifetime_and_cancelled_windup() {
        let mut app = App::new();
        app.insert_resource(Time::<()>::default())
            .init_resource::<Paused>()
            .init_resource::<RunState>()
            .init_resource::<Assets<Image>>();
        let image = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(Image::default());
        let definition = catalog()
            .effects
            .into_iter()
            .find(|clip| clip.id == "melee/sword-slash")
            .unwrap();
        let effect = app
            .world_mut()
            .spawn((
                PixelFxAnimation {
                    clip: definition,
                    image,
                    elapsed: 0.0,
                    start: 0.0,
                    duration: 0.3,
                    rate: 1.0,
                    follow: None,
                    follow_rotation: false,
                },
                Sprite {
                    texture_atlas: Some(TextureAtlas::default()),
                    ..default()
                },
                Transform::default(),
            ))
            .id();
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f32(0.1));
        app.world_mut().resource_mut::<Paused>().0 = true;
        app.world_mut().run_system_once(animate).unwrap();
        assert_eq!(
            app.world().get::<PixelFxAnimation>(effect).unwrap().elapsed,
            0.0
        );
        app.world_mut().resource_mut::<Paused>().0 = false;
        app.world_mut().resource_mut::<RunState>().game_speed = 2.0;
        app.world_mut().run_system_once(animate).unwrap();
        assert_eq!(
            app.world().get::<PixelFxAnimation>(effect).unwrap().elapsed,
            0.2
        );
        app.world_mut().run_system_once(animate).unwrap();
        assert_eq!(
            app.world()
                .get::<Sprite>(effect)
                .unwrap()
                .texture_atlas
                .as_ref()
                .unwrap()
                .index,
            6
        );
        app.world_mut().run_system_once(animate).unwrap();
        assert!(app.world().get_entity(effect).is_err());

        let mut hero = Tower::from_def(crate::data::TowerKind::Arrow.def(), 0, 0);
        hero.hero = true;
        let owner = app.world_mut().spawn((hero, Transform::default())).id();
        let charge = app
            .world_mut()
            .spawn((
                PixelFxAnimation {
                    clip: catalog().effects.remove(0),
                    image: Handle::default(),
                    elapsed: 0.0,
                    start: 0.0,
                    duration: 1.0,
                    rate: 1.0,
                    follow: Some(owner),
                    follow_rotation: false,
                },
                Sprite::default(),
                Transform::default(),
                WindupEffect,
            ))
            .id();
        app.world_mut().run_system_once(animate).unwrap();
        assert!(app.world().get_entity(charge).is_err());

        let owner = app
            .world_mut()
            .spawn(Transform::from_xyz(12.0, 34.0, 0.0).with_rotation(Quat::from_rotation_z(1.0)))
            .id();
        let following = app
            .world_mut()
            .spawn((
                PixelFxAnimation {
                    clip: catalog().effects.remove(0),
                    image: Handle::default(),
                    elapsed: 0.0,
                    start: 0.0,
                    duration: 1.0,
                    rate: 1.0,
                    follow: Some(owner),
                    follow_rotation: true,
                },
                Sprite::default(),
                Transform::from_xyz(0.0, 0.0, 16.5),
            ))
            .id();
        app.world_mut().run_system_once(animate).unwrap();
        assert_eq!(
            app.world().get::<Transform>(following).unwrap().translation,
            Vec3::new(12.0, 34.0, 16.5)
        );
        assert_eq!(
            app.world().get::<Transform>(following).unwrap().rotation,
            Quat::from_rotation_z(1.0)
        );
        app.world_mut().despawn(owner);
        app.world_mut().run_system_once(animate).unwrap();
        assert!(app.world().get_entity(following).is_err());
    }
}
