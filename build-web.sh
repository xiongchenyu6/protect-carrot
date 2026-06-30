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
  local cargo_home="${CARGO_HOME:-$HOME/.cargo}"
  local cargo_bin="$cargo_home/bin/wasm-bindgen"
  if [ -x "$cargo_bin" ] && wasm_bindgen_version_ok "$cargo_bin"; then
    printf '%s\n' "$cargo_bin"
    return
  fi

  if command -v wasm-bindgen >/dev/null 2>&1; then
    local path_bin
    path_bin="$(command -v wasm-bindgen)"
    if wasm_bindgen_version_ok "$path_bin"; then
      printf '%s\n' "$path_bin"
      return
    fi
  fi

  echo ">> installing wasm-bindgen-cli $WASM_BINDGEN_VERSION" >&2
  cargo install -q --locked wasm-bindgen-cli --version "$WASM_BINDGEN_VERSION"
  printf '%s\n' "$cargo_bin"
}

WASM_BINDGEN_BIN="$(resolve_wasm_bindgen)"

echo ">> cargo build ($PROFILE, WebGPU) for $TARGET"
if [ "$PROFILE" = "release" ]; then
  cargo build --target "$TARGET" --release --features webgpu
  WASM="target/$TARGET/release/$NAME.wasm"
else
  cargo build --target "$TARGET" --features webgpu
  WASM="target/$TARGET/debug/$NAME.wasm"
fi

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
    if wasm-opt -Oz --strip-debug --all-features -o "$BG.opt" "$BG" && [ -f "$BG.opt" ]; then
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

echo ">> copying assets -> web/assets (AssetServer fetches these over HTTP)"
rm -rf web/assets
cp -r assets web/assets

# Release: pre-compress the big text/wasm so the server can serve them compressed
# (massive win on slow connections). Images/audio are already compressed — skip.
# Produce both .gz (gzip_static) and .br (brotli_static, ~25-30% smaller on wasm).
if [ "$PROFILE" = "release" ]; then
  for ff in "web/${NAME}_bg.wasm" "web/${NAME}.js"; do
    gzip -9 -k -f "$ff"
    line=">> compressed $(basename "$ff"): $(du -h "$ff" | cut -f1) -> gz $(du -h "$ff.gz" | cut -f1)"
    if command -v brotli >/dev/null; then
      brotli -q 11 -k -f "$ff"
      line="$line, br $(du -h "$ff.br" | cut -f1)"
    fi
    echo "$line"
  done
  # Record the UNCOMPRESSED wasm size so the loader can show an accurate progress bar
  # while the browser/CDN transparently decompresses br/gz (the body streams decoded).
  stat -c%s "web/${NAME}_bg.wasm" > "web/wasm_size.txt" 2>/dev/null \
    || wc -c < "web/${NAME}_bg.wasm" | tr -d ' ' > "web/wasm_size.txt"
  echo ">> wrote web/wasm_size.txt ($(cat web/wasm_size.txt) bytes uncompressed)"
fi

echo ">> done. Serve:  ./tools/serve_https.sh 8443   (HTTPS, gzip-aware)"
