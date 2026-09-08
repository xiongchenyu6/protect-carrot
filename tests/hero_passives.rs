use bevy::{ecs::system::RunSystemOnce, prelude::*};
use bevy_sequential_actions::{SequentialActions, SequentialActionsPlugin};
use protect_carrot::{
    audio, build,
    components::{Enemy, FireGround},
    creatures::{CreatureCfg, Creatures},
    data::{COLS, Element, EnemyKind, ROWS, TowerKind},
    game::{Paused, RunState},
    hero::{self, HeroLoadout, HeroRunMods, HeroWeapon, Race},
    hero_gear::{HeroGear, HeroGearInventory, HeroGearSet, HeroGearSlot},
    hero_passives::{self, CastPhase, HeroCast, HeroSkillCastEvent},
    monster::EliteAffix,
    sprites,
    tower::{
        self, BuffTower, Damage, FixedSummonHome, Projectile, Status, StatusKind, Summon, Tower,
    },
    vfx,
};
use std::time::Duration;

fn setup(weapon: HeroWeapon) -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        SequentialActionsPlugin,
    ))
    .init_asset::<Image>()
    .init_asset::<Mesh>()
    .init_asset::<ColorMaterial>()
    .init_resource::<HeroGearInventory>()
    .init_resource::<build::HeroWalks>()
    .init_resource::<tower::Snapshot>()
    .init_resource::<protect_carrot::board::Board>()
    .init_resource::<protect_carrot::ui::JoystickState>()
    .init_resource::<Paused>()
    .add_message::<Damage>()
    .add_message::<Status>()
    .add_message::<BuffTower>()
    .add_message::<HeroSkillCastEvent>()
    .add_message::<vfx::VfxEvent>()
    .add_message::<audio::SfxEvent>();
    let sprites = sprites::build_sprites(app.world().resource::<AssetServer>());
    app.insert_resource(sprites);
    app.insert_resource(Creatures(
        EnemyKind::ALL
            .into_iter()
            .map(|kind| {
                (
                    kind,
                    CreatureCfg {
                        image: default(),
                        layout: default(),
                        frames: 1,
                        fps: 10.0,
                        atk_image: default(),
                        atk_layout: default(),
                        atk_frames: 1,
                        move_anim: default(),
                        attack_anim: default(),
                    },
                )
            })
            .collect(),
    ));
    let loadout = HeroLoadout {
        weapon,
        race: Race::Human,
        level: 1,
        xp: 0,
        talent_points: 0,
        weapon_talents: [[0; HeroLoadout::TALENT_SLOTS]; HeroWeapon::ALL.len()],
        gear: [None; HeroGearSlot::COUNT],
        skill_cooldowns: [0.0; HeroLoadout::TALENT_SLOTS],
        run_mods: HeroRunMods::default(),
        alive: true,
        respawn_waves: 0,
    };
    let hero = app
        .world_mut()
        .spawn((
            hero::make_hero_tower(&loadout, Vec2::ZERO),
            Transform::default(),
            SequentialActions,
        ))
        .id();
    app.insert_resource(loadout).insert_resource(RunState {
        lives: 10,
        start_lives: 10,
        wave_in_progress: true,
        ..default()
    });
    (app, hero)
}

fn enemy(hp: f32, path_index: usize) -> Enemy {
    Enemy {
        kind: EnemyKind::Normal,
        species_id: 0,
        hp,
        max_hp: 2000.0,
        base_speed: 40.0,
        reward: 5,
        path_index,
        armor: 0.0,
        magic_resist: 0.0,
        element_resist: protect_carrot::data::ElementProfile::none(),
        flying: false,
        invisible: false,
        skill_mult: 1.0,
        stealth: 1.0,
        regen: 0.0,
        boss: false,
        size: 10.0,
        slow_timer: 0.0,
        stun_timer: 0.0,
        frozen: false,
        poison_timer: 0.0,
        poison_damage: 0.0,
        fire_timer: 0.0,
        fire_damage: 0.0,
        fire_element: Element::Fire,
        poison_source_tower: None,
        fire_source_tower: None,
        curse_timer: 0.0,
        armor_reduce: 0.0,
        shield: 0.0,
        max_shield: 0.0,
        splits: 0,
        heal_aura: 0.0,
        charger: false,
        charge_timer: 0.0,
        hit_flash: 0.0,
        last_hit_tower: None,
        blocked: false,
        melee: 0.0,
        elite: false,
        elite_affix: EliteAffix::None,
        boss_skill_timer: 0.0,
        enraged: false,
        phase_timer: 0.0,
        tower_raider: false,
        tower_dps: 0.0,
        silence_aura: 0.0,
        ranged_tower: false,
        ranged_range: 0.0,
        ranged_damage: 0.0,
        ranged_cooldown: 1.5,
        ranged_timer: 0.0,
        explosive: false,
        explode_damage: 0.0,
        explode_radius: 0.0,
        explode_sense: 0.0,
        explode_trigger: 0.0,
        moss_destroy: false,
        moss_destroyed: false,
        incubate: false,
        incubate_timer: 0.0,
        incubate_stacks: 0,
        facing: Vec2::X,
    }
}

fn spawn_enemy(app: &mut App, pos: Vec2, hp: f32, path_index: usize) -> Entity {
    app.world_mut()
        .spawn((
            enemy(hp, path_index),
            Transform::from_translation(pos.extend(5.0)),
        ))
        .id()
}

fn spawn_tower(app: &mut App, hp_fraction: f32) -> Entity {
    let mut tower = Tower::from_def(TowerKind::Arrow.def(), COLS / 2, ROWS / 2);
    tower.hp *= hp_fraction;
    let pos = tower.center();
    app.world_mut()
        .spawn((tower, Transform::from_translation(pos.extend(4.0))))
        .id()
}

fn step(app: &mut App, seconds: f32) {
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_secs_f32(seconds));
    app.world_mut()
        .run_system_once(hero_passives::update_hero_passives)
        .unwrap();
    app.world_mut()
        .run_system_once(protect_carrot::hero_skill_effects::update_skill_fields)
        .unwrap();
    app.world_mut().run_schedule(Last);
}

fn drain<M: Message>(app: &mut App) -> Vec<M> {
    app.world_mut()
        .resource_mut::<Messages<M>>()
        .drain()
        .collect()
}

fn count<C: Component>(app: &mut App) -> usize {
    app.world_mut().query::<&C>().iter(app.world()).count()
}

fn spawn_summon(app: &mut App, owner: Entity, hp: f32) -> Entity {
    app.world_mut()
        .spawn((
            Summon {
                hp,
                max_hp: 100.0,
                damage: 10.0,
                speed: 70.0,
                target: None,
                facing: Vec2::X,
                attack_timer: 0.0,
                owner,
                kind: EnemyKind::Normal,
                lifetime: 30.0,
                buff: 0.0,
            },
            Transform::default(),
        ))
        .id()
}

fn living_owned_summons(app: &mut App, owner: Entity) -> usize {
    app.world_mut()
        .query::<&Summon>()
        .iter(app.world())
        .filter(|summon| summon.owner == owner && summon.hp > 0.0 && summon.lifetime > 0.0)
        .count()
}

fn equip_set(app: &mut App, set: HeroGearSet) {
    let mut loadout = app.world_mut().resource_mut::<HeroLoadout>();
    for slot in HeroGearSlot::ALL {
        loadout.gear[slot.idx()] = Some(
            HeroGear::ALL
                .into_iter()
                .find(|gear| gear.gear_set() == set && gear.def().slot == slot)
                .expect("each gear set must fill all four slots"),
        );
    }
}

fn setup_skill(weapon: HeroWeapon, slot: usize) -> (App, Entity) {
    let (mut app, owner) = setup(weapon);
    let mut loadout = app.world_mut().resource_mut::<HeroLoadout>();
    loadout.level = if slot == 2 { HeroLoadout::MAX_LEVEL } else { 1 };
    loadout.skill_cooldowns = [1000.0; HeroLoadout::TALENT_SLOTS];
    loadout.skill_cooldowns[slot] = 0.0;
    (app, owner)
}

fn assert_no_gameplay_effect(app: &mut App) {
    assert!(drain::<Damage>(app).is_empty());
    assert!(drain::<Status>(app).is_empty());
    assert!(drain::<BuffTower>(app).is_empty());
    assert!(drain::<HeroSkillCastEvent>(app).is_empty());
    assert_eq!(count::<Summon>(app), 0);
    assert_eq!(count::<Projectile>(app), 0);
    assert_eq!(count::<FireGround>(app), 0);
    assert_eq!(
        count::<protect_carrot::hero_skill_effects::SkillField>(app),
        0
    );
}

fn start_cast(app: &mut App, owner: Entity, slot: usize) {
    step(app, 0.01);
    let cast = app
        .world()
        .get::<HeroCast>(owner)
        .expect("valid skill starts winding up");
    assert_eq!(cast.slot, slot);
    assert_eq!(cast.phase, CastPhase::Windup);
    assert!(cast.remaining > 0.0);
}

fn release_cast(app: &mut App, owner: Entity, slot: usize) {
    for _ in 0..120 {
        step(app, 0.025);
        let events = drain::<HeroSkillCastEvent>(app);
        if !events.is_empty() {
            assert_eq!(events.len(), 1);
            assert_eq!(events[0].owner, owner);
            assert_eq!(events[0].slot, slot);
            assert_eq!(
                events[0].weapon,
                app.world().resource::<HeroLoadout>().weapon
            );
            assert_eq!(
                app.world().get::<HeroCast>(owner).unwrap().phase,
                CastPhase::Release
            );
            return;
        }
    }
    panic!("skill {slot} never released");
}

fn finish_cast(app: &mut App, owner: Entity) {
    let mut saw_recovery = false;
    for _ in 0..120 {
        step(app, 0.025);
        match app.world().get::<HeroCast>(owner) {
            Some(cast) => saw_recovery |= cast.phase == CastPhase::Recovery,
            None => {
                assert!(saw_recovery, "release must be followed by recovery");
                assert!(drain::<HeroSkillCastEvent>(app).is_empty());
                return;
            }
        }
    }
    panic!("cast never recovered");
}

fn cast(app: &mut App, owner: Entity, slot: usize) {
    start_cast(app, owner, slot);
    release_cast(app, owner, slot);
}

fn field_kinds(app: &mut App) -> Vec<protect_carrot::hero_skill_effects::SkillFieldKind> {
    app.world_mut()
        .query::<&protect_carrot::hero_skill_effects::SkillField>()
        .iter(app.world())
        .map(|field| field.kind)
        .collect()
}

#[test]
fn all_twenty_seven_skills_have_delayed_class_effects_and_recovery() {
    use protect_carrot::hero_skill_effects::SkillFieldKind as Field;
    for weapon in HeroWeapon::ALL {
        for slot in 0..HeroLoadout::TALENT_SLOTS {
            let (mut app, owner) = setup_skill(weapon, slot);
            let target = spawn_enemy(&mut app, Vec2::new(90.0, 0.0), 500.0, 3);
            spawn_enemy(&mut app, Vec2::new(120.0, 25.0), 1000.0, 2);
            spawn_enemy(&mut app, Vec2::new(60.0, -20.0), 1000.0, 1);
            spawn_tower(&mut app, 0.5);
            if weapon == HeroWeapon::OathShield {
                app.world_mut().get_mut::<Tower>(owner).unwrap().hp *= 0.5;
            }
            if weapon == HeroWeapon::SummonStaff && slot == 1 {
                app.world_mut()
                    .resource_mut::<RunState>()
                    .remember_fallen_enemy(&enemy(0.0, 1), Vec2::ZERO);
            }
            let original_hp = app.world().get::<Tower>(owner).unwrap().hp;
            start_cast(&mut app, owner, slot);
            assert_no_gameplay_effect(&mut app);
            assert_eq!(app.world().get::<Tower>(owner).unwrap().hp, original_hp);
            assert_eq!(
                app.world().get::<Tower>(owner).unwrap().hero_pos,
                Vec2::ZERO
            );
            step(&mut app, 0.1);
            assert_no_gameplay_effect(&mut app);
            release_cast(&mut app, owner, slot);
            let fields = field_kinds(&mut app);
            let damage = drain::<Damage>(&mut app);
            let statuses = drain::<Status>(&mut app);
            match (weapon, slot) {
                (HeroWeapon::BannerSword, 0) => {
                    assert!(app.world().get::<Tower>(owner).unwrap().hero_pos.x > 0.0);
                    assert!(!damage.is_empty());
                    assert!(
                        statuses
                            .iter()
                            .any(|s| matches!(s.kind, StatusKind::Knockback { .. }))
                    );
                }
                (HeroWeapon::BannerSword, 1) => assert!(!damage.is_empty()),
                (HeroWeapon::BannerSword, 2) => assert!(fields.contains(&Field::Quake)),
                (HeroWeapon::StarfireStaff, 0) => {
                    assert_eq!(count::<FireGround>(&mut app), 1);
                    assert!(!damage.is_empty());
                }
                (HeroWeapon::StarfireStaff, 1) => assert!(fields.contains(&Field::FrostPrison)),
                (HeroWeapon::StarfireStaff, 2) => assert!(fields.contains(&Field::MeteorShower)),
                (HeroWeapon::ShadowBow, 0) | (HeroWeapon::NightDagger, 1) => {
                    assert!(count::<Projectile>(&mut app) > 0);
                    assert!(
                        app.world_mut()
                            .query::<&Projectile>()
                            .iter(app.world())
                            .all(|projectile| projectile.poison_duration > 0.0)
                    );
                }
                (HeroWeapon::ShadowBow, 1) => {
                    assert!(!damage.is_empty());
                    assert!(damage.iter().all(|hit| hit.armor_pierce > 0.0));
                }
                (HeroWeapon::ShadowBow, 2) => assert!(fields.contains(&Field::ArrowStorm)),
                (HeroWeapon::OathShield, 0) => {
                    assert!(!damage.is_empty());
                    assert!(
                        statuses
                            .iter()
                            .any(|s| matches!(s.kind, StatusKind::Knockback { .. }))
                    );
                }
                (HeroWeapon::OathShield, 1) => {
                    assert!(app.world().get::<Tower>(owner).unwrap().hp > original_hp);
                    assert!(damage.is_empty());
                }
                (HeroWeapon::OathShield, 2) => assert!(fields.contains(&Field::Sanctuary)),
                (HeroWeapon::StormOrb, 0) => assert!(damage.len() > 1),
                (HeroWeapon::StormOrb, 1) => assert!(fields.contains(&Field::Vortex)),
                (HeroWeapon::StormOrb, 2) => assert!(fields.contains(&Field::Thunderstorm)),
                (HeroWeapon::SentryCrossbow, 0 | 1) => {
                    assert!(count::<Projectile>(&mut app) > 0);
                }
                (HeroWeapon::SentryCrossbow, 2) => assert!(fields.contains(&Field::Sentry)),
                (HeroWeapon::NightDagger, 0) => {
                    assert_ne!(
                        app.world().get::<Tower>(owner).unwrap().hero_pos,
                        Vec2::ZERO
                    );
                    assert!(damage.iter().any(|hit| hit.target == target));
                }
                (HeroWeapon::NightDagger, 2) => assert_eq!(damage.len(), 1),
                (HeroWeapon::SummonStaff, _) => {
                    assert!(count::<Summon>(&mut app) > 0);
                    assert!(damage.is_empty() && statuses.is_empty());
                    assert!(drain::<BuffTower>(&mut app).is_empty());
                    assert_eq!(count::<Projectile>(&mut app), 0);
                    assert!(
                        app.world_mut()
                            .query::<&Summon>()
                            .iter(app.world())
                            .all(|summon| summon.hp > 0.0
                                && summon.damage > 0.0
                                && summon.lifetime > 0.0
                                && summon.owner == owner)
                    );
                }
                (HeroWeapon::ForgeHammer, 0) => assert!(count::<FixedSummonHome>(&mut app) > 0),
                (HeroWeapon::ForgeHammer, 1) => {
                    assert!(count::<Projectile>(&mut app) > 0);
                    assert!(damage.is_empty(), "rockets must travel before damaging");
                }
                (HeroWeapon::ForgeHammer, 2) => {
                    assert!(count::<FixedSummonHome>(&mut app) > 0);
                    assert!(fields.contains(&Field::Workshop));
                }
                _ => unreachable!(),
            }
            assert!(app.world().resource::<HeroLoadout>().skill_cooldowns[slot] > 0.0);
            finish_cast(&mut app, owner);
        }
    }
}

#[test]
fn persistent_skills_keep_delivering_real_local_gameplay_after_release() {
    for (weapon, slot) in [
        (HeroWeapon::BannerSword, 2),
        (HeroWeapon::StarfireStaff, 1),
        (HeroWeapon::StarfireStaff, 2),
        (HeroWeapon::ShadowBow, 2),
        (HeroWeapon::OathShield, 2),
        (HeroWeapon::StormOrb, 1),
        (HeroWeapon::StormOrb, 2),
        (HeroWeapon::SentryCrossbow, 2),
        (HeroWeapon::ForgeHammer, 2),
    ] {
        let (mut app, owner) = setup_skill(weapon, slot);
        let target = spawn_enemy(&mut app, Vec2::new(90.0, 0.0), 1000.0, 2);
        let distant = spawn_enemy(&mut app, Vec2::new(-700.0, -700.0), 1000.0, 1);
        let ally = spawn_tower(&mut app, 0.25);
        app.world_mut().get_mut::<Tower>(owner).unwrap().hp *= 0.25;
        cast(&mut app, owner, slot);
        drain::<Damage>(&mut app);
        drain::<Status>(&mut app);
        let hp_before = app.world().get::<Tower>(owner).unwrap().hp;
        let ally_hp_before = app.world().get::<Tower>(ally).unwrap().hp;
        for _ in 0..80 {
            step(&mut app, 0.025);
        }
        let damage = drain::<Damage>(&mut app);
        let statuses = drain::<Status>(&mut app);
        assert!(damage.iter().all(|hit| hit.target != distant));
        assert!(statuses.iter().all(|status| status.target != distant));
        match (weapon, slot) {
            (HeroWeapon::BannerSword, 2)
            | (HeroWeapon::StarfireStaff, 2)
            | (HeroWeapon::StormOrb, 1 | 2) => {
                assert!(
                    damage
                        .iter()
                        .any(|hit| hit.target == target && hit.amount > 0.0),
                    "{weapon:?} slot {slot} must keep dealing actual damage"
                );
            }
            (HeroWeapon::StarfireStaff, 1) => {
                assert!(statuses.iter().any(|status| status.target == target
                    && matches!(status.kind, StatusKind::Freeze { .. })));
            }
            (HeroWeapon::ShadowBow, 2) | (HeroWeapon::SentryCrossbow, 2) => {
                assert!(count::<Projectile>(&mut app) > 1);
                assert!(
                    app.world_mut()
                        .query::<&Projectile>()
                        .iter(app.world())
                        .all(|projectile| projectile.target == target && projectile.damage > 0.0)
                );
            }
            (HeroWeapon::OathShield, 2) => {
                assert!(app.world().get::<Tower>(owner).unwrap().hp > hp_before);
                assert!(app.world().get::<Tower>(ally).unwrap().hp > ally_hp_before);
                assert!(
                    statuses
                        .iter()
                        .any(|status| matches!(status.kind, StatusKind::Knockback { .. }))
                );
            }
            (HeroWeapon::ForgeHammer, 2) => {
                assert!(app.world().get::<Tower>(ally).unwrap().hp > ally_hp_before);
                assert!(count::<FixedSummonHome>(&mut app) > 0);
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn persistent_field_pulses_freeze_while_paused_and_resume_afterwards() {
    let (mut app, owner) = setup_skill(HeroWeapon::StormOrb, 2);
    spawn_enemy(&mut app, Vec2::new(90.0, 0.0), 1000.0, 1);
    cast(&mut app, owner, 2);
    drain::<Damage>(&mut app);
    app.world_mut().resource_mut::<Paused>().0 = true;
    for _ in 0..20 {
        step(&mut app, 0.1);
    }
    assert!(drain::<Damage>(&mut app).is_empty());
    assert_eq!(
        count::<protect_carrot::hero_skill_effects::SkillField>(&mut app),
        1
    );
    app.world_mut().resource_mut::<Paused>().0 = false;
    for _ in 0..40 {
        step(&mut app, 0.025);
    }
    assert!(!drain::<Damage>(&mut app).is_empty());
}

#[test]
fn empty_or_dead_targets_do_not_start_targeted_skills_or_consume_cooldown() {
    for weapon in HeroWeapon::ALL
        .into_iter()
        .filter(|weapon| *weapon != HeroWeapon::SummonStaff)
    {
        for slot in 0..HeroLoadout::TALENT_SLOTS {
            for dead_targets in [false, true] {
                let (mut app, owner) = setup_skill(weapon, slot);
                if dead_targets {
                    spawn_enemy(&mut app, Vec2::new(50.0, 0.0), 0.0, 1);
                    spawn_tower(&mut app, 0.0);
                }
                step(&mut app, 1.0);
                assert!(
                    app.world().get::<HeroCast>(owner).is_none(),
                    "{weapon:?} slot {slot}"
                );
                assert_eq!(
                    app.world().resource::<HeroLoadout>().skill_cooldowns[slot],
                    0.0
                );
                assert_no_gameplay_effect(&mut app);
                assert!(app.world().resource::<RunState>().message.is_empty());
            }
        }
    }
}

#[test]
fn summoner_builds_a_frontline_before_enemies_enter_range() {
    for slot in 0..HeroLoadout::TALENT_SLOTS {
        for situation in ["empty", "dead", "distant"] {
            let (mut app, owner) = setup_skill(HeroWeapon::SummonStaff, slot);
            match situation {
                "dead" => {
                    spawn_enemy(&mut app, Vec2::new(50.0, 0.0), 0.0, 1);
                }
                "distant" => {
                    spawn_enemy(&mut app, Vec2::new(600.0, 0.0), 2000.0, 1);
                }
                _ => {}
            }
            if slot == 1 {
                app.world_mut()
                    .resource_mut::<RunState>()
                    .remember_fallen_enemy(&enemy(0.0, 1), Vec2::ZERO);
            }
            start_cast(&mut app, owner, slot);
            assert_no_gameplay_effect(&mut app);
            assert_eq!(
                app.world().resource::<HeroLoadout>().skill_cooldowns[slot],
                0.0
            );
            release_cast(&mut app, owner, slot);
            assert_eq!(
                living_owned_summons(&mut app, owner),
                if slot == 2 { 3 } else { 1 },
                "summon staff slot {slot} must establish allies with {situation} targets"
            );
            assert!(app.world().resource::<HeroLoadout>().skill_cooldowns[slot] > 0.0);
            assert!(app.world().resource::<RunState>().fallen_enemies.is_empty());
            finish_cast(&mut app, owner);
        }
    }
}

#[test]
fn recall_without_souls_waits_without_casting_or_consuming_cooldown() {
    let (mut app, owner) = setup_skill(HeroWeapon::SummonStaff, 1);
    for _ in 0..120 {
        step(&mut app, 0.025);
    }
    assert!(app.world().get::<HeroCast>(owner).is_none());
    assert_eq!(
        app.world().resource::<HeroLoadout>().skill_cooldowns[1],
        0.0
    );
    assert_no_gameplay_effect(&mut app);
}

#[test]
fn lone_summon_stays_visible_beside_its_owner_and_survives_wave_preparation() {
    for slot in [0, 1] {
        let (mut app, owner) = setup_skill(HeroWeapon::SummonStaff, slot);
        if slot == 1 {
            app.world_mut()
                .resource_mut::<RunState>()
                .remember_fallen_enemy(&enemy(0.0, 1), Vec2::ZERO);
        }
        cast(&mut app, owner, slot);
        finish_cast(&mut app, owner);
        for frame in 0..3 * 60 {
            if frame == 60 {
                app.world_mut().get_mut::<Tower>(owner).unwrap().hero_pos = Vec2::new(80.0, 0.0);
            }
            step(&mut app, 1.0 / 60.0);
            app.world_mut()
                .run_system_once(tower::build_snapshot)
                .unwrap();
            app.world_mut()
                .run_system_once(hero::hero_doctrine)
                .unwrap();
            app.world_mut()
                .run_system_once(tower::update_summons)
                .unwrap();
        }
        let (summon_entity, lifetime, hp) = {
            let world = app.world_mut();
            let mut summons = world.query::<(Entity, &Summon, &Transform)>();
            let (entity, summon, tf) = summons.single(world).unwrap();
            let hero_pos = world.get::<Tower>(owner).unwrap().center();
            let distance = tf.translation.truncate().distance(hero_pos);
            assert!(
                distance > protect_carrot::data::TILE_SIZE * 0.5,
                "slot {slot} overlapped its owner"
            );
            assert!(
                distance < protect_carrot::data::TILE_SIZE * 1.7,
                "slot {slot} did not follow its owner"
            );
            (entity, summon.lifetime, summon.hp)
        };
        let cooldowns = app.world().resource::<HeroLoadout>().skill_cooldowns;
        app.world_mut().resource_mut::<RunState>().wave_in_progress = false;
        for _ in 0..30 * 60 {
            step(&mut app, 1.0 / 60.0);
            app.world_mut()
                .run_system_once(hero::hero_doctrine)
                .unwrap();
            app.world_mut()
                .run_system_once(tower::update_summons)
                .unwrap();
        }
        let summon = app
            .world()
            .get::<Summon>(summon_entity)
            .expect("preparation must not expire allies");
        assert_eq!(summon.lifetime, lifetime);
        assert_eq!(summon.hp, hp);
        assert_eq!(
            app.world().resource::<HeroLoadout>().skill_cooldowns,
            cooldowns
        );
        app.world_mut().resource_mut::<RunState>().wave_in_progress = true;
        step(&mut app, 1.0 / 60.0);
        app.world_mut()
            .run_system_once(hero::hero_doctrine)
            .unwrap();
        app.world_mut()
            .run_system_once(tower::update_summons)
            .unwrap();
        assert!(app.world().get::<Summon>(summon_entity).unwrap().lifetime < lifetime);
        assert!(app.world().resource::<HeroLoadout>().skill_cooldowns[slot] < cooldowns[slot]);
    }
}

#[test]
fn ultimate_is_locked_until_level_thirty_but_base_skills_need_no_talent_rank() {
    for weapon in HeroWeapon::ALL {
        let (mut app, owner) = setup_skill(weapon, 2);
        spawn_enemy(&mut app, Vec2::new(90.0, 0.0), 1000.0, 1);
        spawn_tower(&mut app, 0.5);
        app.world_mut().resource_mut::<HeroLoadout>().level = 29;
        step(&mut app, 0.1);
        assert!(app.world().get::<HeroCast>(owner).is_none());
        assert_no_gameplay_effect(&mut app);
        app.world_mut().resource_mut::<HeroLoadout>().level = 30;
        cast(&mut app, owner, 2);
    }
}

#[test]
fn skill_training_does_not_change_basic_attack_health_armor_or_movement() {
    for weapon in HeroWeapon::ALL {
        let (mut app, _) = setup(weapon);
        for level in [1, 30] {
            let mut loadout = app.world_mut().resource_mut::<HeroLoadout>();
            loadout.level = level;
            let index = loadout.weapon_index();
            loadout.weapon_talents[index] = [0; HeroLoadout::TALENT_SLOTS];
            let before = hero::make_hero_tower(&loadout, Vec2::ZERO);
            let movement = hero::hero_move_speed(&loadout);
            loadout.weapon_talents[index] =
                [HeroLoadout::TALENT_MAX_RANK; HeroLoadout::TALENT_SLOTS];
            let after = hero::make_hero_tower(&loadout, Vec2::ZERO);
            assert_eq!(hero::hero_move_speed(&loadout), movement, "{weapon:?}");
            let basic_stats = |t: &Tower| {
                [
                    t.max_hp,
                    t.hp,
                    t.armor,
                    t.armor_pierce,
                    t.base_damage,
                    t.damage,
                    t.range,
                    t.cooldown,
                    t.aoe_radius,
                    t.chain_count as f32,
                    t.chain_range,
                    t.slow_duration,
                    t.knock_dist,
                    t.stun_duration,
                    t.freeze_duration,
                    t.armor_reduce,
                    t.curse_duration,
                    t.buff_range,
                    t.dot_damage,
                    t.poison_duration,
                    t.fire_duration,
                ]
            };
            assert_eq!(
                basic_stats(&before),
                basic_stats(&after),
                "{weapon:?} level {level}"
            );
        }
    }
}

#[test]
fn zero_delta_pause_idle_wave_and_dead_hero_freeze_ready_timers() {
    for blocker in [
        "zero_delta",
        "paused",
        "idle_wave",
        "dead_hp",
        "dead_loadout",
        "missing_hero",
        "no_lives",
    ] {
        let (mut app, owner) = setup(HeroWeapon::StormOrb);
        spawn_enemy(&mut app, Vec2::new(90.0, 0.0), 1000.0, 1);
        app.world_mut()
            .resource_mut::<HeroLoadout>()
            .skill_cooldowns = [0.0, 4.0, 8.0];
        match blocker {
            "paused" => app.world_mut().resource_mut::<Paused>().0 = true,
            "idle_wave" => app.world_mut().resource_mut::<RunState>().wave_in_progress = false,
            "dead_hp" => app.world_mut().get_mut::<Tower>(owner).unwrap().hp = 0.0,
            "dead_loadout" => app.world_mut().resource_mut::<HeroLoadout>().alive = false,
            "missing_hero" => {
                app.world_mut().despawn(owner);
            }
            "no_lives" => app.world_mut().resource_mut::<RunState>().lives = 0,
            _ => {}
        }
        step(&mut app, if blocker == "zero_delta" { 0.0 } else { 5.0 });
        assert_eq!(
            app.world().resource::<HeroLoadout>().skill_cooldowns,
            [0.0, 4.0, 8.0],
            "{blocker}"
        );
        assert_no_gameplay_effect(&mut app);
    }
}

#[test]
fn pause_and_between_waves_freeze_cast_phase_and_resume_without_early_impact() {
    for paused in [false, true] {
        let (mut app, owner) = setup_skill(HeroWeapon::StormOrb, 0);
        spawn_enemy(&mut app, Vec2::new(90.0, 0.0), 1000.0, 1);
        start_cast(&mut app, owner, 0);
        let remaining = app.world().get::<HeroCast>(owner).unwrap().remaining;
        let cooldowns = app.world().resource::<HeroLoadout>().skill_cooldowns;
        if paused {
            app.world_mut().resource_mut::<Paused>().0 = true;
        } else {
            app.world_mut().resource_mut::<RunState>().wave_in_progress = false;
        }
        for _ in 0..10 {
            step(&mut app, 1.0);
        }
        assert_eq!(
            app.world().get::<HeroCast>(owner).unwrap().phase,
            CastPhase::Windup
        );
        assert_eq!(
            app.world().get::<HeroCast>(owner).unwrap().remaining,
            remaining
        );
        assert_eq!(
            app.world().resource::<HeroLoadout>().skill_cooldowns,
            cooldowns
        );
        assert_no_gameplay_effect(&mut app);
        app.world_mut().resource_mut::<Paused>().0 = false;
        app.world_mut().resource_mut::<RunState>().wave_in_progress = true;
        release_cast(&mut app, owner, 0);
        assert!(!drain::<Damage>(&mut app).is_empty());
    }
}

#[test]
fn owner_death_equipment_change_or_despawn_cancels_pending_cast() {
    for invalidation in ["hp", "alive", "weapon", "gear", "despawn"] {
        let (mut app, owner) = setup_skill(HeroWeapon::SummonStaff, 0);
        spawn_enemy(&mut app, Vec2::new(90.0, 0.0), 1000.0, 1);
        start_cast(&mut app, owner, 0);
        match invalidation {
            "hp" => app.world_mut().get_mut::<Tower>(owner).unwrap().hp = 0.0,
            "alive" => app.world_mut().resource_mut::<HeroLoadout>().alive = false,
            "weapon" => {
                let mut loadout = app.world_mut().resource_mut::<HeroLoadout>();
                loadout.weapon = HeroWeapon::StormOrb;
                loadout.skill_cooldowns = [1000.0; HeroLoadout::TALENT_SLOTS];
            }
            "gear" => {
                equip_set(&mut app, HeroGearSet::Covenant);
                app.world_mut()
                    .resource_mut::<HeroLoadout>()
                    .skill_cooldowns = [1000.0; HeroLoadout::TALENT_SLOTS];
            }
            "despawn" => {
                app.world_mut().despawn(owner);
            }
            _ => unreachable!(),
        }
        for _ in 0..80 {
            step(&mut app, 0.025);
        }
        assert!(
            app.world().get::<HeroCast>(owner).is_none(),
            "{invalidation}"
        );
        assert_no_gameplay_effect(&mut app);
    }
}

#[test]
fn normal_attacks_cannot_fire_during_any_cast_phase() {
    for weapon in HeroWeapon::ALL {
        let (mut app, owner) = setup_skill(weapon, 0);
        spawn_enemy(&mut app, Vec2::new(45.0, 0.0), 1000.0, 1);
        if weapon == HeroWeapon::SentryCrossbow {
            let anchor = spawn_tower(&mut app, 1.0);
            app.world_mut()
                .get_mut::<Tower>(anchor)
                .unwrap()
                .cooldown_timer = 1000.0;
        }
        start_cast(&mut app, owner, 0);
        let mut phases = [false; 3];
        for _ in 0..120 {
            let Some(cast) = app.world().get::<HeroCast>(owner) else {
                break;
            };
            phases[match cast.phase {
                CastPhase::Windup => 0,
                CastPhase::Release => 1,
                CastPhase::Recovery => 2,
            }] = true;
            drain::<Damage>(&mut app);
            let projectiles_before = count::<Projectile>(&mut app);
            app.world_mut()
                .run_system_once(tower::build_snapshot)
                .unwrap();
            app.world_mut()
                .run_system_once(tower::update_towers)
                .unwrap();
            assert!(
                drain::<Damage>(&mut app)
                    .iter()
                    .all(|hit| hit.source_tower != Some(owner))
            );
            assert_eq!(count::<Projectile>(&mut app), projectiles_before);
            assert_eq!(app.world().get::<Tower>(owner).unwrap().cooldown_timer, 0.0);
            step(&mut app, 0.025);
        }
        assert_eq!(phases, [true; 3], "{weapon:?}");
    }
}

#[test]
fn all_targets_dying_during_windup_prevents_empty_release() {
    let (mut app, owner) = setup_skill(HeroWeapon::StormOrb, 0);
    let target = spawn_enemy(&mut app, Vec2::new(90.0, 0.0), 1000.0, 1);
    start_cast(&mut app, owner, 0);
    app.world_mut().get_mut::<Enemy>(target).unwrap().hp = 0.0;
    for _ in 0..80 {
        step(&mut app, 0.025);
    }
    assert_no_gameplay_effect(&mut app);
    assert_eq!(
        app.world().resource::<HeroLoadout>().skill_cooldowns[0],
        0.0
    );
}

#[test]
fn independent_cooldowns_allow_second_skill_after_first_recovers() {
    let (mut app, owner) = setup_skill(HeroWeapon::StormOrb, 0);
    spawn_enemy(&mut app, Vec2::new(90.0, 0.0), 1000.0, 1);
    app.world_mut()
        .resource_mut::<HeroLoadout>()
        .skill_cooldowns[1] = 0.3;
    cast(&mut app, owner, 0);
    assert_eq!(
        app.world().resource::<HeroLoadout>().skill_cooldowns[1],
        0.0
    );
    for _ in 0..160 {
        step(&mut app, 0.025);
        let events = drain::<HeroSkillCastEvent>(&mut app);
        if events.iter().any(|event| event.slot == 1) {
            assert!(app.world().resource::<HeroLoadout>().skill_cooldowns[0] > 0.0);
            assert!(app.world().resource::<HeroLoadout>().skill_cooldowns[1] > 0.0);
            return;
        }
    }
    panic!("second independent skill never released");
}

#[test]
fn cast_timing_and_cooldowns_use_game_seconds_and_game_speed() {
    let mut elapsed = Vec::new();
    for speed in [1.0, 2.0, 3.0] {
        let (mut app, owner) = setup_skill(HeroWeapon::StormOrb, 0);
        spawn_enemy(&mut app, Vec2::new(90.0, 0.0), 1000.0, 1);
        app.world_mut().resource_mut::<RunState>().game_speed = speed;
        start_cast(&mut app, owner, 0);
        let mut real_seconds = 0.0;
        while drain::<HeroSkillCastEvent>(&mut app).is_empty() {
            assert!(real_seconds < 3.0);
            step(&mut app, 0.01);
            real_seconds += 0.01;
        }
        elapsed.push(real_seconds * speed);
        let before = app.world().resource::<HeroLoadout>().skill_cooldowns[0];
        step(&mut app, 0.1 / speed);
        let after = app.world().resource::<HeroLoadout>().skill_cooldowns[0];
        assert!((before - after - 0.1).abs() < 0.001);
    }
    assert!(
        elapsed
            .iter()
            .all(|seconds| (seconds - elapsed[0]).abs() < 0.06)
    );
}

#[test]
fn cast_trigger_count_matches_at_thirty_and_sixty_frames_per_second() {
    let mut counts = Vec::new();
    for fps in [30, 60] {
        let (mut app, _) = setup_skill(HeroWeapon::StormOrb, 0);
        spawn_enemy(&mut app, Vec2::new(90.0, 0.0), 2000.0, 1);
        let mut triggers = 0;
        for _ in 0..45 * fps {
            step(&mut app, 1.0 / fps as f32);
            triggers += drain::<HeroSkillCastEvent>(&mut app).len();
        }
        counts.push(triggers);
    }
    assert!(counts[0] > 1);
    assert_eq!(counts[0], counts[1]);
}

#[test]
fn movement_orders_and_joystick_block_displacement_but_not_other_skills() {
    for weapon in [HeroWeapon::BannerSword, HeroWeapon::NightDagger] {
        for joystick in [false, true] {
            let (mut app, owner) = setup_skill(weapon, 0);
            spawn_enemy(&mut app, Vec2::new(90.0, 0.0), 1000.0, 1);
            if joystick {
                app.world_mut()
                    .resource_mut::<protect_carrot::ui::JoystickState>()
                    .dir = Vec2::X;
            } else {
                app.world_mut().get_mut::<Tower>(owner).unwrap().move_target =
                    Some(Vec2::new(-200.0, 0.0));
            }
            step(&mut app, 0.1);
            assert!(app.world().get::<HeroCast>(owner).is_none());
            assert_eq!(
                app.world().get::<Tower>(owner).unwrap().hero_pos,
                Vec2::ZERO
            );
            assert_eq!(
                app.world().resource::<HeroLoadout>().skill_cooldowns[0],
                0.0
            );
            assert_no_gameplay_effect(&mut app);
            app.world_mut()
                .resource_mut::<HeroLoadout>()
                .skill_cooldowns[1] = 0.0;
            cast(&mut app, owner, 1);
            assert_eq!(
                app.world().get::<Tower>(owner).unwrap().hero_pos,
                Vec2::ZERO
            );
        }
    }
}

#[test]
fn movement_started_during_windup_cancels_charge_and_blink_without_teleport() {
    for weapon in [HeroWeapon::BannerSword, HeroWeapon::NightDagger] {
        for joystick in [false, true] {
            let (mut app, owner) = setup_skill(weapon, 0);
            spawn_enemy(&mut app, Vec2::new(90.0, 0.0), 1000.0, 1);
            start_cast(&mut app, owner, 0);
            let destination = Vec2::new(-200.0, 0.0);
            if joystick {
                app.world_mut()
                    .resource_mut::<protect_carrot::ui::JoystickState>()
                    .dir = Vec2::X;
            } else {
                app.world_mut().get_mut::<Tower>(owner).unwrap().move_target = Some(destination);
            }
            for _ in 0..80 {
                step(&mut app, 0.025);
            }
            assert!(app.world().get::<HeroCast>(owner).is_none());
            let tower = app.world().get::<Tower>(owner).unwrap();
            assert_eq!(tower.hero_pos, Vec2::ZERO);
            if !joystick {
                assert_eq!(tower.move_target, Some(destination));
            }
            assert_no_gameplay_effect(&mut app);
        }
    }
}

#[test]
fn restoration_heals_living_allies_without_reviving_destroyed_towers() {
    let (mut app, owner) = setup_skill(HeroWeapon::OathShield, 1);
    let ally = spawn_tower(&mut app, 0.5);
    let dead = spawn_tower(&mut app, 0.0);
    app.world_mut().get_mut::<Tower>(owner).unwrap().hp *= 0.5;
    let hp_before = app.world().get::<Tower>(owner).unwrap().hp;
    app.world_mut().resource_mut::<RunState>().lives = 9;
    cast(&mut app, owner, 1);
    assert!(app.world().get::<Tower>(owner).unwrap().hp > hp_before);
    let repaired = app.world().get::<Tower>(ally).unwrap();
    assert!(repaired.hp > repaired.max_hp * 0.5);
    assert_eq!(app.world().get::<Tower>(dead).unwrap().hp, 0.0);
    assert_eq!(app.world().resource::<RunState>().lives, 10);
}

#[test]
fn all_summoner_skills_respect_owner_capacity_without_consuming_unused_souls() {
    for slot in 0..HeroLoadout::TALENT_SLOTS {
        let (mut app, owner) = setup_skill(HeroWeapon::SummonStaff, slot);
        spawn_enemy(&mut app, Vec2::new(90.0, 0.0), 1000.0, 1);
        let existing = (0..12)
            .map(|_| spawn_summon(&mut app, owner, 100.0))
            .collect::<Vec<_>>();
        for _ in 0..3 {
            app.world_mut()
                .resource_mut::<RunState>()
                .remember_fallen_enemy(&enemy(0.0, 1), Vec2::ZERO);
        }
        step(&mut app, 0.1);
        assert!(app.world().get::<HeroCast>(owner).is_none());
        assert_eq!(living_owned_summons(&mut app, owner), 12);
        assert_eq!(app.world().resource::<RunState>().fallen_enemies.len(), 3);
        assert_eq!(
            app.world().resource::<HeroLoadout>().skill_cooldowns[slot],
            0.0
        );
        app.world_mut().get_mut::<Summon>(existing[0]).unwrap().hp = 0.0;
        cast(&mut app, owner, slot);
        assert_eq!(living_owned_summons(&mut app, owner), 12);
        assert_eq!(
            app.world().resource::<RunState>().fallen_enemies.len(),
            if slot == 1 { 2 } else { 3 }
        );
        assert!(drain::<Damage>(&mut app).is_empty());
        assert!(drain::<BuffTower>(&mut app).is_empty());
    }
}

#[test]
fn capacity_filled_during_summon_windup_preserves_recall_souls() {
    let (mut app, owner) = setup_skill(HeroWeapon::SummonStaff, 1);
    spawn_enemy(&mut app, Vec2::new(90.0, 0.0), 1000.0, 1);
    app.world_mut()
        .resource_mut::<RunState>()
        .remember_fallen_enemy(&enemy(0.0, 1), Vec2::ZERO);
    start_cast(&mut app, owner, 1);
    for _ in 0..12 {
        spawn_summon(&mut app, owner, 100.0);
    }
    for _ in 0..80 {
        step(&mut app, 0.025);
    }
    assert_eq!(living_owned_summons(&mut app, owner), 12);
    assert_eq!(app.world().resource::<RunState>().fallen_enemies.len(), 1);
    assert_eq!(
        app.world().resource::<HeroLoadout>().skill_cooldowns[1],
        0.0
    );
    assert!(drain::<HeroSkillCastEvent>(&mut app).is_empty());
}

#[test]
fn summon_budget_includes_equipment_units_but_not_other_owners() {
    for weapon in [
        HeroWeapon::SummonStaff,
        HeroWeapon::ForgeHammer,
        HeroWeapon::ShadowBow,
    ] {
        for set in [HeroGearSet::Covenant, HeroGearSet::Workshop] {
            let (mut app, owner) = setup_skill(weapon, 0);
            equip_set(&mut app, set);
            spawn_enemy(&mut app, Vec2::new(90.0, 0.0), 1000.0, 1);
            for _ in 0..11 {
                spawn_summon(&mut app, owner, 100.0);
            }
            let other = app.world_mut().spawn_empty().id();
            for _ in 0..12 {
                spawn_summon(&mut app, other, 100.0);
            }
            cast(&mut app, owner, 0);
            assert_eq!(
                living_owned_summons(&mut app, owner),
                12,
                "{weapon:?}, {set:?}"
            );
            assert_eq!(living_owned_summons(&mut app, other), 12);
        }
    }
}

#[test]
fn dead_and_expired_summons_do_not_occupy_capacity() {
    let (mut app, owner) = setup_skill(HeroWeapon::SummonStaff, 0);
    spawn_enemy(&mut app, Vec2::new(90.0, 0.0), 1000.0, 1);
    for _ in 0..12 {
        spawn_summon(&mut app, owner, 0.0);
        let expired = spawn_summon(&mut app, owner, 100.0);
        app.world_mut().get_mut::<Summon>(expired).unwrap().lifetime = 0.0;
    }
    cast(&mut app, owner, 0);
    assert_eq!(living_owned_summons(&mut app, owner), 1);
}

#[test]
fn forge_guard_homes_stay_fixed_after_the_owner_moves() {
    let (mut app, owner) = setup_skill(HeroWeapon::ForgeHammer, 0);
    spawn_enemy(&mut app, Vec2::new(90.0, 0.0), 1000.0, 1);
    cast(&mut app, owner, 0);
    let homes_before = app
        .world_mut()
        .query::<&FixedSummonHome>()
        .iter(app.world())
        .map(|home| home.pos)
        .collect::<Vec<_>>();
    assert!(!homes_before.is_empty());
    app.world_mut().get_mut::<Tower>(owner).unwrap().hero_pos = Vec2::new(-250.0, -250.0);
    for _ in 0..30 {
        step(&mut app, 0.025);
    }
    let homes_after = app
        .world_mut()
        .query::<&FixedSummonHome>()
        .iter(app.world())
        .map(|home| home.pos)
        .collect::<Vec<_>>();
    assert_eq!(homes_before, homes_after);
}

#[test]
fn recall_consumes_only_fallen_non_boss_souls_and_never_copies_living_enemies() {
    let (mut app, owner) = setup_skill(HeroWeapon::SummonStaff, 1);
    let mut boss = enemy(0.0, 1);
    boss.boss = true;
    {
        let mut run = app.world_mut().resource_mut::<RunState>();
        run.remember_fallen_enemy(&enemy(1000.0, 1), Vec2::ZERO);
        run.remember_fallen_enemy(&boss, Vec2::ZERO);
        assert!(run.fallen_enemies.is_empty());
    }
    spawn_enemy(&mut app, Vec2::new(90.0, 0.0), 1000.0, 1);
    step(&mut app, 0.1);
    assert!(app.world().get::<HeroCast>(owner).is_none());
    app.world_mut()
        .resource_mut::<RunState>()
        .remember_fallen_enemy(&enemy(0.0, 1), Vec2::ZERO);
    cast(&mut app, owner, 1);
    assert_eq!(count::<Summon>(&mut app), 1);
    assert!(app.world().resource::<RunState>().fallen_enemies.is_empty());
    let mut run = app.world_mut().resource_mut::<RunState>();
    for i in 0..20 {
        run.remember_fallen_enemy(&enemy(0.0, 1), Vec2::new(i as f32, 0.0));
    }
    assert_eq!(run.fallen_enemies.len(), 8);
    assert_eq!(run.fallen_enemies.front().unwrap().pos.x, 12.0);
}

#[test]
fn storm_chain_has_falloff_and_stops_at_a_real_gap() {
    let (mut app, owner) = setup_skill(HeroWeapon::StormOrb, 0);
    let targets = [100.0, 220.0, 340.0, 700.0]
        .into_iter()
        .enumerate()
        .map(|(i, x)| spawn_enemy(&mut app, Vec2::new(x, 0.0), 1000.0, 9 - i))
        .collect::<Vec<_>>();
    cast(&mut app, owner, 0);
    let hits = drain::<Damage>(&mut app);
    assert_eq!(
        hits.iter().map(|hit| hit.target).collect::<Vec<_>>(),
        targets[..3]
    );
    assert!(hits.windows(2).all(|pair| pair[0].amount > pair[1].amount));
}

#[test]
fn sentry_refraction_needs_a_living_anchor() {
    let (mut app, owner) = setup_skill(HeroWeapon::SentryCrossbow, 0);
    spawn_enemy(&mut app, Vec2::new(90.0, 0.0), 1000.0, 1);
    let anchor = spawn_tower(&mut app, 0.0);
    step(&mut app, 0.1);
    assert!(app.world().get::<HeroCast>(owner).is_none());
    assert_no_gameplay_effect(&mut app);
    app.world_mut().get_mut::<Tower>(anchor).unwrap().hp = 100.0;
    cast(&mut app, owner, 0);
    assert!(count::<Projectile>(&mut app) > 0);
}

#[test]
fn deployed_sentry_requires_an_enemy_inside_its_actual_firing_radius() {
    let (mut app, owner) = setup_skill(HeroWeapon::SentryCrossbow, 2);
    let target = spawn_enemy(&mut app, Vec2::new(360.0, 0.0), 1000.0, 1);
    step(&mut app, 0.1);
    assert!(app.world().get::<HeroCast>(owner).is_none());
    assert_no_gameplay_effect(&mut app);
    app.world_mut()
        .get_mut::<Transform>(target)
        .unwrap()
        .translation
        .x = 300.0;
    cast(&mut app, owner, 2);
    assert_eq!(
        count::<protect_carrot::hero_skill_effects::SkillField>(&mut app),
        1
    );
}

#[test]
fn poison_and_binding_projectiles_apply_their_payload_only_on_arrival() {
    for (weapon, slot) in [
        (HeroWeapon::ShadowBow, 0),
        (HeroWeapon::NightDagger, 1),
        (HeroWeapon::SentryCrossbow, 0),
        (HeroWeapon::SentryCrossbow, 1),
    ] {
        let (mut app, owner) = setup_skill(weapon, slot);
        let target = spawn_enemy(&mut app, Vec2::new(160.0, 0.0), 1000.0, 1);
        if weapon == HeroWeapon::SentryCrossbow {
            spawn_tower(&mut app, 1.0);
        }
        cast(&mut app, owner, slot);
        assert!(drain::<Damage>(&mut app).is_empty());
        assert!(drain::<Status>(&mut app).is_empty());
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f32(1.0));
        app.world_mut()
            .run_system_once(tower::build_snapshot)
            .unwrap();
        app.world_mut()
            .run_system_once(tower::update_projectiles)
            .unwrap();
        app.world_mut()
            .run_system_once(tower::update_projectiles)
            .unwrap();
        let damage = drain::<Damage>(&mut app);
        assert!(
            damage
                .iter()
                .any(|hit| hit.target == target && hit.amount > 0.0)
        );
        let statuses = drain::<Status>(&mut app);
        assert!(statuses.iter().all(|status| status.target == target));
        match (weapon, slot) {
            (HeroWeapon::ShadowBow, 0) | (HeroWeapon::NightDagger, 1) => {
                assert!(
                    statuses
                        .iter()
                        .any(|status| matches!(status.kind, StatusKind::Poison { .. }))
                );
            }
            (HeroWeapon::SentryCrossbow, 0) => {
                assert!(
                    statuses
                        .iter()
                        .any(|status| matches!(status.kind, StatusKind::Slow { .. }))
                );
            }
            (HeroWeapon::SentryCrossbow, 1) => {
                assert!(
                    statuses
                        .iter()
                        .any(|status| matches!(status.kind, StatusKind::Freeze { .. }))
                );
                assert!(
                    statuses
                        .iter()
                        .any(|status| matches!(status.kind, StatusKind::Curse { .. }))
                );
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn forge_rockets_travel_retarget_and_explode_with_burn() {
    let (mut app, owner) = setup_skill(HeroWeapon::ForgeHammer, 1);
    let target = spawn_enemy(&mut app, Vec2::new(200.0, 0.0), 1000.0, 2);
    cast(&mut app, owner, 1);
    assert!(count::<Projectile>(&mut app) > 0);
    assert!(drain::<Damage>(&mut app).is_empty());
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_secs_f32(0.1));
    app.world_mut()
        .run_system_once(tower::build_snapshot)
        .unwrap();
    app.world_mut()
        .run_system_once(tower::update_projectiles)
        .unwrap();
    let positions = app
        .world_mut()
        .query_filtered::<&Transform, With<Projectile>>()
        .iter(app.world())
        .map(|transform| transform.translation)
        .collect::<Vec<_>>();
    assert!(positions.iter().all(|pos| pos.x > 0.0 && pos.x < 200.0));
    app.world_mut().despawn(target);
    let replacement = spawn_enemy(&mut app, Vec2::new(120.0, 100.0), 1000.0, 1);
    let splash = spawn_enemy(&mut app, Vec2::new(145.0, 100.0), 1000.0, 0);
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_secs_f32(1.0));
    app.world_mut()
        .run_system_once(tower::build_snapshot)
        .unwrap();
    app.world_mut()
        .run_system_once(tower::update_projectiles)
        .unwrap();
    app.world_mut()
        .run_system_once(tower::update_projectiles)
        .unwrap();
    assert_eq!(count::<Projectile>(&mut app), 0);
    let hits = drain::<Damage>(&mut app);
    assert!(!hits.is_empty());
    assert!(
        hits.iter()
            .all(|hit| [replacement, splash].contains(&hit.target)
                && hit.source_tower == Some(owner))
    );
    assert!(
        drain::<Status>(&mut app)
            .iter()
            .any(|status| matches!(status.kind, StatusKind::Fire { .. }))
    );
}

#[test]
fn fallback_hero_recovers_animated_body_when_atlas_becomes_available() {
    let (mut app, owner) = setup(HeroWeapon::SummonStaff);
    app.world_mut().entity_mut(owner).insert((
        Sprite::default(),
        protect_carrot::hero_paperdoll::HeroPaperdollSprite,
    ));
    let image = app
        .world_mut()
        .resource_mut::<Assets<Image>>()
        .add(Image::default());
    app.world_mut()
        .resource_mut::<build::HeroWalks>()
        .race_worlds
        .insert(
            (HeroWeapon::SummonStaff, Race::Human),
            build::HeroWalkCfg {
                image: image.clone(),
                layout: default(),
                frames: 16,
                size: 60.0,
                idle_anim: default(),
                walk_anim: default(),
                attack_anim: default(),
            },
        );
    app.world_mut()
        .run_system_once(build::refresh_hero_visual)
        .unwrap();
    assert!(app.world().get::<build::HeroWalkAnim>(owner).is_some());
    assert!(
        app.world()
            .get::<protect_carrot::hero_paperdoll::HeroPaperdollSprite>(owner)
            .is_none()
    );
    assert_eq!(app.world().get::<Sprite>(owner).unwrap().image, image);
    assert!(
        app.world()
            .get::<Sprite>(owner)
            .unwrap()
            .texture_atlas
            .is_some()
    );
}

#[test]
fn skill_icons_follow_current_class_and_ultimate_unlock() {
    let (mut app, _) = setup(HeroWeapon::BannerSword);
    let mut icons = Vec::new();
    for slot in 0..HeroLoadout::TALENT_SLOTS {
        let icon = app.world_mut().spawn(ImageNode::default()).id();
        app.world_mut()
            .spawn(protect_carrot::ui::UiAction::HeroTalent(slot))
            .add_child(icon);
        icons.push(icon);
    }
    app.world_mut().resource_mut::<HeroLoadout>().weapon = HeroWeapon::SummonStaff;
    app.world_mut()
        .run_system_once(protect_carrot::ui::update_hero_skill_icons)
        .unwrap();
    for (slot, icon) in icons.iter().enumerate() {
        let expected = &app.world().resource::<sprites::Sprites>().hero_talents
            [&(HeroWeapon::SummonStaff, slot)];
        assert_eq!(
            &app.world().get::<ImageNode>(*icon).unwrap().image,
            expected
        );
    }
    assert_ne!(
        app.world().get::<ImageNode>(icons[2]).unwrap().color,
        Color::WHITE
    );
    app.world_mut().resource_mut::<HeroLoadout>().level = HeroLoadout::MAX_LEVEL;
    app.world_mut()
        .run_system_once(protect_carrot::ui::update_hero_skill_icons)
        .unwrap();
    assert_eq!(
        app.world().get::<ImageNode>(icons[2]).unwrap().color,
        Color::WHITE
    );
}
