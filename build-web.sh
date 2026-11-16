#!/usr/bin/env bash
# Build the WebGPU/wasm version of 保卫萝卜 and stage it under web/.
#
# Run inside the dev shell (deps provided by flake.nix):
#   nix develop --command ./build-web.sh
# then serve and open:
#   python3 -m http.server -d web 8080   # http://localhost:8080
#
# WebGPU on the web currently requires a secure context (localhost counts) and a
# browser with WebGPU enabled (Chrome/Edge 113+).
set -euo pipefail

PROFILE="${1:-release}"
TARGET=wasm32-unknown-unknown
NAME=protect_carrot
WASM_BINDGEN_VERSION="${WASM_BINDGEN_VERSION:-0.2.126}"

wasm_bindgen_version_ok() {
  "$1" --version 2>/dev/null | grep -q "wasm-bindgen $WASM_BINDGEN_VERSION"
}

resolve_wasm_bindgen() {
  # Snapshot the matching CLI in this repository: another game may install a
  # different global wasm-bindgen while our Rust compiler is still running.
  local tool_root="$PWD/target/web-tools/wasm-bindgen-$WASM_BINDGEN_VERSION"
  local pinned="$tool_root/bin/wasm-bindgen"
  if [ -x "$pinned" ] && wasm_bindgen_version_ok "$pinned"; then
    printf '%s\n' "$pinned"; return
  fi
  local cargo_bin="${CARGO_HOME:-$HOME/.cargo}/bin/wasm-bindgen"
  local candidate
  for candidate in "$(command -v wasm-bindgen || true)" "$cargo_bin"; do
    if [ -x "$candidate" ] && wasm_bindgen_version_ok "$candidate"; then
      mkdir -p "$tool_root/bin"
      cp "$candidate" "$pinned.tmp"
      chmod +x "$pinned.tmp"
      if wasm_bindgen_version_ok "$pinned.tmp"; then
        mv "$pinned.tmp" "$pinned"
        printf '%s\n' "$pinned"; return
      fi
    fi
  done
  echo ">> installing isolated wasm-bindgen-cli $WASM_BINDGEN_VERSION" >&2
  cargo install -q --locked --root "$tool_root" wasm-bindgen-cli --version "$WASM_BINDGEN_VERSION"
  wasm_bindgen_version_ok "$pinned"
  printf '%s\n' "$pinned"
}

engine_inputs_sha256="$(python3 tools/web_engine_inputs.py)"
build_engine() {
rm -rf web/snippets
WASM_BINDGEN_BIN="$(resolve_wasm_bindgen)"

echo ">> cargo build ($PROFILE, WebGPU) for $TARGET"
if [ "$PROFILE" = "release" ]; then
  cargo build --bin "$NAME" --target "$TARGET" --release --features webgpu
  WASM="target/$TARGET/release/$NAME.wasm"
else
  cargo build --bin "$NAME" --target "$TARGET" --features webgpu
  WASM="target/$TARGET/debug/$NAME.wasm"
fi

wasm_bindgen_version_ok "$WASM_BINDGEN_BIN" || { echo "Pinned wasm-bindgen version changed" >&2; exit 1; }
echo ">> wasm-bindgen -> web/"
"$WASM_BINDGEN_BIN" --no-typescript --target web --out-dir web --out-name "$NAME" "$WASM"

# Release: shrink + optimize the wasm with binaryen's wasm-opt.
if [ "$PROFILE" = "release" ] && command -v wasm-opt >/dev/null; then
  BG="web/${NAME}_bg.wasm"
  before=$(du -h "$BG" | cut -f1)
  echo ">> wasm-opt -Oz (was $before)"
  # --all-features: modern rustc (1.82+) emits post-MVP ops (bulk-memory,
  # sign-ext, nontrapping-fptoint, ...) that wasm-opt rejects unless enabled.
  # wasm-opt occasionally fails transiently on this box (no .opt produced), so
  # retry up to 3× before giving up rather than aborting the whole build.
  opt_ok=0
  for attempt in 1 2 3; do
    rm -f "$BG.opt"
    # Compact imports currently require an experimental V8 flag in Chrome.
    if wasm-opt -Oz --strip-debug --all-features --disable-compact-imports -o "$BG.opt" "$BG" && [ -f "$BG.opt" ]; then
      mv "$BG.opt" "$BG"
      opt_ok=1
      break
    fi
    echo ">> wasm-opt attempt $attempt failed, retrying..."
    sleep 2
  done
  if [ "$opt_ok" != 1 ]; then
    echo ">> wasm-opt failed 3×; shipping the un-optimized wasm (functional, ~larger)"
  else
    echo ">> wasm-opt done: now $(du -h "$BG" | cut -f1)"
  fi
fi


gzip -n -9 -k -f web/protect_carrot_bg.wasm
}
if [ "$PROFILE" = "release" ] && [ -n "${WASM_REUSE_BASE_URL:-}" ] && python3 tools/reuse_web_engine.py --base-url "$WASM_REUSE_BASE_URL" --output web --fingerprint "$engine_inputs_sha256"; then
  echo "Unchanged compiled inputs: verified published engine reused."
else
  build_engine
fi
rm -rf web/_resources web/_releases
python3 tools/stage_web_assets.py
python3 - "$engine_inputs_sha256" <<'RECEIPT'
import hashlib, json, pathlib, subprocess, sys
sys.path.insert(0, 'tools')
from web_engine_inputs import fingerprint
if fingerprint(pathlib.Path.cwd())[0] != sys.argv[1]:
    raise SystemExit('Compiled inputs changed during build; rebuild before publishing')
wasm=pathlib.Path('web/protect_carrot_bg.wasm')
pathlib.Path('web/build.json').write_text(json.dumps({'revision':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'engine_inputs_sha256':sys.argv[1],'wasm_sha256':hashlib.sha256(wasm.read_bytes()).hexdigest(),'wasm_bytes':wasm.stat().st_size},indent=2)+'\n')
RECEIPT
rm -f web/*.br web/protect_carrot.js.gz web/wasm_size.txt
python3 tools/generate_web_manifest.py web --revision "$(git rev-parse HEAD)" --template tools/sw-template.js
# Raw precompressed legacy files are not part of the published content set.
rm -f web/*.br web/protect_carrot.js.gz web/wasm_size.txt
touch web/.nojekyll
