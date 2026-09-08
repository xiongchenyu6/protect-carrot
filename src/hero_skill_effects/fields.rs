use super::*;
use crate::game::Paused;
use crate::tower::{ProjKind, StatusKind};
use crate::vfx::VfxEvent;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkillFieldKind {
    Quake,
    FrostPrison,
    MeteorShower,
    ArrowStorm,
    Sanctuary,
    Vortex,
    Thunderstorm,
    Sentry,
    Workshop,
}

#[derive(Component)]
pub struct SkillField {
    pub kind: SkillFieldKind,
    pub owner: Entity,
    pub weapon: HeroWeapon,
    pub center: Vec2,
    pub radius: f32,
    pub remaining: usize,
    timer: f32,
    interval: f32,
    step: usize,
    damage: f32,
    max_hp: f32,
    power: f32,
}

pub(super) fn spawn(
    commands: &mut Commands,
    owner: Entity,
    source: PassiveSource,
    weapon: HeroWeapon,
    kind: SkillFieldKind,
    power: f32,
    center: Vec2,
    radius: f32,
) -> Entity {
    let (remaining, interval) = match kind {
        SkillFieldKind::Quake => (3, 0.32),
        SkillFieldKind::FrostPrison => (5, 0.5),
        SkillFieldKind::MeteorShower => (5, 0.55),
        SkillFieldKind::ArrowStorm => (6, 0.42),
        SkillFieldKind::Sanctuary => (6, 0.8),
        SkillFieldKind::Vortex => (8, 0.4),
        SkillFieldKind::Thunderstorm => (6, 0.6),
        SkillFieldKind::Sentry => (8, 0.7),
        SkillFieldKind::Workshop => (5, 1.4),
    };
    commands
        .spawn((
            SkillField {
                kind,
                owner,
                weapon,
                center,
                radius,
                remaining,
                timer: 0.06,
                interval,
                step: 0,
                damage: source.damage,
                max_hp: source.max_hp,
                power,
            },
            Transform::from_translation(center.extend(3.5)),
            Visibility::default(),
            crate::components::LevelEntity,
        ))
        .id()
}

/// Persistent released skills retain their owner and stop with death or weapon changes.
pub fn update_skill_fields(
    mut commands: Commands,
    time: Res<Time>,
    paused: Res<Paused>,
    run: Res<RunState>,
    loadout: Res<HeroLoadout>,
    mut fields: Query<(Entity, &mut SkillField)>,
    mut enemies: Query<(Entity, &mut Enemy, &mut Transform)>,
    mut towers: Query<(Entity, &mut Tower)>,
    mut dmg: MessageWriter<Damage>,
    mut status: MessageWriter<Status>,
    mut vfx: MessageWriter<VfxEvent>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let dt = time.delta_secs() * run.game_speed;
    for (entity, mut field) in &mut fields {
        let alive = loadout.alive
            && run.lives > 0
            && loadout.weapon == field.weapon
            && towers
                .get(field.owner)
                .is_ok_and(|(_, tower)| tower.hero && tower.hp > 0.0);
        if !alive {
            commands.entity(entity).despawn();
            continue;
        }
        if paused.0 || !run.wave_in_progress || dt <= 0.0 {
            continue;
        }
        field.timer -= dt;
        while field.timer <= 0.0 && field.remaining > 0 {
            field.timer += field.interval;
            let owner = field.owner;
            let color = field.weapon.skill_color();
            let center = field.center;
            let mut local: Vec<_> = enemies
                .iter()
                .filter(|(_, e, tf)| {
                    e.hp > 0.0 && tf.translation.truncate().distance(center) <= field.radius
                })
                .map(|(entity, e, tf)| Target {
                    entity,
                    pos: tf.translation.truncate(),
                    hp: e.hp,
                    max_hp: e.max_hp,
                    progress: e.path_index,
                    boss: e.boss,
                    facing: e.facing,
                })
                .collect();
            local.sort_by(|a, b| {
                b.progress
                    .cmp(&a.progress)
                    .then_with(|| a.entity.cmp(&b.entity))
            });
            match field.kind {
                SkillFieldKind::ArrowStorm | SkillFieldKind::Sentry => {
                    for (index, target) in local
                        .iter()
                        .take(if field.kind == SkillFieldKind::Sentry {
                            3
                        } else {
                            4
                        })
                        .copied()
                        .enumerate()
                    {
                        let origin = if field.kind == SkillFieldKind::ArrowStorm {
                            target.pos + Vec2::new(35.0 + index as f32 * 8.0, 180.0)
                        } else {
                            center
                                + Vec2::from_angle(index as f32 * std::f32::consts::TAU / 3.0)
                                    * TILE_SIZE
                                    * 0.8
                        };
                        let mut shot = projectile(
                            owner,
                            target,
                            (48.0 + field.damage * 0.30) * field.power,
                            Element::Physical,
                        );
                        shot.speed = 520.0;
                        shot.armor_pierce = 24.0;
                        if field.kind == SkillFieldKind::Sentry {
                            shot.kind = ProjKind::Slow;
                            shot.slow_duration = 0.8;
                        }
                        crate::tower::spawn_projectile(
                            &mut commands,
                            origin,
                            target.pos,
                            crate::data::TowerKind::Arrow,
                            shot,
                            color,
                            &mut meshes,
                            &mut materials,
                        );
                    }
                }
                SkillFieldKind::Sanctuary | SkillFieldKind::Workshop => {
                    for (_, mut tower) in &mut towers {
                        if tower.hp > 0.0
                            && tower.hp < tower.max_hp
                            && tower.center().distance(center) <= field.radius
                        {
                            tower.hp = (tower.hp
                                + field.max_hp
                                    * if field.kind == SkillFieldKind::Sanctuary {
                                        0.07
                                    } else {
                                        0.035
                                    })
                            .min(tower.max_hp);
                            vfx.write(VfxEvent::Heal {
                                pos: tower.center(),
                            });
                            beam(&mut commands, center, tower.center(), color, 3.0);
                        }
                    }
                    if field.kind == SkillFieldKind::Sanctuary {
                        for index in 0..4 {
                            let foot = center
                                + Vec2::from_angle(index as f32 * std::f32::consts::FRAC_PI_2)
                                    * field.radius
                                    * 0.72;
                            beam(&mut commands, foot, foot + Vec2::Y * 50.0, color, 5.0);
                        }
                        for target in &local {
                            dmg.write(Damage {
                                source_tower: Some(owner),
                                target: target.entity,
                                amount: (30.0 + field.damage * 0.20) * field.power,
                                magic: true,
                                element: Element::Arcane,
                                armor_pierce: 0.0,
                            });
                            status.write(Status {
                                source_tower: Some(owner),
                                target: target.entity,
                                kind: StatusKind::Knockback {
                                    dist: 10.0,
                                    stun: 0.24,
                                },
                            });
                        }
                        vfx.write(VfxEvent::MeleeCleave {
                            pos: center,
                            radius: field.radius,
                            color,
                        });
                    } else {
                        vfx.write(VfxEvent::HammerImpact {
                            pos: center,
                            angle: field.step as f32 * std::f32::consts::FRAC_PI_2,
                            color,
                        });
                    }
                }
                SkillFieldKind::Thunderstorm => {
                    if let Some(target) = local.get(field.step % local.len().max(1)).copied() {
                        beam(
                            &mut commands,
                            target.pos + Vec2::Y * 170.0,
                            target.pos,
                            color,
                            5.0,
                        );
                        dmg.write(Damage {
                            source_tower: Some(owner),
                            target: target.entity,
                            amount: (160.0 + field.damage * 0.62) * field.power,
                            magic: true,
                            element: Element::Storm,
                            armor_pierce: 0.0,
                        });
                        status.write(Status {
                            source_tower: Some(owner),
                            target: target.entity,
                            kind: StatusKind::Freeze { duration: 0.45 },
                        });
                        vfx.write(VfxEvent::Explosion {
                            pos: target.pos,
                            radius: 35.0,
                            color,
                        });
                    }
                }
                SkillFieldKind::MeteorShower => {
                    if let Some(target) = local.get(field.step % local.len().max(1)).copied() {
                        let impact = target.pos;
                        beam(
                            &mut commands,
                            impact + Vec2::new(80.0, 160.0),
                            impact,
                            Element::Fire.color(),
                            14.0,
                        );
                        for hit in local.iter().filter(|t| t.pos.distance(impact) <= 85.0) {
                            dmg.write(Damage {
                                source_tower: Some(owner),
                                target: hit.entity,
                                amount: (145.0 + field.damage * 0.5) * field.power,
                                magic: true,
                                element: Element::Fire,
                                armor_pierce: 0.0,
                            });
                            status.write(Status {
                                source_tower: Some(owner),
                                target: hit.entity,
                                kind: StatusKind::Fire {
                                    dmg: 30.0 * field.power,
                                    duration: 3.0,
                                    element: Element::Fire,
                                },
                            });
                        }
                        vfx.write(VfxEvent::Explosion {
                            pos: impact,
                            radius: 85.0,
                            color: Element::Fire.color(),
                        });
                    }
                }
                SkillFieldKind::Quake | SkillFieldKind::FrostPrison | SkillFieldKind::Vortex => {
                    let radius = if field.kind == SkillFieldKind::Quake {
                        field.radius * (0.6 + field.step as f32 * 0.2)
                    } else {
                        field.radius
                    };
                    for target in local.iter().filter(|t| t.pos.distance(center) <= radius) {
                        let (amount, element, magic) = match field.kind {
                            SkillFieldKind::Quake => (
                                (115.0 + field.damage * 0.55) * field.power,
                                Element::Physical,
                                false,
                            ),
                            SkillFieldKind::FrostPrison => (
                                (30.0 + field.damage * 0.16) * field.power,
                                Element::Frost,
                                true,
                            ),
                            _ => (
                                (24.0 + field.damage * 0.12) * field.power,
                                Element::Storm,
                                true,
                            ),
                        };
                        dmg.write(Damage {
                            source_tower: Some(owner),
                            target: target.entity,
                            amount,
                            element,
                            magic,
                            armor_pierce: 18.0,
                        });
                        if field.kind == SkillFieldKind::Quake {
                            status.write(Status {
                                source_tower: Some(owner),
                                target: target.entity,
                                kind: StatusKind::Knockback {
                                    dist: 18.0,
                                    stun: 0.4,
                                },
                            });
                        } else {
                            status.write(Status {
                                source_tower: Some(owner),
                                target: target.entity,
                                kind: StatusKind::Freeze {
                                    duration: field.interval + 0.1,
                                },
                            });
                            if field.kind == SkillFieldKind::Vortex {
                                if let Ok((_, enemy, mut tf)) = enemies.get_mut(target.entity) {
                                    let delta = center - target.pos;
                                    let pull = delta.normalize_or_zero()
                                        * delta.length().min(if enemy.boss { 6.0 } else { 22.0 });
                                    tf.translation += pull.extend(0.0);
                                }
                                beam(&mut commands, target.pos, center, color, 2.0);
                            }
                        }
                    }
                    match field.kind {
                        SkillFieldKind::Quake => {
                            vfx.write(VfxEvent::HammerImpact {
                                pos: center,
                                angle: field.step as f32 * 2.0,
                                color,
                            });
                            vfx.write(VfxEvent::MeleeCleave {
                                pos: center,
                                radius,
                                color,
                            });
                        }
                        SkillFieldKind::FrostPrison => {
                            for index in 0..6 {
                                let spoke =
                                    Vec2::from_angle(index as f32 * std::f32::consts::TAU / 6.0);
                                let outer = center + spoke * radius;
                                beam(
                                    &mut commands,
                                    outer,
                                    center + spoke * radius * 0.58,
                                    Element::Frost.color(),
                                    5.0,
                                );
                                vfx.write(VfxEvent::Hit {
                                    pos: outer,
                                    color: Element::Frost.color(),
                                    element: Element::Frost,
                                });
                            }
                        }
                        SkillFieldKind::Vortex => {
                            for index in 0..3 {
                                let angle = field.step as f32 * 0.65
                                    + index as f32 * std::f32::consts::TAU / 3.0;
                                let start = center + Vec2::from_angle(angle) * radius;
                                let middle =
                                    center + Vec2::from_angle(angle + 0.55) * radius * 0.72;
                                let end = center + Vec2::from_angle(angle + 1.1) * radius * 0.42;
                                beam(&mut commands, start, middle, color, 3.0);
                                beam(&mut commands, middle, end, color, 2.0);
                            }
                        }
                        _ => unreachable!(),
                    }
                }
            }
            field.remaining -= 1;
            field.step += 1;
        }
        if field.remaining == 0 {
            commands.entity(entity).despawn();
        }
    }
}
