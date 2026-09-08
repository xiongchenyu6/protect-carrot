//! Automatic class casts use the same action queue as ordinary attacks.

use crate::components::Enemy;
pub use crate::data::MAX_HERO_SUMMONS;
use crate::game::{Paused, RunState};
use crate::hero::{HeroLoadout, HeroWeapon};
use crate::hero_skill_effects::{PassiveSource, skill_ready, trigger_weapon_skill};
use crate::sprites::Sprites;
use crate::tower::{BuffTower, Damage, Status, Summon, Tower};
use bevy::prelude::*;
use bevy_sequential_actions::{
    Action, ActionQueue, ActionsProxy, CurrentAction, ManageActions, SequentialActions, StopReason,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CastPhase {
    Windup,
    Release,
    Recovery,
}

#[derive(Component)]
pub struct HeroCast {
    pub weapon: HeroWeapon,
    pub gear: [Option<crate::hero_gear::HeroGear>; crate::hero_gear::HeroGearSlot::COUNT],
    pub slot: usize,
    pub phase: CastPhase,
    pub remaining: f32,
    pub duration: f32,
    pub released: bool,
}

#[derive(Message)]
pub struct HeroSkillCastEvent {
    pub owner: Entity,
    pub weapon: HeroWeapon,
    pub slot: usize,
}

struct CastAction {
    weapon: HeroWeapon,
    gear: [Option<crate::hero_gear::HeroGear>; crate::hero_gear::HeroGearSlot::COUNT],
    slot: usize,
    phase: CastPhase,
    duration: f32,
}

fn combat_running(world: &World) -> bool {
    !world.resource::<Paused>().0
        && world.resource::<RunState>().wave_in_progress
        && world.resource::<RunState>().lives > 0
        && world.resource::<HeroLoadout>().alive
}

impl Action for CastAction {
    fn is_finished(&self, agent: Entity, world: &World) -> bool {
        combat_running(world)
            && world.get::<HeroCast>(agent).is_none_or(|cast| {
                cast.remaining <= 0.0 && (cast.phase != CastPhase::Release || cast.released)
            })
    }

    fn on_start(&mut self, agent: Entity, world: &mut World) -> bool {
        if world.get::<Tower>(agent).is_none_or(|hero| hero.hp <= 0.0)
            || world.resource::<HeroLoadout>().weapon != self.weapon
        {
            return true;
        }
        world.entity_mut(agent).insert(HeroCast {
            weapon: self.weapon,
            gear: self.gear,
            slot: self.slot,
            phase: self.phase,
            remaining: self.duration,
            duration: self.duration,
            released: false,
        });
        if let Some(mut hero) = world.get_mut::<Tower>(agent) {
            hero.hero_attack_timer = self.duration;
        }
        false
    }

    fn on_stop(&mut self, agent: Option<Entity>, world: &mut World, reason: StopReason) {
        if (self.phase == CastPhase::Recovery || reason != StopReason::Finished)
            && let Some(agent) = agent
            && let Ok(mut entity) = world.get_entity_mut(agent)
        {
            entity.remove::<HeroCast>();
            if let Some(mut hero) = entity.get_mut::<Tower>() {
                hero.hero_attack_timer = 0.0;
            }
        }
    }
}

pub fn displaces_hero(weapon: HeroWeapon, slot: usize) -> bool {
    matches!(
        (weapon, slot),
        (HeroWeapon::BannerSword, 0) | (HeroWeapon::NightDagger, 0)
    )
}

fn queue_cast(commands: &mut Commands, owner: Entity, loadout: &HeroLoadout, slot: usize) {
    let weapon = loadout.weapon;
    let gear = loadout.gear;
    let windup = if slot == 2 {
        0.8
    } else if matches!(weapon, HeroWeapon::SummonStaff | HeroWeapon::ForgeHammer) {
        0.65
    } else {
        0.42
    };
    commands.entity(owner).insert(SequentialActions);
    commands.actions(owner).add((
        CastAction {
            weapon,
            gear,
            slot,
            phase: CastPhase::Windup,
            duration: windup,
        },
        CastAction {
            weapon,
            gear,
            slot,
            phase: CastPhase::Release,
            duration: 0.32,
        },
        CastAction {
            weapon,
            gear,
            slot,
            phase: CastPhase::Recovery,
            duration: 0.3,
        },
    ));
}

/// Readiness never consumes resources; effects happen only after windup.
pub fn update_hero_passives(
    mut commands: Commands,
    (time, paused, joystick): (Res<Time>, Res<Paused>, Res<crate::ui::JoystickState>),
    mut run: ResMut<RunState>,
    mut loadout: ResMut<HeroLoadout>,
    mut towers: Query<(Entity, &mut Tower)>,
    enemies: Query<(Entity, &Enemy, &Transform)>,
    summons: Query<&Summon>,
    mut casts: Query<(Entity, &mut HeroCast)>,
    actions: Query<(Option<&CurrentAction>, Option<&ActionQueue>)>,
    sprites: Res<Sprites>,
    creatures: Res<crate::creatures::Creatures>,
    (mut sfx, mut events): (
        MessageWriter<crate::audio::SfxEvent>,
        MessageWriter<HeroSkillCastEvent>,
    ),
    (mut dmg, mut status, mut buff): (
        MessageWriter<Damage>,
        MessageWriter<Status>,
        MessageWriter<BuffTower>,
    ),
    mut vfx: MessageWriter<crate::vfx::VfxEvent>,
    (mut meshes, mut materials): (ResMut<Assets<Mesh>>, ResMut<Assets<ColorMaterial>>),
) {
    // Invalidate before pause gating so obsolete owners cannot resume a cast.
    for (owner, cast) in &mut casts {
        let valid = loadout.alive
            && loadout.weapon == cast.weapon
            && loadout.gear == cast.gear
            && run.lives > 0
            && towers.get(owner).is_ok_and(|(_, t)| t.hero && t.hp > 0.0);
        let moving = towers
            .get(owner)
            .is_ok_and(|(_, t)| t.move_target.is_some())
            || joystick.dir.length_squared() > 0.0025;
        if !valid
            || (displaces_hero(cast.weapon, cast.slot)
                && moving
                && !cast.released
                && cast.phase != CastPhase::Recovery)
        {
            commands.actions(owner).clear();
            commands.entity(owner).remove::<HeroCast>();
            return;
        }
    }
    let dt = time.delta_secs() * run.game_speed;
    if paused.0 || !run.wave_in_progress || !loadout.alive || run.lives <= 0 || dt <= 0.0 {
        return;
    }
    let Some((owner, hero)) = towers.iter().find(|(_, t)| t.hero && t.hp > 0.0) else {
        return;
    };
    let source = PassiveSource {
        pos: hero.center(),
        damage: hero.damage,
        element: hero.element,
        max_hp: hero.max_hp,
        facing: Vec2::from_angle(hero.angle),
    };
    let moving = hero.move_target.is_some() || joystick.dir.length_squared() > 0.0025;
    for cooldown in &mut loadout.skill_cooldowns {
        *cooldown = (*cooldown - dt).max(0.0);
    }
    if let Ok((_, mut cast)) = casts.get_mut(owner) {
        cast.remaining = (cast.remaining - dt).max(0.0);
        if cast.phase != CastPhase::Release || cast.released {
            return;
        }
        if !skill_ready(
            cast.slot, owner, source, &loadout, &towers, &enemies, &summons, &run,
        ) {
            commands.actions(owner).clear();
            commands.entity(owner).remove::<HeroCast>();
            return;
        }
        let mut slots = MAX_HERO_SUMMONS.saturating_sub(
            summons
                .iter()
                .filter(|s| s.owner == owner && s.hp > 0.0 && s.lifetime > 0.0)
                .count(),
        );
        let triggered = trigger_weapon_skill(
            cast.slot,
            &mut commands,
            owner,
            source,
            &mut loadout,
            &mut towers,
            &enemies,
            &sprites,
            &creatures,
            &mut dmg,
            &mut status,
            &mut buff,
            &mut vfx,
            &mut run,
            &mut meshes,
            &mut materials,
            &mut slots,
        );
        cast.released = true;
        if triggered {
            loadout.skill_cooldowns[cast.slot] = loadout.skill_interval(cast.slot);
            events.write(HeroSkillCastEvent {
                owner,
                weapon: loadout.weapon,
                slot: cast.slot,
            });
            sfx.write(crate::audio::SfxEvent(match loadout.weapon {
                HeroWeapon::BannerSword => crate::audio::Sound::Boss,
                HeroWeapon::StarfireStaff => crate::audio::Sound::Meteor,
                HeroWeapon::ShadowBow | HeroWeapon::StormOrb | HeroWeapon::NightDagger => {
                    crate::audio::Sound::Chain
                }
                HeroWeapon::OathShield | HeroWeapon::SummonStaff => crate::audio::Sound::Raise,
                HeroWeapon::SentryCrossbow | HeroWeapon::ForgeHammer => {
                    crate::audio::Sound::Upgrade
                }
            }));
        }
        return;
    }
    if actions.get(owner).is_ok_and(|(current, queue)| {
        current.is_some_and(|a| a.is_some()) || queue.is_some_and(|q| !q.is_empty())
    }) {
        return;
    }
    for slot in [2, 0, 1] {
        if (slot == 2 && loadout.level < HeroLoadout::MAX_LEVEL)
            || loadout.skill_cooldowns[slot] > 0.0
            || (moving && displaces_hero(loadout.weapon, slot))
            || !skill_ready(
                slot, owner, source, &loadout, &towers, &enemies, &summons, &run,
            )
        {
            continue;
        }
        queue_cast(&mut commands, owner, &loadout, slot);
        break;
    }
}
