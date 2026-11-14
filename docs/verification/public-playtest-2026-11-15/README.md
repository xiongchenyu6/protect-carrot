# Public playtest deployment — 2026-11-15

- URL: https://xiongchenyu6.github.io/protect-carrot/
- Runtime source: `4feb04dd779f0ec8743495cd577d28d5d953d86c`.
- Build workflow: https://github.com/xiongchenyu6/protect-carrot/actions/runs/43612985750 (success).
- Pages deployment: https://github.com/xiongchenyu6/protect-carrot/actions/runs/43613546573 (success).
- Live `build.json` revision matches the source. WASM SHA256 `99c35c078f48ec77c6f26c1ffb83cb76a119dc34cb5776dcc9018bd4167606a4`, 36,172,463 bytes.

## Verification

- `cargo fmt --check`: passed.
- `cargo check --locked --all-targets -j2`: passed.
- `cargo test --locked --lib -j2 -- --test-threads=2`: 78 passed.
- `cargo test --locked --test hero_passives -j2 -- --test-threads=2`: 45 passed.
- `node --test tests/web_audio_unlock.mjs`: 2 passed.
- Actual published game: entered the menu, selected level 1, advanced the prologue and selected its defensive route, entered combat, started wave 1, observed enemies and hero attacks, and built an arrow tower. Wallet changed 204 → 154 and the tower's level/upgrade panel appeared.
- Captured desktop 1280×720 and landscape 844×390; JavaScript error report empty. This is an opening smoke test, not a full campaign or touch-device certification. The tutorial overlay was still present in the final capture; dismissal was not certified.

## Browser runner

Headless Chromium's software WebGPU rendered blank captures despite valid command submission. The actual game rendered correctly in a headed Chromium session under Xvfb; no game shader or rendering downgrade was committed. Temporary shader diagnostic changes existed only in an untracked local HTML page, never in the deployed game.

Successful runner: Xvfb `:95`, `DISPLAY=:95`, empty `WAYLAND_DISPLAY`, `NIXOS_OZONE_WL=0`, headed agent-browser, and consistent newline-separated `AGENT_BROWSER_ARGS` on **every invocation**:

```text
--ozone-platform=x11
--enable-unsafe-webgpu
--use-angle=vulkan
--use-vulkan=swiftshader
--enable-features=Vulkan
--disable-vulkan-surface
```

Changing launch environment on subsequent agent-browser invocations can relaunch Chromium and discard these settings. Local full logs: `tmp/deploy-all-nov14/verify.log`; screenshots here are from the unmodified public Pages URL.
