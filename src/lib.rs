//! 保卫萝卜 (Protect the Carrot) — Bevy port, library crate.
//!
//! All gameplay modules live here so both the game binary (`main.rs`) and the
//! headless balance simulator (`bin/sim.rs`) can share the exact same logic —
//! the simulator must never diverge from what players actually run.

pub mod attributes;
pub mod audio;
pub mod bestiary;
pub mod board;
pub mod build;
pub mod components;
pub mod creatures;
pub mod data;
pub mod enemy;
pub mod equipment;
pub mod fluent_i18n;
pub mod game;
pub mod hero;
pub mod hero_gear;
pub mod hero_paperdoll;
pub mod i18n;
pub mod lighting;
pub mod meta;
pub mod monster;
pub mod mutators;
pub mod polish;
pub mod quality;
pub mod roguelite;
pub mod sprites;
pub mod states;
pub mod tower;
pub mod tuning;
pub mod tutorial;
pub mod ui;
pub mod vfx;

use bevy::prelude::Resource;

/// All level definitions, loaded once at startup. Referenced as `crate::Levels`
/// by the modules, so it lives at the crate root.
#[derive(Resource)]
pub struct Levels(pub Vec<data::Level>);
