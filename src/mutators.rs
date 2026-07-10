//! 章节战场机制（Episode Mutators）。
//!
//! 第 2-5 章各有一个专属战场规则，让百关战役玩起来各有策略，而不是
//! 单纯的数值放大：
//! - 第 2 章 余烬沙海 · 沙暴波：每第 3 波敌人乘沙暴加速（+30% 移速），
//!   考验减速/控制布防。
//! - 第 3 章 霜星冰原 · 星陨：周期性星辰坠落在行进路径上，先出现预警圈，
//!   随后冻结并炸伤范围内敌人——天降助力，但位置随机。
//! - 第 4 章 血潮深渊 · 血潮波：每第 4 波敌人自带血护盾（最大生命 25%），
//!   考验持续输出与破盾优先级。
//! - 第 5 章 虚空终章 · 虚空裂隙：路径中段悬着一道裂隙，敌人穿过时获得
//!   1.5 秒相位（不可锁定），塔阵要避开裂隙下游布置。
//!
//! 所有效果都作用于敌方侧（速度/护盾/冰冻/相位复用现有状态字段），不
//! 触碰塔的数值管线；机制在关卡载入与波次开始时通过跑马灯播报。

use bevy::prelude::*;

use crate::board::Board;
use crate::components::{Enemy, LevelEntity};
use crate::data::{episode_of, TILE_SIZE};
use crate::game::{CurrentLevel, Rng, RunState};

/// 星陨间隔（秒，游戏时间）。
const STARFALL_INTERVAL: f32 = 38.0;
/// 星陨预警时长（秒）——先画圈再落，给玩家可读性。
const STARFALL_WARNING: f32 = 1.2;
/// 星陨冻结时长与伤害（伤害按敌人最大生命的百分比，跨章节自动缩放）。
const STARFALL_FREEZE: f32 = 2.4;
const STARFALL_DAMAGE_FRAC: f32 = 0.22;
const STARFALL_RADIUS: f32 = TILE_SIZE * 2.0;
/// 裂隙相位时长（秒）。
const RIFT_PHASE: f32 = 1.5;
const RIFT_RADIUS: f32 = TILE_SIZE * 0.85;

/// 每章的战场机制描述（0 = 无）。用于简报与播报。
pub fn mechanic_name(episode: usize) -> Option<&'static str> {
    match episode {
        1 => Some("沙暴波"),
        2 => Some("星陨"),
        3 => Some("血潮波"),
        4 => Some("虚空裂隙"),
        _ => None,
    }
}

pub fn mechanic_desc(episode: usize) -> Option<&'static str> {
    match episode {
        1 => Some("每第3波沙暴来袭：敌人移速+30%，备好减速与控制"),
        2 => Some("星辰周期性坠落路径：预警圈后冻结并炸伤敌人"),
        3 => Some("每第4波血潮：敌人自带血护盾（最大生命25%）"),
        4 => Some("路径中段悬着虚空裂隙：敌人穿过获得1.5秒相位隐身"),
        _ => None,
    }
}

/// 沙暴波：第 2 章每第 3 波。
pub fn is_sandstorm_wave(level_index: usize, wave: i32) -> bool {
    episode_of(level_index) == 1 && wave > 0 && wave % 3 == 0
}

/// 血潮波：第 4 章每第 4 波。
pub fn is_bloodtide_wave(level_index: usize, wave: i32) -> bool {
    episode_of(level_index) == 3 && wave > 0 && wave % 4 == 0
}

/// 出怪时的波次修正：(移速倍率, 附加护盾占最大生命比例)。
pub fn wave_spawn_mods(level_index: usize, wave: i32) -> (f32, f32) {
    let speed = if is_sandstorm_wave(level_index, wave) {
        1.30
    } else {
        1.0
    };
    let shield = if is_bloodtide_wave(level_index, wave) {
        0.25
    } else {
        0.0
    };
    (speed, shield)
}

/// 机制运行态（每次载入关卡重置）。
#[derive(Resource, Default)]
pub struct MutatorState {
    pub starfall_timer: f32,
    pub rift_pos: Option<Vec2>,
}

/// 正在预警/落下的星陨。
#[derive(Component)]
pub struct StarfallStrike {
    pub pos: Vec2,
    pub countdown: f32,
}

/// 已被裂隙相位过的敌人（防止反复触发）。
#[derive(Component)]
pub struct RiftTouched;

/// OnEnter(Playing)：重置机制状态；第 5 章在路径中段放置裂隙。
pub fn setup_mutators(
    mut state: ResMut<MutatorState>,
    current: Res<CurrentLevel>,
    board: Res<Board>,
) {
    state.starfall_timer = STARFALL_INTERVAL * 0.6;
    state.rift_pos = None;
    if episode_of(current.0) == 4 && board.path_world.len() >= 4 {
        let idx = (board.path_world.len() as f32 * 0.55) as usize;
        state.rift_pos = board.path_world.get(idx).copied();
    }
}

/// 星陨（第 3 章）：计时→预警圈→冻结+按最大生命百分比炸伤范围内敌人。
pub fn starfall(
    mut commands: Commands,
    time: Res<Time>,
    run: Res<RunState>,
    current: Res<CurrentLevel>,
    board: Res<Board>,
    mut state: ResMut<MutatorState>,
    mut rng: ResMut<Rng>,
    mut strikes: Query<(Entity, &mut StarfallStrike)>,
    mut enemies: Query<(&mut Enemy, &Transform)>,
    mut vfx: MessageWriter<crate::vfx::VfxEvent>,
    mut sfx: MessageWriter<crate::audio::SfxEvent>,
) {
    if episode_of(current.0) != 2 {
        return;
    }
    let dt = time.delta_secs() * run.game_speed;
    if dt <= 0.0 {
        return;
    }

    // 已发射的星陨倒计时 → 落地结算。
    for (entity, mut strike) in &mut strikes {
        strike.countdown -= dt;
        if strike.countdown > 0.0 {
            continue;
        }
        for (mut e, tf) in &mut enemies {
            let pos = tf.translation.truncate();
            if pos.distance(strike.pos) <= STARFALL_RADIUS && !e.boss {
                e.frozen = true;
                e.stun_timer = e.stun_timer.max(STARFALL_FREEZE);
                e.hp -= (e.max_hp * STARFALL_DAMAGE_FRAC).max(1.0);
                e.hit_flash = 0.18;
            }
        }
        vfx.write(crate::vfx::VfxEvent::ElementPulse {
            pos: strike.pos,
            color: Color::srgb(0.68, 0.86, 1.0),
            strong: true,
        });
        // 星陨落地：陨石轰击 + 冰封声。
        sfx.write(crate::audio::SfxEvent(crate::audio::Sound::Meteor));
        sfx.write(crate::audio::SfxEvent(crate::audio::Sound::Freeze));
        commands.entity(entity).despawn();
    }

    // 仅波次进行中积累星陨计时（备战期不落）。
    if !run.wave_in_progress {
        return;
    }
    state.starfall_timer -= dt;
    if state.starfall_timer <= 0.0 {
        state.starfall_timer = STARFALL_INTERVAL;
        if board.path_world.len() > 2 {
            let idx = 1 + (rng.frac() * (board.path_world.len() - 2) as f32) as usize;
            let pos = board.path_world[idx.min(board.path_world.len() - 1)];
            commands.spawn((
                StarfallStrike {
                    pos,
                    countdown: STARFALL_WARNING,
                },
                LevelEntity,
            ));
        }
    }
}

/// 虚空裂隙（第 5 章）：敌人穿过裂隙获得短暂相位（复用 phase_timer/invisible）。
pub fn void_rift(
    mut commands: Commands,
    time: Res<Time>,
    current: Res<CurrentLevel>,
    state: Res<MutatorState>,
    mut enemies: Query<(Entity, &mut Enemy, &Transform), Without<RiftTouched>>,
    mut sfx: MessageWriter<crate::audio::SfxEvent>,
    mut sfx_cooldown: Local<f32>,
) {
    if episode_of(current.0) != 4 {
        return;
    }
    let Some(rift) = state.rift_pos else {
        return;
    };
    *sfx_cooldown = (*sfx_cooldown - time.delta_secs()).max(0.0);
    for (entity, mut e, tf) in &mut enemies {
        if e.boss || e.phase_timer > 0.0 {
            continue;
        }
        if tf.translation.truncate().distance(rift) <= RIFT_RADIUS {
            e.phase_timer = RIFT_PHASE;
            e.invisible = true;
            commands.entity(entity).insert(RiftTouched);
            // 相位吞没声（0.5s 节流，防止整波齐过时炸耳）。
            if *sfx_cooldown <= 0.0 {
                *sfx_cooldown = 0.5;
                sfx.write(crate::audio::SfxEvent(crate::audio::Sound::Curse));
            }
        }
    }
}

/// 机制视觉：星陨预警圈与虚空裂隙的脉动标记。
pub fn draw_mutators(
    time: Res<Time>,
    current: Res<CurrentLevel>,
    state: Res<MutatorState>,
    strikes: Query<&StarfallStrike>,
    mut gizmos: Gizmos,
) {
    let t = time.elapsed_secs();
    if episode_of(current.0) == 2 {
        for strike in &strikes {
            // 收缩的预警圈：越接近落点越小越亮。
            let frac = (strike.countdown / STARFALL_WARNING).clamp(0.0, 1.0);
            let r = STARFALL_RADIUS * (0.55 + 0.45 * frac);
            gizmos.circle_2d(strike.pos, r, Color::srgba(0.70, 0.88, 1.0, 0.85));
            gizmos.circle_2d(strike.pos, r * 0.4, Color::srgba(1.0, 1.0, 1.0, 0.5));
        }
    }
    if let Some(rift) = state.rift_pos {
        let pulse = 1.0 + (t * 2.6).sin() * 0.12;
        gizmos.circle_2d(rift, RIFT_RADIUS * pulse, Color::srgba(0.72, 0.5, 1.0, 0.8));
        gizmos.circle_2d(
            rift,
            RIFT_RADIUS * 0.55 * pulse,
            Color::srgba(0.9, 0.78, 1.0, 0.55),
        );
    }
}
