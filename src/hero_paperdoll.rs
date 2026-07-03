//! Runtime hero paperdoll composition.
//!
//! `bevy-paperdoll` owns the actual layer composition. This module only maps the
//! current hero loadout to a doll id, weapon fragment, and hero-only gear
//! fragments, then applies the generated `Image` to the hero world entity.

use bevy::prelude::*;
use bevy_paperdoll::{PaperdollAsset, PaperdollId};
use bevy_spritesheet_animation::prelude::SpritesheetAnimation;

use crate::data::TILE_SIZE;
use crate::hero::{HeroLoadout, HeroWeapon, Race};
use crate::hero_gear::{HeroGear, HeroGearSlot};
use crate::tower::Tower;

pub const HERO_PAPERDOLL_PATH: &str = "paperdoll/hero.ppd";
pub const HERO_WEAPON_SLOT: u32 = 10;
pub const HERO_PAPERDOLL_WORLD_SIZE: f32 = TILE_SIZE * 1.62;
pub const HERO_PAPERDOLL_GHOST_SIZE: f32 = TILE_SIZE * 1.42;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct HeroPaperdollKey {
    race: Race,
    weapon: HeroWeapon,
    gear: [Option<HeroGear>; HeroGearSlot::COUNT],
}

impl HeroPaperdollKey {
    fn from_loadout(loadout: &HeroLoadout) -> Self {
        Self {
            race: loadout.race,
            weapon: loadout.weapon,
            gear: loadout.gear,
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
}

impl HeroPaperdollRuntime {
    pub fn image(&self) -> Option<Handle<Image>> {
        self.image.clone()
    }
}

pub struct HeroPaperdollPlugin;

impl Plugin for HeroPaperdollPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HeroPaperdollRuntime>()
            .add_systems(Startup, load_hero_paperdoll_asset)
            .add_systems(
                Update,
                (refresh_hero_paperdoll_image, apply_hero_paperdoll_image),
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
) {
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
    let Some(mut asset) = paperdolls.get_mut(&assets.source) else {
        return;
    };

    match compose_hero_paperdoll(&mut asset, &loadout) {
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

fn compose_hero_paperdoll(
    asset: &mut PaperdollAsset,
    loadout: &HeroLoadout,
) -> Result<Image, String> {
    let paperdoll_id = asset.create_paperdoll(doll_id_for_race(loadout.race));
    let result = (|| {
        set_fragment(
            asset,
            paperdoll_id,
            HERO_WEAPON_SLOT,
            loadout.weapon_kind().paperdoll_fragment(),
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
        With<HeroPaperdollSprite>,
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

        commands
            .entity(entity)
            .remove::<(crate::build::HeroWalkAnim, SpritesheetAnimation)>()
            .insert(HeroPaperdollApplied { key });
    }
}
