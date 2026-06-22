#!/usr/bin/env bash
# Convert every PNG under assets/ to WebP (q90, alpha preserved) and drop the PNG.
# WebP is ~60-70% smaller than PNG for our Flux-generated sprites and barely-
# compressible-by-gzip PNGs, cutting the browser's first-load asset payload.
# Idempotent: re-run after generating new PNG sprites. Bevy decodes .webp via the
# `webp` feature; all load paths in src/ use the .webp extension.
set -euo pipefail
cd "$(dirname "$0")/.."

# Unused experiment asset — don't ship it.
rm -f assets/ai_test/arrow.png
rmdir assets/ai_test 2>/dev/null || true

count=0
while IFS= read -r -d '' png; do
  webp="${png%.png}.webp"
  magick "$png" -quality 90 -define webp:method=6 "$webp"
  rm -f "$png"
  count=$((count + 1))
done < <(find assets -name '*.png' -print0)
echo "converted $count png -> webp"
