# Repository Guidelines

## Project Overview

This is a Rust/Bevy 0.19 tower-defense game, "保卫萝卜" / "Protect the Carrot",
ported from the legacy single-file browser game in `保卫萝卜.html`.

Treat the Rust/Bevy implementation under `src/` as the active project. The legacy
HTML file remains as source/reference material. `README.md` is the current
high-level guide; `CLAUDE.md` describes the earlier single-file version and is not
authoritative for the Bevy port.

## Development Environment

Use the Nix dev shell so Rust, Bevy native dependencies, wasm tools, and asset
helpers are available:

```bash
direnv allow
# or
nix develop
```

Common commands:

```bash
cargo check
cargo run
cargo run --release
./build-web.sh
python3 -m http.server -d web 8080
./tools/serve_https.sh 8443
```

For web builds, keep the `wasm-bindgen` crate version in `Cargo.toml` matched with
the `wasm-bindgen-cli` version supplied by `flake.nix`.

## Code Layout

- `src/main.rs` wires Bevy plugins, resources, states, messages, and system order.
- `src/data.rs` holds tower, enemy archetype, level, lore, and elemental tuning data.
- `src/monster.rs` holds the 100-species monster catalog, boss skill mapping, and
  spawn-pool logic.
- `src/equipment.rs` holds the 20-item equipment catalog, inventory, drops, and
  tower-equipping logic.
- `src/components.rs` contains ECS components.
- `src/states.rs` defines the app state machine.
- `src/game.rs` owns per-run state, difficulty, level loading, pause, speed, and
  wave control.
- `src/board.rs` builds level paths and buildable cells.
- `src/enemy.rs`, `src/tower.rs`, and `src/build.rs` implement simulation,
  combat, placement, selection, upgrades, and selling.
- `src/ui.rs` owns menus, HUD, overlays, tooltips, and panel interactions.
- `src/audio.rs`, `src/sprites.rs`, `src/creatures.rs`, `src/vfx.rs`,
  `src/meta.rs`, and `src/bestiary.rs` provide presentation and progression
  systems.

## Conventions

- Prefer existing ECS patterns: resources for shared state, components for entity
  data, messages for combat/effects, and state-gated systems.
- Keep simulation systems deterministic and framerate-independent by using Bevy
  time deltas and the existing `RunState.game_speed`.
- Keep Chinese UI strings consistent with the current game text.
- Add tuning/content changes in `src/data.rs` when possible instead of scattering
  literals through systems.
- Add new monster identities in `src/monster.rs`; keep `EnemyKind` as the compact
  behavior/art archetype layer unless a new mechanic truly needs a new archetype.
- Boss-wave cadence and boss species selection live in `src/monster.rs`; keep the
  numeric interval tuning in `src/data.rs`.
- Use `tools/gen_sprites_storyos_comfy.mjs` for StoryOS/ComfyUI sprite batches.
  `species` generates 100 species portraits; `full` also regenerates tower and
  enemy-archetype sprites.
- Use `tools/gen_species_sprites.py` for offline deterministic placeholders when
  ComfyUI is unavailable. It writes the same `assets/sprites/species/*.png` files
  that ComfyUI later overwrites.
- Do not casually regenerate or overwrite `assets/`, `web/`, `target/`, `tmp/`, or
  generated wasm files unless the task is specifically about those outputs.
- Use `cargo fmt` for Rust formatting before handing off substantial Rust edits.

## Verification

For normal Rust changes, run:

```bash
cargo check
```

For behavior that affects web packaging or asset loading, also run:

```bash
./build-web.sh
```

Then serve `web/` and verify in a WebGPU-capable browser. `localhost` is a secure
context, and `tools/serve_https.sh` serves pre-gzipped wasm/js when testing over
HTTPS.
