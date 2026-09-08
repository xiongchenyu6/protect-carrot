//! Equipment proc effects share the primary skill release and summon capacity.

use super::*;
use crate::hero_gear::HeroGearSet;
use crate::tower::{FixedSummonHome, StatusKind, TemporaryGuard};
use crate::vfx::VfxEvent;

#[allow(clippy::too_many_arguments)]
pub(super) fn trigger(
    commands: &mut Commands,
    owner: Entity,
    source: PassiveSource,
    loadout: &HeroLoadout,
    towers: &mut Query<(Entity, &mut Tower)>,
    targets: &[Target],
    sprites: &Sprites,
    creatures: &crate::creatures::Creatures,
    dmg: &mut MessageWriter<Damage>,
    status: &mut MessageWriter<Status>,
    buff: &mut MessageWriter<BuffTower>,
    vfx: &mut MessageWriter<VfxEvent>,
    summon_slots: &mut usize,
) {
    let Some(set) = crate::hero_gear::active_four_piece_set(&loadout.gear) else {
        return;
    };
    let power = loadout.skill_damage_mult();
    match set {
        HeroGearSet::Vanguard => {
            if let Ok((_, mut hero)) = towers.get_mut(owner) {
                hero.hp = (hero.hp + source.max_hp * 0.25).min(hero.max_hp);
            }
            repair(
                owner,
                source.pos,
                TILE_SIZE * 3.4,
                0.08,
                2,
                towers,
                buff,
                vfx,
            );
            vfx.write(VfxEvent::MeleeCleave {
                pos: source.pos,
                radius: TILE_SIZE * 3.4,
                color: Color::srgb(1.0, 0.72, 0.24),
            });
        }
        HeroGearSet::Spellweave => {
            for target in targets.iter().take(6) {
                let hp_scale = (target.max_hp * if target.boss { 0.012 } else { 0.035 })
                    .min(900.0 + source.damage * power);
                dmg.write(Damage {
                    source_tower: Some(owner),
                    target: target.entity,
                    amount: (90.0 + source.damage * 0.65) * power + hp_scale,
                    magic: true,
                    element: Element::Arcane,
                    armor_pierce: 0.0,
                });
                status.write(Status {
                    source_tower: Some(owner),
                    target: target.entity,
                    kind: StatusKind::Freeze { duration: 0.65 },
                });
                vfx.write(VfxEvent::Explosion {
                    pos: target.pos,
                    radius: 32.0,
                    color: Color::srgb(0.46, 0.7, 1.0),
                });
            }
        }
        HeroGearSet::Hunt => {
            let mut wounded = targets.to_vec();
            wounded.sort_by(|a, b| {
                (a.hp / a.max_hp.max(1.0))
                    .total_cmp(&(b.hp / b.max_hp.max(1.0)))
                    .then_with(|| b.progress.cmp(&a.progress))
            });
            for target in wounded.iter().take(4) {
                let missing = (1.0 - target.hp / target.max_hp.max(1.0)).clamp(0.0, 1.0);
                let execute = (target.max_hp * missing * if target.boss { 0.045 } else { 0.12 })
                    .min(1800.0 + source.damage * power * 2.0);
                dmg.write(Damage {
                    source_tower: Some(owner),
                    target: target.entity,
                    amount: (72.0 + source.damage * 0.48) * power + execute,
                    magic: false,
                    element: Element::Physical,
                    armor_pierce: 55.0,
                });
                beam(
                    commands,
                    source.pos,
                    target.pos,
                    Color::srgb(0.72, 1.0, 0.48),
                    2.0,
                );
            }
        }
        HeroGearSet::Covenant => {
            if *summon_slots == 0 {
                return;
            }
            *summon_slots -= 1;
            let stats = crate::hero_gear::active_stats_for_weapon(&loadout.gear, loadout.weapon);
            let summon_power = 1.0 + stats.summon_power_add.max(0.0);
            let facing = source.facing.normalize_or_zero();
            let pos =
                clamp_guard_pos(source.pos + Vec2::new(-facing.y, facing.x) * TILE_SIZE * 0.68);
            crate::tower::spawn_mythic_ally(
                commands,
                sprites.mythic_summon.clone(),
                pos,
                (source.max_hp * 0.34 * summon_power).max(260.0),
                ((48.0 + source.damage * 0.44) * power * summon_power).max(70.0),
                86.0,
                16.0 + summon_power * 3.0,
                owner,
            );
            vfx.write(VfxEvent::HolyStrike { pos, strong: true });
        }
        HeroGearSet::Workshop => {
            repair(
                owner,
                source.pos,
                TILE_SIZE * 3.8,
                0.1,
                3,
                towers,
                buff,
                vfx,
            );
            if *summon_slots == 0 {
                return;
            }
            *summon_slots -= 1;
            let pos = guard_positions(source, 1, targets)[0];
            let guard = crate::tower::spawn_ally(
                commands,
                creatures,
                crate::data::EnemyKind::Shielded,
                pos,
                (source.max_hp * 0.34).max(280.0),
                ((52.0 + source.damage * 0.62) * power).max(72.0),
                50.0,
                18.0,
                0.76,
                owner,
            );
            commands.entity(guard).insert((
                TemporaryGuard,
                FixedSummonHome {
                    pos,
                    range: TILE_SIZE * 2.25,
                },
            ));
            vfx.write(VfxEvent::HammerImpact {
                pos,
                angle: source.facing.to_angle(),
                color: Color::srgb(0.34, 0.94, 0.92),
            });
        }
    }
}

fn repair(
    owner: Entity,
    center: Vec2,
    radius: f32,
    fraction: f32,
    stacks: usize,
    towers: &mut Query<(Entity, &mut Tower)>,
    buff: &mut MessageWriter<BuffTower>,
    vfx: &mut MessageWriter<VfxEvent>,
) {
    for (entity, mut tower) in towers.iter_mut() {
        if entity == owner || tower.hp <= 0.0 || tower.center().distance(center) > radius {
            continue;
        }
        if tower.hp < tower.max_hp {
            tower.hp = (tower.hp + tower.max_hp * fraction).min(tower.max_hp);
            vfx.write(VfxEvent::Heal {
                pos: tower.center(),
            });
        }
        for _ in 0..stacks {
            buff.write(BuffTower { target: entity });
        }
    }
}
