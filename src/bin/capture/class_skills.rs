use super::*;
use protect_carrot::hero_passives::{CastPhase, HeroCast, HeroSkillCastEvent};

#[derive(Resource, Default)]
pub(super) struct CaptureHeroPassiveSmoke {
    cursor: usize,
    pending: bool,
    released: bool,
    phase_images: [bool; 3],
    wait_frames: u32,
    settle_frames: u32,
    output: Option<PathBuf>,
    final_summoner: bool,
}

#[derive(Component)]
pub(super) struct ClassSkillTarget(usize);

const TARGET_POSITIONS: [Vec2; 3] = [
    Vec2::new(-10.0, 0.0),
    Vec2::new(35.0, 28.0),
    Vec2::new(65.0, -25.0),
];

fn phase_output(job: &CaptureJob) -> PathBuf {
    match job.mode {
        CaptureMode::Frames => job.output.join("skill-phases"),
        CaptureMode::Screenshot => job.output.with_file_name(format!(
            "{}-phases",
            job.output.file_stem().unwrap_or_default().to_string_lossy()
        )),
    }
}

pub(super) fn drive_hero_passive_smoke(
    mut commands: Commands,
    scenario: Res<CaptureScenario>,
    mut state: ResMut<CaptureHeroPassiveSmoke>,
    mut prepared: ResMut<CapturePrepared>,
    mut loadout: ResMut<hero::HeroLoadout>,
    mut run: ResMut<RunState>,
    mut roguelite: ResMut<roguelite::RogueliteRun>,
    (mut selection, mut panels): (ResMut<Selection>, ResMut<ui::HudPanels>),
    mut towers: Query<(Entity, &mut tower::Tower)>,
    mut enemies: Query<(
        Entity,
        &mut Enemy,
        &mut Transform,
        Option<&ClassSkillTarget>,
    )>,
    casts: Query<&HeroCast>,
    transients: Query<
        Entity,
        Or<(
            With<tower::Summon>,
            With<tower::Projectile>,
            With<protect_carrot::hero_skill_effects::SkillField>,
            With<protect_carrot::components::FireGround>,
        )>,
    >,
    mut events: MessageReader<HeroSkillCastEvent>,
    (target, job, summons): (Res<CaptureTarget>, Res<CaptureJob>, Query<&tower::Summon>),
    (creatures, sprites, talents, board): (
        Res<creatures::Creatures>,
        Res<sprites::Sprites>,
        Res<meta::Talents>,
        Res<Board>,
    ),
) {
    if *scenario != CaptureScenario::HeroPassiveSmoke
        || !prepared.level_ready
        || prepared.scenario_ready
    {
        return;
    }
    let Some((owner, _)) = towers.iter().find(|(_, tower)| tower.hero) else {
        return;
    };

    run.wave_in_progress = true;
    run.auto_wave = false;
    run.spawned = run.spawn_target;
    run.game_speed = 1.0;
    roguelite.draft = None;
    run.message.clear();
    run.message_queue.clear();
    for (_, mut tower) in &mut towers {
        tower.cooldown_timer = 1000.0;
    }
    for (_, mut enemy, _, probe) in &mut enemies {
        if probe.is_some() {
            enemy.hp = enemy.max_hp;
        }
    }

    if state.output.is_none() {
        let output = phase_output(&job);
        std::fs::create_dir_all(&output).expect("create skill-phase screenshot directory");
        state.output = Some(output);
        for (entity, _, _, _) in &enemies {
            commands.entity(entity).despawn();
        }
        for (index, pos) in TARGET_POSITIONS.into_iter().enumerate() {
            let mut enemy = capture_probe_enemy();
            enemy.hp = 1_000_000.0;
            enemy.max_hp = enemy.hp;
            enemy.path_index = 0;
            enemy.frozen = false;
            enemy.stun_timer = 0.0;
            let (sprite, animation, sheet) = creatures.sprite(data::EnemyKind::Normal, 58.0);
            commands.spawn((
                enemy,
                sprite,
                animation,
                sheet,
                Transform::from_translation(pos.extend(5.0)),
                Visibility::Visible,
                ClassSkillTarget(index),
                protect_carrot::components::LevelEntity,
            ));
        }
        let anchor = board
            .buildable
            .iter()
            .copied()
            .min_by(|a, b| {
                cell_center(a.0 as f32, a.1 as f32)
                    .length_squared()
                    .total_cmp(&cell_center(b.0 as f32, b.1 as f32).length_squared())
            })
            .expect("capture board contains a buildable anchor");
        build::spawn_tower(
            &mut commands,
            TowerKind::Arrow,
            anchor.0,
            anchor.1,
            &sprites,
            &talents,
        );
        loadout.skill_cooldowns = [1000.0; hero::HeroLoadout::TALENT_SLOTS];
        // Assets and UI need a few rendered frames before collecting proof images.
        state.settle_frames = 45;
        return;
    }
    if state.settle_frames > 0 {
        state.settle_frames -= 1;
        return;
    }

    if state.pending {
        let weapon = hero::HeroWeapon::ALL[state.cursor / hero::HeroLoadout::TALENT_SLOTS];
        let slot = state.cursor % hero::HeroLoadout::TALENT_SLOTS;
        for event in events.read() {
            if event.owner == owner {
                assert_eq!(event.weapon, weapon, "capture released the wrong class");
                assert_eq!(event.slot, slot, "capture released the wrong skill");
                assert!(!state.released, "capture skill released more than once");
                if weapon == hero::HeroWeapon::SummonStaff {
                    let pos = towers.get(owner).unwrap().1.center();
                    assert!(
                        enemies.iter().all(|(_, enemy, tf, _)| {
                            enemy.hp <= 0.0 || tf.translation.truncate().distance(pos) > 420.0
                        }),
                        "summoner capture must have no enemies in cast range"
                    );
                    let count = summons
                        .iter()
                        .filter(|summon| {
                            summon.owner == owner && summon.hp > 0.0 && summon.lifetime > 0.0
                        })
                        .count();
                    assert_eq!(
                        count,
                        [1, 2, 3][slot],
                        "summoner must create fighting allies"
                    );
                    println!(
                        "[capture/class-skills] summoner level {} skill {}: {count} living allies, no enemies in cast range",
                        loadout.level,
                        slot + 1
                    );
                }
                state.released = true;
            }
        }
        if let Ok(cast) = casts.get(owner) {
            let (phase, label) = match cast.phase {
                CastPhase::Windup => (0, "windup"),
                CastPhase::Release => (1, "release"),
                CastPhase::Recovery => (2, "recovery"),
            };
            if !state.phase_images[phase] && cast.remaining <= cast.duration * 0.55 {
                let path = state.output.as_ref().unwrap().join(format!(
                    "{:02}-{}-{label}.png",
                    state.cursor / 3,
                    slot + 1
                ));
                commands
                    .spawn(Screenshot::image(target.0.clone()))
                    .observe(save_to_disk(path.clone()));
                state.phase_images[phase] = true;
                println!(
                    "[capture/class-skills] {:?} skill {} {label}: {}",
                    weapon,
                    slot + 1,
                    path.display()
                );
            }
        } else if state.released {
            assert_eq!(
                state.phase_images, [true; 3],
                "capture must observe all cast phases"
            );
            println!(
                "[capture/class-skills] verified {:?} skill {} complete automatic cast",
                weapon,
                slot + 1
            );
            state.cursor += 1;
            state.pending = false;
            state.settle_frames = 8;
            return;
        }
        state.wait_frames += 1;
        assert!(
            state.wait_frames <= 180,
            "[capture/class-skills] {:?} skill {} never completed",
            weapon,
            slot + 1
        );
        return;
    }

    if state.cursor == hero::HeroWeapon::ALL.len() * hero::HeroLoadout::TALENT_SLOTS {
        if !state.final_summoner {
            for entity in &transients {
                commands.entity(entity).despawn();
            }
            loadout.weapon = hero::HeroWeapon::SummonStaff;
            loadout.skill_cooldowns = [1000.0, 1000.0, 0.0];
            for (_, mut tower) in &mut towers {
                if tower.hero {
                    hero::apply_loadout_to_tower(&loadout, &mut tower);
                    tower.hero_pos = Vec2::new(-105.0, 0.0);
                }
            }
            state.final_summoner = true;
            state.wait_frames = 0;
            return;
        }
        if loadout.skill_cooldowns[2] <= 0.0 {
            state.wait_frames += 1;
            assert!(
                state.wait_frames <= 180,
                "final summoner showcase did not release"
            );
            return;
        }
        panels.hero_open = true;
        selection.selected = Some(owner);
        prepared.scenario_ready = true;
        println!(
            "[capture/class-skills] all 27 skills verified with 81 windup/release/recovery screenshots"
        );
        return;
    }

    for entity in &transients {
        commands.entity(entity).despawn();
    }
    let weapon = hero::HeroWeapon::ALL[state.cursor / hero::HeroLoadout::TALENT_SLOTS];
    let slot = state.cursor % hero::HeroLoadout::TALENT_SLOTS;
    for (_, mut enemy, mut transform, probe) in &mut enemies {
        if let Some(probe) = probe {
            enemy.hp = enemy.max_hp;
            // Freeze distant probes so route projection cannot move them back into cast range.
            enemy.frozen = weapon == hero::HeroWeapon::SummonStaff;
            enemy.stun_timer = if weapon == hero::HeroWeapon::SummonStaff {
                1000.0
            } else {
                0.0
            };
            enemy.slow_timer = 0.0;
            enemy.poison_timer = 0.0;
            enemy.fire_timer = 0.0;
            let offset = if weapon == hero::HeroWeapon::SummonStaff {
                Vec2::new(600.0, 0.0)
            } else {
                Vec2::ZERO
            };
            transform.translation = (TARGET_POSITIONS[probe.0] + offset).extend(5.0);
        }
    }
    loadout.weapon = weapon;
    loadout.race = hero::Race::Human;
    loadout.level = if weapon == hero::HeroWeapon::SummonStaff && slot < 2 {
        1
    } else {
        hero::HeroLoadout::MAX_LEVEL
    };
    loadout.gear = [None; hero_gear::HeroGearSlot::COUNT];
    loadout.skill_cooldowns = [1000.0; hero::HeroLoadout::TALENT_SLOTS];
    loadout.skill_cooldowns[slot] = 0.0;
    loadout.alive = true;
    let weapon_index = loadout.weapon_index();
    loadout.weapon_talents[weapon_index] = if weapon == hero::HeroWeapon::SummonStaff {
        [0; hero::HeroLoadout::TALENT_SLOTS]
    } else {
        [3, 3, 0]
    };
    run.fallen_enemies.clear();
    if weapon == hero::HeroWeapon::SummonStaff && slot == 1 {
        let mut soul = capture_probe_enemy();
        soul.hp = 0.0;
        for _ in 0..3 {
            run.remember_fallen_enemy(&soul, Vec2::ZERO);
        }
    }
    for (_, mut tower) in &mut towers {
        if tower.hero {
            hero::apply_loadout_to_tower(&loadout, &mut tower);
            tower.hero_pos = Vec2::new(-105.0, 0.0);
            tower.move_target = None;
            tower.angle = 0.0;
        }
        tower.hp = tower.max_hp * 0.5;
    }
    selection.selected = Some(owner);
    selection.build_kind = None;
    panels.dock_open = false;
    panels.hero_open = false;
    panels.settings_open = false;
    state.pending = true;
    state.released = false;
    state.phase_images = [false; 3];
    state.wait_frames = 0;
    println!(
        "[capture/class-skills] prepared {:?} skill {}",
        weapon,
        slot + 1
    );
}
