# 保卫萝卜 → Bevy 移植计划 (Bevy 0.18.1, single-player)

Porting the single-file HTML tower-defense game (`保卫萝卜.html`) to an idiomatic Bevy
ECS project, with a WebGPU/wasm build for browser learning. Multiplayer is deferred.

Each stage must `cargo check` (native) clean before moving on. Render uses primitive
shapes (`Sprite` colored rects + `Mesh2d` circles) — no image assets, matching the original.

## Stage 1: Foundation & playfield
**Goal**: Window opens, 20×15 grid drawn, path + buildable tiles + carrot rendered for level 0.
**Success Criteria**: `cargo run` shows the board exactly matching level 0's path layout.
**Modules**: `main.rs`, `data.rs` (TOWER_TYPES/ENEMY_TYPES/LEVELS as Rust), `grid.rs`, `states.rs`.
**Status**: Complete

## Stage 2: Enemies & waves
**Goal**: Enemies spawn on waves, walk the path, reach carrot → lose life; all 9 enemy types & boss waves.
**Success Criteria**: Start a wave, enemies traverse path, lives decrement, status effects fields present.
**Tests**: path-following reaches each waypoint; enemy stats scale per level/wave.
**Status**: Complete

## Stage 3: Towers, targeting & projectiles
**Goal**: Build/select/upgrade/sell towers; all 18 tower types with their behaviors (single/aoe/chain/laser/homing/slow/freeze/curse/heal/detect/poison/fire/summon/knockback).
**Success Criteria**: Each tower acquires targets, fires, applies its effect; damage uses armor/magic-resist + detection gating for invisible.
**Status**: Complete

## Stage 4: Economy, UI & flow
**Goal**: Gold/lives/wave HUD, build menu (4 category tabs), pause/speed, win/lose, level select, localStorage-equivalent progress save.
**Success Criteria**: Full playable loop across the 20 levels; progress persists.
**Status**: Complete

## Stage 5: WebGPU / wasm export
**Goal**: `wasm32-unknown-unknown` build via wasm-bindgen, WebGPU backend, served `index.html`.
**Success Criteria**: Game runs in a WebGPU-capable browser; documented build command.
**Status**: Complete

## Stage 6: Polish & learning notes
**Goal**: Particles/explosions, code comments explaining Bevy concepts, README for learners.
**Status**: Complete
