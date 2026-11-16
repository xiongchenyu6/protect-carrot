# Demand downloads and mobile delivery verification

The launcher downloads only the menu/core pack. Selecting campaign or endless play
prepares one shared combat pack, verifies every SHA-256 digest, then waits for real
PixelFX decoding before entering gameplay. Returning to the menu releases the pack
pin. Updates require acceptance and reuse unchanged content hashes; changed files
are replaced whole (this is not binary patching). Browser storage eviction still
requires redownloading the affected content.

## Automated checks

- Native `cargo check --all-targets` and `cargo fmt --check` pass.
- Release WASM build passes, including wasm-bindgen and wasm-opt.
- 14 Python packaging/manifest/retention/engine reuse tests pass.
- 10 real-browser cache/update/integrity tests and 11 demand/cancel/offline tests pass.
- Launcher fixture verifies pre-load language, installation guidance, first install,
  cancellation and the real-readiness gate. The engine in that test is a fixture;
  the screenshots below use the full WebGPU game.
- Existing audio bootstrap tests pass (2 tests).

## Actual game evidence

`tools/verify_web_game.cjs` uses headed Chromium with SwiftShader Vulkan on Xvfb.
The 844×390 touch run clicks the visible settings gear and downloads entry, swipes
both settings and chapter list, and asserts the canvas does not move. It also
asserts canvas bitmap dimensions agree with CSS dimensions × device pixel ratio.
This reproduces and fixes the former touch mismatch (884×409 backing store for an
844×390 visible viewport). No scaled click-coordinate workaround is used.

- `menus-mobile.png`: readable touch chapter menu.
- `settings-mobile-top.png`, `settings-mobile-bottom.png`: scrollable settings.
- `downloads-mobile.png`: downloads manager reached through the real settings row.
- `menu-mobile-scrolled.png`: lower chapter rows and menu actions reachable.
- `prepare-desktop.png`: explicit shared battle download.
- `briefing-desktop.png`: actual chapter-one battle after preparation.
- `offline/briefing-desktop.png`: full browser restart offline reaches cached battle.
- JSON companions record requests and JavaScript errors; logs record test output.

This is browser touch emulation, not certification on a physical Android handset.
Public Pages release `65b419a4fe7e5121cf8b87a364fc9d25af7c5dd6` is verified.
The final mobile/desktop and offline JSON reports now record the public URL;
all report zero JavaScript errors. The offline briefing screenshot was reviewed:
the cached chapter scene, HUD and tutorial render after a full browser restart.
`public-release.json` records matching hosted build/manifest revisions and
independently downloaded immutable resource hashes. Physical phone testing remains outstanding.
