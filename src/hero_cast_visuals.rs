//! Cast poses are driven by simulation progress, so pause and speed stay in sync.

use crate::hero::HeroWeapon;
use crate::hero_passives::{CastPhase, HeroCast};
use crate::tower::Tower;
use bevy::prelude::*;

pub fn pose_casts(
    mut heroes: Query<(&Tower, &HeroCast, &crate::build::HeroWalkAnim, &mut Sprite)>,
) {
    for (_, cast, animation, mut sprite) in &mut heroes {
        let Some(atlas) = sprite.texture_atlas.as_mut() else {
            continue;
        };
        let progress = (1.0 - cast.remaining / cast.duration).clamp(0.0, 1.0);
        let fraction = match cast.phase {
            CastPhase::Windup => progress * 0.4,
            CastPhase::Release => 0.4 + progress * 0.6,
            CastPhase::Recovery => 1.0 - progress,
        };
        atlas.index = (fraction * animation.frames.saturating_sub(1) as f32).round() as usize;
    }
}

pub fn draw_casts(mut gizmos: Gizmos, heroes: Query<(&Tower, &HeroCast)>) {
    for (hero, cast) in &heroes {
        let p = (1.0 - cast.remaining / cast.duration).clamp(0.0, 1.0);
        let alpha = match cast.phase {
            CastPhase::Windup => 0.35 + p * 0.6,
            CastPhase::Release => 0.95,
            CastPhase::Recovery => (1.0 - p) * 0.6,
        };
        let color = cast.weapon.skill_color().with_alpha(alpha);
        let center = hero.center();
        let dir = Vec2::from_angle(hero.angle);
        let side = dir.perp();
        let size = if cast.slot == 2 { 46.0 } else { 30.0 };
        match cast.weapon {
            HeroWeapon::SummonStaff => {
                // A closing portal and three contract runes precede the ally spawn.
                let radius = size * (1.3 - p * 0.3);
                gizmos.circle_2d(center, radius, color);
                for i in 0..3 {
                    let a = i as f32 * std::f32::consts::TAU / 3.0;
                    let point = center + Vec2::from_angle(a) * radius;
                    gizmos.linestrip_2d(
                        [
                            point + Vec2::Y * 7.0,
                            point + Vec2::X * 5.0,
                            point - Vec2::Y * 7.0,
                            point - Vec2::X * 5.0,
                            point + Vec2::Y * 7.0,
                        ],
                        color,
                    );
                }
            }
            HeroWeapon::BannerSword | HeroWeapon::NightDagger => {
                let reach = size * (0.7 + p * 0.6);
                gizmos.linestrip_2d(
                    [
                        center + side * 18.0,
                        center + dir * reach,
                        center - side * 18.0,
                    ],
                    color,
                );
            }
            HeroWeapon::ShadowBow | HeroWeapon::SentryCrossbow => {
                let end = center + dir * size * 2.0;
                gizmos.line_2d(center + dir * 22.0, end, color);
                gizmos.line_2d(end - side * 9.0, end + side * 9.0, color);
                gizmos.line_2d(end - dir * 9.0, end + dir * 9.0, color);
            }
            HeroWeapon::ForgeHammer => {
                gizmos.linestrip_2d(
                    [
                        center + Vec2::new(-size, -size * 0.55),
                        center + Vec2::new(-size, size * 0.55),
                        center + Vec2::new(size, size * 0.55),
                        center + Vec2::new(size, -size * 0.55),
                        center + Vec2::new(-size, -size * 0.55),
                    ],
                    color,
                );
                gizmos.line_2d(center - Vec2::X * size, center + Vec2::X * size, color);
            }
            HeroWeapon::OathShield => {
                gizmos.linestrip_2d(
                    [
                        center + Vec2::new(-size, 16.0),
                        center + Vec2::new(0.0, 24.0),
                        center + Vec2::new(size, 16.0),
                        center + Vec2::new(size * 0.7, -14.0),
                        center - Vec2::Y * size,
                        center + Vec2::new(-size * 0.7, -14.0),
                        center + Vec2::new(-size, 16.0),
                    ],
                    color,
                );
            }
            HeroWeapon::StormOrb | HeroWeapon::StarfireStaff => {
                for i in 0..4 {
                    let a = i as f32 * std::f32::consts::FRAC_PI_2;
                    let ray = Vec2::from_angle(a);
                    gizmos.linestrip_2d(
                        [
                            center + ray * size,
                            center + ray * size * 0.55 + ray.perp() * 7.0,
                            center + ray * 10.0,
                        ],
                        color,
                    );
                }
            }
        }
    }
}
