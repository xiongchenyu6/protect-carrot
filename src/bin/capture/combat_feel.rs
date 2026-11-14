//! Confirmed-damage contact sequences, separate from the 27-skill phase probe.
use super::*;
use protect_carrot::combat_feel::ImpactTier;

#[derive(Resource, Default)]
pub(super) struct CaptureCombatFeel {
    case: usize,
    frame: u32,
    warmup: u32,
    target: Option<Entity>,
    source: Option<Entity>,
    matched: usize,
}

const CASES: [(&str, ImpactTier); 6] = [
    ("ordinary", ImpactTier::Normal),
    ("heavy", ImpactTier::Heavy),
    ("empowered", ImpactTier::Empowered),
    ("finisher", ImpactTier::Finisher),
    ("ultimate", ImpactTier::Ultimate),
    ("tower-swarm", ImpactTier::Normal),
];

pub(super) fn drive_combat_feel(
    mut commands: Commands,
    scenario: Res<CaptureScenario>,
    mut prepared: ResMut<CapturePrepared>,
    mut state: ResMut<CaptureCombatFeel>,
    mut run: ResMut<RunState>,
    mut hero: ResMut<hero::HeroLoadout>,
    mut towers: Query<(Entity, &mut tower::Tower)>,
    enemies: Query<Entity, With<Enemy>>,
    transients: Query<
        Entity,
        Or<(
            With<tower::Projectile>,
            With<tower::Summon>,
            With<protect_carrot::components::Particle>,
            With<protect_carrot::components::FloatText>,
            With<protect_carrot::components::Shockwave>,
            With<protect_carrot::pixelfx::PixelFxAnimation>,
        )>,
    >,
    (creatures, effects, assets, cards): (
        Res<creatures::Creatures>,
        Res<protect_carrot::pixelfx::PixelFxLibrary>,
        Res<AssetServer>,
        Query<(), With<protect_carrot::polish::EpisodeCard>>,
    ),
    target: Res<CaptureTarget>,
    job: Res<CaptureJob>,
    mut impacts: MessageReader<vfx::VfxEvent>,
    mut damage: MessageWriter<tower::Damage>,
) {
    if *scenario != CaptureScenario::CombatFeel
        || !prepared.level_ready
        || prepared.scenario_ready
        || !effects.ready(&assets)
        || !cards.is_empty()
    {
        return;
    }
    run.wave_in_progress = false;
    run.auto_wave = false;
    run.message.clear();
    run.message_queue.clear();
    hero.skill_cooldowns = [1000.0; 3];
    for (_, mut tower) in &mut towers {
        tower.cooldown_timer = 1000.0;
    }
    if state.warmup < 90 {
        state.warmup += 1;
        return;
    }
    if state.case >= CASES.len() {
        prepared.scenario_ready = true;
        println!(
            "[capture/feel] all six confirmed-hit cases verified with 48 contact/recovery frames"
        );
        return;
    }
    let (name, tier) = CASES[state.case];
    for impact in impacts.read() {
        if let vfx::VfxEvent::Impact {
            target,
            tier: actual,
            ..
        } = impact
        {
            if Some(*target) == state.target {
                assert_eq!(
                    *actual, tier,
                    "[capture/feel] incorrect feedback tier for {name}"
                );
                state.matched += 1;
            }
        }
    }
    if state.frame == 0 {
        for enemy in &enemies {
            commands.entity(enemy).despawn();
        }
        for entity in &transients {
            commands.entity(entity).despawn();
        }
        let tower_source = towers
            .iter()
            .find(|(_, tower)| !tower.hero)
            .map(|(entity, _)| entity);
        hero.weapon = if state.case == 1 {
            hero::HeroWeapon::ForgeHammer
        } else {
            hero::HeroWeapon::BannerSword
        };
        hero.race = hero::Race::Human;
        hero.level = hero::HeroLoadout::MAX_LEVEL;
        hero.gear = [None; hero_gear::HeroGearSlot::COUNT];
        for (entity, mut tower) in &mut towers {
            if tower.hero {
                commands
                    .entity(entity)
                    .remove::<protect_carrot::hero_passives::HeroCast>();
                hero::apply_loadout_to_tower(&hero, &mut tower);
                tower.hp = tower.max_hp;
                tower.hero_pos = Vec2::new(-50.0, 0.0);
                tower.move_target = None;
                tower.angle = 0.0;
                tower.hero_attack_timer = 0.3;
                state.source = if state.case == 0 || state.case == 5 {
                    tower_source
                } else {
                    Some(entity)
                };
                if state.case == 4 {
                    commands
                        .entity(entity)
                        .insert(protect_carrot::hero_passives::HeroCast {
                            weapon: hero.weapon,
                            gear: hero.gear,
                            slot: 2,
                            phase: protect_carrot::hero_passives::CastPhase::Release,
                            remaining: 0.06,
                            duration: 0.12,
                            released: true,
                        });
                }
            }
        }
        let mut enemy = capture_probe_enemy();
        let source_damage = state
            .source
            .and_then(|entity| towers.get(entity).ok())
            .map(|(_, tower)| tower.damage)
            .unwrap_or(20.0);
        enemy.hp = if state.case == 3 {
            source_damage * 0.75
        } else {
            1_000_000.0
        };
        enemy.max_hp = if state.case == 3 {
            source_damage * 8.0
        } else {
            enemy.hp
        };
        enemy.shield = 0.0;
        enemy.armor = 0.0;
        enemy.magic_resist = 0.0;
        enemy.base_speed = 0.0;
        enemy.element_resist = data::ElementProfile::none();
        enemy.element_resist.physical = if state.case == 2 { -0.3 } else { 0.0 };
        enemy.boss = state.case == 3;
        let (sprite, animation, sheet) = creatures.sprite(data::EnemyKind::Normal, 58.0);
        state.target = Some(
            commands
                .spawn((
                    enemy,
                    sprite,
                    animation,
                    sheet,
                    Transform::from_xyz(15.0, 0.0, 5.0),
                    Visibility::Visible,
                    protect_carrot::components::LevelEntity,
                ))
                .id(),
        );
        state.matched = 0;
    } else if state.frame == 3 {
        let source = state.source.expect("contact case source exists");
        let (_, actor) = towers.get(source).expect("contact source alive");
        let count = if state.case == 5 { 20 } else { 1 };
        for _ in 0..count {
            damage.write(tower::Damage {
                ultimate: state.case == 4,
                source_tower: Some(source),
                target: state.target.unwrap(),
                amount: actor.damage.max(10.0),
                magic: false,
                element: data::Element::Physical,
                armor_pierce: 0.0,
            });
        }
    }
    if (3..11).contains(&state.frame) {
        let output = job.output.with_file_name(format!(
            "{}-contacts",
            job.output.file_stem().unwrap().to_string_lossy()
        ));
        std::fs::create_dir_all(&output).expect("contact frame directory");
        let path = output.join(format!("{name}-{:02}.png", state.frame - 3));
        commands
            .spawn(Screenshot::image(target.0.clone()))
            .observe(save_to_disk(path));
    }
    if state.frame >= 12 {
        let expected = if state.case == 5 { 20 } else { 1 };
        assert_eq!(
            state.matched, expected,
            "[capture/feel] {name} must be actual simulation damage"
        );
        println!("[capture/feel] {name}: {expected} real hits, eight contact/recovery frames");
        state.case += 1;
        state.frame = 0;
    } else {
        state.frame += 1;
    }
}
