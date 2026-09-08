use super::*;
use crate::data::{EnemyKind, TowerKind};
use crate::tower::{FixedSummonHome, ProjKind, StatusKind, TemporaryGuard};
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
    sprites: &Sprites,
    creatures: &crate::creatures::Creatures,
    dmg: &mut MessageWriter<Damage>,
    status: &mut MessageWriter<Status>,
    vfx: &mut MessageWriter<VfxEvent>,
    run: &mut RunState,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    summon_slots: &mut usize,
) -> bool {
    let color = weapon.skill_color();
    match (weapon, slot) {
        (HeroWeapon::OathShield, 0) => {
            let radius = 112.0 + rank * 8.0;
            let Some(first) = targets
                .iter()
                .find(|t| t.pos.distance(source.pos) <= radius)
            else {
                return false;
            };
            let dir = (first.pos - source.pos).normalize_or_zero();
            for target in targets.iter().filter(|t| {
                t.pos.distance(source.pos) <= radius
                    && ((t.pos - source.pos).normalize_or_zero().dot(dir) >= 0.0
                        || t.pos.distance(source.pos) < 20.0)
            }) {
                dmg.write(Damage {
                    source_tower: Some(owner),
                    target: target.entity,
                    amount: (80.0 + source.damage * 0.8) * power,
                    magic: false,
                    element: source.element,
                    armor_pierce: 22.0,
                });
                status.write(Status {
                    source_tower: Some(owner),
                    target: target.entity,
                    kind: StatusKind::Knockback {
                        dist: 30.0 + rank * 6.0,
                        stun: 0.65 + rank * 0.1,
                    },
                });
            }
            if let Ok((_, mut hero)) = towers.get_mut(owner) {
                hero.angle = dir.to_angle();
            }
            vfx.write(VfxEvent::HammerImpact {
                pos: source.pos + dir * 40.0,
                angle: dir.to_angle(),
                color,
            });
        }
        (HeroWeapon::OathShield, 1) => {
            let mut healed = false;
            let radius = 150.0 + rank * 14.0;
            for (_, mut tower) in towers.iter_mut() {
                if tower.hp > 0.0
                    && tower.hp < tower.max_hp
                    && tower.center().distance(source.pos) <= radius
                {
                    tower.hp = (tower.hp + tower.max_hp * (0.16 + rank * 0.025)).min(tower.max_hp);
                    healed = true;
                    beam(commands, source.pos, tower.center(), color, 3.0);
                    vfx.write(VfxEvent::Heal {
                        pos: tower.center(),
                    });
                }
            }
            if run.lives < run.start_lives {
                run.lives += 1;
                healed = true;
            }
            if !healed {
                return false;
            }
            vfx.write(VfxEvent::Burst {
                pos: source.pos,
                radius,
                color,
            });
        }
        (HeroWeapon::OathShield, 2) => {
            let injured = towers.iter().any(|(_, t)| {
                t.hp > 0.0 && t.hp < t.max_hp && t.center().distance(source.pos) <= 180.0
            });
            if !injured
                && !targets.iter().any(|t| t.pos.distance(source.pos) <= 180.0)
                && run.lives >= run.start_lives
            {
                return false;
            }
            if run.lives < run.start_lives {
                run.lives += 1;
            }
            fields::spawn(
                commands,
                owner,
                source,
                weapon,
                SkillFieldKind::Sanctuary,
                power,
                source.pos,
                180.0,
            );
            vfx.write(VfxEvent::MeleeCleave {
                pos: source.pos,
                radius: 180.0,
                color,
            });
        }
        (HeroWeapon::SentryCrossbow, 0) => {
            let anchors: Vec<_> = towers
                .iter()
                .filter(|(entity, t)| {
                    *entity != owner
                        && t.hp > 0.0
                        && t.center().distance(source.pos) <= 180.0 + rank * 12.0
                })
                .map(|(_, t)| t.center())
                .collect();
            let mut hits = 0;
            for anchor in anchors {
                for target in targets
                    .iter()
                    .filter(|t| t.pos.distance(anchor) <= 225.0 + rank * 10.0)
                    .take(1 + rank as usize / 2)
                    .copied()
                {
                    let mut shot = projectile(
                        owner,
                        target,
                        (65.0 + source.damage * 0.5) * power,
                        Element::Frost,
                    );
                    shot.kind = ProjKind::Slow;
                    shot.slow_duration = 1.4 + rank * 0.15;
                    crate::tower::spawn_projectile(
                        commands,
                        anchor,
                        target.pos,
                        TowerKind::Arrow,
                        shot,
                        color,
                        meshes,
                        materials,
                    );
                    beam(commands, source.pos, anchor, color, 2.0);
                    hits += 1;
                }
            }
            if hits == 0 {
                return false;
            }
        }
        (HeroWeapon::SentryCrossbow, 1) => {
            let Some(target) = targets.first().copied() else {
                return false;
            };
            let mut shot = projectile(
                owner,
                target,
                (85.0 + source.damage * 0.7) * power,
                Element::Frost,
            );
            shot.kind = ProjKind::Curse;
            shot.freeze_duration = 0.9 + rank * 0.16;
            shot.armor_reduce = 16.0 + rank * 4.0;
            shot.curse_duration = 3.0 + rank * 0.2;
            crate::tower::spawn_projectile(
                commands,
                source.pos,
                target.pos,
                TowerKind::Ice,
                shot,
                Element::Frost.color(),
                meshes,
                materials,
            );
            vfx.write(VfxEvent::Muzzle {
                pos: source.pos,
                dir: (target.pos - source.pos).normalize_or_zero(),
                color,
            });
        }
        (HeroWeapon::SentryCrossbow, 2) => {
            if !targets.iter().any(|t| t.pos.distance(source.pos) <= 310.0) {
                return false;
            }
            let field = fields::spawn(
                commands,
                owner,
                source,
                weapon,
                SkillFieldKind::Sentry,
                power,
                source.pos,
                310.0,
            );
            for i in 0..3 {
                let offset =
                    Vec2::from_angle(i as f32 * std::f32::consts::TAU / 3.0) * TILE_SIZE * 0.8;
                commands.entity(field).with_children(|p| {
                    p.spawn((
                        Sprite {
                            image: sprites
                                .towers
                                .get(&TowerKind::Arrow)
                                .cloned()
                                .unwrap_or_default(),
                            custom_size: Some(Vec2::splat(TILE_SIZE * 0.85)),
                            color: Color::srgb(0.72, 0.9, 1.0),
                            ..default()
                        },
                        Transform::from_translation(offset.extend(2.0)),
                    ));
                });
                vfx.write(VfxEvent::ElementPulse {
                    pos: source.pos + offset,
                    color,
                    strong: false,
                });
            }
        }
        (HeroWeapon::SummonStaff, 0) => {
            let count = (1 + rank as usize / 3).min(*summon_slots);
            if count == 0 {
                return false;
            }
            *summon_slots -= count;
            for pos in guard_positions(source, count, targets) {
                crate::tower::spawn_mythic_ally(
                    commands,
                    sprites.mythic_summon.clone(),
                    pos,
                    (source.max_hp * (0.26 + rank * 0.035)).max(220.0),
                    (52.0 + source.damage * 0.42) * power,
                    78.0 + rank * 4.0,
                    14.0 + rank * 1.2,
                    owner,
                );
                vfx.write(VfxEvent::HolyStrike { pos, strong: true });
            }
        }
        (HeroWeapon::SummonStaff, 1) => {
            let count = (2 + rank as usize / 2)
                .min(*summon_slots)
                .min(run.fallen_enemies.len());
            if count == 0 {
                return false;
            }
            *summon_slots -= count;
            for (index, soul) in run.fallen_enemies.drain(..count).enumerate() {
                let pos = clamp_guard_pos(
                    source.pos
                        + Vec2::from_angle(index as f32 * std::f32::consts::TAU / count as f32)
                            * TILE_SIZE
                            * 0.65,
                );
                crate::tower::spawn_ally(
                    commands,
                    creatures,
                    soul.kind,
                    pos,
                    (soul.max_hp * (0.24 + rank * 0.02))
                        .clamp(170.0, (source.max_hp * 0.45).max(170.0)),
                    (40.0 + source.damage * 0.34) * power,
                    74.0,
                    12.0 + rank,
                    0.7,
                    owner,
                );
                beam(commands, soul.pos, pos, color, 2.0);
                vfx.write(VfxEvent::HolyStrike { pos, strong: false });
            }
        }
        (HeroWeapon::SummonStaff, 2) => {
            if *summon_slots == 0 {
                return false;
            }
            let count = 3.min(*summon_slots);
            *summon_slots -= count;
            for (index, pos) in guard_positions(source, count, targets)
                .into_iter()
                .enumerate()
            {
                let elder = index == 0;
                crate::tower::spawn_ally(
                    commands,
                    creatures,
                    if elder {
                        EnemyKind::Tank
                    } else {
                        EnemyKind::Fast
                    },
                    pos,
                    if elder {
                        (source.max_hp * 0.9).max(720.0)
                    } else {
                        (source.max_hp * 0.24).max(210.0)
                    },
                    (if elder {
                        150.0 + source.damage * 0.9
                    } else {
                        48.0 + source.damage * 0.35
                    }) * power,
                    if elder { 58.0 } else { 112.0 },
                    26.0,
                    if elder { 1.4 } else { 0.68 },
                    owner,
                );
                vfx.write(VfxEvent::HolyStrike { pos, strong: elder });
            }
        }
        (HeroWeapon::ForgeHammer, 0 | 2) => {
            if targets.is_empty() {
                return false;
            }
            let workshop = slot == 2;
            let count = (if workshop { 4 } else { 2 + rank as usize / 3 }).min(*summon_slots);
            if count == 0 {
                return false;
            }
            *summon_slots -= count;
            for pos in guard_positions(source, count, targets) {
                let guard = crate::tower::spawn_ally(
                    commands,
                    creatures,
                    EnemyKind::Shielded,
                    pos,
                    (source.max_hp * if workshop { 0.42 } else { 0.22 + rank * 0.025 }).max(190.0),
                    (38.0 + source.damage * if workshop { 0.65 } else { 0.42 }) * power,
                    52.0,
                    if workshop { 22.0 } else { 11.0 + rank },
                    if workshop { 0.88 } else { 0.72 },
                    owner,
                );
                commands.entity(guard).insert((
                    TemporaryGuard,
                    FixedSummonHome {
                        pos,
                        range: TILE_SIZE * if workshop { 2.6 } else { 2.0 + rank * 0.1 },
                    },
                ));
                vfx.write(VfxEvent::HammerImpact {
                    pos,
                    angle: source.facing.to_angle(),
                    color,
                });
            }
            if workshop {
                fields::spawn(
                    commands,
                    owner,
                    source,
                    weapon,
                    SkillFieldKind::Workshop,
                    power,
                    source.pos,
                    185.0,
                );
            }
        }
        (HeroWeapon::ForgeHammer, 1) => {
            if targets.is_empty() {
                return false;
            }
            let count = (3 + rank as usize / 2).min(targets.len());
            for (index, target) in targets.iter().take(count).copied().enumerate() {
                let origin = source.pos
                    + Vec2::new(-source.facing.y, source.facing.x)
                        * ((index as f32 - (count - 1) as f32 * 0.5) * 12.0);
                let mut shot = projectile(
                    owner,
                    target,
                    (110.0 + source.damage * 0.5) * power,
                    Element::Fire,
                );
                shot.kind = ProjKind::Missile;
                shot.speed = 360.0;
                shot.aoe_radius = 60.0 + rank * 4.0;
                shot.dot_damage = (24.0 + rank * 4.0) * power;
                shot.poison_duration = 2.4 + rank * 0.2;
                shot.armor_pierce = 20.0;
                crate::tower::spawn_projectile(
                    commands,
                    origin,
                    target.pos,
                    TowerKind::Cannon,
                    shot,
                    color,
                    meshes,
                    materials,
                );
            }
        }
        _ => return false,
    }
    true
}
