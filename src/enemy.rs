//! Enemy spawning, path-following, status effects, death and carrot arrival.
//! Ported from `spawnEnemy` / `getEnemyPoolForWave` / `updateEnemies`.

use crate::board::Board;
use crate::components::{Enemy, HpBarFg, LevelEntity, ShieldBarFg};
use crate::creatures::Creatures;
use crate::data::{Element, EnemyKind, MOSS_TOWER_SENSE, TILE_SIZE, TOWER_RAIDER_SENSE};
use crate::equipment::{
    equipment_set_bonus, return_equipment_to_inventory, roll_drop, EquipmentInventory, Rarity,
};
use crate::game::{CurrentLevel, Rng, RunState, AUTO_WAVE_DELAY, KILL_COMBO_WINDOW};
use crate::monster::{
    boss_skill, default_species_id, pick_boss, pick_elite_affix, pick_regular, species_by_id,
    BossSkill, EliteAffix, MonsterSpecies, MONSTER_SPECIES,
};
use crate::sprites::Sprites;
use crate::states::GameState;
use crate::ui::UiFont;
use crate::Levels;
use bevy::prelude::*;
use bevy::sprite::Anchor;

const HEAL_AURA_RADIUS: f32 = 90.0;
const BOSS_CAST_WINDUP: f32 = 0.75;
const BOSS_ENRAGE_HP_FRACTION: f32 = 0.35;
const BOSS_ENRAGE_SKILL_RATE: f32 = 1.55;
#[derive(Component)]
pub struct PendingBossCast {
    pub(crate) skill: BossSkill,
    pub(crate) timer: f32,
    pub(crate) max_timer: f32,
    pub(crate) radius: f32,
}

fn special_trait_badge(
    def: &crate::data::EnemyDef,
    tower_raider: bool,
    silence_aura: f32,
    heal_aura: f32,
    shield: f32,
    splits: i32,
) -> Option<(&'static str, Color)> {
    if tower_raider {
        Some(("攻城", Color::srgb(1.0, 0.58, 0.22)))
    } else if silence_aura > 0.0 {
        Some(("静默", Color::srgb(0.82, 0.45, 1.0)))
    } else if heal_aura > 0.0 {
        Some(("治疗", Color::srgb(0.42, 1.0, 0.52)))
    } else if shield > 0.0 {
        Some(("护盾", Color::srgb(0.45, 0.82, 1.0)))
    } else if splits > 0 {
        Some(("分裂", Color::srgb(0.82, 0.56, 1.0)))
    } else if def.charger {
        Some(("冲锋", Color::srgb(1.0, 0.72, 0.28)))
    } else if def.regen > 0.0 {
        Some(("再生", Color::srgb(0.45, 1.0, 0.62)))
    } else if def.invisible {
        Some(("隐形", Color::srgb(0.72, 0.78, 0.86)))
    } else if def.flying {
        Some(("飞行", Color::srgb(0.46, 0.78, 1.0)))
    } else {
        None
    }
}

/// Build the component + visual for one enemy at the path start.
fn spawn_one(
    commands: &mut Commands,
    species: &MonsterSpecies,
    board: &Board,
    level_hp: f32,
    level_speed: f32,
    level_reward: f32,
    wave: i32,
    level_index: usize,
    sprites: &Sprites,
    font: &UiFont,
    diff: crate::game::Difficulty,
    elite_affix: EliteAffix,
    endless: bool,
) {
    let kind = species.kind;
    let def = kind.def();
    let is_elite = elite_affix != EliteAffix::None;
    let wave_mult = 1.0 + (wave - 1) as f32 * 0.35 + level_index as f32 * 0.08;
    let endless_wave = if endless { wave.max(1) as f32 } else { 0.0 };
    let endless_hp = if endless {
        1.0 + (endless_wave / 10.0).powf(1.08) * 0.18
    } else {
        1.0
    };
    let endless_speed = if endless {
        1.0 + (endless_wave * 0.006).min(0.45)
    } else {
        1.0
    };
    let endless_reward = if endless {
        1.0 + (endless_wave * 0.012).min(0.9)
    } else {
        1.0
    };
    let endless_defense = if endless {
        ((wave - 1).max(0) as f32 * 0.8).min(60.0)
    } else {
        0.0
    };
    let endless_melee = if endless {
        1.0 + (endless_wave * 0.010).min(1.0)
    } else {
        1.0
    };
    let elite_hp = if is_elite { 2.6 } else { 1.0 };
    let elite_rw = if is_elite { 3.0 } else { 1.0 };
    let affix_hp = match elite_affix {
        EliteAffix::Carapace => 1.22,
        EliteAffix::Bloodrite | EliteAffix::Siege => 1.10,
        _ => 1.0,
    };
    let affix_speed = match elite_affix {
        EliteAffix::Frenzy => 1.28,
        EliteAffix::Carapace => 0.88,
        EliteAffix::Siege => 0.94,
        _ => 1.0,
    };
    let hp = (level_hp
        * wave_mult
        * def.hp_mod
        * species.hp_mult
        * diff.hp_mult()
        * endless_hp
        * elite_hp
        * affix_hp)
        .floor();
    // px/sec: original moved `speed*dt/16` with speed = level.speed*TILE/60*mod.
    let base_speed =
        level_speed * def.speed_mod * species.speed_mult * affix_speed * endless_speed * TILE_SIZE
            / 60.0
            * (1000.0 / 16.0);
    let reward = (level_reward
        * def.reward_mod
        * species.reward_mult
        * diff.reward_mult()
        * endless_reward
        * elite_rw)
        .floor() as i32;
    let armor = species.armor()
        + match elite_affix {
            EliteAffix::Carapace => 18.0,
            EliteAffix::Siege => 8.0,
            _ => 0.0,
        }
        + endless_defense;
    let magic_resist = species.magic_resist()
        + match elite_affix {
            EliteAffix::YellowSign => 14.0,
            EliteAffix::Bloodrite => 8.0,
            _ => 0.0,
        }
        + endless_defense * 0.7;
    let shield_wave_mult = wave_mult * endless_hp;
    let mut shield = if is_elite {
        (def.shield * shield_wave_mult).max(hp * 0.25).floor()
    } else {
        (def.shield * shield_wave_mult).floor()
    };
    if elite_affix == EliteAffix::Carapace {
        shield += (hp * 0.22).floor();
    }
    // `splits` now means remaining split GENERATIONS (not a one-shot splinter
    // count): a 普通 splitter splits 1 generation, 中级 2, 高级 4 — tier derived
    // from the species' grade. Each generation halves the splinter's size & stats.
    // The Brood elite affix grants one extra generation.
    let splits = if def.splits > 0 {
        crate::monster::SkillTier::from_grade(species.grade()).split_generations()
            + match elite_affix {
                EliteAffix::Brood => 1,
                _ => 0,
            }
    } else {
        0
    };
    let regen = def.regen
        + match elite_affix {
            EliteAffix::Bloodrite => 0.012,
            _ => 0.0,
        };
    let heal_aura = def.heal_aura
        + match elite_affix {
            EliteAffix::Bloodrite => 18.0,
            _ => 0.0,
        };
    let tower_raider = def.tower_raider || elite_affix == EliteAffix::Siege;
    let tower_dps = if elite_affix == EliteAffix::Siege {
        def.tower_dps.max(18.0 + wave as f32 * 1.2)
    } else {
        def.tower_dps
    };
    let silence_aura = if elite_affix == EliteAffix::YellowSign {
        def.silence_aura.max(82.0)
    } else {
        def.silence_aura
    };
    // 技能分级：每个怪物的技能都有 普通/中级/高级 三个级别（由品级推导）。高级版
    // 数值更强 —— 再生更快、护盾更厚、硬化（护甲/抗性）更高、治疗/静默范围更大、
    // 攻塔更狠、飞行更快。（分裂走的是“代数”而非倍率，已在上面按级别设置。）
    let skill_mult = crate::monster::SkillTier::from_grade(species.grade()).power_mult();
    let regen = regen * skill_mult;
    let heal_aura = heal_aura * skill_mult;
    let tower_dps = tower_dps * skill_mult;
    let silence_aura = silence_aura * skill_mult;
    let shield = (shield * skill_mult).floor();
    // 硬化：护甲与魔抗按级别提升（普通不变，中级 ×1.5，高级 ×2）。
    let armor = armor * skill_mult;
    let magic_resist = magic_resist * skill_mult;
    // 飞行：高级飞行单位飞得更快（直线最短路线 + 加速，更难拦截）。
    let base_speed = if def.flying {
        base_speed * (1.0 + (skill_mult - 1.0) * 0.6)
    } else {
        base_speed
    };
    // 隐形：级别越高越难被探测——探测塔的有效射程要乘以这个折扣才能照出它。
    // 普通 1.0、中级 ~0.67、高级 0.5。冲锋的爆发速度、攻塔的索敌范围则在
    // update_enemies 里按 skill_mult 放大（见下）。
    let stealth = if def.invisible { 1.0 / skill_mult } else { 1.0 };
    let start = board.spawn_pos();
    let px = def.size * 4.5 * if is_elite { 1.45 } else { 1.0 };
    let bar_w = (px * 0.5).max(18.0);
    let bar_y = px * 0.5 + 2.0;
    let hp_color = if def.boss {
        Color::srgb(1.0, 0.28, 0.18)
    } else if is_elite {
        Color::srgb(1.0, 0.74, 0.24)
    } else {
        Color::srgb(0.25, 0.9, 0.35)
    };
    let image = sprites
        .species
        .get(&species.id)
        .or_else(|| sprites.enemies.get(&kind))
        .expect("monster species or enemy archetype sprite must be loaded")
        .clone();
    let mut sprite = Sprite::from_image(image);
    sprite.custom_size = Some(Vec2::splat(px));
    if is_elite {
        sprite.color = elite_affix_color(elite_affix);
    }
    let melee_mult = if is_elite { 1.5 } else { 1.0 }
        * match elite_affix {
            EliteAffix::Frenzy => 1.35,
            EliteAffix::Siege => 1.20,
            _ => 1.0,
        };

    commands
        .spawn((
            Enemy {
                kind,
                species_id: species.id,
                hp,
                max_hp: hp,
                base_speed,
                reward,
                path_index: 0,
                armor,
                magic_resist,
                element_resist: species.resist_profile(),
                flying: def.flying,
                invisible: def.invisible,
                skill_mult,
                stealth,
                regen,
                boss: def.boss,
                size: def.size,
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
                shield,
                max_shield: shield,
                splits,
                heal_aura,
                charger: def.charger,
                charge_timer: 0.0,
                hit_flash: 0.0,
                last_hit_tower: None,
                blocked: false,
                melee: (if def.boss {
                    40.0
                } else {
                    6.0 + def.hp_mod * 6.0
                }) * melee_mult
                    * endless_melee,
                elite: is_elite,
                elite_affix,
                boss_skill_timer: if def.boss {
                    boss_skill(species.id).cooldown() * 0.45
                } else {
                    0.0
                },
                enraged: false,
                phase_timer: 0.0,
                tower_raider,
                tower_dps,
                silence_aura,
                moss_destroy: def.moss_destroy,
                moss_destroyed: false,
                facing: Vec2::ZERO,
            },
            sprite,
            Transform::from_translation(start.extend(5.0)),
            LevelEntity,
        ))
        .with_children(|p| {
            // HP bar background (dark) + foreground (green, anchored left so it
            // shrinks toward the left as `update_hp_bars` scales its x).
            p.spawn((
                Sprite {
                    color: Color::srgb(0.08, 0.08, 0.08),
                    custom_size: Some(Vec2::new(bar_w, 4.0)),
                    ..default()
                },
                Transform::from_xyz(0.0, bar_y, 0.1),
            ));
            p.spawn((
                Sprite {
                    color: hp_color,
                    custom_size: Some(Vec2::new(bar_w, 4.0)),
                    ..default()
                },
                Anchor::CENTER_LEFT,
                Transform::from_xyz(-bar_w / 2.0, bar_y, 0.2),
                HpBarFg,
            ));
            p.spawn((
                Sprite {
                    color: Color::srgb(0.35, 0.75, 1.0),
                    custom_size: Some(Vec2::new(bar_w, 3.0)),
                    ..default()
                },
                Anchor::CENTER_LEFT,
                Transform::from_xyz(-bar_w / 2.0, bar_y + 4.0, 0.3),
                ShieldBarFg,
            ));
            if def.boss {
                let skill = boss_skill(species.id);
                let text = if skill == BossSkill::None {
                    crate::i18n::tf("首领·{}", &[&crate::i18n::t(species.name)])
                } else {
                    crate::i18n::tf(
                        "首领·{}\n{}",
                        &[&crate::i18n::t(species.name), &crate::i18n::t(skill.name())],
                    )
                };
                p.spawn((
                    Text2d::new(text),
                    TextFont {
                        font: FontSource::Handle(font.0.clone()),
                        font_size: FontSize::Px(11.0),
                        ..default()
                    },
                    TextColor(boss_skill_color(skill)),
                    Transform::from_xyz(0.0, bar_y + 14.0, 0.7),
                ));
            } else if is_elite {
                p.spawn((
                    Text2d::new(crate::i18n::t(elite_affix.name())),
                    TextFont {
                        font: FontSource::Handle(font.0.clone()),
                        font_size: FontSize::Px(12.0),
                        ..default()
                    },
                    TextColor(elite_affix_color(elite_affix)),
                    Transform::from_xyz(0.0, bar_y + 12.0, 0.6),
                ));
            } else if let Some((label, color)) =
                special_trait_badge(def, tower_raider, silence_aura, heal_aura, shield, splits)
            {
                p.spawn((
                    Sprite {
                        color: Color::srgba(0.02, 0.02, 0.04, 0.78),
                        custom_size: Some(Vec2::new(30.0, 12.0)),
                        ..default()
                    },
                    Transform::from_xyz(0.0, bar_y + 12.0, 0.55),
                ));
                p.spawn((
                    Text2d::new(crate::i18n::t(label)),
                    TextFont {
                        font: FontSource::Handle(font.0.clone()),
                        font_size: FontSize::Px(10.0),
                        ..default()
                    },
                    TextColor(color),
                    Transform::from_xyz(0.0, bar_y + 11.0, 0.65),
                ));
            }
        });
}

/// Keep each enemy's HP and shield bars scaled to their current fractions.
pub fn update_hp_bars(
    enemies: Query<(&Enemy, &Children)>,
    mut hp_bars: Query<&mut Transform, (With<HpBarFg>, Without<ShieldBarFg>)>,
    mut shield_bars: Query<&mut Transform, (With<ShieldBarFg>, Without<HpBarFg>)>,
) {
    for (e, children) in &enemies {
        let hp_frac = (e.hp / e.max_hp).clamp(0.0, 1.0);
        let shield_frac = if e.max_shield > 0.0 {
            (e.shield / e.max_shield).clamp(0.0, 1.0)
        } else {
            0.0
        };
        for child in children.iter() {
            if let Ok(mut tf) = hp_bars.get_mut(child) {
                tf.scale.x = hp_frac;
            }
            if let Ok(mut tf) = shield_bars.get_mut(child) {
                tf.scale.x = shield_frac;
            }
        }
    }
}

pub fn draw_silence_auras(mut gizmos: Gizmos, enemies: Query<(&Enemy, &Transform)>) {
    for (enemy, tf) in &enemies {
        if enemy.silence_aura <= 0.0 {
            continue;
        }
        let pos = tf.translation.truncate();
        let color = Color::srgb(0.72, 0.28, 1.0);
        gizmos.circle_2d(pos, enemy.silence_aura, color.with_alpha(0.55));
        gizmos.circle_2d(pos, enemy.silence_aura * 0.72, color.with_alpha(0.22));
    }
}

pub fn draw_heal_auras(mut gizmos: Gizmos, enemies: Query<(&Enemy, &Transform)>) {
    for (enemy, tf) in &enemies {
        if enemy.heal_aura <= 0.0 || enemy.hp <= 0.0 {
            continue;
        }
        let pos = tf.translation.truncate();
        let color = Color::srgb(0.35, 1.0, 0.45);
        gizmos.circle_2d(pos, HEAL_AURA_RADIUS, color.with_alpha(0.45));
        gizmos.circle_2d(pos, HEAL_AURA_RADIUS * 0.55, color.with_alpha(0.18));
    }
}

pub fn draw_boss_cast_telegraphs(
    mut gizmos: Gizmos,
    bosses: Query<(&Transform, &PendingBossCast)>,
    towers: Query<&crate::tower::Tower>,
) {
    for (tf, cast) in &bosses {
        let pos = tf.translation.truncate();
        let color = boss_skill_color(cast.skill);
        let progress = (1.0 - cast.timer / cast.max_timer).clamp(0.0, 1.0);
        let sweep = cast.radius * (0.2 + progress * 0.8);
        gizmos.circle_2d(pos, cast.radius, color.with_alpha(0.28 + progress * 0.28));
        gizmos.circle_2d(pos, sweep, color.with_alpha(0.75));
        if !boss_skill_threatens_towers(cast.skill) {
            continue;
        }
        for tower in &towers {
            if tower.hp <= 0.0 {
                continue;
            }
            let tower_pos = tower.center();
            if tower_pos.distance(pos) > cast.radius {
                continue;
            }
            let tower_radius = TILE_SIZE * (0.4 + tower.footprint as f32 * 0.28);
            gizmos.circle_2d(tower_pos, tower_radius, color.with_alpha(0.86));
            gizmos.line_2d(pos, tower_pos, color.with_alpha(0.24 + progress * 0.32));
        }
    }
}

pub fn draw_elite_auras(mut gizmos: Gizmos, enemies: Query<(&Enemy, &Transform)>) {
    for (enemy, tf) in &enemies {
        if !enemy.elite || enemy.hp <= 0.0 {
            continue;
        }
        let pos = tf.translation.truncate();
        let color = elite_affix_color(enemy.elite_affix);
        let radius = enemy.size * 4.5 * 0.72;
        gizmos.circle_2d(pos, radius, color.with_alpha(0.72));
        gizmos.circle_2d(pos, radius * 0.72, color.with_alpha(0.28));
    }
}

/// Drive spawning during a wave and detect wave completion.
pub fn spawn_enemies(
    mut commands: Commands,
    time: Res<Time>,
    mut run: ResMut<RunState>,
    board: Res<Board>,
    levels: Res<Levels>,
    current: Res<CurrentLevel>,
    mut rng: ResMut<Rng>,
    enemies: Query<(), With<Enemy>>,
    sprites: Res<Sprites>,
    font: Res<UiFont>,
    diff: Res<crate::game::GameDifficulty>,
    mut next: ResMut<NextState<GameState>>,
    mut vfx: MessageWriter<crate::vfx::VfxEvent>,
) {
    if !run.wave_in_progress {
        return;
    }
    let level = &levels.0[current.0];

    run.spawn_timer += time.delta_secs() * run.game_speed;
    if run.spawn_timer >= run.spawn_interval && run.spawned < run.spawn_target {
        let boss_wave = run.is_boss_wave_number(run.wave);
        let species = if boss_wave && run.spawned == run.spawn_target - 1 {
            if let Some(boss) = run
                .pending_boss_species
                .and_then(species_by_id)
                .filter(|s| s.is_boss())
            {
                boss
            } else {
                let boss = pick_boss(run.wave, run.boss_pick_total_waves(), current.0, &mut rng);
                run.pending_boss_species = Some(boss.id);
                boss
            }
        } else {
            pick_regular(run.wave, current.0, &mut rng)
        };
        // Elite chance grows with wave & difficulty (bosses are never "elite").
        let elite_cap = if run.is_endless() { 0.75 } else { 0.5 };
        let endless_elite_bonus = if run.is_endless() {
            run.wave as f32 * 0.003
        } else {
            0.0
        };
        let elite_chance = if run.wave >= 4 {
            ((0.04 + run.wave as f32 * 0.012 + endless_elite_bonus) * diff.0.elite_mult())
                .min(elite_cap)
        } else {
            0.0
        };
        let is_elite = !species.is_boss() && rng.frac() < elite_chance;
        let elite_affix = if is_elite {
            pick_elite_affix(run.wave, current.0, rng.frac())
        } else {
            EliteAffix::None
        };
        let first_encounter = run.encountered_species.insert(species.id);
        spawn_one(
            &mut commands,
            species,
            &board,
            level.enemies.hp,
            level.enemies.speed,
            level.enemies.reward,
            run.wave,
            current.0,
            &sprites,
            &font,
            diff.0,
            elite_affix,
            run.is_endless(),
        );
        // Dramatic boss entrance: heavy shockwave + shake + banner at the gate.
        if species.is_boss() {
            let entrance = board.path_world.first().copied().unwrap_or_default();
            vfx.write(crate::vfx::VfxEvent::BossEntrance {
                pos: entrance,
                color: boss_skill_color(boss_skill(species.id)),
                name: species.name.to_string(),
            });
        }
        if first_encounter {
            let rare =
                species.is_boss() || is_elite || species.min_level >= 10 || species.min_wave >= 10;
            let color = if species.is_boss() {
                boss_skill_color(boss_skill(species.id))
            } else if is_elite {
                elite_affix_color(elite_affix)
            } else if species.kind.def().silence_aura > 0.0 {
                Color::srgb(0.82, 0.45, 1.0)
            } else if species.kind.def().tower_raider || species.kind.def().moss_destroy {
                Color::srgb(0.95, 0.62, 0.18)
            } else {
                Color::srgb(0.58, 0.86, 1.0)
            };
            let label = if species.is_boss() {
                crate::i18n::tf("首领现身 {}", &[&crate::i18n::t(species.name)])
            } else if is_elite {
                crate::i18n::tf(
                    "新威胁 {}·{}",
                    &[&crate::i18n::t(elite_affix.name()), &crate::i18n::t(species.name)],
                )
            } else {
                crate::i18n::tf("新威胁 {}", &[&crate::i18n::t(species.name)])
            };
            run.show_for(
                crate::i18n::tf(
                    "侦测到新威胁：{}\n{}",
                    &[&crate::i18n::t(species.name), &species.traits()],
                ),
                if rare { 3.2 } else { 2.3 },
            );
            // Show the "首领现身/新威胁" announcement at the TOP-CENTER of the board
            // instead of at the spawn portal: on phones the portal is crowded with
            // towers + the hero, and the burst was covering them during boss waves.
            vfx.write(crate::vfx::VfxEvent::ThreatIntro {
                pos: Vec2::new(0.0, crate::data::BOARD_H * 0.40),
                species_id: species.id,
                label,
                color,
                rare,
            });
        }
        if elite_affix != EliteAffix::None {
            vfx.write(crate::vfx::VfxEvent::Text {
                pos: board.spawn_pos(),
                text: crate::i18n::tf("精英·{}", &[&crate::i18n::t(elite_affix.name())]),
                color: elite_affix_color(elite_affix),
                size: 14.0,
                life: 1.0,
            });
        }
        run.spawned += 1;
        run.spawn_timer = 0.0;
    }

    // Wave complete: all spawned and none alive.
    if run.spawned >= run.spawn_target && enemies.iter().count() == 0 {
        run.wave_in_progress = false;
        run.pending_boss_species = None;
        if !run.is_endless() && run.wave >= run.total_waves {
            next.set(GameState::Victory);
        } else {
            // Interest: earn 10% of current gold (capped) for surviving the wave.
            let interest_cap = if run.is_endless() {
                (60 + run.wave * 2).min(160)
            } else {
                60
            };
            let interest = ((run.gold as f32 * 0.10).floor() as i32).min(interest_cap);
            let perfect_bonus = if run.wave_perfect
                && run.wave_start_lives > 0
                && run.lives >= run.wave_start_lives
            {
                let cap = if run.is_endless() { 120 } else { 60 };
                (8 + run.wave * 2).min(cap)
            } else {
                0
            };
            run.gold += interest + perfect_bonus;
            if perfect_bonus > 0 {
                vfx.write(crate::vfx::VfxEvent::PerfectWave {
                    pos: board.carrot_pos(),
                    wave: run.wave,
                    gold: perfect_bonus,
                });
            }
            if run.auto_wave {
                run.auto_wave_timer = AUTO_WAVE_DELAY;
                if perfect_bonus > 0 {
                    run.show_for(
                        crate::i18n::tf(
                            "波次完成！利息 +{}，完美防守 +{} · 自动下一波 {}s",
                            &[
                                &interest.to_string(),
                                &perfect_bonus.to_string(),
                                &format!("{:.0}", AUTO_WAVE_DELAY),
                            ],
                        ),
                        2.4,
                    );
                } else {
                    run.show(crate::i18n::tf(
                        "波次完成！利息 +{} · 自动下一波 {}s",
                        &[&interest.to_string(), &format!("{:.0}", AUTO_WAVE_DELAY)],
                    ));
                }
            } else if perfect_bonus > 0 {
                run.show_for(
                    crate::i18n::tf(
                        "波次完成！利息 +{}，完美防守 +{}",
                        &[&interest.to_string(), &perfect_bonus.to_string()],
                    ),
                    2.4,
                );
            } else {
                run.show(crate::i18n::tf("波次完成！利息 +{}", &[&interest.to_string()]));
            }
            run.wave_perfect = false;
        }
    }
}

#[derive(Clone, Copy)]
struct BossCast {
    skill: BossSkill,
    pos: Vec2,
    path_index: usize,
    radius: f32,
}

fn species_name(species_id: usize) -> &'static str {
    MONSTER_SPECIES
        .iter()
        .find(|s| s.id == species_id)
        .map(|s| s.name)
        .unwrap_or("未知首领")
}

fn elite_affix_color(affix: EliteAffix) -> Color {
    match affix {
        EliteAffix::None => Color::WHITE,
        EliteAffix::Frenzy => Color::srgb(1.0, 0.55, 0.22),
        EliteAffix::Carapace => Color::srgb(0.95, 0.86, 0.45),
        EliteAffix::YellowSign => Color::srgb(1.0, 0.95, 0.20),
        EliteAffix::Brood => Color::srgb(0.68, 1.0, 0.42),
        EliteAffix::Bloodrite => Color::srgb(1.0, 0.32, 0.44),
        EliteAffix::Siege => Color::srgb(0.85, 0.55, 1.0),
    }
}

/// Lightweight procedural animation for static species portraits. Movement still
/// belongs to `Transform`; this only changes the sprite size/tint so pathing,
/// tower targeting, and HP bar children stay stable.
pub fn animate_enemy_sprites(
    time: Res<Time>,
    run: Res<RunState>,
    mut q: Query<(&Enemy, &mut Sprite)>,
) {
    let elapsed = time.elapsed_secs() * run.game_speed.max(0.25);
    for (enemy, mut sprite) in &mut q {
        let base = enemy.size * 4.5 * if enemy.elite { 1.45 } else { 1.0 };
        let status_slow = if enemy.frozen || enemy.stun_timer > 0.0 {
            0.25
        } else if enemy.blocked {
            0.55
        } else {
            1.0
        };
        let species_phase = enemy.species_id as f32 * 0.73 + enemy.kind as u8 as f32 * 0.19;
        let tempo = if enemy.boss && enemy.enraged {
            2.75
        } else if enemy.boss {
            2.0
        } else if enemy.flying {
            3.4
        } else if enemy.charger {
            3.0
        } else {
            2.35
        } * status_slow;
        let phase = elapsed * tempo + species_phase;
        let breath = phase.sin();
        let gait = (phase * 1.7).sin().abs();
        let boss_pulse = if enemy.boss && enemy.enraged {
            0.055 + 0.055 * (phase * 1.05).sin().max(0.0)
        } else if enemy.boss {
            0.035 + 0.035 * (phase * 0.85).sin().max(0.0)
        } else {
            0.0
        };
        let elite_pressure = if enemy.elite { 0.018 } else { 0.0 };
        let flying_lift = if enemy.flying { 0.055 * breath } else { 0.0 };
        let raider_sway = if enemy.tower_raider || enemy.moss_destroy {
            0.028 * (phase * 0.7).cos()
        } else {
            0.0
        };
        let width = base * (1.0 + boss_pulse + elite_pressure + 0.018 * gait + raider_sway);
        let height = base * (1.0 + boss_pulse + elite_pressure + flying_lift - 0.012 * breath);
        sprite.custom_size = Some(Vec2::new(width.max(6.0), height.max(6.0)));

        let mut color = if enemy.elite {
            elite_affix_color(enemy.elite_affix)
        } else {
            Color::WHITE
        };
        if enemy.boss {
            let pulse = ((phase * 0.75).sin() * 0.5 + 0.5).clamp(0.0, 1.0);
            color = color.mix(&Color::srgb(1.0, 0.28, 0.18), 0.12 + 0.08 * pulse);
            if enemy.enraged {
                let rage = ((phase * 1.5).sin() * 0.5 + 0.5).clamp(0.0, 1.0);
                color = color.mix(&Color::srgb(1.0, 0.06, 0.02), 0.22 + 0.16 * rage);
            }
        }
        if enemy.frozen || enemy.stun_timer > 0.0 {
            color = color.mix(&Color::srgb(0.62, 0.9, 1.0), 0.48);
        } else if enemy.fire_timer > 0.0 {
            color = color.mix(&enemy.fire_element.color(), 0.30);
        } else if enemy.poison_timer > 0.0 {
            color = color.mix(&Color::srgb(0.45, 1.0, 0.32), 0.28);
        } else if enemy.curse_timer > 0.0 {
            color = color.mix(&Color::srgb(0.75, 0.35, 1.0), 0.25);
        }

        let mut alpha = if enemy.invisible { 0.54 } else { 1.0 };
        if enemy.phase_timer > 0.0 {
            alpha = 0.34 + 0.22 * (phase * 2.1).sin().abs();
        }
        if enemy.hp < enemy.max_hp * 0.25 {
            alpha *= 0.88 + 0.12 * (phase * 5.0).sin().abs();
        }
        sprite.color = color.with_alpha(alpha.clamp(0.18, 1.0));
    }
}

fn boss_skill_color(skill: BossSkill) -> Color {
    match skill {
        BossSkill::None => Color::WHITE,
        BossSkill::SerpentRush => Color::srgb(0.3, 0.95, 0.35),
        BossSkill::AbyssalShield => Color::srgb(0.35, 0.75, 1.0),
        BossSkill::YellowSilence => Color::srgb(1.0, 0.88, 0.2),
        BossSkill::StormSurge => Color::srgb(0.45, 0.8, 1.0),
        BossSkill::FurnaceBurn => Color::srgb(1.0, 0.35, 0.1),
        BossSkill::BroodHeal => Color::srgb(0.55, 1.0, 0.45),
        BossSkill::VoidPhase => Color::srgb(0.65, 0.35, 1.0),
        BossSkill::StarforgedBulwark => Color::srgb(0.95, 0.82, 0.3),
        BossSkill::MossCrush => Color::srgb(0.18, 0.78, 0.35),
        BossSkill::DreamEclipse => Color::srgb(0.7, 0.15, 0.95),
    }
}

fn boss_skill_radius(skill: BossSkill) -> f32 {
    match skill {
        BossSkill::None => 0.0,
        BossSkill::SerpentRush => TILE_SIZE * 1.6,
        BossSkill::AbyssalShield => 150.0,
        BossSkill::YellowSilence => 185.0,
        BossSkill::StormSurge => 160.0,
        BossSkill::FurnaceBurn => 175.0,
        BossSkill::BroodHeal => 170.0,
        BossSkill::VoidPhase => 130.0,
        BossSkill::StarforgedBulwark => 175.0,
        BossSkill::MossCrush => 190.0,
        BossSkill::DreamEclipse => 250.0,
    }
}

fn kill_combo_bonus(combo: i32) -> i32 {
    if combo >= 5 && combo % 5 == 0 {
        (combo / 5).min(10) * 3
    } else {
        0
    }
}

fn boss_skill_threatens_towers(skill: BossSkill) -> bool {
    matches!(
        skill,
        BossSkill::SerpentRush
            | BossSkill::YellowSilence
            | BossSkill::StormSurge
            | BossSkill::FurnaceBurn
            | BossSkill::MossCrush
            | BossSkill::DreamEclipse
    )
}

fn boss_enrage_ready(enemy: &Enemy) -> bool {
    enemy.boss
        && !enemy.enraged
        && enemy.max_hp > 0.0
        && enemy.hp <= enemy.max_hp * BOSS_ENRAGE_HP_FRACTION
}

fn grant_shield(enemy: &mut Enemy, amount: f32, cap: f32) {
    enemy.max_shield = enemy.max_shield.max(cap);
    enemy.shield = (enemy.shield + amount).min(enemy.max_shield);
}

fn rush_forward(enemy: &mut Enemy, tf: &mut Transform, board: &Board, distance: f32) {
    let path = &board.path_world;
    let last = path.len().saturating_sub(1);
    let mut remaining = distance;
    while remaining > 0.0 && enemy.path_index < last {
        let target = path[enemy.path_index + 1];
        let pos = tf.translation.truncate();
        let delta = target - pos;
        let dist = delta.length();
        if dist <= 1.0 {
            enemy.path_index += 1;
            continue;
        }
        if dist <= remaining {
            tf.translation.x = target.x;
            tf.translation.y = target.y;
            enemy.path_index += 1;
            remaining -= dist;
        } else {
            let step = delta / dist * remaining;
            tf.translation.x += step.x;
            tf.translation.y += step.y;
            break;
        }
    }
}

fn damage_towers_in_radius(
    commands: &mut Commands,
    run: &mut RunState,
    inv: &mut EquipmentInventory,
    towers: &mut Query<(Entity, &mut crate::tower::Tower)>,
    vfx: &mut MessageWriter<crate::vfx::VfxEvent>,
    origin: Vec2,
    radius: f32,
    raw_damage: f32,
    cooldown_delay: f32,
    destroyed_msg: &'static str,
    color: Color,
) -> usize {
    let mut touched = 0;
    let mut destroyed = Vec::new();
    for (entity, mut tower) in towers.iter_mut() {
        if tower.hp <= 0.0 {
            continue;
        }
        let pos = tower.center();
        if pos.distance(origin) > radius {
            continue;
        }
        touched += 1;
        if cooldown_delay > 0.0 {
            tower.cooldown_timer = tower.cooldown_timer.max(cooldown_delay);
        }
        if raw_damage <= 0.0 {
            vfx.write(crate::vfx::VfxEvent::Text {
                pos: pos + Vec2::new(0.0, TILE_SIZE * 0.55),
                text: crate::i18n::t("停火"),
                color,
                size: 14.0,
                life: 0.75,
            });
            continue;
        }
        let set_bonus = equipment_set_bonus(&tower.equipment);
        let effective_armor = (tower.armor + set_bonus.armor_add).max(0.0);
        let actual = raw_damage * (100.0 / (100.0 + effective_armor));
        tower.hp -= actual;
        vfx.write(crate::vfx::VfxEvent::Hit {
            pos,
            color,
            element: crate::data::Element::Physical,
        });
        vfx.write(crate::vfx::VfxEvent::TaggedNumber {
            pos,
            amount: actual,
            color,
            label: "首领",
        });
        let hp_frac = if tower.max_hp > 0.0 {
            (tower.hp / tower.max_hp).clamp(0.0, 1.0)
        } else {
            0.0
        };
        if tower.hp > 0.0 && hp_frac <= 0.3 && !tower.low_hp_warned {
            tower.low_hp_warned = true;
            run.show_for(
                crate::i18n::tf(
                    "{}防御塔被首领重创，按 R 修理！",
                    &[&crate::i18n::t(tower.kind.def().name)],
                ),
                2.6,
            );
            vfx.write(crate::vfx::VfxEvent::Text {
                pos: pos + Vec2::new(0.0, TILE_SIZE * 0.78),
                text: crate::i18n::t("防御塔濒危"),
                color: Color::srgb(1.0, 0.22, 0.14),
                size: 15.0,
                life: 1.0,
            });
        }
        if tower.hp <= 0.0 {
            return_equipment_to_inventory(inv, &tower);
            destroyed.push((entity, pos, tower.element.color(), tower.footprint > 1));
        }
    }
    for (entity, pos, death_color, big) in destroyed {
        commands.entity(entity).despawn();
        run.show(crate::i18n::t(destroyed_msg));
        vfx.write(crate::vfx::VfxEvent::Death {
            pos,
            color: death_color,
            big,
        });
    }
    if touched > 0 {
        vfx.write(crate::vfx::VfxEvent::Explosion {
            pos: origin,
            radius,
            color,
        });
    }
    touched
}

/// Timed species-specific boss casts. MOSS keeps its first-tower destruction in
/// `tower::enemy_vs_tower`; this system adds repeatable boss pressure patterns.
pub fn boss_specials(
    time: Res<Time>,
    mut run: ResMut<RunState>,
    board: Res<Board>,
    creatures: Res<Creatures>,
    mut inv: ResMut<EquipmentInventory>,
    mut commands: Commands,
    mut enemies: Query<(
        Entity,
        &mut Enemy,
        &mut Transform,
        Option<&mut PendingBossCast>,
    )>,
    mut towers: Query<(Entity, &mut crate::tower::Tower)>,
    mut vfx: MessageWriter<crate::vfx::VfxEvent>,
) {
    if !run.wave_in_progress {
        return;
    }

    let dt = time.delta_secs() * run.game_speed;
    let mut casts = Vec::new();

    for (entity, mut enemy, mut tf, pending) in &mut enemies {
        if !enemy.boss || enemy.hp <= 0.0 {
            continue;
        }
        let skill = boss_skill(enemy.species_id);
        if skill == BossSkill::None {
            continue;
        }

        if boss_enrage_ready(&enemy) {
            enemy.enraged = true;
            enemy.slow_timer = 0.0;
            enemy.stun_timer = 0.0;
            enemy.frozen = false;
            let max_hp = enemy.max_hp;
            grant_shield(&mut enemy, max_hp * 0.08, max_hp * 0.50);
            enemy.boss_skill_timer = enemy.boss_skill_timer.max(skill.cooldown() * 0.72);
            let pos = tf.translation.truncate();
            let color = boss_skill_color(skill);
            run.show(crate::i18n::tf(
                "{}进入狂怒：技能频率提升",
                &[&crate::i18n::t(species_name(enemy.species_id))],
            ));
            vfx.write(crate::vfx::VfxEvent::Text {
                pos,
                text: crate::i18n::t("狂怒阶段"),
                color,
                size: 20.0,
                life: 1.0,
            });
            vfx.write(crate::vfx::VfxEvent::Explosion {
                pos,
                radius: boss_skill_radius(skill).max(TILE_SIZE * 2.2),
                color,
            });
        }

        if let Some(mut pending) = pending {
            pending.timer -= dt;
            if pending.timer > 0.0 {
                continue;
            }
            let skill = pending.skill;
            let radius = pending.radius;
            commands.entity(entity).remove::<PendingBossCast>();

            match skill {
                BossSkill::SerpentRush => {
                    enemy.slow_timer = 0.0;
                    enemy.stun_timer = 0.0;
                    enemy.frozen = false;
                    let max_hp = enemy.max_hp;
                    grant_shield(&mut enemy, max_hp * 0.08, max_hp * 0.35);
                    rush_forward(&mut enemy, &mut tf, &board, TILE_SIZE * 2.4);
                }
                BossSkill::StormSurge => {
                    enemy.slow_timer = 0.0;
                    enemy.stun_timer = 0.0;
                    enemy.frozen = false;
                    rush_forward(&mut enemy, &mut tf, &board, TILE_SIZE * 1.4);
                }
                BossSkill::VoidPhase => {
                    enemy.phase_timer = 3.0;
                    enemy.invisible = true;
                    let max_hp = enemy.max_hp;
                    grant_shield(&mut enemy, max_hp * 0.10, max_hp * 0.45);
                }
                BossSkill::StarforgedBulwark => {
                    enemy.armor_reduce = 0.0;
                    enemy.curse_timer = 0.0;
                    let max_hp = enemy.max_hp;
                    grant_shield(&mut enemy, max_hp * 0.16, max_hp * 0.60);
                }
                BossSkill::AbyssalShield
                | BossSkill::YellowSilence
                | BossSkill::FurnaceBurn
                | BossSkill::BroodHeal
                | BossSkill::MossCrush
                | BossSkill::DreamEclipse
                | BossSkill::None => {}
            }

            let pos = tf.translation.truncate();
            run.show(crate::i18n::tf(
                "{}释放：{}",
                &[
                    &crate::i18n::t(species_name(enemy.species_id)),
                    &crate::i18n::t(skill.name()),
                ],
            ));
            casts.push(BossCast {
                skill,
                pos,
                path_index: enemy.path_index,
                radius,
            });
            continue;
        }

        let skill_rate = if enemy.enraged {
            BOSS_ENRAGE_SKILL_RATE
        } else {
            1.0
        };
        enemy.boss_skill_timer += dt * skill_rate;
        if enemy.boss_skill_timer < skill.cooldown() {
            continue;
        }
        enemy.boss_skill_timer = 0.0;

        let pos = tf.translation.truncate();
        let radius = boss_skill_radius(skill);
        let color = boss_skill_color(skill);
        commands.entity(entity).insert(PendingBossCast {
            skill,
            timer: BOSS_CAST_WINDUP,
            max_timer: BOSS_CAST_WINDUP,
            radius,
        });
        run.show(crate::i18n::tf(
            "{}准备：{}",
            &[
                &crate::i18n::t(species_name(enemy.species_id)),
                &crate::i18n::t(skill.name()),
            ],
        ));
        vfx.write(crate::vfx::VfxEvent::BossCast {
            pos,
            radius,
            color,
            label: skill.name(),
        });
    }

    for cast in casts {
        let color = boss_skill_color(cast.skill);
        match cast.skill {
            BossSkill::SerpentRush => {
                damage_towers_in_radius(
                    &mut commands,
                    &mut run,
                    &mut inv,
                    &mut towers,
                    &mut vfx,
                    cast.pos,
                    cast.radius,
                    28.0,
                    0.6,
                    "蛇父撞碎了一座防御塔！",
                    color,
                );
            }
            BossSkill::AbyssalShield => {
                for (_, mut ally, tf, _) in &mut enemies {
                    if ally.hp <= 0.0 || tf.translation.truncate().distance(cast.pos) > cast.radius
                    {
                        continue;
                    }
                    let max_hp = ally.max_hp;
                    grant_shield(&mut ally, max_hp * 0.14, max_hp * 0.55);
                    ally.hp = (ally.hp + max_hp * 0.04).min(max_hp);
                }
                vfx.write(crate::vfx::VfxEvent::Explosion {
                    pos: cast.pos,
                    radius: cast.radius,
                    color,
                });
            }
            BossSkill::YellowSilence => {
                damage_towers_in_radius(
                    &mut commands,
                    &mut run,
                    &mut inv,
                    &mut towers,
                    &mut vfx,
                    cast.pos,
                    cast.radius,
                    0.0,
                    2.4,
                    "黄印让防御塔沉默崩塌！",
                    color,
                );
            }
            BossSkill::StormSurge => {
                damage_towers_in_radius(
                    &mut commands,
                    &mut run,
                    &mut inv,
                    &mut towers,
                    &mut vfx,
                    cast.pos,
                    cast.radius,
                    36.0,
                    1.2,
                    "雷暴撕裂了一座防御塔！",
                    color,
                );
            }
            BossSkill::FurnaceBurn => {
                damage_towers_in_radius(
                    &mut commands,
                    &mut run,
                    &mut inv,
                    &mut towers,
                    &mut vfx,
                    cast.pos,
                    cast.radius,
                    58.0,
                    0.9,
                    "赤星焚毁了一座防御塔！",
                    color,
                );
            }
            BossSkill::BroodHeal => {
                for (_, mut ally, tf, _) in &mut enemies {
                    if ally.hp <= 0.0 || tf.translation.truncate().distance(cast.pos) > cast.radius
                    {
                        continue;
                    }
                    let max_hp = ally.max_hp;
                    ally.hp = (ally.hp + max_hp * 0.12).min(max_hp);
                }
                let child_hp = (24.0 + run.wave as f32 * 5.0).min(140.0);
                for offset in [
                    Vec2::new(-18.0, -10.0),
                    Vec2::new(16.0, 8.0),
                    Vec2::new(0.0, 20.0),
                ] {
                    spawn_child(
                        &mut commands,
                        &creatures,
                        cast.pos + offset,
                        cast.path_index,
                        child_hp,
                    );
                }
                vfx.write(crate::vfx::VfxEvent::Explosion {
                    pos: cast.pos,
                    radius: cast.radius,
                    color,
                });
            }
            BossSkill::VoidPhase => {
                vfx.write(crate::vfx::VfxEvent::Explosion {
                    pos: cast.pos,
                    radius: cast.radius,
                    color,
                });
            }
            BossSkill::StarforgedBulwark => {
                for (_, mut ally, tf, _) in &mut enemies {
                    if ally.hp <= 0.0 || tf.translation.truncate().distance(cast.pos) > cast.radius
                    {
                        continue;
                    }
                    ally.armor_reduce = 0.0;
                    ally.curse_timer = 0.0;
                    let max_hp = ally.max_hp;
                    grant_shield(&mut ally, max_hp * 0.18, max_hp * 0.70);
                }
                vfx.write(crate::vfx::VfxEvent::Explosion {
                    pos: cast.pos,
                    radius: cast.radius,
                    color,
                });
            }
            BossSkill::MossCrush => {
                damage_towers_in_radius(
                    &mut commands,
                    &mut run,
                    &mut inv,
                    &mut towers,
                    &mut vfx,
                    cast.pos,
                    cast.radius,
                    44.0,
                    1.7,
                    "MOSS的菌毯压垮了一座防御塔！",
                    color,
                );
            }
            BossSkill::DreamEclipse => {
                damage_towers_in_radius(
                    &mut commands,
                    &mut run,
                    &mut inv,
                    &mut towers,
                    &mut vfx,
                    cast.pos,
                    cast.radius,
                    32.0,
                    3.0,
                    "梦蚀吞没了一座防御塔！",
                    color,
                );
                for (_, mut ally, tf, _) in &mut enemies {
                    if ally.hp <= 0.0
                        || tf.translation.truncate().distance(cast.pos) > cast.radius * 0.88
                    {
                        continue;
                    }
                    let max_hp = ally.max_hp;
                    grant_shield(&mut ally, max_hp * 0.10, max_hp * 0.50);
                }
                let child_hp = (36.0 + run.wave as f32 * 6.0).min(180.0);
                for offset in [Vec2::new(-24.0, 0.0), Vec2::new(24.0, 0.0)] {
                    spawn_child(
                        &mut commands,
                        &creatures,
                        cast.pos + offset,
                        cast.path_index,
                        child_hp,
                    );
                }
            }
            BossSkill::None => {}
        }
    }
}

/// Move enemies along the path, apply status effects, handle death and arrival.
pub fn update_enemies(
    mut commands: Commands,
    time: Res<Time>,
    mut run: ResMut<RunState>,
    board: Res<Board>,
    creatures: Res<Creatures>,
    mut rng: ResMut<Rng>,
    mut inv: ResMut<EquipmentInventory>,
    mut bestiary: ResMut<crate::bestiary::Bestiary>,
    mut hero_loadout: ResMut<crate::hero::HeroLoadout>,
    mut towers: Query<&mut crate::tower::Tower>,
    mut q: Query<(Entity, &mut Enemy, &mut Transform)>,
    mut next: ResMut<NextState<GameState>>,
    mut vfx: MessageWriter<crate::vfx::VfxEvent>,
    mut died: MessageWriter<crate::tower::EnemyDied>,
) {
    let dt = time.delta_secs() * run.game_speed;
    if run.kill_combo_timer > 0.0 {
        run.kill_combo_timer -= dt;
        if run.kill_combo_timer <= 0.0 {
            run.kill_combo = 0;
            run.kill_combo_timer = 0.0;
            run.kill_combo_window = 0.0;
        }
    }
    let path = &board.path_world;
    let last = path.len().saturating_sub(1);
    let tower_positions: Vec<Vec2> = towers.iter().map(|t| t.center()).collect();
    // Aggro (仇恨): enemies within this range of the hero break off the path and
    // chase it instead, then brawl once adjacent (see `enemy_vs_ally`).
    let hero_aggro_pos: Option<Vec2> = towers.iter().find(|t| t.hero).map(|t| t.center());
    const HERO_AGGRO_RANGE: f32 = TILE_SIZE * 2.6;

    for (entity, mut e, mut tf) in &mut q {
        // Damage-over-time (with sparse colored sparks so the drain is visible).
        let pos = tf.translation.truncate();
        if e.poison_timer > 0.0 {
            e.poison_timer -= dt;
            let mult = (1.0 - e.element_resist.get(Element::Toxic)).clamp(0.25, 1.75);
            let damage = e.poison_damage * mult * dt;
            let dealt = damage.min(e.hp.max(0.0));
            e.hp -= damage;
            if dealt > 0.0 {
                e.last_hit_tower = e.poison_source_tower;
                if let Some(source_tower) = e.poison_source_tower {
                    if let Ok(mut tower) = towers.get_mut(source_tower) {
                        tower.damage_done += dealt;
                    }
                }
            }
            if rng.frac() < 0.2 {
                vfx.write(crate::vfx::VfxEvent::Hit {
                    pos,
                    color: Color::srgb(0.4, 0.9, 0.3),
                    element: crate::data::Element::Toxic,
                });
            }
        }
        if e.fire_timer > 0.0 {
            e.fire_timer -= dt;
            let mult = (1.0 - e.element_resist.get(e.fire_element)).clamp(0.25, 1.75);
            let damage = e.fire_damage * mult * dt;
            let dealt = damage.min(e.hp.max(0.0));
            e.hp -= damage;
            if dealt > 0.0 {
                e.last_hit_tower = e.fire_source_tower;
                if let Some(source_tower) = e.fire_source_tower {
                    if let Ok(mut tower) = towers.get_mut(source_tower) {
                        tower.damage_done += dealt;
                    }
                }
            }
            if rng.frac() < 0.2 {
                vfx.write(crate::vfx::VfxEvent::Hit {
                    pos,
                    color: e.fire_element.color(),
                    element: e.fire_element,
                });
            }
        }
        // Regeneration.
        if e.regen > 0.0 && e.hp > 0.0 && e.hp < e.max_hp {
            // JS: hp += maxHp*regen*dt_ms/1000  ==  maxHp*regen*seconds (dt is seconds).
            e.hp = (e.hp + e.max_hp * e.regen * dt).min(e.max_hp);
        }
        // Curse expiry: armor_reduce is applied as an *effective* modifier in
        // `apply_damage`, so we just clear it here (no base-stat mutation).
        if e.curse_timer > 0.0 {
            e.curse_timer -= dt;
            if e.curse_timer <= 0.0 {
                e.armor_reduce = 0.0;
            }
        }
        // Stun / freeze expiry.
        if e.stun_timer > 0.0 {
            e.stun_timer -= dt;
            if e.stun_timer <= 0.0 {
                e.frozen = false;
            }
        }
        if e.slow_timer > 0.0 {
            e.slow_timer -= dt;
        }
        if e.phase_timer > 0.0 {
            e.phase_timer -= dt;
            if e.phase_timer <= 0.0 {
                e.invisible = e.kind.def().invisible;
            }
        }

        // Chargers periodically burst forward (1s burst every 4s). 冲锋按级别加强：
        // 普通 ×2、中级 ×2.5、高级 ×3（爆发倍率 = 1 + 技能倍率）。
        let mut speed = e.current_speed();
        if e.charger {
            e.charge_timer += dt;
            if e.charge_timer % 4.0 < 1.0 {
                speed *= 1.0 + e.skill_mult;
            }
        }
        if !e.frozen && !e.blocked && e.path_index < last {
            let pos = tf.translation.truncate();
            // Flying units ignore the winding ground path and beeline straight to
            // the carrot (the final path point) — the shortest possible route.
            let target = if e.flying {
                path[last]
            } else {
                path[e.path_index + 1]
            };
            let raid_target = if e.tower_raider || e.moss_destroy {
                // 吞塔按级别加强：除了 tower_dps 随级别提升外，索敌半径也按技能倍率
                // 放大（普通 ×1、中级 ×1.1、高级 ×1.2），高级吞塔更早脱离路线扑塔。
                let sense = if e.moss_destroy {
                    MOSS_TOWER_SENSE
                } else {
                    TOWER_RAIDER_SENSE * (0.8 + 0.2 * e.skill_mult)
                };
                tower_positions
                    .iter()
                    .filter_map(|tpos| {
                        let dist = pos.distance(*tpos);
                        (dist <= sense && dist > TILE_SIZE * 0.75).then_some((*tpos, dist))
                    })
                    .min_by(|a, b| a.1.total_cmp(&b.1))
                    .map(|(tpos, _)| tpos)
            } else {
                None
            };
            // Hero aggro takes top priority, then tower-raiding, then the path.
            let aggro = hero_aggro_pos.filter(|hp| pos.distance(*hp) <= HERO_AGGRO_RANGE);
            let goal = aggro.or(raid_target).unwrap_or(target);
            let off_path = aggro.is_some() || raid_target.is_some();
            let delta = goal - pos;
            let dist = delta.length();
            if !off_path && dist < 5.0 {
                // Ground units advance one waypoint; a flyer that reaches the carrot
                // point (its straight-line goal) is done.
                if e.flying {
                    e.path_index = last;
                } else {
                    e.path_index += 1;
                }
            } else {
                let mult = if off_path { 0.9 } else { 1.0 };
                let step_len = (speed * mult * dt).min(dist);
                if dist > 0.5 {
                    let unit = delta / dist;
                    e.facing = unit; // for assassin backstab detection
                    let step = unit * step_len;
                    tf.translation.x += step.x;
                    tf.translation.y += step.y;
                }
            }
        }

        // Reached the carrot.
        if e.path_index >= last {
            run.kill_combo = 0;
            run.kill_combo_timer = 0.0;
            run.kill_combo_window = 0.0;
            run.wave_perfect = false;
            run.lives -= 1;
            let lives_left = run.lives.max(0);
            let max_lives = run.start_lives.max(1);
            let carrot_pos = board.carrot_pos();
            vfx.write(crate::vfx::VfxEvent::CarrotHit {
                pos: carrot_pos,
                lives: lives_left,
                max_lives,
            });
            if lives_left > 0 {
                run.show_for(
                    crate::i18n::tf(
                        "{}突破封印！剩余生命 {}",
                        &[
                            &crate::i18n::t(species_name(e.species_id)),
                            &lives_left.to_string(),
                        ],
                    ),
                    if lives_left * 3 <= max_lives {
                        3.0
                    } else {
                        2.0
                    },
                );
            }
            commands.entity(entity).despawn();
            if run.lives <= 0 {
                next.set(GameState::GameOver);
            }
            continue;
        }

        // Death.
        if e.hp <= 0.0 {
            // Hero doctrine bounty (赏金猎手/影袭赏金) adds a gold fraction per kill.
            let bounty = (e.reward as f32 * run.hero_gold_bonus).round() as i32;
            run.gold += e.reward + bounty;
            run.kills += 1;
            if let Some(source_tower) = e.last_hit_tower {
                if let Ok(mut tower) = towers.get_mut(source_tower) {
                    tower.kills += 1;
                    if tower.hero {
                        let xp = if e.boss {
                            120
                        } else if e.elite {
                            52
                        } else {
                            18
                        };
                        let gained = hero_loadout.gain_xp(xp);
                        if gained > 0 {
                            crate::hero::apply_loadout_to_tower(&hero_loadout, &mut tower);
                            run.show(crate::i18n::tf(
                                "英雄升级至 Lv{}，获得 {} 点天赋点",
                                &[&hero_loadout.level.to_string(), &gained.to_string()],
                            ));
                            vfx.write(crate::vfx::VfxEvent::Burst {
                                pos: tower.center(),
                                radius: 74.0,
                                color: hero_loadout.class.skill_color(),
                            });
                        } else if xp >= 50 {
                            vfx.write(crate::vfx::VfxEvent::Text {
                                pos: tower.center() + Vec2::new(0.0, 24.0),
                                text: crate::i18n::tf("英雄经验 +{}", &[&xp.to_string()]),
                                color: hero_loadout.class.skill_color(),
                                size: 14.0,
                                life: 0.85,
                            });
                        }
                    }
                }
            }
            run.kill_combo = if run.kill_combo_timer > 0.0 {
                run.kill_combo + 1
            } else {
                1
            };
            let combo_window = if e.boss {
                KILL_COMBO_WINDOW * 1.8
            } else {
                KILL_COMBO_WINDOW
            };
            run.kill_combo_timer = combo_window;
            run.kill_combo_window = combo_window;
            run.best_combo = run.best_combo.max(run.kill_combo);
            let first_seen = bestiary.record(e.species_id);
            let dpos = tf.translation.truncate();
            vfx.write(crate::vfx::VfxEvent::Death {
                pos: dpos,
                color: e.kind.def().color,
                big: e.boss,
            });
            // Floating "+gold" bounty so kills feel rewarding.
            if e.reward + bounty > 0 {
                vfx.write(crate::vfx::VfxEvent::TaggedNumber {
                    pos: dpos + Vec2::new(0.0, 10.0),
                    amount: (e.reward + bounty) as f32,
                    color: if bounty > 0 {
                        Color::srgb(1.0, 0.95, 0.5)
                    } else {
                        Color::srgb(1.0, 0.86, 0.3)
                    },
                    label: "金",
                });
            }
            let combo = run.kill_combo;
            let combo_bonus = kill_combo_bonus(combo);
            if combo_bonus > 0 {
                run.gold += combo_bonus;
                run.show_for(
                    crate::i18n::tf(
                        "连杀 x{} +{} 金",
                        &[&combo.to_string(), &combo_bonus.to_string()],
                    ),
                    1.25,
                );
                vfx.write(crate::vfx::VfxEvent::ComboReward {
                    pos: dpos,
                    combo,
                    gold: combo_bonus,
                });
            } else if combo == 3 {
                vfx.write(crate::vfx::VfxEvent::Text {
                    pos: dpos + Vec2::new(0.0, 24.0),
                    text: crate::i18n::t("连杀 x3"),
                    color: Color::srgb(1.0, 0.78, 0.32),
                    size: 15.0,
                    life: 0.8,
                });
            }
            if first_seen {
                let species = species_by_id(e.species_id);
                let name = species.map(|species| species.name).unwrap_or("未知物种");
                vfx.write(crate::vfx::VfxEvent::Discovery {
                    pos: dpos,
                    species_id: e.species_id,
                    label: crate::i18n::tf("图鉴更新 {}", &[&crate::i18n::t(name)]),
                    color: Color::srgb(0.78, 0.58, 1.0),
                    rare: species.map(|species| species.is_boss()).unwrap_or(e.boss),
                });
            }
            // Notify necromancer towers (they may raise this corpse as an ally).
            died.write(crate::tower::EnemyDied {
                pos: dpos,
                kind: e.kind,
                max_hp: e.max_hp,
            });
            // Equipment drop: bosses always, elites often, normal enemies sometimes.
            if let Some(item) = roll_drop(&mut rng, e.boss, e.elite, run.wave) {
                let def = item.def();
                inv.add(item);
                run.show(crate::i18n::tf(
                    "掉落 {}装备：{}！",
                    &[&def.rarity.label(), &crate::i18n::t(def.name)],
                ));
                vfx.write(crate::vfx::VfxEvent::Loot {
                    pos: dpos,
                    item,
                    label: crate::i18n::tf(
                        "+{} {}",
                        &[&def.rarity.label(), &crate::i18n::t(def.name)],
                    ),
                    color: def.rarity.color(),
                    rare: matches!(
                        def.rarity,
                        Rarity::Epic | Rarity::Legendary | Rarity::Mythic
                    ),
                });
            }
            // Tiered split: on death, a splitter with generations remaining splits
            // into two smaller copies of itself, each halved in size & stats and
            // able to split one fewer generation. 普通 splits once, 中级 twice,
            // 高级 four times — a cascading shower of ever-tinier splinters.
            if e.splits > 0 {
                let pos = tf.translation.truncate();
                for _ in 0..2 {
                    spawn_splinter(&mut commands, &creatures, &*e, pos);
                }
            }
            commands.entity(entity).despawn();
        }
    }
}

/// Spawn one splinter from a dying splitter: a half-scale clone of the parent
/// that inherits its art, species and path progress, with size and stats halved
/// and one fewer split generation left. If it dies with generations remaining it
/// splits again — producing the cascading 普通/中级/高级 tier effect.
fn spawn_splinter(commands: &mut Commands, creatures: &Creatures, parent: &Enemy, pos: Vec2) {
    let size = (parent.size * 0.5).max(6.0);
    let hp = (parent.max_hp * 0.5).max(6.0);
    let (sprite, anim) = creatures.sprite(parent.kind, size * 4.5);
    commands.spawn((
        Enemy {
            kind: parent.kind,
            species_id: parent.species_id,
            hp,
            max_hp: hp,
            base_speed: parent.base_speed,
            reward: (parent.reward / 2).max(1),
            path_index: parent.path_index,
            armor: parent.armor * 0.5,
            magic_resist: parent.magic_resist * 0.5,
            element_resist: parent.element_resist,
            flying: parent.flying,
            invisible: false,
            skill_mult: parent.skill_mult,
            stealth: 1.0,
            regen: 0.0,
            boss: false,
            size,
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
            splits: parent.splits - 1,
            heal_aura: 0.0,
            charger: false,
            charge_timer: 0.0,
            hit_flash: 0.0,
            last_hit_tower: None,
            blocked: false,
            melee: parent.melee * 0.5,
            elite: false,
            elite_affix: EliteAffix::None,
            boss_skill_timer: 0.0,
            enraged: false,
            phase_timer: 0.0,
            tower_raider: false,
            tower_dps: 0.0,
            silence_aura: 0.0,
            moss_destroy: false,
            moss_destroyed: false,
            facing: Vec2::ZERO,
        },
        sprite,
        anim,
        Transform::from_translation(pos.extend(5.0)),
        LevelEntity,
    ));
}

/// Spawn a small swarmling add (boss summon skills like BroodHeal). Uses the
/// swarmer art at a fixed hp, inherits path progress, and never splits.
fn spawn_child(
    commands: &mut Commands,
    creatures: &Creatures,
    pos: Vec2,
    path_index: usize,
    hp: f32,
) {
    let def = EnemyKind::Swarmer.def();
    let size = def.size * 0.9;
    let (sprite, anim) = creatures.sprite(EnemyKind::Swarmer, size * 4.5);
    commands.spawn((
        Enemy {
            kind: EnemyKind::Swarmer,
            species_id: default_species_id(EnemyKind::Swarmer),
            hp,
            max_hp: hp,
            base_speed: 1.4 * def.speed_mod * TILE_SIZE / 60.0 * (1000.0 / 16.0),
            reward: 2,
            path_index,
            armor: 0.0,
            magic_resist: 0.0,
            element_resist: def.resist,
            flying: false,
            invisible: false,
            skill_mult: 1.0,
            stealth: 1.0,
            regen: 0.0,
            boss: false,
            size,
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
            melee: 4.0,
            elite: false,
            elite_affix: EliteAffix::None,
            boss_skill_timer: 0.0,
            enraged: false,
            phase_timer: 0.0,
            tower_raider: false,
            tower_dps: 0.0,
            silence_aura: 0.0,
            moss_destroy: false,
            moss_destroyed: false,
            facing: Vec2::ZERO,
        },
        sprite,
        anim,
        Transform::from_translation(pos.extend(5.0)),
        LevelEntity,
    ));
}

/// Healer enemies restore HP to nearby allies each frame.
pub fn heal_auras(
    time: Res<Time>,
    run: Res<RunState>,
    mut rng: ResMut<Rng>,
    mut enemies: Query<(&mut Enemy, &Transform)>,
    mut vfx: MessageWriter<crate::vfx::VfxEvent>,
) {
    let dt = time.delta_secs() * run.game_speed;
    // Collect (pos, heal/sec) of healers, then apply to allies in range.
    let healers: Vec<(Vec2, f32)> = enemies
        .iter()
        .filter(|(e, _)| e.heal_aura > 0.0 && e.hp > 0.0)
        .map(|(e, tf)| (tf.translation.truncate(), e.heal_aura))
        .collect();
    if healers.is_empty() {
        return;
    }
    for (mut e, tf) in &mut enemies {
        if e.hp <= 0.0 || e.hp >= e.max_hp {
            continue;
        }
        let pos = tf.translation.truncate();
        let mut heal = 0.0;
        for (hpos, amt) in &healers {
            if hpos.distance(pos) <= HEAL_AURA_RADIUS {
                heal += amt;
            }
        }
        if heal > 0.0 {
            let before = e.hp;
            e.hp = (e.hp + heal * dt).min(e.max_hp);
            if e.hp > before && rng.frac() < (0.07 * run.game_speed).min(0.25) {
                vfx.write(crate::vfx::VfxEvent::Heal {
                    pos: pos + Vec2::new(0.0, 8.0),
                });
            }
        }
    }
}
