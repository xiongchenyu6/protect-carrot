# 技术栈 / Tech Stack — 保卫萝卜 (Bevy port)

A single-player Bevy port of 保卫萝卜 (tower defense), targeting **WebGPU/wasm** in the
browser. This document maps the technologies, the build/deploy pipeline, the code
modules, and the (AI-assisted) asset pipeline.

> The original prototype is a single-file HTML/JS game (`保卫萝卜.html`). This Rust/Bevy
> version is the active codebase.

## Runtime & engine

| Layer | Choice | Notes |
|---|---|---|
| Language | **Rust** (edition 2024) | |
| Engine | **Bevy 0.19** (ECS) | `features = ["mp3", "webp"]` |
| Render backend | **WebGPU** (wasm) / wgpu | `--features webgpu` makes `webgpu` win over `webgl2` |
| Target | `wasm32-unknown-unknown` | also runs natively for debugging |
| Persistence | browser **localStorage** | via `wasm-bindgen` inline JS (hero, progress, settings) |

ECS model: state lives in **Resources** (`RunState`, `HeroLoadout`, `Sprites`,
`Abilities`, `Talents`, …) and **Components** (`Tower`, `Enemy`, `Summon`, `Particle`,
…). Systems are gated by a `GameState` (`Menu / Story / Briefing / Playing / …`) and a
`Paused` flag; the simulation tuple runs under `in_state(Playing).and_then(not_paused)`.

## Build & deploy pipeline

- **Nix flake** (`flake.nix` + `.envrc`/direnv) provides the toolchain: Rust, the
  matching `wasm-bindgen-cli` (pinned to `wasm-bindgen = "=0.2.121"` in `Cargo.toml`),
  `wasm-opt`, clang. Enter with `nix develop`.
- **`build-web.sh`** (run as `nix develop --command ./build-web.sh`):
  1. `cargo build --target wasm32-unknown-unknown --release` (profile: `opt-level="z"`,
     `lto="fat"`, `codegen-units=1`, `panic="abort"`, `strip`).
  2. `wasm-bindgen` → `web/`.
  3. `wasm-opt -Oz` (size pass).
  4. copy `assets/` → `web/assets/` (served over HTTP by the `AssetServer`).
  5. gzip + brotli precompress `*.wasm` / `*.js`.
- **Serve:** `python3 -u tools/serve_gzip.py 8443` — HTTPS + gzip-aware static server
  (serves the precompressed `.gz` with `Content-Encoding: gzip`).
- **Loading screen** (`web/index.html`) fetches `protect_carrot_bg.wasm.gz` directly to
  show real download progress (TransformStream), decompresses in-browser
  (`DecompressionStream`), then waits for the Rust side to signal `carrot_game_ready`
  before hiding — so the loader never fades into a blank frame.

Stale-build guard: always confirm `target/wasm32-unknown-unknown/release/protect_carrot.wasm`
timestamp advanced after a build.

## Source modules (`src/`)

| Module | Role |
|---|---|
| `main.rs` | App/plugin/system registration, schedules, state wiring, game-ready signal |
| `states.rs` | `GameState` enum |
| `data.rs` | Board constants, `TowerKind`/`EnemyKind`/`Element`/`Behavior` tables, levels, `cell_center` |
| `game.rs` | `RunState`, level load, board/carrot/portal draw, `Rng`, keyboard, speed (1/2/4/8×) |
| `board.rs` | `Board` (walkable path + buildable set), spawn point |
| `tower.rs` | `Tower`, `Snapshot` (targeting), `update_towers` (behavior dispatch), projectiles, summons, **GodTower**, damage/status resolution, adjacency synergy |
| `enemy.rs` | `Enemy`, spawn, path-follow + hero aggro, death/rewards/bounty, boss specials, facing (for backstab) |
| `monster.rs` | `MONSTER_SPECIES` (100 species), boss skills/portraits |
| `hero.rs` | `Race`/`Class`, **Doctrine** (per-class passive), `HeroLoadout`, talents + lvl-30 ultimate, `apply_loadout_to_tower`, `hero_doctrine` aura, persistence |
| `build.rs` | Placement/selection, `spawn_tower`/`spawn_hero`, hero movement, **afterimage**, **hero walk animation** (`HeroWalks`/`animate_hero_walk`), god-tower summon, render/flip/bob |
| `equipment.rs` | Equipment items, set bonuses, stat application |
| `meta.rs` | Global tower talents; **Abilities** (Meteor/Freeze/GoldRush) + `cast_abilities` |
| `vfx.rs` | `VfxEvent` + `spawn_vfx`, particles/shockwaves, screen shake, **full-screen ability FX** (MeteorStorm/GoldExplosion/FrostNova) |
| `creatures.rs` | Animated enemy/summon sprite-sheets (`TextureAtlas`): **locomotion + attack** sheets, switched on `Enemy::blocked` |
| `sprites.rs` | `Sprites` handle tables; `build_sprites` (built before `app.run()` to win the OnEnter race) |
| `audio.rs` | SFX + BGM |
| `quality.rs` | Graphics quality (MSAA) tiers; resolution is adaptive |
| `bestiary.rs` | Kill tracking / first-sighting discovery |
| `components.rs` | Shared components (`Enemy`, `Particle`, `Summon` markers, …) |
| `ui.rs` | All HUD / menu / briefing / tooltips / loadout dock / settings / mobile left-rail + joystick |

## Assets

- Format: **WebP** (converted from PNG via `tools/to_webp.sh`, ~67% smaller; barely
  gzip-compressible PNGs were the bulk of first-load weight). Story portraits are WebP.
- Layout: `assets/sprites/{towers,enemies,species,equipment,abilities,talents,heroes,
  hero_skills,hero_talents,bosses,races,ui,backgrounds}`, plus `creatures/` (animation
  sheets), `heroes_walk/` (hero walk-cycle strips), `audio/`, `fonts/`, `story/`.
- **Sprite-sheet animation**: horizontal strips of square frames (frame size = sheet
  height, count = width/height), played via Bevy `TextureAtlas`. Used for monsters
  (locomotion + attack), summons, and the 9 hero walk cycles (`[idle, walk1..walkN]`).
- Large binaries tracked with **Git LFS** (`.gitattributes`: png/jpg/webp… via LFS).

## Asset-generation pipeline (AI-assisted, `tools/`)

A remote GPU box runs **ComfyUI** (systemd `comfyui.service`, models on `/data`),
reached over an SSH tunnel (`ssh -N -L 8188:127.0.0.1:8188 root@<host>`).

| Tool | Purpose |
|---|---|
| `comfy_gen.py` | Flux.1-dev txt2img — icons, portraits, props (e.g. `meteor_fx`) |
| `comfy_kontext.py` | **Flux.1-Kontext** image-edit — keep the SAME character, change pose (hero walk frames) |
| `comfy_img2img.py` | Flux img2img (low-denoise variations) |
| `gen_hero_walks.sh` | Full hero walk pipeline: Kontext frames → black-frame retry → normalize (bg-remove/trim/feet-to-bottom/square) → assemble strip |
| `to_webp.sh` | Convert any `assets/**/*.png` → WebP (idempotent) |
| `import_monster_pack_sprites.py` / packs in `New Folder With Items/` | Source the monster locomotion + attack sheets |
| `gen_sfx*.sh`, `gen_story_voice.py` | Audio (SFX/voice) |

ComfyUI models in use: `flux1-dev-fp8` (txt2img) and `flux1-dev-kontext_fp8_scaled`
(+ `clip_l`, `t5xxl_fp8_e4m3fn`, `ae` VAE) for character-consistent pose editing.

> ⚠️ Some `tools/` scripts historically took API keys (PixelLab / ElevenLabs / Retro
> Diffusion). **Never hardcode keys** — pass via env vars at runtime only, and rotate
> any that were ever exposed.

## Multiplayer (optional, not in this repo)

Co-op over a WebSocket server (default `ws://localhost:8765`). The host simulates and
broadcasts `sync_state`; peers reconcile by enemy `id`. The server is external; the
client speaks a JSON protocol. (Single-player is the primary mode.)
