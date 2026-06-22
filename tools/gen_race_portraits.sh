#!/usr/bin/env bash
# Generate 27 class×race hero portraits. Human = the existing class portrait (copy);
# Elf / Orc = Kontext race edits (keep class gear/pose, change race traits).
# Output: assets/sprites/heroes_combo/<class>_<race>.webp
set -u
cd "$(dirname "$0")/.."
mkdir -p /tmp/rp assets/sprites/heroes_combo
CLASSES="warrior mage ranger guardian stormcaller warden assassin priest engineer"
ELF="Make this character a NIGHT ELF. DRAMATICALLY change the face and head: very LONG POINTED elf ears, pale lavender skin, slender delicate face, glowing eyes, long flowing hair. Keep the same class outfit, armor and weapons and pose."
ORC="Make this character a brutal ORC. DRAMATICALLY change the face and skin: BRIGHT GREEN skin, two large white TUSKS jutting up from the lower jaw, heavy jaw, bulky muscular brute, fierce scowl. Keep the same class outfit, armor and weapons and pose."
GUID=4.0
seed=600
for c in $CLASSES; do
  src="assets/sprites/heroes/${c}.webp"
  [ -f "$src" ] || { echo "SKIP $c"; continue; }
  # human = copy of the base portrait
  magick "$src" -quality 90 "assets/sprites/heroes_combo/${c}_human.webp"
  echo "DONE ${c}_human (copy)"
  base="/tmp/rp/${c}.png"; magick "$src" "$base"
  for race in elf orc; do
    prompt=$([ "$race" = elf ] && echo "$ELF" || echo "$ORC")
    out="/tmp/rp/${c}_${race}.png"
    python3 tools/comfy_kontext.py "$base" "$out" "$seed" "$prompt" "$GUID" >/dev/null 2>&1
    seed=$((seed+1))
    m=$(magick "$out" -colorspace Gray -format "%[fx:mean]" info: 2>/dev/null || echo 0)
    if awk "BEGIN{exit !(${m:-0} < 0.20)}"; then
      python3 tools/comfy_kontext.py "$base" "$out" "$((seed+300))" "$prompt" "$GUID" >/dev/null 2>&1
    fi
    if [ -f "$out" ]; then
      magick "$out" -resize 384x384 -quality 88 "assets/sprites/heroes_combo/${c}_${race}.webp"
      echo "DONE ${c}_${race}"
    else
      echo "FAIL ${c}_${race}"
    fi
  done
done
echo "ALL_PORTRAITS_DONE"
