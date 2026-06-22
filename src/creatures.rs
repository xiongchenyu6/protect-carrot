//! Animated enemy sprites from downloaded creature packs (sprite-sheet animation
//! via `TextureAtlas`). Each enemy kind maps to a creature's locomotion sheet AND an
//! attack sheet; `animate_creatures` cycles frames and swaps to the attack sheet while
//! the enemy is engaged (`Enemy::blocked`), so monsters visibly strike towers/heroes.
//!
//! Sheets live in `assets/creatures/<name>.webp` (locomotion) and
//! `assets/creatures/<name>_attack.webp` (attack) as horizontal strips of square
//! frames (frame size = sheet height, frame count = width / height).

use crate::components::Enemy;
use crate::data::EnemyKind;
use bevy::prelude::*;
use std::collections::HashMap;

pub struct CreatureCfg {
    pub image: Handle<Image>,
    pub layout: Handle<TextureAtlasLayout>,
    pub frames: usize,
    pub fps: f32,
    /// Attack animation (played while the enemy is striking).
    pub atk_image: Handle<Image>,
    pub atk_layout: Handle<TextureAtlasLayout>,
    pub atk_frames: usize,
}

#[derive(Resource)]
pub struct Creatures(pub HashMap<EnemyKind, CreatureCfg>);

/// Per-entity animation cursor.
#[derive(Component)]
pub struct CreatureAnim {
    pub timer: Timer,
    pub frames: usize,
    pub kind: EnemyKind,
    /// Currently showing the attack sheet (vs locomotion).
    pub attacking: bool,
}

/// (enemy kind, sheet stem, square frame px, locomotion frames, attack frames).
fn mapping() -> [(EnemyKind, &'static str, u32, usize, usize); 16] {
    use EnemyKind::*;
    [
        (Normal, "goblin", 150, 8, 8),
        (Fast, "rat", 70, 8, 12),
        (Tank, "mimic", 146, 6, 13),
        (Flying, "flyingeye", 150, 8, 8),
        (Invisible, "bat", 87, 11, 11),
        (Regenerating, "slime", 156, 6, 19),
        (Armored, "skeleton", 150, 4, 8),
        (Swarmer, "rat", 70, 8, 12),
        (Boss, "wizard", 140, 8, 13),
        (Shielded, "mushroom", 150, 8, 8),
        (Splitter, "slime", 156, 6, 19),
        (Healer, "wizard", 140, 8, 13),
        (Charger, "fireworm", 90, 9, 16),
        (Climber, "fireworm", 90, 9, 16),
        (Silencer, "bat", 87, 11, 11),
        (Moss, "wizard", 140, 8, 13),
    ]
}

pub fn load_creatures(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let mut m = HashMap::new();
    for (kind, file, frame, frames, atk_frames) in mapping() {
        let image = assets.load(format!("creatures/{}.webp", file));
        let layout = layouts.add(TextureAtlasLayout::from_grid(
            UVec2::splat(frame),
            frames as u32,
            1,
            None,
            None,
        ));
        let atk_image = assets.load(format!("creatures/{}_attack.webp", file));
        let atk_layout = layouts.add(TextureAtlasLayout::from_grid(
            UVec2::splat(frame),
            atk_frames as u32,
            1,
            None,
            None,
        ));
        m.insert(
            kind,
            CreatureCfg {
                image,
                layout,
                frames,
                fps: 10.0,
                atk_image,
                atk_layout,
                atk_frames,
            },
        );
    }
    commands.insert_resource(Creatures(m));
}

impl Creatures {
    /// Build the animated `Sprite` + `CreatureAnim` for an enemy kind at a given
    /// on-screen size (pixels across). Starts on the locomotion sheet.
    pub fn sprite(&self, kind: EnemyKind, px: f32) -> (Sprite, CreatureAnim) {
        let cfg = &self.0[&kind];
        let mut sprite = Sprite::from_atlas_image(
            cfg.image.clone(),
            TextureAtlas {
                layout: cfg.layout.clone(),
                index: 0,
            },
        );
        sprite.custom_size = Some(Vec2::splat(px));
        let anim = CreatureAnim {
            timer: Timer::from_seconds(1.0 / cfg.fps, TimerMode::Repeating),
            frames: cfg.frames,
            kind,
            attacking: false,
        };
        (sprite, anim)
    }
}

/// Advance creature frames; swap between locomotion and attack sheets based on whether
/// the enemy is currently engaged (`blocked`). Summons have a `CreatureAnim` but no
/// `Enemy`, so they always animate their locomotion sheet.
pub fn animate_creatures(
    time: Res<Time>,
    creatures: Res<Creatures>,
    mut q: Query<(Option<&Enemy>, &mut CreatureAnim, &mut Sprite)>,
) {
    for (enemy, mut a, mut sprite) in &mut q {
        let attacking = enemy.is_some_and(|e| e.blocked && e.hp > 0.0);
        if attacking != a.attacking {
            a.attacking = attacking;
            let cfg = &creatures.0[&a.kind];
            let (img, lay, frames) = if attacking {
                (cfg.atk_image.clone(), cfg.atk_layout.clone(), cfg.atk_frames)
            } else {
                (cfg.image.clone(), cfg.layout.clone(), cfg.frames)
            };
            sprite.image = img;
            if let Some(atlas) = &mut sprite.texture_atlas {
                atlas.layout = lay;
                atlas.index = 0;
            }
            a.frames = frames;
            a.timer.reset();
        }
        a.timer.tick(time.delta());
        if a.timer.just_finished() {
            if let Some(atlas) = &mut sprite.texture_atlas {
                atlas.index = (atlas.index + 1) % a.frames.max(1);
            }
        }
    }
}
