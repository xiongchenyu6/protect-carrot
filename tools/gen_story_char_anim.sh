#!/usr/bin/env bash
# Animate the two story characters (guardian commander, void warlord) into living
# portraits via WAN i2v; pack as compact 4x4 grid atlases (16 frames @160x240) WebP.
# Output: assets/story/<name>_anim.webp
set -u
cd "$(dirname "$0")/.."
mkdir -p /tmp/scanim
seed=1300
gen() { # $1 = src webp, $2 = out name, $3 = prompt
  local src_png="/tmp/scanim/$2_src.png"
  magick "$1" -resize 256x384^ -gravity center -extent 256x384 -background black "$src_png"
  local fdir="/tmp/scanim/$2"; rm -rf "$fdir"; mkdir -p "$fdir"
  python3 tools/comfy_wan_i2v.py "$src_png" "$fdir" "$seed" 21 256 384 "$3" >/dev/null 2>&1
  seed=$((seed+1))
  local n=$(ls "$fdir"/frame_*.png 2>/dev/null | wc -l)
  if [ "$n" -lt 8 ]; then echo "FAIL $2 ($n frames)"; return; fi
  rm -f /tmp/scanim/sel_*.png
  for i in $(seq 0 15); do
    idx=$(python3 -c "print(min($n-1, round($i*($n-1)/15)))")
    magick "$(printf "$fdir/frame_%03d.png" $idx)" -resize 160x240 "$(printf /tmp/scanim/sel_%02d.png $i)"
  done
  magick montage /tmp/scanim/sel_*.png -tile 4x4 -geometry 160x240+0+0 -background none /tmp/scanim/grid.png
  magick /tmp/scanim/grid.png -quality 70 -define webp:method=6 "assets/story/$2.webp"
  echo "DONE $2 -> $(identify -format '%wx%h' assets/story/$2.webp) $(du -h assets/story/$2.webp|cut -f1)"
}
gen assets/story/portrait_guardian_commander.webp guardian_anim "the same female guardian commander standing proud, subtle living motion: breathing, cloak and hair gently swaying, soft golden magic aura pulsing, plain background, looping idle, cinematic"
gen assets/story/portrait_void_warlord.webp warlord_anim "the same menacing void warlord standing, subtle living motion: breathing, dark cape and shadow tendrils slowly writhing, purple void energy pulsing, plain background, looping idle, cinematic"
echo "STORY_CHAR_ANIM_DONE"
