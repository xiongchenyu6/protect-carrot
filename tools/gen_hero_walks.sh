#!/usr/bin/env bash
# Generate per-class hero walk-cycle strips via Flux Kontext (same character, new
# poses). Output: assets/heroes_walk/<class>.webp = [idle, walk1..walk6] @128px.
# Auto-retries frames that come back near-black (Kontext occasionally fails a frame).
set -u
cd "$(dirname "$0")/.."
mkdir -p /tmp/hw assets/heroes_walk

POSES=(
  "left leg stepped far forward heel down, right leg trailing back, right arm swung forward"
  "left leg planted under body, right knee lifting forward to pass, slight crouch"
  "right leg swinging forward in mid-air, left leg pushing off behind, arms swapping"
  "right leg stepped far forward heel down, left leg trailing back, left arm swung forward"
  "right leg planted under body, left knee lifting forward to pass, slight crouch"
  "left leg swinging forward in mid-air, right leg pushing off behind, arms swapping"
)

norm() { # $1 in -> $2 out : bg-remove, trim, feet-to-bottom, 128 square
  magick "$1" -bordercolor white -border 1 -fuzz 24% -fill none -draw "alpha 0,0 floodfill" \
    -shave 1x1 -trim +repage -resize 104x104\> -background none -gravity south -extent 128x128 "$2" 2>/dev/null
}

brightness() { magick "$1" -colorspace Gray -format "%[fx:mean]" info: 2>/dev/null || echo 0; }

for cls in "$@"; do
  portrait="assets/sprites/heroes/${cls}.webp"
  [ -f "$portrait" ] || { echo "SKIP $cls (no portrait)"; continue; }
  base="/tmp/hw/${cls}_base.png"
  magick "$portrait" "$base"
  P="the exact same character from the reference image, full-body, facing right, plain flat white background, centered, consistent size, 2D game sprite, walking"
  ok=1
  for i in 0 1 2 3 4 5; do
    f="/tmp/hw/${cls}_f$((i+1)).png"
    seed=$(( (i+1)*13 + RANDOM % 4000 ))
    python3 tools/comfy_kontext.py "$base" "$f" "$seed" "$P, ${POSES[$i]}" >/dev/null 2>&1
    m=$(brightness "$f")
    if awk "BEGIN{exit !(${m:-0} < 0.30)}"; then
      # too dark / failed -> one retry with a different seed
      python3 tools/comfy_kontext.py "$base" "$f" "$((seed + 777))" "$P, ${POSES[$i]}" >/dev/null 2>&1
      m=$(brightness "$f")
    fi
    [ -f "$f" ] || ok=0
  done
  if [ "$ok" = 1 ]; then
    norm "$base" "/tmp/hw/${cls}_n0.png"
    args=("/tmp/hw/${cls}_n0.png")
    for i in 1 2 3 4 5 6; do norm "/tmp/hw/${cls}_f$i.png" "/tmp/hw/${cls}_n$i.png"; args+=("/tmp/hw/${cls}_n$i.png"); done
    magick "${args[@]}" +append -quality 92 "assets/heroes_walk/${cls}.webp"
    echo "DONE $cls -> $(identify -format '%wx%h' assets/heroes_walk/${cls}.webp 2>/dev/null)"
  else
    echo "FAIL $cls"
  fi
done
echo "ALL_WALKS_DONE"
