//! Runtime hero paperdoll composition.
//!
//! `bevy-paperdoll` owns the actual layer composition. This module only maps the
//! current hero loadout to a doll id, weapon fragment, and hero-only gear
//! fragments, then applies the generated `Image` to the hero world entity.

use bevy::prelude::*;
use bevy_paperdoll::{PaperdollAsset, PaperdollId};

use crate::data::TILE_SIZE;
use crate::hero::{HeroLoadout, HeroWeapon, Race};
use crate::hero_gear::{HeroGear, HeroGearSlot};
use crate::tower::Tower;

pub const HERO_PAPERDOLL_PATH: &str = "paperdoll/hero.ppd";
pub const HERO_WEAPON_SLOT: u32 = 10;
/// 英雄达到该等级后，武器换用带元素光晕的辉光变体（fragment id +50）。
pub const GLOW_WEAPON_LEVEL: u8 = 20;
const GLOW_FRAGMENT_OFFSET: u32 = 50;
pub const HERO_PAPERDOLL_WORLD_SIZE: f32 = TILE_SIZE * 1.62;
pub const HERO_PAPERDOLL_GHOST_SIZE: f32 = TILE_SIZE * 1.42;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct HeroPaperdollKey {
    race: Race,
    weapon: HeroWeapon,
    gear: [Option<HeroGear>; HeroGearSlot::COUNT],
    /// 高级武器辉光（Lv20+）：等级跨过阈值时纸娃娃需要重新合成。
    glow: bool,
}

impl HeroPaperdollKey {
    fn from_loadout(loadout: &HeroLoadout) -> Self {
        Self {
            race: loadout.race,
            weapon: loadout.weapon,
            gear: loadout.gear,
            glow: loadout.level >= GLOW_WEAPON_LEVEL,
        }
    }
}

#[derive(Component)]
pub struct HeroPaperdollSprite;

#[derive(Component, Clone, Copy)]
struct HeroPaperdollApplied {
    key: HeroPaperdollKey,
}

#[derive(Resource, Clone)]
struct HeroPaperdollAssets {
    source: Handle<PaperdollAsset>,
}

#[derive(Resource, Default)]
pub struct HeroPaperdollRuntime {
    key: Option<HeroPaperdollKey>,
    image: Option<Handle<Image>>,
    error_key: Option<HeroPaperdollKey>,
    world_key: Option<HeroPaperdollKey>,
    world_source: Option<Handle<Image>>,
    world_image: Option<Handle<Image>>,
}

impl HeroPaperdollRuntime {
    pub fn image(&self) -> Option<Handle<Image>> {
        self.image.clone()
    }

    /// Never show a previous race/weapon/outfit while the new one is loading.
    pub fn image_for(&self, loadout: &HeroLoadout) -> Option<Handle<Image>> {
        (self.key == Some(HeroPaperdollKey::from_loadout(loadout)))
            .then(|| self.image())
            .flatten()
    }

    pub fn world_image_for(&self, loadout: &HeroLoadout) -> Option<Handle<Image>> {
        (self.world_key == Some(HeroPaperdollKey::from_loadout(loadout)))
            .then(|| self.world_image.clone())
            .flatten()
    }
}

pub struct HeroPaperdollPlugin;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outfit_layer_alpha_preserves_empty_pixels_and_overlays_opaque_gear() {
        let mut pixel = [30, 60, 90, 255];
        blend_rgba(&mut pixel, &[255, 0, 0, 0]);
        assert_eq!(pixel, [30, 60, 90, 255]);
        blend_rgba(&mut pixel, &[0, 200, 100, 255]);
        assert_eq!(pixel, [0, 200, 100, 255]);
        let mut empty = [0; 4];
        blend_rgba(&mut empty, &[20, 40, 60, 128]);
        assert_eq!(empty, [20, 40, 60, 128]);
    }

    #[test]
    fn cached_preview_is_only_valid_for_exact_current_outfit() {
        let mut loadout = HeroLoadout::default();
        loadout.gear = [None; HeroGearSlot::COUNT];
        let runtime = HeroPaperdollRuntime {
            key: Some(HeroPaperdollKey::from_loadout(&loadout)),
            image: Some(Handle::default()),
            error_key: None,
            ..default()
        };
        assert!(runtime.image_for(&loadout).is_some());
        loadout.gear[HeroGearSlot::Armor.idx()] = Some(HeroGear::VowPlate);
        assert!(runtime.image_for(&loadout).is_none());
    }
}

impl Plugin for HeroPaperdollPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HeroPaperdollRuntime>()
            .add_systems(Startup, load_hero_paperdoll_asset)
            .add_systems(
                Update,
                (refresh_hero_paperdoll_image, apply_hero_paperdoll_image).chain(),
            )
            .add_systems(
                Update,
                apply_animated_outfit.after(crate::build::refresh_hero_visual),
            );
    }
}

fn load_hero_paperdoll_asset(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(HeroPaperdollAssets {
        source: assets.load(HERO_PAPERDOLL_PATH),
    });
}

fn refresh_hero_paperdoll_image(
    loadout: Res<HeroLoadout>,
    assets: Option<Res<HeroPaperdollAssets>>,
    mut runtime: ResMut<HeroPaperdollRuntime>,
    mut paperdolls: ResMut<Assets<PaperdollAsset>>,
    mut images: ResMut<Assets<Image>>,
    mut source_events: MessageReader<AssetEvent<PaperdollAsset>>,
) {
    // A failed composition must not remain permanently cached after the source
    // asset finishes loading or is repaired through hot reload.
    let source_changed = source_events.read().any(|event| match event {
        AssetEvent::Modified { id } | AssetEvent::LoadedWithDependencies { id } => assets
            .as_ref()
            .is_some_and(|assets| assets.source.id() == *id),
        _ => false,
    });
    if source_changed {
        runtime.key = None;
        runtime.error_key = None;
        runtime.world_key = None;
    }
    let key = HeroPaperdollKey::from_loadout(&loadout);
    if runtime.key == Some(key) && runtime.image.is_some() {
        return;
    }
    if runtime.error_key == Some(key) {
        return;
    }

    let Some(assets) = assets else {
        return;
    };
    // Creating/removing runtime dolls does not change the source artwork. Do
    // not emit Modified here: that would invalidate our own cache every frame.
    let Some(asset) = paperdolls.get_mut_untracked(&assets.source) else {
        return;
    };

    match compose_hero_paperdoll(asset, &loadout) {
        Ok(image) => {
            runtime.image = Some(images.add(image));
            runtime.key = Some(key);
            runtime.error_key = None;
        }
        Err(err) => {
            warn!("hero paperdoll composition failed: {err}");
            runtime.error_key = Some(key);
        }
    }
}

/// Apply the same equipped fragments to every animation cell, rather than
/// replacing an animated hero with a static portrait. Always compose from the
/// original sheet so removing gear cannot leave stale layers behind.
fn apply_animated_outfit(
    loadout: Res<HeroLoadout>,
    walks: Res<crate::build::HeroWalks>,
    source: Option<Res<HeroPaperdollAssets>>,
    paperdolls: Res<Assets<PaperdollAsset>>,
    layouts: Res<Assets<TextureAtlasLayout>>,
    mut images: ResMut<Assets<Image>>,
    mut runtime: ResMut<HeroPaperdollRuntime>,
    mut heroes: Query<&mut Sprite, With<crate::build::HeroWalkAnim>>,
) {
    let Some(cfg) = walks
        .race_worlds
        .get(&(loadout.weapon, loadout.race))
        .or_else(|| walks.weapon_walks.get(&loadout.weapon))
    else {
        return;
    };
    let key = HeroPaperdollKey::from_loadout(&loadout);
    if runtime.world_key != Some(key) || runtime.world_source.as_ref() != Some(&cfg.image) {
        let Some(asset) = source
            .as_ref()
            .and_then(|source| paperdolls.get(&source.source))
        else {
            return;
        };
        let (Some(base), Some(layout)) = (images.get(&cfg.image), layouts.get(&cfg.layout)) else {
            return;
        };
        if !matches!(
            base.texture_descriptor.format,
            bevy::render::render_resource::TextureFormat::Rgba8Unorm
                | bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb
        ) {
            return;
        }
        let mut composed = base.clone();
        let width = composed.width();
        let Some(pixels) = composed.data.as_mut() else {
            return;
        };
        for item in loadout.gear.iter().flatten() {
            let def = item.def();
            let fragments = asset.get_fragments_by_slot(def.slot.paperdoll_slot());
            let Some(fragment) = fragments
                .into_iter()
                .find(|fragment| fragment.id() == def.paperdoll_fragment)
            else {
                return;
            };
            let layer = &fragment.image;
            if layer.width == 0 || layer.height == 0 {
                return;
            }
            for cell in &layout.textures {
                for y in 0..cell.height() {
                    for x in 0..cell.width() {
                        let sx = x * layer.width / cell.width();
                        let sy = y * layer.height / cell.height();
                        let src = ((sy * layer.width + sx) * 4) as usize;
                        let dst = (((cell.min.y + y) * width + cell.min.x + x) * 4) as usize;
                        if src + 4 <= layer.pixels.len() && dst + 4 <= pixels.len() {
                            if *item == HeroGear::CarrotWings {
                                // Back attachments must not cover the face,
                                // hands or the already-equipped front layers.
                                let mut behind = [0; 4];
                                behind.copy_from_slice(&layer.pixels[src..src + 4]);
                                blend_rgba(&mut behind, &pixels[dst..dst + 4]);
                                pixels[dst..dst + 4].copy_from_slice(&behind);
                            } else {
                                blend_rgba(&mut pixels[dst..dst + 4], &layer.pixels[src..src + 4]);
                            }
                        }
                    }
                }
            }
        }
        runtime.world_image = Some(images.add(composed));
        runtime.world_source = Some(cfg.image.clone());
        runtime.world_key = Some(key);
    }
    if let Some(image) = runtime.world_image_for(&loadout) {
        for mut sprite in &mut heroes {
            sprite.image = image.clone();
        }
    }
}

fn blend_rgba(dst: &mut [u8], src: &[u8]) {
    let alpha = src[3] as f32 / 255.0;
    if alpha == 0.0 {
        return;
    }
    let base_alpha = dst[3] as f32 / 255.0;
    let out_alpha = alpha + base_alpha * (1.0 - alpha);
    for channel in 0..3 {
        dst[channel] = ((src[channel] as f32 * alpha
            + dst[channel] as f32 * base_alpha * (1.0 - alpha))
            / out_alpha)
            .round() as u8;
    }
    dst[3] = (out_alpha * 255.0).round() as u8;
}

fn compose_hero_paperdoll(
    asset: &mut PaperdollAsset,
    loadout: &HeroLoadout,
) -> Result<Image, String> {
    let paperdoll_id = asset.create_paperdoll(doll_id_for_race(loadout.race));
    let result = (|| {
        let mut weapon_fragment = loadout.weapon_kind().paperdoll_fragment();
        if loadout.level >= GLOW_WEAPON_LEVEL {
            weapon_fragment += GLOW_FRAGMENT_OFFSET;
        }
        set_fragment(
            asset,
            paperdoll_id,
            HERO_WEAPON_SLOT,
            weapon_fragment,
            "hero weapon",
        )?;

        for gear in loadout.gear.iter().flatten().copied() {
            let def = gear.def();
            set_fragment(
                asset,
                paperdoll_id,
                def.slot.paperdoll_slot(),
                def.paperdoll_fragment,
                def.name,
            )?;
        }

        asset
            .take_texture(paperdoll_id)
            .ok_or_else(|| "paperdoll texture was not produced".to_owned())
    })();

    asset.remove_paperdoll(paperdoll_id);
    result
}

fn set_fragment(
    asset: &mut PaperdollAsset,
    paperdoll_id: PaperdollId,
    slot_id: u32,
    fragment_id: u32,
    label: &str,
) -> Result<(), String> {
    asset
        .slot_use_fragment(paperdoll_id, slot_id, fragment_id)
        .map_err(|err| format!("{label} slot {slot_id} -> fragment {fragment_id}: {err}"))
}

fn doll_id_for_race(race: Race) -> u32 {
    match race {
        Race::Human => 0,
        Race::Elf => 1,
        Race::Orc => 2,
    }
}

fn apply_hero_paperdoll_image(
    mut commands: Commands,
    runtime: Res<HeroPaperdollRuntime>,
    mut heroes: Query<
        (Entity, &Tower, &mut Sprite, Option<&HeroPaperdollApplied>),
        (
            With<HeroPaperdollSprite>,
            Without<crate::build::HeroWalkAnim>,
        ),
    >,
) {
    let (Some(key), Some(image)) = (runtime.key, runtime.image()) else {
        return;
    };

    for (entity, tower, mut sprite, applied) in &mut heroes {
        if !tower.hero || applied.map(|applied| applied.key == key).unwrap_or(false) {
            continue;
        }

        sprite.image = image.clone();
        sprite.texture_atlas = None;
        sprite.rect = None;
        sprite.color = Color::WHITE;
        sprite.custom_size = Some(Vec2::splat(HERO_PAPERDOLL_WORLD_SIZE));

        commands.entity(entity).insert(HeroPaperdollApplied { key });
    }
}
