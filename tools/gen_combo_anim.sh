#!/usr/bin/env bash
# Animate each of the 27 class×race portraits into a short "living portrait" loop via
# WAN 2.2 i2v, packed compactly as a 4x4 grid atlas (16 frames @192px) WebP.
# Output: assets/story/combo_anim/<class>_<race>.webp   (cutscene backdrop per combo)
set -u
cd "$(dirname "$0")/.."
mkdir -p assets/story/combo_anim /tmp/canim
PROMPT="the same fantasy game character standing idle, subtle living motion: breathing, cape and hair gently swaying, soft glowing magic aura pulsing around them, slight weapon shimmer, plain background, smooth looping idle animation, cinematic"
seed=900
for p in assets/sprites/heroes_combo/*.webp; do
  combo=$(basename "$p" .webp)
  out="assets/story/combo_anim/${combo}.webp"
  [ -f "$out" ] && { echo "SKIP $combo (exists)"; continue; }
  src="/tmp/canim/${combo}_src.png"; magick "$p" -resize 288x288 -background black -gravity center -extent 288x288 "$src"
  fdir="/tmp/canim/${combo}"; rm -rf "$fdir"; mkdir -p "$fdir"
  python3 tools/comfy_wan_i2v.py "$src" "$fdir" "$seed" 21 288 288 "$PROMPT" >/dev/null 2>&1
  seed=$((seed+1))
  n=$(ls "$fdir"/frame_*.png 2>/dev/null | wc -l)
  if [ "$n" -lt 8 ]; then echo "FAIL $combo (got $n frames)"; continue; fi
  # subsample to 16, downscale to 192, pack 4x4 grid
  rm -f /tmp/canim/sel_*.png
  for i in $(seq 0 15); do
    idx=$(python3 -c "print(min($n-1, round($i*($n-1)/15)))")
    magick "$(printf "$fdir/frame_%03d.png" $idx)" -resize 192x192 "$(printf /tmp/canim/sel_%02d.png $i)"
  done
  magick montage /tmp/canim/sel_*.png -tile 4x4 -geometry 192x192+0+0 -background none /tmp/canim/grid.png
  magick /tmp/canim/grid.png -quality 68 -define webp:method=6 "$out"
  echo "DONE $combo -> $(identify -format '%wx%h' "$out") $(du -h "$out"|cut -f1)"
done
echo "ALL_COMBO_ANIM_DONE"
