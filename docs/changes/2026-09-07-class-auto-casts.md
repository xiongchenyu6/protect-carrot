# Class-Specific Automatic Skills

## Goal

Replace attribute-only hero talents with complete, class-specific automatic casts. Keep stat-only build upgrades in Hex choices.

## Scope

- Three genuine skills per weapon: two trainable casts available at rank zero, and one automatically unlocked level-30 ultimate. Training improves only that cast, never baseline attacks, movement, health or armor.
- Independent cooldowns, visible windup, delayed gameplay release and recovery. No manual cast controls. Pause freezes casting; death and equipment changes cancel it; player movement prevents involuntary charge/blink.
- Kits: sword charge / sweeping slash / quake combo; fire staff flame corridor / frost prison / meteor shower; bow venom volley / piercing shot / arrow storm; shield bash / restoration / sanctuary; storm chain / vortex / thunderstorm; crossbow anchor refraction / binding bolt / sentry field; dagger ambush / poison knives / execution; summon staff mythic pact / fallen-soul recall / elder summon; hammer guard assembly / homing rockets / workshop deployment.
- Summoner skills only summon actual fighting allies. Shared owner capacity remains twelve, and recall consumes only souls actually raised.
- Summoner casts establish allies as soon as combat starts, without requiring enemies in range. Mythic and elder summons need capacity; recall also needs stored souls. Lone mobile allies keep an offset from their owner, and preparation time consumes neither summon lifetime nor cooldown.
- Remove old stat talents and stat-only ultimates, update skill tooltips, localization, Hex modifiers, save format and simulator/capture harness. Reuse available sprites and the existing sequential-action library.

## Assumptions

The user's autonomous-work instructions and approval of class-specific skills authorize implementation without another confirmation gate. Existing weapon identity, race, equipment, progression and non-skill doctrine remain in scope only where required for integration. Obsolete save formats are removed, not maintained as compatibility code.

## Observable Requirements

- WHEN a cast becomes ready with a valid situation, THEN the hero winds up before any gameplay effect, releases its class effect, and recovers before another cast or normal attack.
- WHEN a targeted skill has no valid target, or a summon skill has no capacity or required souls, THEN no empty cast or cooldown is consumed.
- WHEN combat starts with no nearby enemies, THEN rank-zero level-one mythic pact still winds up and creates a fighting ally. Recall does the same with stored souls, and elder summon does the same once unlocked at level 30.
- WHEN a lone mobile ally idles or follows its owner, THEN it keeps a separate visible formation position instead of merging into the hero sprite.
- WHEN preparing between waves, THEN summon lifetime and skill cooldowns both remain unchanged; both resume when combat resumes.
- WHEN paused or between waves, THEN cooldowns and cast phases do not advance.
- WHEN a hero dies, changes weapon or moves during a movement-displacing cast, THEN the pending cast cannot teleport or damage on behalf of the invalid owner.
- WHEN skill ranks change, THEN baseline health, armor, movement and basic-attack attributes remain unchanged.
- WHEN using any summoner skill, THEN fighting summons appear, without an unrelated direct damage or stat-buff substitute.

## Verification

- `nix develop -c cargo test --all-targets`: all existing relevant tests plus every class/slot effect, delayed release, cancellation, cooldown independence and summon capacity regressions.
- `nix develop -c cargo check --all-targets`, `cargo fmt --check` and content/asset audits.
- Native capture harness: skill panel plus windup/release/recovery screenshots, desktop and mobile; inspect actual images for nonblank sprites, class effects and overlap.
- Rebuild web output and serve the updated game; report browser verification separately from native evidence.

## Risks

Casting must cooperate with normal attack sequencing, paperdoll rendering and joystick movement. Multiple cooldowns affect capture/simulator observations. Old talent allocations no longer describe the same content and must not silently become unrelated skills.

## Recorded Evidence

- Full native regression before the intro-resource fix: `tmp/class-regression-final.log`, 83 tests passed across five suites. Includes all 27 releases and 30 skill integration tests.
- Final-source unit and integration regression: `tmp/class-final-unit-integration.log`, 81 tests passed across two suites (51 unit tests and 30 skill integration tests). Includes the new all-class/all-race intro atlas regression.
- `cargo check --all-targets`: `tmp/class-check-verified.log`, zero errors. Five warnings originate in the existing `vendor/bevy_firefly` dependency.
- Formatting and whitespace: `cargo fmt --check` and `git diff --check` passed.
- Content audit: `tmp/content_audit.json`, 39 checks passed, including 27 effect branches and skill icons, actual summoner units, no baseline talent bonuses, and 141 sprite assets.
- Final-source native automatic-cast capture: `tmp/class-desktop-release.log` and `tmp/class-mobile-release.log`; each completes all 27 skills and all 81 cast phases. Images are in `screenshots/class-skills/desktop-phases/` and `mobile-phases/`, with final skill panels in `desktop.png` and `mobile.png`.
- Image inspection: nine-class overview plus summoner windup, release, recovery and skill panels checked at 1280x720 and 844x390. All 162 final phase images are nonblank (minimum pixel standard deviation 8794.36). Summoner ultimate windup/release normalized pixel RMSE is 0.027352 desktop and 0.0245285 mobile.
- Presentation regressions found during capture were corrected: skill icons now follow the selected class and ultimate unlock, and the static paperdoll no longer replaces an animated battlefield hero. Summoner intro reuses its existing 768x768 world atlas because no separate summoner intro atlas exists.
- Final web package: `tmp/class-release-assets.log`, successful release build and packaging; wasm 25,928,815 bytes, gzip 9.4 MB, Brotli 6.2 MB. Served at `http://127.0.0.1:8765/`.
- Browser limitation: final-package loading, menu, prologue and summoner briefing were visibly verified in a fresh browser tab. The previous missing summoner-intro request no longer occurred. After deployment, both viewport and full-page screenshots timed out in `Page.captureScreenshot`; therefore browser combat rendering and automatic casts are not claimed as verified. Native screenshots are separate evidence, not browser evidence.

## Save Format

The runtime now reads only v4's 27 skill ranks. Old v3 browser hero saves reset to defaults; there is no compatibility parser. The existing native hero save was backed up to `tmp/hero-before-class-skills.txt` and converted to v4 while preserving its Orc/Sword identity, level 2, XP 16, one skill point and empty gear. Capture scenarios run from isolated temporary directories and do not overwrite the player's save.

## Summoner Follow-Up: 2026-09-08

The reported missing summons had three observable causes: readiness and release required nearby enemies, a lone mobile summon returned to the hero's exact center, and preparation consumed summon lifetime while freezing replacement cooldowns. Summoner readiness now checks capacity and, for recall, stored souls. A lone mobile ally uses its formation offset. Both lifetime decay and doctrine lifetime extension stop between waves; combat resumes their timers normally. The level-30 ultimate gate, twelve-unit capacity and automatic windup/release/recovery remain unchanged.

- PASS final-source regression: `nix develop -c cargo test --all-targets`, recorded in `tmp/summon-tests-verified-0908.log`; 87 passed, zero failed (51 library, three simulator and 33 skill integration tests).
- PASS no-target summons: `summoner_builds_a_frontline_before_enemies_enter_range` verifies every summoner slot with no enemies, dead enemies and distant enemies, including delayed release, exact unit counts, cooldown consumption and recovery. `recall_without_souls_waits_without_casting_or_consuming_cooldown` verifies the resource gate.
- PASS idle visibility and preparation: `lone_summon_stays_visible_beside_its_owner_and_survives_wave_preparation` checks mythic and recalled single allies following a moved owner without overlap, thirty seconds of preparation without lifetime or cooldown changes, then resumed combat timers.
- PASS `cargo check --all-targets` (zero errors, five existing vendor warnings), `cargo fmt --check` and `git diff --check`. The check log is `tmp/summon-check-0908.log`; a subsequent check also passed after the capture-only probe correction.
- PASS native capture at 1280x720 and 844x390: `tmp/summon-desktop-final-0908.log` and `tmp/summon-mobile-0908.log`. Each completes 27 automatic casts and 81 phase images. With no enemies in cast range, the summoner logs exactly one level-one mythic ally, two level-one recalled allies and three level-thirty ultimate allies. Every captured summoner has rank-zero skills and no equipment.
- PASS image inspection: actual allied sprites are visible in `screenshots/summon-fix-0908/desktop-phases/07-*-recovery.png` and the corresponding mobile images. All 162 phase images are nonblank. The first capture attempt was rejected because route projection moved distant probes into range; probes now remain frozen for this test. The failed attempt is retained in `tmp/summon-desktop-0908.log` and is not acceptance evidence.
- PASS web release build and packaging: `tmp/summon-web-0908.log`; wasm 25,928,821 bytes, gzip 9.4 MB, Brotli 6.2 MB. The existing preview at `http://127.0.0.1:8765/` serves the new wasm; its HTTP body and local file both hash to `002076b6180331512610a18017f45241b16845769b91bfa3280f8e893f11964e` (SHA-256).
- UNVERIFIED browser combat: isolated Chrome sessions recognize the NVIDIA WebGPU adapter, but game loading loses the device. Explicit Vulkan reports `VK_ERROR_OUT_OF_DEVICE_MEMORY`; default backend selection reports that the external instance reference no longer exists. Reproduced without native capture running. Logs: `tmp/summon-browser-vulkan-0908.log` and `tmp/summon-browser-default-0908.log`. `screenshots/summon-fix-0908/browser-desktop.png` records the stalled loader, not gameplay. Browser combat is not claimed as passing.

This follow-up used isolated native capture directories and browser sessions; it did not modify the player's save or source art assets.
