//! Top-level game flow states. Mirrors the original `gameState` string flag, but as
//! a typed Bevy `States` enum so we can gate systems with `.run_if(in_state(...))`
//! and run setup/teardown on `OnEnter` / `OnExit`.

use bevy::prelude::*;

#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameState {
    /// Initial asset gate; critical tuning assets are loaded before the menu.
    #[default]
    Loading,
    /// Start screen / level select.
    Menu,
    /// Opening story scene with generated key art and character portraits.
    Story,
    /// Pre-level briefing: lore, threat intel, and animated tactical transition.
    Briefing,
    /// Hero deploy cutscene: the chosen class×race portrait reveal before the level.
    HeroIntro,
    /// Actively playing a level. (Pause is a separate `Paused` resource flag so
    /// toggling it does not re-trigger `OnEnter(Playing)` / reload the level.)
    Playing,
    GameOver,
    Victory,
    /// Monster bestiary screen (reached from the menu).
    Bestiary,
    /// Persistent equipment collection screen (reached from the menu).
    Armory,
    /// Tower catalog screen (reached from the menu).
    TowerArchive,
    /// Derived achievement/milestone screen (reached from the menu).
    Milestones,
    /// Campaign lore and level threat dossier (reached from the menu).
    CampaignDossier,
    /// Hero codex: browse all classes × races, read doctrines/skills/ultimates, and
    /// pick the deploy hero (moved off the cluttered main menu).
    HeroCodex,
}
