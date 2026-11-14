//! Shared combat presentation tuning. Damage, cooldowns and movement stay in
//! their existing simulation systems; presentation must never freeze the world.

use crate::hero::HeroWeapon;

/// Contact happens after the fast forward stroke, not when release begins.
pub const CONTACT_FRACTION: f32 = 0.35;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum ImpactTier {
    #[default]
    Normal,
    Heavy,
    Empowered,
    Ultimate,
    Finisher,
}

impl ImpactTier {
    pub fn hold(self) -> f32 {
        match self {
            Self::Normal => 0.0,
            Self::Heavy => 0.045,
            Self::Empowered => 0.06,
            Self::Ultimate => 0.075,
            Self::Finisher => 0.085,
        }
    }
    pub fn scale(self) -> f32 {
        match self {
            Self::Normal => 0.65,
            Self::Heavy => 1.0,
            Self::Empowered => 1.25,
            Self::Ultimate => 1.5,
            Self::Finisher => 1.65,
        }
    }
    pub fn shake(self) -> f32 {
        match self {
            Self::Normal => 0.0,
            Self::Heavy => 0.08,
            Self::Empowered => 0.13,
            Self::Ultimate => 0.20,
            Self::Finisher => 0.24,
        }
    }
}

/// Describe real damage, never introduce a new critical-hit roll.
pub fn impact_tier(
    hero: bool,
    heavy: bool,
    ratio: f32,
    weak: bool,
    boss_finish: bool,
) -> ImpactTier {
    if boss_finish {
        ImpactTier::Finisher
    } else if ratio >= 0.65 && weak {
        ImpactTier::Empowered
    } else if hero && (ratio >= 1.8 || (heavy && ratio >= 0.65)) {
        ImpactTier::Heavy
    } else {
        ImpactTier::Normal
    }
}

/// Reserve important particles even when ordinary tower fire fills its budget.
#[derive(Default)]
pub struct ImpactBudget {
    ordinary: u8,
    important: u8,
    ultimate: u8,
    finisher: u8,
    shake_wait: f32,
    last_shake: ImpactTier,
}

impl ImpactBudget {
    pub fn begin_frame(&mut self, dt: f32) {
        self.ordinary = 0;
        self.important = 0;
        self.ultimate = 0;
        self.finisher = 0;
        self.shake_wait = (self.shake_wait - dt).max(0.0);
        if self.shake_wait == 0.0 {
            self.last_shake = ImpactTier::Normal;
        }
    }
    pub fn admit(&mut self, tier: ImpactTier) -> bool {
        let (count, limit) = match tier {
            ImpactTier::Normal => (&mut self.ordinary, 6),
            ImpactTier::Ultimate => (&mut self.ultimate, 2),
            ImpactTier::Finisher => (&mut self.finisher, 1),
            _ => (&mut self.important, 4),
        };
        if *count >= limit {
            return false;
        }
        *count += 1;
        true
    }
    pub fn take_shake(&mut self, tier: ImpactTier) -> f32 {
        if tier == ImpactTier::Normal
            || (self.shake_wait > 0.0 && !(tier >= ImpactTier::Ultimate && tier > self.last_shake))
        {
            return 0.0;
        }
        self.shake_wait = 0.14;
        self.last_shake = tier;
        tier.shake()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CastRhythm {
    pub windup: f32,
    pub release: f32,
    pub recovery: f32,
}

pub fn melee_rhythm(weapon: HeroWeapon) -> CastRhythm {
    match weapon {
        HeroWeapon::NightDagger => CastRhythm {
            windup: 0.055,
            release: 0.055,
            recovery: 0.12,
        },
        HeroWeapon::ForgeHammer => CastRhythm {
            windup: 0.14,
            release: 0.09,
            recovery: 0.22,
        },
        _ => CastRhythm {
            windup: 0.08,
            release: 0.08,
            recovery: 0.16,
        },
    }
}

pub fn cast_rhythm(weapon: HeroWeapon, slot: usize) -> CastRhythm {
    use HeroWeapon::*;
    let (windup, release, recovery) = match weapon {
        NightDagger => (0.16, 0.07, 0.16),
        ShadowBow => (0.23, 0.08, 0.18),
        SentryCrossbow => (0.20, 0.07, 0.20),
        BannerSword => (0.28, 0.10, 0.24),
        OathShield => (0.34, 0.11, 0.28),
        StormOrb => (0.30, 0.09, 0.22),
        StarfireStaff => (0.36, 0.10, 0.26),
        ForgeHammer => (0.46, 0.12, 0.34),
        SummonStaff => (0.52, 0.12, 0.32),
    };
    if slot == 2 {
        CastRhythm {
            windup: windup + 0.24,
            release: release + 0.02,
            recovery: recovery + 0.08,
        }
    } else {
        CastRhythm {
            windup,
            release,
            recovery,
        }
    }
}

/// Slow preparation, a fast strike, then an ease-out return to neutral.
/// The result is a signed forward displacement, not a new simulation position.
pub fn cast_extension(phase: crate::hero_passives::CastPhase, progress: f32) -> f32 {
    use crate::hero_passives::CastPhase::*;
    let t = progress.clamp(0.0, 1.0);
    match phase {
        Windup => -0.35 * t * t,
        Release => {
            let snap = 1.0 - (1.0 - t).powi(3);
            -0.35 + 1.35 * snap
        }
        Recovery => (1.0 - t).powi(3),
    }
}

pub fn cast_lunge(weapon: HeroWeapon) -> f32 {
    match weapon {
        HeroWeapon::ForgeHammer => 14.0,
        HeroWeapon::BannerSword => 12.0,
        HeroWeapon::OathShield => 10.0,
        HeroWeapon::NightDagger => 9.0,
        _ => 4.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hero_passives::CastPhase;

    #[test]
    fn tower_swarm_cannot_starve_heavy_impacts_or_stack_camera_shake() {
        let mut budget = ImpactBudget::default();
        budget.begin_frame(1.0 / 60.0);
        assert_eq!(
            (0..1000)
                .filter(|_| budget.admit(ImpactTier::Normal))
                .count(),
            6
        );
        assert!(budget.admit(ImpactTier::Finisher));
        assert_eq!(budget.take_shake(ImpactTier::Normal), 0.0);
        assert!(budget.take_shake(ImpactTier::Heavy) > 0.0);
        assert_eq!(budget.take_shake(ImpactTier::Heavy), 0.0);
        budget.begin_frame(0.15);
        assert!(budget.take_shake(ImpactTier::Finisher) > 0.0);
    }

    #[test]
    fn weak_dot_ticks_are_not_fake_critical_hits() {
        assert_eq!(
            impact_tier(true, true, 0.1, true, false),
            ImpactTier::Normal
        );
        assert_eq!(
            impact_tier(false, false, 1.0, false, false),
            ImpactTier::Normal
        );
        assert_eq!(
            impact_tier(true, true, 1.0, false, false),
            ImpactTier::Heavy
        );
        assert_eq!(
            impact_tier(true, false, 1.0, true, false),
            ImpactTier::Empowered
        );
        assert_eq!(
            impact_tier(false, false, 1.0, false, true),
            ImpactTier::Finisher
        );
    }

    #[test]
    fn important_spam_cannot_starve_ultimate_or_boss_finisher() {
        let mut budget = ImpactBudget::default();
        budget.begin_frame(0.0);
        for _ in 0..100 {
            budget.admit(ImpactTier::Empowered);
        }
        assert!(budget.admit(ImpactTier::Ultimate));
        assert!(budget.admit(ImpactTier::Finisher));
        assert!(budget.take_shake(ImpactTier::Heavy) > 0.0);
        assert!(budget.take_shake(ImpactTier::Ultimate) > 0.0);
        assert!(budget.take_shake(ImpactTier::Finisher) > 0.0);
        assert_eq!(budget.take_shake(ImpactTier::Finisher), 0.0);
    }

    #[test]
    fn rhythms_distinguish_light_heavy_and_ultimate_attacks() {
        let dagger = cast_rhythm(HeroWeapon::NightDagger, 0);
        let hammer = cast_rhythm(HeroWeapon::ForgeHammer, 0);
        assert!(dagger.windup < hammer.windup);
        assert!(dagger.recovery < hammer.recovery);
        for weapon in HeroWeapon::ALL {
            let basic = cast_rhythm(weapon, 0);
            let ultimate = cast_rhythm(weapon, 2);
            assert!(basic.release < basic.windup);
            assert!(basic.release < basic.recovery);
            assert!(ultimate.windup > basic.windup);
            assert!(ultimate.recovery > basic.recovery);
        }
    }

    #[test]
    fn pose_curve_is_continuous_and_returns_to_neutral() {
        assert_eq!(cast_extension(CastPhase::Windup, 0.0), 0.0);
        assert_eq!(
            cast_extension(CastPhase::Windup, 1.0),
            cast_extension(CastPhase::Release, 0.0)
        );
        assert_eq!(
            cast_extension(CastPhase::Release, 1.0),
            cast_extension(CastPhase::Recovery, 0.0)
        );
        assert_eq!(cast_extension(CastPhase::Recovery, 1.0), 0.0);
        assert!(cast_extension(CastPhase::Release, 0.5) > 0.8);
        assert!(cast_extension(CastPhase::Recovery, 0.5) < 0.2);
    }
}
