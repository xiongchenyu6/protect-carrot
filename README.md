# 保卫萝卜 · Bevy 移植版 (Protect the Carrot, in Bevy)

A from-scratch reimplementation of the single-file browser game `保卫萝卜.html`
in the [Bevy](https://bevyengine.org) game engine (Rust, **Bevy 0.18**), built as a
**learning project** — single-player, with a **WebGPU/wasm** build for the browser.

The original HTML game still lives in `保卫萝卜.html`; this Rust port is a parallel,
idiomatic ECS version you can read alongside it.

---

## Quick start

All commands run inside the Nix dev shell (provides Rust, the wasm toolchain, and
Bevy's native libs):

```bash
direnv allow          # or: nix develop
```

### Native (desktop)

```bash
cargo run             # debug
cargo run --release   # smooth
```

### Web (WebGPU)

```bash
./build-web.sh                          # compiles wasm + runs wasm-bindgen -> web/
python3 -m http.server -d web 8080      # then open http://localhost:8080
```

WebGPU needs a secure context (localhost counts) and a WebGPU-capable browser
(Chrome/Edge 113+, or Firefox with the flag). Bevy makes the `webgpu` feature take
precedence over the default WebGL2 backend, so `--features webgpu` is all it takes
(already wired into `build-web.sh`).

> The wasm CLI (`wasm-bindgen-cli`) and the `wasm-bindgen` crate **must be the same
> version**. The flake pins the CLI and `Cargo.toml` pins the crate to match
> (`=0.2.121`). If you bump one, bump the other.

---

## Controls

| Input | Action |
|-------|--------|
| Click a tower button (right panel) | choose a tower to build |
| Left-click a buildable tile | build the chosen tower |
| Left-click a placed tower | select it (shows range) |
| Right-click / Esc | cancel build / deselect |
| `U` / 升级 button | upgrade selected tower (max Lv3) |
| `R` / 修理 button | repair selected tower to full HP |
| `Tab` | select the most damaged tower for emergency repair |
| `T` / 目标 button | cycle selected tower target priority |
| `Z` / 卸装 button | unequip all relics from the selected tower |
| `X` / 出售 button | sell selected tower and return equipped relics |
| Space / 开始波次 | start the next wave |
| `A` / 自动波 button | toggle auto-start with a short between-wave countdown |
| `P` / 暂停 | pause |
| `F` / 倍速 | cycle speed 1× → 2× → 3× |
| `Q` / `W` / `E` | cast meteor, full-field freeze, or gold-rush sacrifice |
| Number keys `1`–`0` | quick-pick the first ten tower types |

---

## How it maps to Bevy (the learning part)

The original game keeps everything in module-level mutable globals and one big
`requestAnimationFrame` loop. The Bevy port splits that into **data**, **components**,
**resources**, **systems**, **states**, and **messages** — the core ECS vocabulary.

| File | Role | Bevy concept |
|------|------|--------------|
| `data.rs` | tower/enemy archetype/level/lore tables | plain `const`/`static` design data |
| `monster.rs` | 100 monster species layered over enemy archetypes | content catalog + spawn pools |
| `equipment.rs` | 20 named equipment drops across rarities | loot/inventory tuning data + sprite keys |
| `components.rs` | `Enemy`, `Carrot`, `LevelEntity` | `#[derive(Component)]` data on entities |
| `board.rs` | path + buildable cells for a level | a `Resource` |
| `game.rs` | gold/lives/wave (`RunState`), `Paused`, RNG, level load, waves | `Resource`s + `OnEnter` system |
| `states.rs` | `Menu / Playing / GameOver / Victory / Bestiary / Armory / TowerArchive / Milestones / CampaignDossier` | `#[derive(States)]` + `run_if(in_state(..))` |
| `enemy.rs` | spawning, path-following, status/DOT, death | `Update` systems gated on state |
| `tower.rs` | tower behaviors, projectiles, summons, tower HP/siege combat | systems + **`Message`** events |
| `build.rs` | place/select/upgrade/sell, range gizmos | mouse/keyboard input, `Gizmos` |
| `ui.rs` | HUD, build palette, menu, win/lose overlays | `bevy_ui` `Node` trees + `Button` |
| `main.rs` | wires plugins, resources, system order | the `App` |

### Patterns worth studying

- **Combat as messages.** Instead of mutating enemies in-place from towers,
  projectiles, and summons (as the JS does), those systems emit `Damage` / `Status`
  messages, and two small systems (`apply_damage`, `apply_status`) apply them. This
  keeps each system's data access simple and avoids query aliasing. (Bevy 0.18
  renamed buffered events to **Messages**: `MessageWriter` / `MessageReader` /
  `add_message`.)
- **A per-frame `Snapshot`.** Targeting needs to read all enemies/towers while the
  same frame mutates them. `build_snapshot` collects a read-only view first, so the
  mutating systems never alias the live queries.
- **State-driven flow.** Level load runs on `OnEnter(Playing)`; the HUD spawns there
  and is despawned on `OnExit(Playing)`. Pause is a **separate `Paused` resource**
  (not a state) precisely so toggling it doesn't re-trigger `OnEnter(Playing)` and
  reload the level — a good cautionary example.
- **Time, not frames.** The JS scaled by `dt/16`; here every system uses
  `time.delta_secs() * game_speed`, so behavior is framerate-independent.

### Content scope

Current scope: **19 towers** across attack/control/support/special categories,
**16 enemy behavior archetypes**, **100 named monster species**, **20 named equipment
drops**, elemental damage/resistance, tower HP/armor, tower-raider enemies, silencer
enemies, MOSS-style tower-eating bosses, ten species-specific boss skills, and
**20 levels** with their paths and Lovecraftian lore. Two JS quirks were
deliberately *not* reproduced (they read like bugs): permanent `baseSpeed` decay
from stacked slows, and double-subtracted curse armor — here slow is a timer and
curse armor reduction is applied once as an effective modifier.

Boss waves now happen every fifth wave and on each level's final wave, with the
late campaign biased toward newly unlocked bosses and the final level's last wave
reserved for the sealed sleeping god.

Build buttons show each tower's element marker, and tower/equipment hover tooltips
summarize which catalog monsters are notably weak or resistant to that element.
Fire towers now ignite enemies hit by the initial blast and leave a lingering
burning patch, while both burn paths respect fire resistance.
Shielded enemies and bosses show blue shield bars above HP, making absorbed damage
visible during fights.
The carrot seal itself has a board-space life bar, hit pulse, breach ring, and
low-life warning text, so leaks feel like base damage instead of only a HUD number
changing.
Selected towers show their three equipment slots, and selling a tower returns its
socketed equipment to the inventory.
Selected towers can cycle target priority between nearest, front, strongest,
weakest, and boss/siege threats with `T` or the target button, making mixed late
waves more controllable.
Selected tower panels also show per-run damage dealt and kills, including
poison/burn attribution, so upgrade and targeting decisions have immediate
feedback.
Towers also render board-space HP bars, so climbers, MOSS, and other siege
pressure are visible without needing to keep the tower selected.
Damaged towers can be repaired with gold from the selected-tower panel or `R`.
Tower HP bars shift from green to amber to red as danger rises, and active siege
damage emits throttled sparks, `攻塔` numbers, and a one-shot `防御塔濒危`
warning before a tower collapses.
Tower-raider enemies actively leave the lane toward nearby towers before chewing
through tower HP; MOSS senses farther and can still devour the first tower it
reaches.
Tower-raiders and MOSS draw danger links to their current tower target plus a
threat ring around the endangered tower.
Silencer enemies render purple suppression rings; affected towers tint purple and
show `静默中` when selected.
Healer enemies render green healing rings and emit quiet restoration sparks when
they actually restore allied HP.
Bosses now telegraph special skills with colored wind-up rings, cast-name text,
and an audio cue before the skill resolves.
Tower-threatening boss casts also outline each endangered tower and draw threat
links during wind-up; when they land, impacted towers show `首领` damage or
`停火` feedback.
Boss enemies keep persistent name/skill labels and use red boss HP bars, so they
remain identifiable after the wave banner fades.
During active boss fights, the board HUD also shows a dedicated boss HP bar plus
a skill charge/cast meter, so cooldown pressure remains readable in crowded waves.
At low HP, bosses enter a visible `狂怒` phase: they shed control, gain a shield,
move faster, charge skills more quickly, pulse red, shake the world, and update
the HUD status/cooldown line.
Summoned and necromancer-raised allies use archetype-tinted sprites and compact HP
bars, making front-line blocking and ally losses readable during crowded waves.
The HUD includes wave intel for the active or next wave: notable monsters, special
traits, boss skill details, resistances, and recommended counter-elements.
Auto-wave mode can be toggled with `A` or the side-panel button; it keeps a short
visible countdown after each cleared wave before starting the next one.
Short run notices are queued, so rapid drops, repair results, wave warnings, and
boss callouts display in order instead of overwriting each other immediately.
Later waves can roll elite mutations such as 狂乱, 硬壳, 黄印, 血祭, and 攻城;
the wave intel lists unlocked mutations with short behavior descriptions.
Elite enemies keep compact mutation labels above their HP bars, so the affix
remains identifiable after the spawn callout fades.
Regular special enemies keep compact trait badges such as `攻城`, `静默`, `治疗`,
`护盾`, and `分裂` above their HP bars, so non-elite utility threats remain
readable during crowded waves.
Runtime enemy sprites use procedural breathing, flying lift, boss pulses, status
tints, and phasing alpha, so the 100 static species portraits feel alive in
combat without needing full animation sheets yet.
Rapid kills build a visible `连杀` meter with a countdown, gold bonus milestones
every five kills, and a dedicated reward burst so good tower setups feel
immediately profitable.
Flawless waves add a visible `完美防守` gold bonus at the carrot seal, turning
clean path control into a readable payout moment.
Explosions, boss casts/deaths, and rare relic drops add a short world-camera
shake, keeping high-impact moments physical while the UI stays stable.
Active skills on `Q/W/E` now create board-space impact VFX: meteor marks its
blast and hit count, freeze flashes the whole field, and gold rush shows the
instant payout; cooldown failures report remaining seconds.
Equipment buttons show live inventory counts and dim to zero-stock labels, so
socketing decisions are visible without opening a separate inventory screen.
The in-run equipment palette also renders relic icons, with unavailable items
visually dimmed.
Selected towers show live socket icons for their three equipped relic slots, so
loadouts are readable while tuning or replacing gear; clicking one socket removes
only that relic and returns it to inventory.
The main menu includes a tower archive with all 19 towers, sprites, element
types, costs, combat stats, durability, behavior roles, and counter examples.
The main menu includes an equipment armory that lists all 20 relics, owned
counts, stat lines, rarity colors, drop sources, set-bonus rules, and duplicate
refinement controls that convert 3 matching relics into 1 higher-rarity relic.
The main menu also includes a read-only seal achievement screen with derived
milestones for campaign ratings, full bestiary discovery, boss kills, total
clears, equipment variety, deep inventory, mythic relics, and tower archive
completion.
The campaign dossier screen lists all 20 fronts with unlock/rating status,
Lovecraftian level lore, economy/wave stats, boss-wave forecasts, likely boss
skills, elite mutation forecasts, and recommended counter-elements.
Equipment now has derived set bonuses: matching elemental conversions trigger
resonance damage, while high-grade relic sets add effective anti-siege armor.
Towers with active equipment set bonuses draw board-space resonance auras, using
the resonant element color or a gold high-grade ring so tuned builds are visible
without selecting the tower.
Socketed equipment is returned to inventory when towers are sold, destroyed, a
run ends, or level entities are cleared, so persistent gear is not silently lost.
Equipment tooltips explain rarity-specific drop sources: normal monsters scale
with wave, elites are high-value targets, and bosses always drop gear.
Combat equipment drops show the actual relic icon in the world with a rarity
ring, sparks, sound, and floating label, so loot pickups read as tangible rewards
instead of only banner text.
The first time each monster species enters a run, the spawn point emits a portrait
ring and `新威胁` callout using that species sprite and trait summary, while the
persistent bestiary still only records actual kills.
First-time monster discoveries also show the discovered species portrait with a
purple codex ring and floating `图鉴更新` label, making bestiary unlocks visible
inside combat instead of only after returning to the menu.
Damage numbers now label meaningful elemental matchups with `易伤` or `抗性`,
making tower-element choices readable during combat.
Unlocked levels, equipment inventory, and bestiary kill counts persist across
sessions. Victory awards 1-3 seal ratings based on remaining lives, stores the
best rating per level, and grants a persistent seal chest whose item count and
rarity odds improve with rating, difficulty, and campaign depth. The victory
overlay renders those chest rewards as rarity-colored relic icon cards. Native dev
builds write `tmp/progress_unlocked.txt`,
`tmp/progress_stars.txt`, `tmp/equipment_counts.txt`, and `tmp/bestiary_counts.txt`,
while web builds use browser `localStorage`.

### Asset generation

Use StoryOS's ComfyUI workflow for high-volume sprite generation:

```bash
COMFY_BASE_URL=https://your-comfy.example/api/comfy/v1 \
COMFY_API_TOKEN=optional-token \
node tools/gen_sprites_storyos_comfy.mjs species
```

Modes: `towers`, `enemies`, `equipment`, `species`, `all` (tower + enemy
archetypes + equipment), and `full` (tower + enemy archetypes + equipment + 100
species portraits). The script reads
`crates/storyos-image/src/workflows/flux2_klein.json` from the local StoryOS repo;
set `STORYOS_ROOT=/path/to/storyos` if it lives elsewhere.

Review prompts before spending a large Comfy batch:

```bash
node tools/gen_sprites_storyos_comfy.mjs manifest full
```

This writes `tmp/comfy/prompt_manifest.json` without requiring `COMFY_BASE_URL`.
It also writes `tmp/comfy/prompt_report.md` for quick prompt review.

For offline development, generate deterministic placeholder species/equipment
sprites first:

```bash
.venv/bin/python tools/gen_species_sprites.py
.venv/bin/python tools/gen_equipment_icons.py
```

Both paths write `assets/sprites/species/000.png` through `099.png`; the StoryOS
ComfyUI output can overwrite the placeholders without code changes. Equipment
icons live under `assets/sprites/equipment/`.

After any large sprite batch, build review sheets:

```bash
.venv/bin/python tools/make_sprite_contact_sheet.py all
.venv/bin/python tools/verify_sprite_assets.py
.venv/bin/python tools/audit_release_content.py
```

This writes `tmp/species_contact_sheet.png`, `tmp/towers_contact_sheet.png`,
`tmp/enemies_contact_sheet.png`, and `tmp/equipment_contact_sheet.png` for quick
visual QA. The verifier writes
`tmp/sprite_manifest.json` plus `tmp/sprite_report.md`, and fails if any required
tower, enemy archetype, equipment, or species sprite is missing, unreadable,
nonsquare, tiny, or fully transparent. The release audit writes
`tmp/content_audit.json` plus `tmp/content_audit.md`, and checks catalog counts,
boss skills, collection screens, campaign dossier, equipment progression,
siege/silence systems, sprite coverage, and StoryOS/Comfy prompt coverage.

### Not ported (yet) — good exercises

- WebSocket co-op multiplayer (the original's `mp*` code; server isn't in the repo).
- More per-species animation sheets. Runtime enemies and the bestiary already use
  the 100 per-species static sprites under `assets/sprites/species/`; summons and
  raised units still use the compact creature-sheet archetypes.

---

## Project layout

```
保卫萝卜.html        # original single-file browser game (unchanged)
Cargo.toml          # Bevy 0.18 + webgpu feature + pinned wasm-bindgen
flake.nix           # dev shell: Rust, wasm-bindgen-cli, trunk, lld, Bevy native libs
build-web.sh        # wasm build + bindgen -> web/
web/index.html      # loads the wasm module (WebGPU)
src/                # the Bevy game (see table above)
IMPLEMENTATION_PLAN.md
```
