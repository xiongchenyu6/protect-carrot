# Protect the Carrot: demand updates and mobile delivery

Scope: SHA-256 verified, explicit incremental updates; core menu versus shared battle
content; cold offline play; reusable compiled engine; install guidance; pre-load
language selection; stable mobile viewport/touch mode; real desktop/mobile proof.

Implemented and locally verified:
- Source-owned launcher, manifest and localized install/update/download dialogs.
- Heavy asset read gate and level-selection preparation. Battle effects must load
  before transitioning. Removed the 240-frame readiness bypass.
- Shared pack for campaign/endless; no duplicated per-level art downloads.
- Publish only referenced PixelFX clips; compressed background music to Vorbis.
- Input fingerprint and verified published-engine reuse including bindgen snippets.
- Touch layout selected from device capability before the first gesture.

Release acceptance:
- [x] Native/WASM checks; cache/hash/cancel/old-version retention tests.
- [x] Real desktop game reaches battle; 844x390 touch menu, settings and downloads.
- [x] Full browser restart offline reaches cached menu and battle.
- [ ] Published receipt and asset hashes match tested commit; changes committed/pushed.

Existing untracked .agents/, .codex/ and screenshots/ are not part of this work.
