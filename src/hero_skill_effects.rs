//! Class-specific releases. Casting and cooldown scheduling live in hero_passives.

use bevy::prelude::*;

use crate::components::Enemy;
use crate::data::{BOARD_H, BOARD_W, Element, MAX_HERO_SUMMONS, TILE_SIZE};
use crate::game::RunState;
use crate::hero::{HeroLoadout, HeroWeapon};
use crate::sprites::Sprites;
use crate::tower::{BuffTower, Damage, Status, Summon, Tower};

mod fields;
mod keystones;
mod magic;
mod martial;
mod support;

pub use fields::{SkillField, SkillFieldKind, update_skill_fields};

#[derive(Clone, Copy)]
pub(crate) struct PassiveSource {
    pub pos: Vec2,
    pub damage: f32,
    pub element: Element,
    pub max_hp: f32,
    pub facing: Vec2,
}

#[derive(Clone, Copy)]
struct Target {
    entity: Entity,
    pos: Vec2,
    hp: f32,
    max_hp: f32,
    progress: usize,
    boss: bool,
    facing: Vec2,
}

fn targets(source: PassiveSource, enemies: &Query<(Entity, &Enemy, &Transform)>) -> Vec<Target> {
    let mut targets: Vec<_> = enemies
        .iter()
        .filter(|(_, enemy, tf)| {
            enemy.hp > 0.0 && source.pos.distance(tf.translation.truncate()) <= 420.0
        })
        .map(|(entity, enemy, tf)| Target {
            entity,
            pos: tf.translation.truncate(),
            hp: enemy.hp,
            max_hp: enemy.max_hp,
            progress: enemy.path_index,
            boss: enemy.boss,
            facing: enemy.facing,
        })
        .collect();
    targets.sort_by(|a, b| {
        b.progress
            .cmp(&a.progress)
            .then_with(|| a.entity.cmp(&b.entity))
    });
    targets
}

pub(crate) fn skill_ready(
    slot: usize,
    hero_entity: Entity,
    source: PassiveSource,
    loadout: &HeroLoadout,
    towers: &Query<(Entity, &mut Tower)>,
    enemies: &Query<(Entity, &Enemy, &Transform)>,
    summons: &Query<&Summon>,
    run: &RunState,
) -> bool {
    if slot > 2 || (slot == 2 && loadout.level < HeroLoadout::MAX_LEVEL) {
        return false;
    }
    let rank = loadout.talent_rank(slot) as f32;
    let targets = targets(source, enemies);
    let capacity = summons
        .iter()
        .filter(|s| s.owner == hero_entity && s.hp > 0.0 && s.lifetime > 0.0)
        .count()
        < MAX_HERO_SUMMONS;
    let wounded = towers.iter().any(|(_, tower)| {
        tower.hp > 0.0
            && tower.hp < tower.max_hp
            && tower.center().distance(source.pos) <= 150.0 + rank * 14.0
    }) || run.lives < run.start_lives;
    match (loadout.weapon, slot) {
        (HeroWeapon::BannerSword, 0) => targets
            .iter()
            .any(|t| t.pos.distance(source.pos) <= 250.0 + rank * 16.0),
        (HeroWeapon::BannerSword, 1) => targets
            .iter()
            .any(|t| t.pos.distance(source.pos) <= 115.0 + rank * 9.0),
        (HeroWeapon::OathShield, 0) => targets
            .iter()
            .any(|t| t.pos.distance(source.pos) <= 112.0 + rank * 8.0),
        (HeroWeapon::OathShield, 1) => wounded,
        (HeroWeapon::OathShield, 2) => {
            wounded || targets.iter().any(|t| t.pos.distance(source.pos) <= 180.0)
        }
        (HeroWeapon::BannerSword, 2) => targets.iter().any(|t| t.pos.distance(source.pos) <= 180.0),
        (HeroWeapon::SentryCrossbow, 0) => towers.iter().any(|(entity, tower)| {
            entity != hero_entity
                && tower.hp > 0.0
                && tower.center().distance(source.pos) <= 180.0 + rank * 12.0
                && targets
                    .iter()
                    .any(|t| t.pos.distance(tower.center()) <= 225.0 + rank * 10.0)
        }),
        (HeroWeapon::SentryCrossbow, 2) => {
            targets.iter().any(|t| t.pos.distance(source.pos) <= 310.0)
        }
        // Summoning is proactive: establish the frontline before enemies reach
        // the hero. Recall only needs stored souls; the other summon casts only
        // need capacity and can form up using the hero's facing direction.
        (HeroWeapon::SummonStaff, 1) => capacity && !run.fallen_enemies.is_empty(),
        (HeroWeapon::SummonStaff, _) => capacity,
        (HeroWeapon::ForgeHammer, 0 | 2) => capacity && !targets.is_empty(),
        _ => !targets.is_empty(),
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn trigger_weapon_skill(
    slot: usize,
    commands: &mut Commands,
    hero_entity: Entity,
    source: PassiveSource,
    loadout: &mut HeroLoadout,
    towers: &mut Query<(Entity, &mut Tower)>,
    enemies: &Query<(Entity, &Enemy, &Transform)>,
    sprites: &Sprites,
    creatures: &crate::creatures::Creatures,
    dmg: &mut MessageWriter<Damage>,
    status: &mut MessageWriter<Status>,
    buff: &mut MessageWriter<BuffTower>,
    vfx: &mut MessageWriter<crate::vfx::VfxEvent>,
    run: &mut RunState,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    summon_slots: &mut usize,
) -> bool {
    if slot > 2 || (slot == 2 && loadout.level < HeroLoadout::MAX_LEVEL) {
        return false;
    }
    let candidates = targets(source, enemies);
    let rank = loadout.talent_rank(slot) as f32;
    let power = loadout.skill_damage_mult() * (1.0 + rank * 0.18);
    let triggered = match loadout.weapon {
        HeroWeapon::BannerSword | HeroWeapon::ShadowBow | HeroWeapon::NightDagger => {
            martial::release(
                slot,
                commands,
                hero_entity,
                source,
                loadout.weapon,
                rank,
                power,
                &candidates,
                towers,
                dmg,
                status,
                vfx,
                meshes,
                materials,
            )
        }
        HeroWeapon::StarfireStaff | HeroWeapon::StormOrb => magic::release(
            slot,
            commands,
            hero_entity,
            source,
            loadout.weapon,
            rank,
            power,
            &candidates,
            dmg,
            status,
            vfx,
        ),
        _ => support::release(
            slot,
            commands,
            hero_entity,
            source,
            loadout.weapon,
            rank,
            power,
            &candidates,
            towers,
            sprites,
            creatures,
            dmg,
            status,
            vfx,
            run,
            meshes,
            materials,
            summon_slots,
        ),
    };
    if triggered && slot == 0 {
        keystones::trigger(
            commands,
            hero_entity,
            source,
            loadout,
            towers,
            &candidates,
            sprites,
            creatures,
            dmg,
            status,
            buff,
            vfx,
            summon_slots,
        );
    }
    triggered
}

fn point_segment_distance(point: Vec2, start: Vec2, end: Vec2) -> f32 {
    let segment = end - start;
    let t =
        ((point - start).dot(segment) / segment.length_squared().max(f32::EPSILON)).clamp(0.0, 1.0);
    point.distance(start + segment * t)
}

fn clamp_guard_pos(pos: Vec2) -> Vec2 {
    let margin = TILE_SIZE * 0.35;
    Vec2::new(
        pos.x.clamp(-BOARD_W * 0.5 + margin, BOARD_W * 0.5 - margin),
        pos.y.clamp(-BOARD_H * 0.5 + margin, BOARD_H * 0.5 - margin),
    )
}

fn guard_positions(source: PassiveSource, count: usize, targets: &[Target]) -> Vec<Vec2> {
    let dir = targets
        .first()
        .map(|t| (t.pos - source.pos).normalize_or_zero())
        .filter(|d| d.length_squared() > 0.01)
        .unwrap_or(source.facing);
    let side = Vec2::new(-dir.y, dir.x);
    (0..count)
        .map(|i| {
            clamp_guard_pos(
                source.pos
                    + dir * TILE_SIZE * (0.8 - (i / 3) as f32 * 0.55)
                    + side
                        * ((i % 3) as f32 - ((count.min(3) - 1) as f32 * 0.5))
                        * TILE_SIZE
                        * 0.64,
            )
        })
        .collect()
}

fn projectile(
    owner: Entity,
    target: Target,
    amount: f32,
    element: Element,
) -> crate::tower::Projectile {
    crate::tower::Projectile {
        source_tower: Some(owner),
        target: target.entity,
        speed: 440.0,
        damage: amount,
        magic: matches!(
            element,
            Element::Arcane | Element::Frost | Element::Storm | Element::Shadow
        ),
        element,
        armor_pierce: 0.0,
        kind: crate::tower::ProjKind::Normal,
        aoe_radius: 0.0,
        slow_duration: 0.0,
        freeze_duration: 0.0,
        dot_damage: 0.0,
        poison_duration: 0.0,
        armor_reduce: 0.0,
        curse_duration: 0.0,
        knock_dist: 0.0,
        stun_duration: 0.0,
    }
}

fn beam(commands: &mut Commands, start: Vec2, end: Vec2, color: Color, width: f32) {
    let d = end - start;
    commands.spawn((
        Sprite {
            color,
            custom_size: Some(Vec2::new(d.length().max(1.0), width)),
            ..default()
        },
        Transform {
            translation: ((start + end) * 0.5).extend(10.7),
            rotation: Quat::from_rotation_z(d.to_angle()),
            ..default()
        },
        crate::tower::ShotFx {
            life: 0.22,
            max_life: 0.22,
            alpha: 0.95,
            shrink_x: false,
        },
        crate::components::LevelEntity,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corridors_clamp_to_both_ends_and_handle_zero_length() {
        let start = Vec2::ZERO;
        let end = Vec2::new(10.0, 0.0);
        assert!((point_segment_distance(Vec2::new(5.0, 3.0), start, end) - 3.0).abs() < 0.001);
        assert!((point_segment_distance(Vec2::new(14.0, 3.0), start, end) - 5.0).abs() < 0.001);
        assert!((point_segment_distance(Vec2::new(3.0, 4.0), start, start) - 5.0).abs() < 0.001);
    }
}
