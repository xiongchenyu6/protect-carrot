//! Visual juice: hit sparks, floating damage numbers, explosion shockwaves, and
//! the enemy hit-pop. Gameplay systems emit `VfxEvent`s; `spawn_vfx` turns them
//! into short-lived entities that the update systems animate and despawn.

use crate::components::{Enemy, FloatText, LevelEntity, Particle, Shockwave};
use crate::data::Element;
use crate::equipment::Equipment;
use crate::game::Rng;
use crate::sprites::Sprites;
use crate::ui::UiFont;
use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct ScreenShake {
    trauma: f32,
}

impl ScreenShake {
    fn add(&mut self, amount: f32) {
        self.trauma = (self.trauma + amount).clamp(0.0, 1.0);
    }
}

#[derive(Component)]
pub struct ShakeCamera {
    pub base: Vec3,
}

/// A melee sword-swing arc: the slash sprite sweeps through `arc` radians around
/// `base` (the attack direction) over its short life, popping in scale and fading
/// out — so a melee hero reads as actually chopping, not flashing a static decal.
#[derive(Component)]
pub struct SwordSwing {
    pub life: f32,
    pub max_life: f32,
    pub base: f32,
    pub arc: f32,
}

/// A request for a one-off visual effect at a world position.
#[derive(Message)]
pub enum VfxEvent {
    /// Spark burst (a tower hit landed). `element` adds flavored particles
    /// (frost shards, toxic drips, fire embers, …) on top of the base impact.
    Hit {
        pos: Vec2,
        color: Color,
        element: Element,
    },
    /// Muzzle flash + forward sparks when a tower fires (`dir` = firing direction).
    Muzzle { pos: Vec2, dir: Vec2, color: Color },
    /// Melee slash arc (for close-range hero classes). `angle` orients the swoosh,
    /// `poison` picks the toxic variant.
    Slash {
        pos: Vec2,
        angle: f32,
        color: Color,
        poison: bool,
    },
    /// Local melee cleave ring for warrior-style area hits. This deliberately does
    /// not shake the camera; the normal per-enemy hit events provide impact.
    MeleeCleave {
        pos: Vec2,
        radius: f32,
        color: Color,
    },
    /// Quiet green restoration sparks.
    Heal { pos: Vec2 },
    /// Floating damage number.
    Number { pos: Vec2, amount: f32 },
    /// Floating combat number with a short label such as weakness/resistance.
    TaggedNumber {
        pos: Vec2,
        amount: f32,
        color: Color,
        label: &'static str,
    },
    /// Small silent ring used to make elemental weakness/resistance readable.
    ElementPulse {
        pos: Vec2,
        color: Color,
        strong: bool,
    },
    /// Priest hit feedback: a gold-white holy strike attached to the enemy body.
    HolyStrike { pos: Vec2, strong: bool },
    /// Floating label for discoveries and other non-damage feedback.
    Text {
        pos: Vec2,
        text: String,
        color: Color,
        size: f32,
        life: f32,
    },
    /// Enemy breach feedback at the protected carrot seal.
    CarrotHit {
        pos: Vec2,
        lives: i32,
        max_lives: i32,
    },
    /// First sighting during the current run; separate from persistent bestiary kills.
    ThreatIntro {
        pos: Vec2,
        species_id: usize,
        label: String,
        color: Color,
        rare: bool,
    },
    /// First-time bestiary discovery flourish with the monster portrait.
    Discovery {
        pos: Vec2,
        species_id: usize,
        label: String,
        color: Color,
        rare: bool,
    },
    /// Equipment reward flourish: gold chime, ring, sparks, and named label.
    Loot {
        pos: Vec2,
        item: Equipment,
        label: String,
        color: Color,
        rare: bool,
    },
    /// Combo economy reward burst.
    ComboReward { pos: Vec2, combo: i32, gold: i32 },
    /// Flawless wave reward at the protected carrot seal.
    PerfectWave { pos: Vec2, wave: i32, gold: i32 },
    /// Death burst (bigger for bosses).
    Death { pos: Vec2, color: Color, big: bool },
    /// Boss skill warning: named cast text + colored danger ring.
    BossCast {
        pos: Vec2,
        radius: f32,
        color: Color,
        label: &'static str,
    },
    /// Boss arrival: heavy multi-ring shockwave, big shake, and a名牌 banner.
    BossEntrance {
        pos: Vec2,
        color: Color,
        name: String,
    },
    /// Expanding shockwave ring + sparks (AoE / explosion).
    Explosion {
        pos: Vec2,
        radius: f32,
        color: Color,
    },
    /// Silent ring + spark burst for player actions (build / upgrade / sell).
    /// No sound — the caller plays the appropriate SFX.
    Burst {
        pos: Vec2,
        radius: f32,
        color: Color,
    },
    /// Full-screen meteor storm (Meteor ability): streaks rain across the board with
    /// staggered impact rings, plus a big blast at the damage `center`.
    MeteorStorm { center: Vec2, radius: f32 },
    /// Full-screen gold explosion (GoldRush ability): coins burst out everywhere.
    GoldExplosion { center: Vec2 },
    /// Full-screen frost nova (Freeze ability): expanding ice sweep + shards.
    FrostNova { center: Vec2 },
}

fn spark(
    commands: &mut Commands,
    rng: &mut Rng,
    pos: Vec2,
    color: Color,
    speed_min: f32,
    speed_max: f32,
    size: f32,
    life: f32,
) {
    let ang = rng.frac() * std::f32::consts::TAU;
    let spd = speed_min + rng.frac() * (speed_max - speed_min);
    let vel = Vec2::new(ang.cos(), ang.sin()) * spd;
    commands.spawn((
        Sprite {
            color,
            custom_size: Some(Vec2::splat(size)),
            ..default()
        },
        Transform::from_translation(pos.extend(15.0)),
        Particle {
            vel,
            life,
            max_life: life,
        },
        LevelEntity,
    ));
}

/// Spawn one particle with an explicit velocity (no randomness) — for directional
/// flourishes like toxic drips, fire embers, and muzzle cones.
fn spark_vel(commands: &mut Commands, pos: Vec2, color: Color, vel: Vec2, size: f32, life: f32) {
    commands.spawn((
        Sprite {
            color,
            custom_size: Some(Vec2::splat(size)),
            ..default()
        },
        Transform::from_translation(pos.extend(15.0)),
        Particle {
            vel,
            life,
            max_life: life,
        },
        LevelEntity,
    ));
}

/// Element-specific impact flair layered on top of the generic [`VfxEvent::Hit`]
/// burst: frost shards, toxic drips, fire embers, storm arcs, shadow wisps.
fn element_hit_flourish(commands: &mut Commands, rng: &mut Rng, pos: Vec2, element: Element) {
    let jitter = |rng: &mut Rng, spread: f32| (rng.frac() - 0.5) * spread;
    match element {
        Element::Fire => {
            // Embers rise and drift.
            for _ in 0..4 {
                let vel = Vec2::new(jitter(rng, 70.0), 70.0 + rng.frac() * 90.0);
                let col = Color::srgb(1.0, 0.55 + rng.frac() * 0.3, 0.15);
                spark_vel(commands, pos, col, vel, 3.6, 0.4);
            }
        }
        Element::Frost => {
            // Sharp pale shards that linger and barely move.
            for _ in 0..5 {
                let ang = rng.frac() * std::f32::consts::TAU;
                let vel = Vec2::from_angle(ang) * (20.0 + rng.frac() * 50.0);
                let col = Color::srgb(0.7, 0.9, 1.0);
                spark_vel(commands, pos, col, vel, 5.0, 0.55);
            }
        }
        Element::Toxic => {
            // Droplets that fall.
            for _ in 0..4 {
                let vel = Vec2::new(jitter(rng, 60.0), -50.0 - rng.frac() * 80.0);
                let col = Color::srgb(0.45, 0.95, 0.3);
                spark_vel(commands, pos, col, vel, 3.4, 0.5);
            }
        }
        Element::Storm => {
            // Fast bright arcs in all directions.
            for _ in 0..6 {
                let ang = rng.frac() * std::f32::consts::TAU;
                let vel = Vec2::from_angle(ang) * (220.0 + rng.frac() * 200.0);
                let col = Color::srgb(0.7, 0.9, 1.0);
                spark_vel(commands, pos, col, vel, 2.8, 0.18);
            }
        }
        Element::Shadow => {
            // Slow dark-purple wisps drifting upward.
            for _ in 0..4 {
                let vel = Vec2::new(jitter(rng, 40.0), 25.0 + rng.frac() * 45.0);
                let col = Color::srgb(0.45, 0.2, 0.6);
                spark_vel(commands, pos, col, vel, 4.2, 0.6);
            }
        }
        // Physical / Arcane: the generic impact already reads well.
        _ => {}
    }
}

fn spawn_holy_strike(
    commands: &mut Commands,
    rng: &mut Rng,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    pos: Vec2,
    strong: bool,
) {
    let gold = Color::srgb(1.0, 0.88, 0.36);
    let white = Color::srgb(1.0, 0.98, 0.82);
    let radius = if strong { 28.0 } else { 20.0 };
    let life = if strong { 0.34 } else { 0.24 };

    // A vertical light column pins the impact to the monster body instead of
    // reading as another projectile fired from the priest.
    commands.spawn((
        Sprite {
            color: white.with_alpha(if strong { 0.95 } else { 0.72 }),
            custom_size: Some(Vec2::new(if strong { 10.0 } else { 7.0 }, radius * 2.8)),
            ..default()
        },
        Transform::from_translation((pos + Vec2::new(0.0, radius * 0.35)).extend(18.2)),
        Particle {
            vel: Vec2::ZERO,
            life,
            max_life: life,
        },
        LevelEntity,
    ));
    commands.spawn((
        Sprite {
            color: gold.with_alpha(if strong { 0.9 } else { 0.62 }),
            custom_size: Some(Vec2::new(radius * 1.9, if strong { 7.0 } else { 5.0 })),
            ..default()
        },
        Transform::from_translation(pos.extend(18.3)),
        Particle {
            vel: Vec2::ZERO,
            life: life * 0.82,
            max_life: life * 0.82,
        },
        LevelEntity,
    ));
    spawn_ring(
        commands,
        meshes,
        materials,
        pos,
        radius,
        gold.mix(&Color::WHITE, 0.35),
        if strong { 0.68 } else { 0.42 },
        life,
        18.0,
    );
    let sparks = if strong { 9 } else { 5 };
    for _ in 0..sparks {
        let ang = -std::f32::consts::FRAC_PI_2 + (rng.frac() - 0.5) * 1.1;
        let vel = Vec2::from_angle(ang) * (70.0 + rng.frac() * 90.0);
        spark_vel(
            commands,
            pos + Vec2::new((rng.frac() - 0.5) * 14.0, 10.0),
            gold.mix(&Color::WHITE, rng.frac() * 0.55),
            vel,
            if strong { 4.2 } else { 3.2 },
            life + rng.frac() * 0.16,
        );
    }
}

fn spawn_ring(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    pos: Vec2,
    radius: f32,
    color: Color,
    alpha: f32,
    life: f32,
    z: f32,
) {
    let outer = radius.max(8.0);
    let inner = (outer - 7.0).max(outer * 0.82);
    commands.spawn((
        Mesh2d(meshes.add(Annulus::new(inner, outer))),
        MeshMaterial2d(materials.add(color.with_alpha(alpha))),
        Transform::from_translation(pos.extend(z)),
        Shockwave {
            life,
            max_life: life,
            radius: outer,
        },
        LevelEntity,
    ));
}

fn spawn_number(
    commands: &mut Commands,
    rng: &mut Rng,
    font: &UiFont,
    pos: Vec2,
    amount: f32,
    color: Color,
    label: Option<&'static str>,
) {
    let jitter = Vec2::new(rng.frac() * 10.0 - 5.0, 0.0);
    let text = if let Some(label) = label {
        format!("{} {}", amount.round() as i32, crate::i18n::t(label))
    } else {
        format!("{}", amount.round() as i32)
    };
    commands.spawn((
        Text2d::new(text),
        TextFont {
            font: FontSource::Handle(font.0.clone()),
            font_size: FontSize::Px(if label.is_some() { 17.0 } else { 18.0 }),
            ..default()
        },
        TextColor(color),
        Transform::from_translation((pos + jitter + Vec2::new(0.0, 14.0)).extend(20.0)),
        FloatText {
            life: 0.7,
            max_life: 0.7,
        },
        LevelEntity,
    ));
}

fn spawn_text(
    commands: &mut Commands,
    rng: &mut Rng,
    font: &UiFont,
    pos: Vec2,
    text: String,
    color: Color,
    size: f32,
    life: f32,
) {
    let jitter = Vec2::new(rng.frac() * 14.0 - 7.0, 0.0);
    commands.spawn((
        Text2d::new(text),
        TextFont {
            font: FontSource::Handle(font.0.clone()),
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(color),
        Transform::from_translation((pos + jitter + Vec2::new(0.0, 32.0)).extend(24.0)),
        FloatText {
            life,
            max_life: life,
        },
        LevelEntity,
    ));
}

/// Turn queued `VfxEvent`s into entities.
pub fn spawn_vfx(
    mut commands: Commands,
    mut events: MessageReader<VfxEvent>,
    mut rng: ResMut<Rng>,
    font: Res<UiFont>,
    sprites: Res<Sprites>,
    mut shake: ResMut<ScreenShake>,
    mut sfx: MessageWriter<crate::audio::SfxEvent>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    use crate::audio::{SfxEvent, Sound};
    for ev in events.read() {
        match ev {
            VfxEvent::Hit {
                pos,
                color,
                element,
            } => {
                sfx.write(SfxEvent(Sound::Hit));
                // Bright white core flash (stationary, very short) for a snappy pop.
                spark(
                    &mut commands,
                    &mut rng,
                    *pos,
                    Color::WHITE,
                    0.0,
                    0.0,
                    12.0,
                    0.1,
                );
                // A larger element-tinted flash behind it.
                spark(&mut commands, &mut rng, *pos, *color, 0.0, 0.0, 18.0, 0.16);
                // Quick expanding impact ring.
                spawn_ring(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    *pos,
                    15.0,
                    *color,
                    0.5,
                    0.26,
                    16.5,
                );
                // Directional debris sparks (more + faster than before).
                for _ in 0..9 {
                    spark(
                        &mut commands,
                        &mut rng,
                        *pos,
                        *color,
                        70.0,
                        220.0,
                        4.0,
                        0.34,
                    );
                }
                // Element-specific flourish on top of the generic impact.
                element_hit_flourish(&mut commands, &mut rng, *pos, *element);
            }
            VfxEvent::Muzzle { pos, dir, color } => {
                // Bright stationary flash at the barrel.
                spark(
                    &mut commands,
                    &mut rng,
                    *pos,
                    Color::WHITE,
                    0.0,
                    0.0,
                    11.0,
                    0.08,
                );
                spark(&mut commands, &mut rng, *pos, *color, 0.0, 0.0, 16.0, 0.12);
                // A short cone of sparks blown forward along the firing direction.
                let base = dir.to_angle();
                for _ in 0..5 {
                    let a = base + (rng.frac() - 0.5) * 0.7;
                    let spd = 150.0 + rng.frac() * 170.0;
                    spark_vel(
                        &mut commands,
                        *pos,
                        *color,
                        Vec2::from_angle(a) * spd,
                        3.4,
                        0.16,
                    );
                }
            }
            VfxEvent::Slash {
                pos,
                angle,
                color,
                poison,
            } => {
                let img = if *poison {
                    sprites.slash_poison.clone()
                } else {
                    sprites.slash.clone()
                };
                // The blade sweeps through an arc (劈砍): start raised, chop through
                // the target. `animate_sword_swing` drives the rotation/scale/fade.
                let arc = 2.35; // ~135° sweep
                let life = 0.30;
                commands.spawn((
                    Sprite {
                        image: img,
                        color: color.with_alpha(0.95),
                        custom_size: Some(Vec2::splat(crate::data::TILE_SIZE * 2.35)),
                        ..default()
                    },
                    Transform::from_translation(pos.extend(16.0))
                        .with_rotation(Quat::from_rotation_z(*angle + arc * 0.5)),
                    SwordSwing {
                        life,
                        max_life: life,
                        base: *angle,
                        arc,
                    },
                    LevelEntity,
                ));
                // A bright glint at the impact point sells the contact.
                spark(
                    &mut commands,
                    &mut rng,
                    *pos,
                    color.mix(&Color::WHITE, 0.6),
                    60.0,
                    220.0,
                    3.0,
                    0.22,
                );
                // A few sparks fly off along the swing direction.
                for _ in 0..7 {
                    let jitter = (rng.frac() - 0.5) * 0.6;
                    spark_vel(
                        &mut commands,
                        *pos,
                        color.mix(&Color::WHITE, 0.35),
                        Vec2::from_angle(*angle + jitter) * (90.0 + rng.frac() * 90.0),
                        3.0,
                        0.26,
                    );
                }
            }
            VfxEvent::MeleeCleave { pos, radius, color } => {
                spawn_ring(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    *pos,
                    *radius,
                    color.mix(&Color::WHITE, 0.18),
                    0.46,
                    0.20,
                    12.5,
                );
                spawn_ring(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    *pos,
                    *radius * 0.62,
                    *color,
                    0.30,
                    0.16,
                    12.7,
                );
                for _ in 0..10 {
                    spark(
                        &mut commands,
                        &mut rng,
                        *pos,
                        *color,
                        70.0,
                        190.0,
                        3.4,
                        0.25,
                    );
                }
            }
            VfxEvent::Heal { pos } => {
                for _ in 0..4 {
                    spark(
                        &mut commands,
                        &mut rng,
                        *pos,
                        Color::srgb(0.45, 1.0, 0.55),
                        35.0,
                        100.0,
                        3.0,
                        0.45,
                    );
                }
            }
            VfxEvent::Number { pos, amount } => {
                spawn_number(
                    &mut commands,
                    &mut rng,
                    &font,
                    *pos,
                    *amount,
                    Color::srgb(1.0, 0.95, 0.6),
                    None,
                );
            }
            VfxEvent::TaggedNumber {
                pos,
                amount,
                color,
                label,
            } => {
                spawn_number(
                    &mut commands,
                    &mut rng,
                    &font,
                    *pos,
                    *amount,
                    *color,
                    Some(label),
                );
            }
            VfxEvent::ElementPulse { pos, color, strong } => {
                let radius = if *strong { 26.0 } else { 18.0 };
                spawn_ring(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    *pos,
                    radius,
                    *color,
                    if *strong { 0.55 } else { 0.34 },
                    if *strong { 0.45 } else { 0.32 },
                    18.5,
                );
                let sparks = if *strong { 8 } else { 4 };
                for _ in 0..sparks {
                    spark(
                        &mut commands,
                        &mut rng,
                        *pos,
                        *color,
                        45.0,
                        if *strong { 150.0 } else { 90.0 },
                        if *strong { 3.5 } else { 2.6 },
                        if *strong { 0.38 } else { 0.24 },
                    );
                }
            }
            VfxEvent::HolyStrike { pos, strong } => {
                spawn_holy_strike(
                    &mut commands,
                    &mut rng,
                    &mut meshes,
                    &mut materials,
                    *pos,
                    *strong,
                );
            }
            VfxEvent::Text {
                pos,
                text,
                color,
                size,
                life,
            } => {
                spawn_text(
                    &mut commands,
                    &mut rng,
                    &font,
                    *pos,
                    text.clone(),
                    *color,
                    *size,
                    *life,
                );
            }
            VfxEvent::CarrotHit {
                pos,
                lives,
                max_lives,
            } => {
                let max_lives = (*max_lives).max(1);
                let lives = (*lives).max(0);
                let frac = (lives as f32 / max_lives as f32).clamp(0.0, 1.0);
                let danger = frac <= 0.33;
                let color = if danger {
                    Color::srgb(1.0, 0.14, 0.10)
                } else {
                    Color::srgb(1.0, 0.66, 0.18)
                };
                sfx.write(SfxEvent(Sound::Hit));
                shake.add(if danger { 0.30 } else { 0.16 });
                spawn_ring(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    *pos,
                    if danger { 58.0 } else { 46.0 },
                    color,
                    0.68,
                    0.52,
                    18.4,
                );
                spawn_ring(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    *pos,
                    if danger { 34.0 } else { 26.0 },
                    Color::WHITE,
                    0.20,
                    0.42,
                    18.5,
                );
                for _ in 0..(if danger { 20 } else { 12 }) {
                    spark(
                        &mut commands,
                        &mut rng,
                        *pos,
                        color,
                        80.0,
                        if danger { 245.0 } else { 170.0 },
                        if danger { 5.0 } else { 3.8 },
                        if danger { 0.62 } else { 0.45 },
                    );
                }
                spawn_text(
                    &mut commands,
                    &mut rng,
                    &font,
                    *pos + Vec2::new(0.0, 32.0),
                    crate::i18n::tf(
                        "封印受损 {}/{}",
                        &[&lives.to_string(), &max_lives.to_string()],
                    ),
                    color,
                    if danger { 20.0 } else { 17.0 },
                    if danger { 1.10 } else { 0.90 },
                );
                if danger && lives > 0 {
                    spawn_text(
                        &mut commands,
                        &mut rng,
                        &font,
                        *pos + Vec2::new(0.0, 54.0),
                        crate::i18n::t("封印濒危"),
                        Color::srgb(1.0, 0.32, 0.18),
                        16.0,
                        0.95,
                    );
                }
            }
            VfxEvent::ThreatIntro {
                pos,
                species_id,
                label,
                color,
                rare,
            } => {
                sfx.write(SfxEvent(if *rare { Sound::Wave } else { Sound::Click }));
                shake.add(if *rare { 0.18 } else { 0.05 });
                let radius = if *rare { 42.0 } else { 30.0 };
                spawn_ring(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    *pos,
                    radius,
                    *color,
                    if *rare { 0.60 } else { 0.38 },
                    0.65,
                    18.15,
                );
                spawn_ring(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    *pos,
                    radius * 0.62,
                    Color::WHITE,
                    if *rare { 0.22 } else { 0.14 },
                    0.52,
                    18.2,
                );
                let sparks = if *rare { 16 } else { 8 };
                for _ in 0..sparks {
                    spark(
                        &mut commands,
                        &mut rng,
                        *pos,
                        *color,
                        55.0,
                        if *rare { 210.0 } else { 125.0 },
                        if *rare { 4.0 } else { 2.8 },
                        if *rare { 0.58 } else { 0.38 },
                    );
                }
                if let Some(image) = sprites.species.get(species_id) {
                    commands.spawn((
                        Sprite {
                            image: image.clone(),
                            color: Color::srgba(1.0, 1.0, 1.0, 0.82),
                            custom_size: Some(Vec2::splat(if *rare { 42.0 } else { 34.0 })),
                            ..default()
                        },
                        Transform::from_translation((*pos + Vec2::new(0.0, 16.0)).extend(24.5)),
                        Particle {
                            vel: Vec2::new(0.0, if *rare { 34.0 } else { 24.0 }),
                            life: if *rare { 1.05 } else { 0.82 },
                            max_life: if *rare { 1.05 } else { 0.82 },
                        },
                        LevelEntity,
                    ));
                }
                spawn_text(
                    &mut commands,
                    &mut rng,
                    &font,
                    *pos,
                    label.clone(),
                    *color,
                    if *rare { 18.0 } else { 15.0 },
                    if *rare { 1.05 } else { 0.82 },
                );
            }
            VfxEvent::Discovery {
                pos,
                species_id,
                label,
                color,
                rare,
            } => {
                sfx.write(SfxEvent(Sound::Click));
                let radius = if *rare { 46.0 } else { 32.0 };
                spawn_ring(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    *pos,
                    radius,
                    *color,
                    if *rare { 0.72 } else { 0.48 },
                    0.75,
                    18.2,
                );
                spawn_ring(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    *pos,
                    radius * 0.55,
                    Color::WHITE,
                    if *rare { 0.30 } else { 0.18 },
                    0.75,
                    18.3,
                );
                let sparks = if *rare { 20 } else { 10 };
                for _ in 0..sparks {
                    spark(
                        &mut commands,
                        &mut rng,
                        *pos,
                        *color,
                        70.0,
                        if *rare { 240.0 } else { 145.0 },
                        if *rare { 4.8 } else { 3.2 },
                        if *rare { 0.70 } else { 0.46 },
                    );
                }
                if let Some(image) = sprites.species.get(species_id) {
                    commands.spawn((
                        Sprite {
                            image: image.clone(),
                            color: Color::WHITE,
                            custom_size: Some(Vec2::splat(if *rare { 48.0 } else { 38.0 })),
                            ..default()
                        },
                        Transform::from_translation((*pos + Vec2::new(0.0, 18.0)).extend(25.0)),
                        Particle {
                            vel: Vec2::new(0.0, if *rare { 42.0 } else { 30.0 }),
                            life: if *rare { 1.15 } else { 0.95 },
                            max_life: if *rare { 1.15 } else { 0.95 },
                        },
                        LevelEntity,
                    ));
                }
                spawn_text(
                    &mut commands,
                    &mut rng,
                    &font,
                    *pos,
                    label.clone(),
                    *color,
                    if *rare { 19.0 } else { 16.0 },
                    if *rare { 1.15 } else { 0.95 },
                );
            }
            VfxEvent::Loot {
                pos,
                item,
                label,
                color,
                rare,
            } => {
                sfx.write(SfxEvent(Sound::Gold));
                shake.add(if *rare { 0.20 } else { 0.04 });
                let radius = if *rare { 44.0 } else { 30.0 };
                spawn_ring(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    *pos,
                    radius,
                    *color,
                    if *rare { 0.7 } else { 0.45 },
                    0.55,
                    18.0,
                );
                let sparks = if *rare { 22 } else { 12 };
                for _ in 0..sparks {
                    spark(
                        &mut commands,
                        &mut rng,
                        *pos,
                        *color,
                        85.0,
                        if *rare { 260.0 } else { 180.0 },
                        if *rare { 5.5 } else { 4.0 },
                        if *rare { 0.75 } else { 0.5 },
                    );
                }
                let icon_size = if *rare { 44.0 } else { 34.0 };
                commands.spawn((
                    Sprite {
                        image: sprites.equipment[item].clone(),
                        color: Color::WHITE,
                        custom_size: Some(Vec2::splat(icon_size)),
                        ..default()
                    },
                    Transform::from_translation((*pos + Vec2::new(0.0, 18.0)).extend(25.0)),
                    Particle {
                        vel: Vec2::new(0.0, if *rare { 44.0 } else { 32.0 }),
                        life: if *rare { 1.15 } else { 0.9 },
                        max_life: if *rare { 1.15 } else { 0.9 },
                    },
                    LevelEntity,
                ));
                spawn_text(
                    &mut commands,
                    &mut rng,
                    &font,
                    *pos,
                    label.clone(),
                    *color,
                    if *rare { 20.0 } else { 17.0 },
                    if *rare { 1.15 } else { 0.9 },
                );
            }
            VfxEvent::ComboReward { pos, combo, gold } => {
                sfx.write(SfxEvent(Sound::Gold));
                let combo = (*combo).max(1);
                let gold = (*gold).max(0);
                let strength = (combo as f32 / 20.0).clamp(0.25, 1.0);
                let color = Color::srgb(1.0, 0.80, 0.18);
                shake.add(0.06 + strength * 0.12);
                spawn_ring(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    *pos,
                    26.0 + strength * 26.0,
                    color,
                    0.55,
                    0.48,
                    18.25,
                );
                spawn_ring(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    *pos,
                    15.0 + strength * 16.0,
                    Color::WHITE,
                    0.18,
                    0.38,
                    18.3,
                );
                for _ in 0..(10 + (strength * 12.0).round() as usize) {
                    spark(
                        &mut commands,
                        &mut rng,
                        *pos,
                        color,
                        75.0,
                        185.0 + strength * 80.0,
                        3.6 + strength * 1.8,
                        0.42 + strength * 0.18,
                    );
                }
                spawn_text(
                    &mut commands,
                    &mut rng,
                    &font,
                    *pos + Vec2::new(0.0, 26.0),
                    crate::i18n::tf("连杀 x{}  +{}金", &[&combo.to_string(), &gold.to_string()]),
                    color,
                    16.0 + strength * 4.0,
                    0.95 + strength * 0.18,
                );
            }
            VfxEvent::PerfectWave { pos, wave, gold } => {
                sfx.write(SfxEvent(Sound::Gold));
                let gold = (*gold).max(0);
                let color = Color::srgb(0.50, 1.0, 0.58);
                shake.add(0.12);
                spawn_ring(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    *pos,
                    56.0,
                    color,
                    0.62,
                    0.65,
                    18.15,
                );
                spawn_ring(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    *pos,
                    33.0,
                    Color::WHITE,
                    0.22,
                    0.48,
                    18.2,
                );
                for _ in 0..18 {
                    spark(&mut commands, &mut rng, *pos, color, 85.0, 230.0, 4.2, 0.58);
                }
                spawn_text(
                    &mut commands,
                    &mut rng,
                    &font,
                    *pos + Vec2::new(0.0, 34.0),
                    crate::i18n::tf(
                        "完美防守 第{}波  +{}金",
                        &[&wave.to_string(), &gold.to_string()],
                    ),
                    color,
                    19.0,
                    1.2,
                );
            }
            VfxEvent::Death { pos, color, big } => {
                sfx.write(SfxEvent(Sound::Death));
                shake.add(if *big { 0.42 } else { 0.035 });
                // A burst flash + expanding ring so a kill reads as a pop, not just
                // scattered dots. Bosses/elites (`big`) get a much larger blast.
                let scale = if *big { 1.0 } else { 0.5 };
                spark(
                    &mut commands,
                    &mut rng,
                    *pos,
                    Color::WHITE,
                    0.0,
                    0.0,
                    14.0 * scale + 8.0,
                    0.14,
                );
                spawn_ring(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    *pos,
                    if *big { 40.0 } else { 18.0 },
                    *color,
                    0.6,
                    if *big { 0.5 } else { 0.32 },
                    16.2,
                );
                let n = if *big { 28 } else { 10 };
                for _ in 0..n {
                    spark(&mut commands, &mut rng, *pos, *color, 80.0, 240.0, 5.0, 0.6);
                }
            }
            VfxEvent::BossCast {
                pos,
                radius,
                color,
                label,
            } => {
                sfx.write(SfxEvent(Sound::Meteor));
                shake.add(0.22);
                spawn_ring(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    *pos,
                    *radius,
                    *color,
                    0.55,
                    0.9,
                    16.0,
                );
                spawn_ring(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    *pos,
                    *radius * 0.55,
                    *color,
                    0.24,
                    0.9,
                    16.1,
                );
                commands.spawn((
                    Text2d::new(format!("{}!", crate::i18n::t(label))),
                    TextFont {
                        font: FontSource::Handle(font.0.clone()),
                        font_size: FontSize::Px(17.0),
                        ..default()
                    },
                    TextColor(color.with_alpha(0.95)),
                    Transform::from_translation((*pos + Vec2::new(0.0, 30.0)).extend(24.0)),
                    FloatText {
                        life: 0.9,
                        max_life: 0.9,
                    },
                    LevelEntity,
                ));
            }
            VfxEvent::BossEntrance { pos, color, name } => {
                sfx.write(SfxEvent(Sound::Boss));
                shake.add(0.6);
                // Triple expanding shockwave for weight.
                for (i, r) in [40.0_f32, 80.0, 130.0].into_iter().enumerate() {
                    spawn_ring(
                        &mut commands,
                        &mut meshes,
                        &mut materials,
                        *pos,
                        r,
                        *color,
                        0.6 - i as f32 * 0.12,
                        0.95,
                        16.0 + i as f32 * 0.1,
                    );
                }
                // Dark debris burst.
                for _ in 0..22 {
                    spark(&mut commands, &mut rng, *pos, *color, 90.0, 300.0, 5.5, 0.7);
                }
                // Big arrival banner.
                commands.spawn((
                    Text2d::new(crate::i18n::tf("⚠ 首领降临 ⚠\n{}", &[name.as_str()])),
                    TextFont {
                        font: FontSource::Handle(font.0.clone()),
                        font_size: FontSize::Px(26.0),
                        ..default()
                    },
                    TextColor(color.mix(&Color::WHITE, 0.2)),
                    Transform::from_translation((*pos + Vec2::new(0.0, 44.0)).extend(25.0)),
                    FloatText {
                        life: 1.8,
                        max_life: 1.8,
                    },
                    LevelEntity,
                ));
            }
            VfxEvent::Explosion { pos, radius, color } => {
                sfx.write(SfxEvent(Sound::Explosion));
                shake.add((*radius / 520.0).clamp(0.05, 0.34));
                // Expanding ring (its own material so we can fade alpha).
                spawn_ring(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    *pos,
                    *radius,
                    *color,
                    0.8,
                    0.35,
                    14.0,
                );
                let sparks = ((*radius / 16.0).round() as usize).clamp(8, 24);
                for _ in 0..sparks {
                    spark(
                        &mut commands,
                        &mut rng,
                        *pos,
                        *color,
                        100.0,
                        260.0,
                        5.0,
                        0.45,
                    );
                }
            }
            VfxEvent::Burst { pos, radius, color } => {
                // White core flash, expanding ring, and a spray of sparks.
                spark(
                    &mut commands,
                    &mut rng,
                    *pos,
                    Color::WHITE,
                    0.0,
                    0.0,
                    14.0,
                    0.13,
                );
                spawn_ring(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    *pos,
                    *radius,
                    *color,
                    0.6,
                    0.4,
                    16.4,
                );
                let n = ((*radius / 9.0).round() as usize).clamp(7, 16);
                for _ in 0..n {
                    spark(
                        &mut commands,
                        &mut rng,
                        *pos,
                        *color,
                        60.0,
                        190.0,
                        4.2,
                        0.42,
                    );
                }
            }
            VfxEvent::MeteorStorm { center, radius } => {
                use crate::data::{BOARD_H, BOARD_W};
                shake.add(0.5);
                // Meteors streak in from the upper-right and rain across the board.
                for _ in 0..18 {
                    let tx = (rng.frac() - 0.5) * BOARD_W;
                    let ty = (rng.frac() - 0.5) * BOARD_H * 0.75;
                    let impact = Vec2::new(tx, ty);
                    let start = Vec2::new(
                        tx + 130.0 + rng.frac() * 90.0,
                        BOARD_H * 0.5 + 110.0 + rng.frac() * 200.0,
                    );
                    let dir = (impact - start).normalize_or_zero();
                    let spd = 900.0 + rng.frac() * 360.0;
                    let life = 0.5 + rng.frac() * 0.25;
                    let sz = 46.0 + rng.frac() * 22.0;
                    // The meteor sprite points DOWN (-Y); rotate so it aims along `dir`.
                    let rot = Quat::from_rotation_z(dir.to_angle() + std::f32::consts::FRAC_PI_2);
                    commands.spawn((
                        Sprite {
                            image: sprites.ui["meteor_fx"].clone(),
                            custom_size: Some(Vec2::splat(sz)),
                            ..default()
                        },
                        Transform {
                            translation: start.extend(16.0),
                            rotation: rot,
                            ..default()
                        },
                        Particle {
                            vel: dir * spd,
                            life,
                            max_life: life,
                        },
                        LevelEntity,
                    ));
                    for t in 1..3 {
                        spark_vel(
                            &mut commands,
                            start - dir * (t as f32 * 16.0),
                            Color::srgb(1.0, 0.4, 0.12),
                            dir * spd * 0.92,
                            5.0,
                            0.35,
                        );
                    }
                    spawn_ring(
                        &mut commands,
                        &mut meshes,
                        &mut materials,
                        impact,
                        24.0,
                        Color::srgb(1.0, 0.42, 0.12),
                        0.5,
                        0.5,
                        18.0,
                    );
                    for _ in 0..3 {
                        spark(
                            &mut commands,
                            &mut rng,
                            impact,
                            Color::srgb(1.0, 0.55, 0.18),
                            50.0,
                            170.0,
                            4.0,
                            0.4,
                        );
                    }
                }
                // Big central blast on the actual damage zone.
                spawn_ring(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    *center,
                    *radius + 6.0,
                    Color::srgb(1.0, 0.35, 0.08),
                    0.65,
                    0.6,
                    18.3,
                );
                spawn_ring(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    *center,
                    *radius * 0.55,
                    Color::srgb(1.0, 0.85, 0.4),
                    0.4,
                    0.45,
                    18.4,
                );
                for _ in 0..16 {
                    spark(
                        &mut commands,
                        &mut rng,
                        *center,
                        Color::srgb(1.0, 0.55, 0.15),
                        90.0,
                        280.0,
                        5.5,
                        0.55,
                    );
                }
            }
            VfxEvent::GoldExplosion { center } => {
                shake.add(0.25);
                let gold = Color::srgb(1.0, 0.84, 0.24);
                // A fountain of coin sprites bursting outward across the screen.
                for _ in 0..44 {
                    let ang = rng.frac() * std::f32::consts::TAU;
                    let spd = 130.0 + rng.frac() * 420.0;
                    let vel = Vec2::from_angle(ang) * spd + Vec2::new(0.0, 140.0);
                    let life = 0.85 + rng.frac() * 0.55;
                    commands.spawn((
                        Sprite {
                            image: sprites.ui["coin"].clone(),
                            custom_size: Some(Vec2::splat(15.0 + rng.frac() * 13.0)),
                            ..default()
                        },
                        Transform::from_translation(center.extend(16.0)),
                        Particle {
                            vel,
                            life,
                            max_life: life,
                        },
                        LevelEntity,
                    ));
                }
                spawn_ring(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    *center,
                    64.0,
                    gold,
                    0.55,
                    0.5,
                    18.0,
                );
                for _ in 0..18 {
                    spark(
                        &mut commands,
                        &mut rng,
                        *center,
                        gold,
                        70.0,
                        260.0,
                        4.5,
                        0.5,
                    );
                }
            }
            VfxEvent::FrostNova { center } => {
                use crate::data::{BOARD_H, BOARD_W};
                shake.add(0.2);
                let diag = (BOARD_W * BOARD_W + BOARD_H * BOARD_H).sqrt();
                spawn_ring(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    *center,
                    diag * 0.5,
                    Color::srgb(0.45, 0.82, 1.0),
                    0.4,
                    0.75,
                    18.0,
                );
                spawn_ring(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    *center,
                    70.0,
                    Color::srgb(0.8, 0.95, 1.0),
                    0.45,
                    0.5,
                    18.1,
                );
                for _ in 0..28 {
                    let ang = rng.frac() * std::f32::consts::TAU;
                    let vel = Vec2::from_angle(ang) * (120.0 + rng.frac() * 280.0);
                    spark_vel(
                        &mut commands,
                        *center,
                        Color::srgb(0.72, 0.92, 1.0),
                        vel,
                        6.0,
                        0.6,
                    );
                }
            }
        }
    }
}

pub fn update_camera_shake(
    time: Res<Time>,
    mut shake: ResMut<ScreenShake>,
    mut q: Query<(&ShakeCamera, &mut Transform), With<Camera2d>>,
) {
    let Ok((camera, mut tf)) = q.single_mut() else {
        return;
    };
    let dt = time.delta_secs();
    if shake.trauma <= 0.001 {
        shake.trauma = 0.0;
        tf.translation = camera.base;
        tf.rotation = Quat::IDENTITY;
        return;
    }

    let power = shake.trauma * shake.trauma;
    let phase = time.elapsed_secs() * 67.0;
    let offset = Vec2::new((phase * 1.31).sin(), (phase * 1.73).cos()) * 9.0 * power;
    tf.translation = camera.base + Vec3::new(offset.x, offset.y, 0.0);
    tf.rotation = Quat::from_rotation_z((phase * 0.43).sin() * 0.012 * power);
    shake.trauma = (shake.trauma - dt * 1.7).max(0.0);
}

/// Drive the melee sword-swing arc: sweep rotation from raised → chopped-through,
/// pop the scale, and fade out. Despawns when its life expires.
pub fn animate_sword_swing(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut SwordSwing, &mut Transform, &mut Sprite)>,
) {
    let dt = time.delta_secs();
    for (e, mut s, mut tf, mut sprite) in &mut q {
        s.life -= dt;
        if s.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        let progress = 1.0 - (s.life / s.max_life); // 0 → 1
        // Smoothstep for a fast "whoosh" through the middle of the swing.
        let ease = progress * progress * (3.0 - 2.0 * progress);
        // Sweep from +arc/2 (blade raised) down through −arc/2 (chopped through).
        let angle = s.base + s.arc * (0.5 - ease);
        // Scale pops out at mid-swing then settles — sin gives 0→1→0 over the life.
        let pop = 0.78 + 0.42 * (progress * std::f32::consts::PI).sin();
        tf.rotation = Quat::from_rotation_z(angle);
        tf.scale = Vec3::splat(pop);
        // Bright at the strike, fading toward the end.
        sprite.color.set_alpha((1.0 - progress).powf(0.55) * 0.95);
    }
}

pub fn update_particles(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut Particle, &mut Transform, &mut Sprite)>,
) {
    let dt = time.delta_secs();
    for (e, mut p, mut tf, mut sprite) in &mut q {
        p.life -= dt;
        if p.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        let t = p.life / p.max_life;
        tf.translation.x += p.vel.x * dt;
        tf.translation.y += p.vel.y * dt;
        p.vel *= 1.0 - 3.0 * dt; // drag
        sprite.color.set_alpha(t);
    }
}

pub fn update_float_text(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut FloatText, &mut Transform, &mut TextColor)>,
) {
    let dt = time.delta_secs();
    for (e, mut ft, mut tf, mut color) in &mut q {
        ft.life -= dt;
        if ft.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        tf.translation.y += 36.0 * dt;
        color.0.set_alpha(ft.life / ft.max_life);
    }
}

pub fn update_shockwaves(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(
        Entity,
        &mut Shockwave,
        &mut Transform,
        &MeshMaterial2d<ColorMaterial>,
    )>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let dt = time.delta_secs();
    for (e, mut sw, mut tf, mat) in &mut q {
        sw.life -= dt;
        if sw.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        let t = 1.0 - sw.life / sw.max_life; // 0..1 expansion
        let start = (18.0 / sw.radius.max(18.0)).min(0.65);
        let finish = 1.0 + (sw.radius / 600.0).min(0.18);
        tf.scale = Vec3::splat(start + t * (finish - start));
        if let Some(mut m) = materials.get_mut(&mat.0) {
            m.color.set_alpha((1.0 - t) * 0.8);
        }
    }
}

/// Brief scale-pop on enemies that were just damaged.
pub fn enemy_hit_pop(time: Res<Time>, mut q: Query<(&mut Enemy, &mut Transform)>) {
    let dt = time.delta_secs();
    for (mut e, mut tf) in &mut q {
        if e.hit_flash > 0.0 {
            e.hit_flash = (e.hit_flash - dt).max(0.0);
            let pop = 1.0 + 0.35 * (e.hit_flash / 0.12);
            tf.scale = Vec3::splat(pop);
        } else if tf.scale != Vec3::ONE {
            tf.scale = Vec3::ONE;
        }
    }
}
