//! Map combat releases and confirmed hits onto PixelFX clips.
use super::*;

/// One release recipe for each of the game's 27 class skills.
pub fn skill_effect(weapon: HeroWeapon, slot: usize) -> (&'static str, bool) {
    use HeroWeapon::*;
    match (weapon, slot) {
        (BannerSword, 0) => ("melee/thrust", true),
        (BannerSword, 1) => ("melee/spin-slash", false),
        (BannerSword, _) => ("nature/quake", false),
        (OathShield, 0) => ("impacts/shield-block", true),
        (OathShield, 1) => ("holy/heal-aoe", false),
        (OathShield, _) => ("holy/divine-shield", false),
        (ShadowBow, 0) => ("projectiles/arrow", true),
        (ShadowBow, 1) => ("projectiles/bullet-tracer", true),
        (ShadowBow, _) => ("nature/leaf-storm", false),
        (SentryCrossbow, 0) => ("arcane/mana-burst", false),
        (SentryCrossbow, 1) => ("ice/ice-lance", true),
        (SentryCrossbow, _) => ("scifi/laser-grid", false),
        (StormOrb, 0) => ("lightning/spark-arc", true),
        (StormOrb, 1) => ("nature/tornado", false),
        (StormOrb, _) => ("lightning/storm-cloud", false),
        (StarfireStaff, 0) => ("fire/flamethrower", true),
        (StarfireStaff, 1) => ("ice/frost-nova", false),
        (StarfireStaff, _) => ("fire/meteor", false),
        (NightDagger, 0) => ("arcane/blink-out", false),
        (NightDagger, 1) => ("melee/dagger-flurry", true),
        (NightDagger, _) => ("shadow/dark-explosion", false),
        (SummonStaff, 0) => ("spawn/summon-rise", false),
        (SummonStaff, 1) => ("holy/resurrection", false),
        (SummonStaff, _) => ("shadow/necro-skulls", false),
        (ForgeHammer, 0) => ("scifi/warp-in", false),
        (ForgeHammer, 1) => ("projectiles/rocket", true),
        (ForgeHammer, _) => ("scifi/force-field", false),
    }
}

fn release(id: &str, pos: Vec2, angle: f32, duration: f32) -> PixelFxRequest {
    PixelFxRequest {
        angle,
        scale: 0.65,
        duration: Some(duration),
        start_at_contact: true,
        ..PixelFxRequest::new(id, pos)
    }
}

pub(super) fn route_skills(
    mut events: MessageReader<HeroSkillCastEvent>,
    heroes: Query<(Entity, &Tower, Option<&HeroCast>)>,
    projectiles: Query<
        (Entity, &crate::tower::Projectile, &Transform),
        Added<crate::tower::Projectile>,
    >,
    fields: Query<
        (Entity, &crate::hero_skill_effects::SkillField),
        Added<crate::hero_skill_effects::SkillField>,
    >,
    mut requests: MessageWriter<PixelFxRequest>,
    mut charging: Local<HashSet<Entity>>,
) {
    let mut active = HashSet::new();
    for (entity, hero, cast) in &heroes {
        if !hero.hero {
            continue;
        }
        if let Some(cast) = cast
            && cast.phase == CastPhase::Windup
        {
            active.insert(entity);
            if !charging.contains(&entity) {
                let mut request = PixelFxRequest::new("arcane/spell-charge", hero.center());
                request.duration = Some(cast.duration);
                request.follow = Some(entity);
                request.windup_only = true;
                request.important = true;
                request.scale = 0.55;
                requests.write(request);
            }
        }
    }
    *charging = active;
    for event in events.read() {
        let Ok((_, hero, _)) = heroes.get(event.owner) else {
            continue;
        };
        let (id, directional) = skill_effect(event.weapon, event.slot);
        let field = fields
            .iter()
            .find(|(_, field)| field.owner == event.owner && field.weapon == event.weapon);
        let position = field.map_or(hero.center(), |(_, field)| field.center);
        let mut request = release(
            id,
            position,
            if directional {
                event.direction.to_angle()
            } else {
                0.0
            },
            if event.slot == 2 { 1.3 } else { 0.7 },
        );
        request.important = true;
        request.scale = if event.slot == 2 { 1.0 } else { 0.8 };
        if let Some((entity, field)) = field {
            request.duration = Some(field.remaining_duration().max(0.1));
            request.follow = Some(entity);
        } else if id.starts_with("projectiles/") || id == "ice/ice-lance" {
            let mut attached = false;
            for (entity, projectile, transform) in &projectiles {
                if projectile.source_tower != Some(event.owner) {
                    continue;
                }
                let mut flight = request.clone();
                flight.position = transform.translation.truncate();
                flight.angle = transform.rotation.to_euler(EulerRot::XYZ).2;
                flight.duration = Some(3.0);
                flight.follow = Some(entity);
                flight.follow_rotation = true;
                requests.write(flight);
                attached = true;
            }
            if attached {
                continue;
            }
        }
        requests.write(request);
    }
}

pub(super) fn route_impacts(
    mut events: MessageReader<VfxEvent>,
    mut requests: MessageWriter<PixelFxRequest>,
    time: Res<Time>,
    mut budget: Local<crate::combat_feel::ImpactBudget>,
) {
    use crate::combat_feel::ImpactTier;
    budget.begin_frame(time.delta_secs());
    for event in events.read() {
        let request = match event {
            VfxEvent::Impact {
                pos,
                direction,
                tier,
                ..
            } if budget.admit(*tier) => {
                let id = match tier {
                    ImpactTier::Normal => "impacts/hit-spark",
                    ImpactTier::Heavy => "impacts/blunt-hit",
                    ImpactTier::Empowered => "lightning/zap-hit",
                    ImpactTier::Ultimate => "impacts/crit-hit",
                    ImpactTier::Finisher => "holy/smite",
                };
                let mut request = release(
                    id,
                    *pos,
                    if *tier == ImpactTier::Finisher {
                        0.0
                    } else {
                        direction.to_angle()
                    },
                    if *tier == ImpactTier::Finisher {
                        0.7
                    } else {
                        0.3
                    },
                );
                request.important = *tier >= ImpactTier::Ultimate;
                request.scale = match tier {
                    ImpactTier::Normal => 0.38,
                    ImpactTier::Heavy | ImpactTier::Empowered => 0.6,
                    ImpactTier::Ultimate => 0.8,
                    ImpactTier::Finisher => 1.0,
                };
                Some(request)
            }
            VfxEvent::Slash {
                pos, angle, poison, ..
            } => Some(release(
                if *poison {
                    "melee/dagger-flurry"
                } else {
                    "melee/sword-slash"
                },
                *pos,
                *angle,
                0.3,
            )),
            VfxEvent::HammerImpact { pos, angle, .. } => {
                Some(release("melee/hammer-smash", *pos, *angle, 0.45))
            }
            VfxEvent::Heal { pos } if budget.admit(ImpactTier::Normal) => {
                Some(release("holy/heal", *pos, 0.0, 0.6))
            }
            VfxEvent::HolyStrike { pos, .. } if budget.admit(ImpactTier::Heavy) => {
                Some(release("holy/smite", *pos, 0.0, 0.45))
            }
            _ => None,
        };
        if let Some(request) = request {
            requests.write(request);
        }
    }
}
