# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

A single-file browser tower-defense game (保卫萝卜 / "Protect the Carrot"). The entire game — HTML, CSS, and JavaScript — lives in `保卫萝卜.html`. There is no build step, no bundler, no dependencies, and no test suite. The game runs by opening the file in a browser.

UI strings and code comments are in Chinese; keep that convention when editing.

## Running

- Open `保卫萝卜.html` directly in a browser, or serve the directory (e.g. `python3 -m http.server`) and load the file.
- The Nix flake (`flake.nix` + `.envrc` with direnv) provides a Rust/clang dev shell. It is **not** used by the game — it's scaffolding for a future/companion native component (e.g. a multiplayer server). The browser game needs none of it.

## Architecture

Everything is in `保卫萝卜.html`: `<style>` (lines ~7–495), markup + `<canvas id="gameCanvas">` (~496–572), and one `<script>` (573–end). State is held in module-level mutable globals (`gameState`, `gold`, `lives`, `wave`, `enemies`, `towers`, `projectiles`, etc.) — no framework, no classes for game objects (plain object literals pushed into arrays).

### Game loop
`gameLoop(timestamp)` (~1684) is the `requestAnimationFrame` driver. Each frame it computes `dt`, then (when `gameState === 'playing'`) calls the update functions in order — `updateEnemies`, `updateTowers`, `updateSummons`, `updateProjectiles`, `updateParticles` — followed by `render()` and `updateUI()`. `gameSpeed` (1x/2x) scales `dt`. `gameState` is the central mode flag: `'menu' | 'playing' | 'paused' | 'gameover' | 'victory'`.

### Data tables (edit these to tune/extend content)
All near the top of the script, before any functions:
- `TOWER_TYPES` (~588) — every tower keyed by id, with `cost`, `damage`, `range`, `speed` (cooldown ms), `category`, and a behavior `type` (`single`/`aoe`/`chain`/`laser`/`homing`/`slow`/`knockback`/`freeze`/`curse`/`heal`/`detect`/`poison`/`fire`/`summon`). `TOWER_CATEGORIES` (~581) groups them into the build-menu tabs.
- `UPGRADE_MULTIPLIERS` (~614) — per-stat multipliers applied in `doUpgradeTower`.
- `ENEMY_TYPES` (~626) — keyed enemy archetypes with `hpMod`/`speedMod`/`rewardMod`/`armor`/`magicResist` and flags (`flying`, `invisible`, `regen`, `boss`). `BOSS_WAVES` lists waves that spawn a boss.
- `LEVELS` (~641) — 20 levels, each a `path` of `[col,row]` grid waypoints plus base enemy stats. The grid is `COLS`×`ROWS` (20×15) of `TILE_SIZE` 40px. `generatePathAndBuildable` (~890) turns the waypoints into the walkable path and the set of buildable tiles.

A tower's behavior is dispatched on its `type` field inside `updateTowers` (~1308); adding a new mechanic means adding a `type` and handling it there (and in `updateProjectiles`/`chainLightning`/`updateSummons` if it spawns those). Damage resolution funnels through `applyDamage` (~1267), targeting through `canTarget`/`getNearestEnemy`; invisible enemies are gated by `isDetected` (require a detection tower in range).

### Persistence
Progress (unlocked levels) is saved to `localStorage` under `carrot_defense_save_v2` via `saveProgress`. Multiplayer server URL and player name persist under `carrot_defense_server` / `carrot_defense_name`.

### Multiplayer
Optional co-op over a WebSocket server (default `ws://localhost:8765`, configurable in the UI). **The server is not in this repo** — the client speaks a JSON protocol to an external server.

- Connection + lobby flow: `mpConnect` (~2112), then `mpCreateRoom`/`mpJoinRoom`/`mpToggleReady`/`mpStartGame`.
- Outgoing messages go through `sendWs` (~2200): `create_room`, `join_room`, `set_ready`, `start_game`, `start_wave`, `build_tower`, `upgrade_tower`, `sell_tower`, `sync_state`, `chat`, `game_over`, `victory`.
- Incoming messages are handled in `handleWsMessage` (~2206): `room_created`, `room_joined`, `player_joined/ready/left`, `host_changed`, `game_started`, `wave_started`, `tower_built/upgraded/sold`, `state_sync`, `game_over`, `victory`, `chat`, `error`.
- Authority model: the **host** simulates enemies/economy and broadcasts `sync_state` (`sendStateSync`, ~1733) ~every frame; non-hosts apply it in the `state_sync` case (reconciling enemies by `id`) and otherwise run locally. Tower actions are echoed to peers and replayed via `buildTowerAt`/`doUpgradeTower`/`doSellTower` guarded by `msg.player_id !== myPlayerId` so the originator doesn't double-apply. If you change enemy/economy fields, update both `sendStateSync` and the `state_sync` reconciliation to keep them in sync.

## Conventions

- No build/lint/test tooling — verify changes by loading the file in a browser and playing. There is nothing to compile.
- Keep the game self-contained in the single HTML file unless deliberately introducing a separate asset/server.
- Match the existing style: terse object-literal data tables, Chinese comments/UI strings, behavior dispatched on string `type` fields.
