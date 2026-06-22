//! 保卫萝卜 (Protect the Carrot) — Bevy port, library crate.
//!
//! All gameplay modules live here so both the game binary (`main.rs`) and the
//! headless balance simulator (`bin/sim.rs`) can share the exact same logic —
//! the simulator must never diverge from what players actually run.

pub mod audio;
pub mod bestiary;
pub mod board;
pub mod build;
pub mod components;
pub mod creatures;
pub mod data;
pub mod enemy;
pub mod equipment;
pub mod game;
pub mod hero;
pub mod i18n;
pub mod meta;
pub mod monster;
pub mod quality;
pub mod sprites;
pub mod states;
pub mod tower;
pub mod ui;
pub mod vfx;

use bevy::prelude::Resource;

/// All level definitions, loaded once at startup. Referenced as `crate::Levels`
/// by the modules, so it lives at the crate root.
#[derive(Resource)]
pub struct Levels(pub Vec<data::Level>);
