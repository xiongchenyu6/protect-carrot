//! Runtime-tunable balance assets loaded from RON/JSON.
//!
//! Values here are small, high-leverage knobs that are useful to tune without
//! recompiling Rust. The code still keeps defaults so capture/sim binaries can
//! run even when they do not install the asset-loading state.

use crate::data::TowerKind;
use bevy::prelude::*;
use bevy_asset_loader::prelude::AssetCollection;
use serde::Deserialize;

#[derive(AssetCollection, Resource)]
pub struct TuningAssets {
    #[asset(path = "tuning/focus_beams.focus.ron")]
    pub focus_beams: Handle<FocusBeamTuningAsset>,
}

#[derive(Asset, TypePath, Deserialize, Clone, Debug)]
pub struct FocusBeamTuningAsset {
    pub laser: FocusBeamProfile,
    pub prism: FocusBeamProfile,
}

#[derive(Deserialize, Clone, Copy, Debug)]
pub struct FocusBeamProfile {
    pub base_dps_mult: f32,
    pub charge_rate: f32,
    pub dps_cap: f32,
    pub visual_full_charge: f32,
    pub width_bonus: f32,
    pub hit_radius_base: f32,
    pub hit_radius_bonus: f32,
}

impl FocusBeamProfile {
    pub const fn new(
        base_dps_mult: f32,
        charge_rate: f32,
        dps_cap: f32,
        visual_full_charge: f32,
        width_bonus: f32,
        hit_radius_base: f32,
        hit_radius_bonus: f32,
    ) -> Self {
        Self {
            base_dps_mult,
            charge_rate,
            dps_cap,
            visual_full_charge,
            width_bonus,
            hit_radius_base,
            hit_radius_bonus,
        }
    }

    pub fn sanitized(self, fallback: Self) -> Self {
        Self {
            base_dps_mult: self.base_dps_mult.clamp(0.1, 20.0),
            charge_rate: self.charge_rate.clamp(0.0, 8.0),
            dps_cap: self.dps_cap.max(fallback.dps_cap * 0.25).min(50_000.0),
            visual_full_charge: self.visual_full_charge.clamp(0.25, 20.0),
            width_bonus: self.width_bonus.clamp(0.0, 80.0),
            hit_radius_base: self.hit_radius_base.clamp(1.0, 120.0),
            hit_radius_bonus: self.hit_radius_bonus.clamp(0.0, 160.0),
        }
    }
}

pub const DEFAULT_LASER_FOCUS: FocusBeamProfile =
    FocusBeamProfile::new(1.0, 1.0, 3_000.0, 5.0, 7.0, 15.0, 0.0);

pub const DEFAULT_PRISM_FOCUS: FocusBeamProfile =
    FocusBeamProfile::new(2.25, 1.35, 9_000.0, 4.0, 14.0, 18.0, 16.0);

pub fn default_focus_profile(kind: TowerKind) -> Option<FocusBeamProfile> {
    match kind {
        TowerKind::Laser => Some(DEFAULT_LASER_FOCUS),
        TowerKind::Prism => Some(DEFAULT_PRISM_FOCUS),
        _ => None,
    }
}

impl FocusBeamTuningAsset {
    pub fn profile(&self, kind: TowerKind) -> Option<FocusBeamProfile> {
        match kind {
            TowerKind::Laser => Some(self.laser.sanitized(DEFAULT_LASER_FOCUS)),
            TowerKind::Prism => Some(self.prism.sanitized(DEFAULT_PRISM_FOCUS)),
            _ => None,
        }
    }
}

pub fn focus_profile_from_assets(
    kind: TowerKind,
    handles: Option<&TuningAssets>,
    assets: Option<&Assets<FocusBeamTuningAsset>>,
) -> Option<FocusBeamProfile> {
    handles
        .and_then(|handles| assets.and_then(|assets| assets.get(&handles.focus_beams)))
        .and_then(|tuning| tuning.profile(kind))
        .or_else(|| default_focus_profile(kind))
}
