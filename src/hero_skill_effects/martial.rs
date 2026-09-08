use super::*;
use crate::data::TowerKind;
use crate::tower::{ProjKind, StatusKind};
use crate::vfx::VfxEvent;

#[allow(clippy::too_many_arguments)]
pub(super) fn release(
    slot: usize,
    commands: &mut Commands,
    owner: Entity,
    source: PassiveSource,
    weapon: HeroWeapon,
    rank: f32,
    power: f32,
    targets: &[Target],
    towers: &mut Query<(Entity, &mut Tower)>,
    dmg: &mut MessageWriter<Damage>,
    status: &mut MessageWriter<Status>,
    vfx: &mut MessageWriter<VfxEvent>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
) -> bool {
    let color = weapon.skill_color();
    let Some(first) = targets.first().copied() else {
        return false;
    };
    match (weapon, slot) {
        (HeroWeapon::BannerSword, 0) => {
            let Some(target) = targets
                .iter()
                .find(|t| t.pos.distance(source.pos) <= 250.0 + rank * 16.0)
            else {
                return false;
            };
            let dir = (target.pos - source.pos).normalize_or_zero();
            let destination = clamp_guard_pos(target.pos - dir * TILE_SIZE * 0.42);
            let Ok((_, mut hero)) = towers.get_mut(owner) else {
                return false;
            };
            hero.hero_pos = destination;
            hero.move_target = None;
            hero.angle = dir.to_angle();
            for target in targets.iter().filter(|t| {
                point_segment_distance(t.pos, source.pos, destination) <= 36.0 + rank * 4.0
                    || t.pos.distance(destination) <= 72.0
            }) {
                dmg.write(Damage {
                    source_tower: Some(owner),
                    target: target.entity,
                    amount: (125.0 + source.damage * 0.95) * power,
                    magic: false,
                    element: source.element,
                    armor_pierce: 18.0 + rank * 4.0,
                });
                status.write(Status {
                    source_tower: Some(owner),
                    target: target.entity,
                    kind: StatusKind::Knockback {
                        dist: 30.0 + rank * 4.0,
                        stun: 0.5,
                    },
                });
            }
            beam(commands, source.pos, destination, color, 9.0);
            vfx.write(VfxEvent::Slash {
                pos: destination,
                angle: dir.to_angle(),
                color,
                poison: false,
            });
        }
        (HeroWeapon::BannerSword, 1) => {
            let radius = 115.0 + rank * 9.0;
            let Some(target) = targets
                .iter()
                .find(|t| t.pos.distance(source.pos) <= radius)
            else {
                return false;
            };
            let dir = (target.pos - source.pos).normalize_or_zero();
            for target in targets.iter().filter(|t| {
                t.pos.distance(source.pos) <= radius
                    && ((t.pos - source.pos).normalize_or_zero().dot(dir) >= -0.2
                        || t.pos.distance(source.pos) < 20.0)
            }) {
                dmg.write(Damage {
                    source_tower: Some(owner),
                    target: target.entity,
                    amount: (100.0 + source.damage * 1.15) * power,
                    magic: false,
                    element: source.element,
                    armor_pierce: 28.0,
                });
            }
            if let Ok((_, mut hero)) = towers.get_mut(owner) {
                hero.angle = dir.to_angle();
            }
            vfx.write(VfxEvent::Slash {
                pos: source.pos + dir * 45.0,
                angle: dir.to_angle(),
                color,
                poison: false,
            });
            vfx.write(VfxEvent::MeleeCleave {
                pos: source.pos,
                radius,
                color,
            });
        }
        (HeroWeapon::BannerSword, 2) => {
            if !targets.iter().any(|t| t.pos.distance(source.pos) <= 180.0) {
                return false;
            }
            fields::spawn(
                commands,
                owner,
                source,
                weapon,
                SkillFieldKind::Quake,
                power,
                source.pos,
                180.0,
            );
        }
        (HeroWeapon::ShadowBow, 0) | (HeroWeapon::NightDagger, 1) => {
            let dir = (first.pos - source.pos).normalize_or_zero();
            let daggers = weapon == HeroWeapon::NightDagger;
            let count = if daggers { 3 } else { 4 } + (rank as usize / 2);
            for target in targets
                .iter()
                .filter(|t| {
                    !daggers
                        || t.pos.distance(source.pos) < 20.0
                        || (t.pos - source.pos).normalize_or_zero().dot(dir) >= 0.2
                })
                .take(count)
                .copied()
            {
                let mut shot = projectile(
                    owner,
                    target,
                    (62.0 + source.damage * 0.5) * power,
                    if daggers {
                        Element::Shadow
                    } else {
                        Element::Physical
                    },
                );
                shot.kind = ProjKind::Poison;
                shot.dot_damage = (28.0 + rank * 5.0) * power;
                shot.poison_duration = 4.0 + rank * 0.3;
                shot.armor_pierce = if daggers { 12.0 } else { 22.0 };
                shot.speed = if daggers { 500.0 } else { 460.0 };
                crate::tower::spawn_projectile(
                    commands,
                    source.pos,
                    target.pos,
                    TowerKind::Arrow,
                    shot,
                    color,
                    meshes,
                    materials,
                );
                vfx.write(VfxEvent::Muzzle {
                    pos: source.pos,
                    dir: (target.pos - source.pos).normalize_or_zero(),
                    color,
                });
            }
        }
        (HeroWeapon::ShadowBow, 1) => {
            let dir = (first.pos - source.pos).normalize_or_zero();
            let end = source.pos + dir * 420.0;
            for target in targets
                .iter()
                .filter(|t| point_segment_distance(t.pos, source.pos, end) <= 22.0 + rank * 2.0)
            {
                dmg.write(Damage {
                    source_tower: Some(owner),
                    target: target.entity,
                    amount: (140.0 + source.damage * 1.25) * power,
                    magic: false,
                    element: Element::Physical,
                    armor_pierce: 80.0 + rank * 8.0,
                });
            }
            beam(commands, source.pos, end, color, 5.0);
            vfx.write(VfxEvent::Muzzle {
                pos: source.pos,
                dir,
                color,
            });
        }
        (HeroWeapon::ShadowBow, 2) => {
            fields::spawn(
                commands,
                owner,
                source,
                weapon,
                SkillFieldKind::ArrowStorm,
                power,
                first.pos,
                230.0,
            );
        }
        (HeroWeapon::NightDagger, 0) => {
            let target = targets
                .iter()
                .max_by(|a, b| {
                    a.boss.cmp(&b.boss).then_with(|| {
                        (b.hp / b.max_hp.max(1.0)).total_cmp(&(a.hp / a.max_hp.max(1.0)))
                    })
                })
                .copied()
                .unwrap_or(first);
            let facing = if target.facing.length_squared() > 0.01 {
                target.facing.normalize()
            } else {
                (target.pos - source.pos).normalize_or_zero()
            };
            let destination = clamp_guard_pos(target.pos - facing * TILE_SIZE * 0.55);
            let Ok((_, mut hero)) = towers.get_mut(owner) else {
                return false;
            };
            hero.hero_pos = destination;
            hero.move_target = None;
            hero.angle = facing.to_angle();
            dmg.write(Damage {
                source_tower: Some(owner),
                target: target.entity,
                amount: (180.0 + source.damage * 1.4) * power * if target.boss { 1.2 } else { 1.0 },
                magic: true,
                element: Element::Shadow,
                armor_pierce: 30.0,
            });
            status.write(Status {
                source_tower: Some(owner),
                target: target.entity,
                kind: StatusKind::Curse {
                    reduce: 18.0 + rank * 3.0,
                    duration: 3.0,
                },
            });
            vfx.write(VfxEvent::ElementPulse {
                pos: source.pos,
                color,
                strong: false,
            });
            vfx.write(VfxEvent::Slash {
                pos: target.pos,
                angle: facing.to_angle(),
                color,
                poison: false,
            });
        }
        (HeroWeapon::NightDagger, 2) => {
            let target = targets
                .iter()
                .min_by(|a, b| (a.hp / a.max_hp.max(1.0)).total_cmp(&(b.hp / b.max_hp.max(1.0))))
                .copied()
                .unwrap_or(first);
            let missing = (target.max_hp - target.hp).max(0.0);
            let execution =
                (missing * if target.boss { 0.16 } else { 0.5 }).min(1500.0 + source.damage * 2.0);
            dmg.write(Damage {
                source_tower: Some(owner),
                target: target.entity,
                amount: (260.0 + source.damage * 1.8) * power + execution,
                magic: true,
                element: Element::Shadow,
                armor_pierce: 100.0,
            });
            let angle = (target.pos - source.pos).to_angle();
            vfx.write(VfxEvent::Slash {
                pos: target.pos,
                angle: angle + 0.6,
                color,
                poison: false,
            });
            vfx.write(VfxEvent::Slash {
                pos: target.pos,
                angle: angle - 0.6,
                color,
                poison: false,
            });
            beam(commands, source.pos, target.pos, color, 2.0);
        }
        _ => return false,
    }
    true
}
