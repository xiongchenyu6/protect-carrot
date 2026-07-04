#!/usr/bin/env bash
# Generate AI paperdoll layers via the Codex CLI image_gen tool.
#
# Bodies are full-canvas character art; items are generated as STANDALONE
# transparent props — tools/pack_paperdoll_ai.py places them at fixed anchors,
# so alignment never depends on the model.
#
# Usage:
#   ./tools/gen_paperdoll_ai.sh bodies      # 3 race bases
#   ./tools/gen_paperdoll_ai.sh weapons     # 9 weapons
#   ./tools/gen_paperdoll_ai.sh gear        # 15 gear items
#   ./tools/gen_paperdoll_ai.sh all
# Existing files are skipped (resumable). Raw items land in tmp/paperdoll_ai/raw/,
# bodies in tmp/paperdoll_ai/.
set -uo pipefail
cd "$(dirname "$0")/.."

STYLE="Art style must match the attached reference image exactly: cute chibi fantasy game art, glossy cel shading, bold dark outlines, vibrant saturated colors, soft highlights. Square canvas, truly transparent background (no floor, no shadow blob unless asked)."
REF="assets/sprites/heroes/warrior.webp"
OUT=tmp/paperdoll_ai
mkdir -p "$OUT/raw"

gen() { # gen <outfile> <prompt>
  local file="$1" prompt="$2"
  if [ -s "$file" ]; then echo "skip $file"; return 0; fi
  echo ">>> $file"
  codex exec -C "$(pwd)" -s workspace-write --skip-git-repo-check \
    "Use the image generation tool to create: $prompt $STYLE Save it EXACTLY as ./$file (overwrite existing)." \
    -i "$REF" >/dev/null 2>&1
  if [ -s "$file" ]; then echo "  ok"; else echo "  FAILED $file"; fi
}

bodies() {
  gen "$OUT/body_human.png" "a chibi HUMAN adventurer: front view, standing straight and symmetrical, full body centered filling about 85% of the frame height, arms relaxed slightly away from the body so the hands are visible, bare hands, NO weapon, NO helmet, NO cape. Wearing only a simple plain brown cloth tunic with a belt and simple dark shoes. Brown hair, big expressive friendly eyes, small smile."
  gen "$OUT/body_elf.png" "a chibi ELF adventurer: front view, standing straight and symmetrical, full body centered filling about 85% of the frame height, arms relaxed slightly away from the body so the hands are visible, bare hands, NO weapon, NO helmet, NO cape. Long pointed elf ears, long blonde hair, emerald green eyes, elegant friendly face. Wearing only a simple plain light-green cloth tunic with a belt and simple soft shoes."
  gen "$OUT/body_orc.png" "a chibi ORC adventurer: front view, standing straight and symmetrical, full body centered filling about 85% of the frame height, arms relaxed slightly away from the body so the hands are visible, bare hands, NO weapon, NO helmet, NO cape. Green skin, two small lower tusks, dark topknot hair, sturdy build, friendly grin. Wearing only a simple plain brown leather vest with a belt and simple dark boots."
}

weapons() {
  gen "$OUT/raw/weapon_sword.png" "a single fantasy longsword game prop, blade pointing straight up, bright polished steel blade with glossy highlight, golden crossguard, leather-wrapped grip. Item only, nothing else."
  gen "$OUT/raw/weapon_staff.png" "a single wooden mage staff game prop, vertical, with a glowing fiery orange-red crystal orb on top. Item only, nothing else."
  gen "$OUT/raw/weapon_bow.png" "a single elegant wooden recurve bow game prop with taut string, vertical orientation. Item only, nothing else."
  gen "$OUT/raw/weapon_shield.png" "a single heroic kite shield game prop, front view, red and gold heraldry with a star emblem, steel rim. Item only, nothing else."
  gen "$OUT/raw/weapon_storm_orb.png" "a single crackling storm orb game prop: a glassy blue sphere with a yellow lightning bolt inside, electric sparks around it. Item only, nothing else."
  gen "$OUT/raw/weapon_sentry_bow.png" "a single wooden crossbow game prop, front view, with steel limbs and a taut string. Item only, nothing else."
  gen "$OUT/raw/weapon_dagger.png" "a single curved assassin dagger game prop, blade pointing up, dark steel with a purple-wrapped grip. Item only, nothing else."
  gen "$OUT/raw/weapon_censer.png" "a single golden holy censer game prop hanging from a short chain, ornate, with soft glowing light and a wisp of incense smoke. Item only, nothing else."
  gen "$OUT/raw/weapon_hammer.png" "a single mighty forge warhammer game prop, vertical handle, massive steel head with orange ember glow in the seams. Item only, nothing else."
}

gear() {
  gen "$OUT/raw/gear_vow_plate.png" "a chibi-proportioned steel breastplate cuirass game armor prop, front view, with round pauldrons on both sides and a golden emblem in the center, glossy metal. Garment only on invisible mannequin, nothing else."
  gen "$OUT/raw/gear_starweave_robe.png" "a chibi-proportioned deep-blue mage robe game garment prop, front view, flowing to the ground, gold trim at the collar, scattered tiny golden stars glowing on the fabric. Garment only on invisible mannequin, nothing else."
  gen "$OUT/raw/gear_windrunner_cloak.png" "a chibi-proportioned emerald green traveler cloak game garment prop, front view, OPEN in the middle showing nothing behind, two side panels draping down, golden clasp at the throat. Garment only on invisible mannequin, nothing else."
  gen "$OUT/raw/gear_thunder_charm.png" "a single small necklace pendant game prop: a golden lightning bolt charm on a thin leather cord, softly glowing. Item only, nothing else."
  gen "$OUT/raw/gear_saint_bell.png" "a single small ornate golden hand bell game prop with a soft holy glow. Item only, nothing else."
  gen "$OUT/raw/gear_night_mask.png" "a single sleek dark-violet domino eye mask game prop, front view, with two glowing ice-blue eye slits. Item only, nothing else."
  gen "$OUT/raw/gear_forge_gauntlet.png" "a single armored forge gauntlet game prop (right hand fist, knuckles forward), orange-red metal with glowing ember seams. Item only, nothing else."
  gen "$OUT/raw/gear_warden_scope.png" "a single small brass monocle scope game prop with a glowing green lens and crosshair. Item only, nothing else."
  gen "$OUT/raw/gear_carrot_halo.png" "a single glowing golden angel halo ring game prop, seen at a slight angle so it forms a wide ellipse, sparkling. Item only, nothing else."
  gen "$OUT/raw/gear_wayfarer_boots.png" "a pair of sturdy brown leather traveler boots game prop, front view, side by side with a small gap between them, golden buckles. Items only, nothing else."
  gen "$OUT/raw/gear_bloodstep_greaves.png" "a pair of crimson red armored greaves game prop, front view, side by side with a small gap between them, faint red glow. Items only, nothing else."
  gen "$OUT/raw/gear_starpath_sandals.png" "a pair of indigo blue mystical sandals game prop, front view, side by side with a small gap between them, tiny glowing golden stars on the straps. Items only, nothing else."
  gen "$OUT/raw/gear_engineer_treads.png" "a pair of rugged steampunk engineer boots game prop, front view, side by side with a small gap between them, with brass gears and rivets. Items only, nothing else."
  gen "$OUT/raw/gear_summoner_greaves.png" "a pair of dark purple arcane greaves game prop, front view, side by side with a small gap between them, glowing spectral green runes. Items only, nothing else."
  gen "$OUT/raw/gear_carrot_wings.png" "a pair of glowing golden-orange angel wings game prop, spread symmetrically left and right with a clear gap in the middle, feathered, radiant. Items only, nothing else."
}

case "${1:-all}" in
  bodies) bodies ;;
  weapons) weapons ;;
  gear) gear ;;
  all) bodies; weapons; gear ;;
  *) echo "usage: $0 {bodies|weapons|gear|all}"; exit 1 ;;
esac
echo "batch done"
