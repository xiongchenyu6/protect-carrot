use super::*;
use crate::tower::StatusKind;
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
    dmg: &mut MessageWriter<Damage>,
    status: &mut MessageWriter<Status>,
    vfx: &mut MessageWriter<VfxEvent>,
) -> bool {
    let Some(first) = targets.first().copied() else {
        return false;
    };
    let color = weapon.skill_color();
    match (weapon, slot) {
        (HeroWeapon::StarfireStaff, 0) => {
            let center = first.pos;
            let corridor = center - source.pos;
            let length = corridor.length().max(1.0);
            let width = 30.0 + rank * 4.0;
            let life = 4.0 + rank * 0.4;
            commands.spawn((
                Sprite {
                    color: Element::Fire.color().with_alpha(0.32),
                    custom_size: Some(Vec2::new(length, width * 2.0)),
                    ..default()
                },
                Transform {
                    translation: ((source.pos + center) * 0.5).extend(3.55),
                    rotation: Quat::from_rotation_z(corridor.to_angle()),
                    ..default()
                },
                crate::components::FireGround {
                    half_len: length * 0.5,
                    half_width: width,
                    angle: corridor.to_angle(),
                    dps: (32.0 + source.damage * 0.15) * power,
                    element: Element::Fire,
                    source_tower: Some(owner),
                    life,
                    max_life: life,
                    ember_timer: 0.0,
                },
                crate::components::LevelEntity,
            ));
            for target in targets
                .iter()
                .filter(|t| point_segment_distance(t.pos, source.pos, center) <= width)
            {
                dmg.write(Damage {
                    source_tower: Some(owner),
                    target: target.entity,
                    amount: (115.0 + source.damage * 0.7) * power,
                    magic: true,
                    element: Element::Fire,
                    armor_pierce: 0.0,
                });
                status.write(Status {
                    source_tower: Some(owner),
                    target: target.entity,
                    kind: StatusKind::Fire {
                        dmg: 24.0 * power,
                        duration: life,
                        element: Element::Fire,
                    },
                });
            }
            beam(commands, source.pos, center, Element::Fire.color(), 14.0);
            vfx.write(VfxEvent::Explosion {
                pos: center,
                radius: width * 1.5,
                color: Element::Fire.color(),
            });
        }
        (HeroWeapon::StarfireStaff, 1) => {
            beam(commands, source.pos, first.pos, Element::Frost.color(), 4.0);
            fields::spawn(
                commands,
                owner,
                source,
                weapon,
                SkillFieldKind::FrostPrison,
                power,
                first.pos,
                82.0 + rank * 10.0,
            );
            vfx.write(VfxEvent::ElementPulse {
                pos: first.pos,
                color: Element::Frost.color(),
                strong: true,
            });
        }
        (HeroWeapon::StarfireStaff, 2) => {
            fields::spawn(
                commands,
                owner,
                source,
                weapon,
                SkillFieldKind::MeteorShower,
                power,
                first.pos,
                250.0,
            );
            vfx.write(VfxEvent::MeteorStorm {
                center: first.pos,
                radius: 250.0,
            });
        }
        (HeroWeapon::StormOrb, 0) => {
            let mut chain = vec![first];
            let jump_range = 160.0 + rank * 14.0;
            while chain.len() < 5 + rank as usize {
                let from = chain.last().expect("chain starts with a target").pos;
                let next = targets
                    .iter()
                    .filter(|target| {
                        !chain.iter().any(|hit| hit.entity == target.entity)
                            && target.pos.distance(from) <= jump_range
                    })
                    .min_by(|a, b| {
                        a.pos
                            .distance_squared(from)
                            .total_cmp(&b.pos.distance_squared(from))
                    })
                    .copied();
                let Some(next) = next else {
                    break;
                };
                chain.push(next);
            }
            let mut previous = source.pos;
            for (index, target) in chain.into_iter().enumerate() {
                dmg.write(Damage {
                    source_tower: Some(owner),
                    target: target.entity,
                    amount: (160.0 + source.damage * 0.7) * power * 0.86_f32.powi(index as i32),
                    magic: true,
                    element: Element::Storm,
                    armor_pierce: 0.0,
                });
                status.write(Status {
                    source_tower: Some(owner),
                    target: target.entity,
                    kind: StatusKind::Slow {
                        duration: 1.1 + rank * 0.15,
                    },
                });
                let mid = (previous + target.pos) * 0.5
                    + Vec2::new(-(target.pos - previous).y, (target.pos - previous).x)
                        .normalize_or_zero()
                        * 10.0;
                beam(commands, previous, mid, color, 3.0);
                beam(commands, mid, target.pos, color, 3.0);
                vfx.write(VfxEvent::Explosion {
                    pos: target.pos,
                    radius: 23.0,
                    color,
                });
                previous = target.pos;
            }
        }
        (HeroWeapon::StormOrb, 1) => {
            fields::spawn(
                commands,
                owner,
                source,
                weapon,
                SkillFieldKind::Vortex,
                power,
                first.pos,
                112.0 + rank * 9.0,
            );
            vfx.write(VfxEvent::MeleeCleave {
                pos: first.pos,
                radius: 112.0 + rank * 9.0,
                color,
            });
        }
        (HeroWeapon::StormOrb, 2) => {
            fields::spawn(
                commands,
                owner,
                source,
                weapon,
                SkillFieldKind::Thunderstorm,
                power,
                first.pos,
                260.0,
            );
        }
        _ => return false,
    }
    true
}
