#!/usr/bin/env bash
# Generate tower/enemy sprites with the Codex CLI's built-in gpt-image-2 tool.
# Billed against your ChatGPT plan (run `codex login` first) — no API key.
#
# Usage (from project root):
#   ./tools/gen_sprites_ai.sh towers      # only towers (5)
#   ./tools/gen_sprites_ai.sh enemies     # only enemies (16)
#   ./tools/gen_sprites_ai.sh all         # everything (21)  [default]
#   ./tools/gen_sprites_ai.sh one towers arrow "an archer tower ..."  # single
#
# Each sprite overwrites assets/sprites/{towers,enemies}/<name>.png so the game
# loader picks them up with no code change. After running: ./build-web.sh dev
set -euo pipefail
cd "$(dirname "$0")/.."

STYLE="top-down 2D tower-defense game sprite, vibrant cartoon style with bold dark \
outline and soft shading, centered, FULLY TRANSPARENT background, no text, no \
watermark, no shadow on ground, square 512x512"

CTHULHU="eldritch Lovecraftian horror, eerie bioluminescence, tentacles/eyes where \
fitting, unsettling but readable as a game enemy"

gen() { # dir name subject
  local dir="$1" name="$2" subject="$3"
  local out="./assets/sprites/${dir}/${name}.png"
  echo ">> [$dir/$name] generating..."
  codex exec -C "$(pwd)" -s workspace-write --skip-git-repo-check \
    "Use the image generation tool to create: ${subject}. Style: ${STYLE}. \
Save it EXACTLY as ${out} (overwrite if it exists). Ensure the background is \
truly transparent (alpha), trimming any solid backdrop." >/dev/null 2>&1 \
    && echo "   done: $out" || echo "   FAILED: $out"
}

declare -A TOWERS=(
  [arrow]="an archer/arrow tower: round stone turret base with a mounted crossbow"
  [cannon]="a cannon tower: stone base with a stubby black iron cannon barrel"
  [magic]="a wizard magic tower: arcane spire with a glowing purple crystal orb"
  [ice]="an ice tower: blue crystalline turret radiating frost"
  [detection]="a detection/watch tower: lavender turret with a large mystical eye lens"
)

declare -A ENEMIES=(
  [normal]="a small red blob creature with two eyes, ${CTHULHU}"
  [fast]="a sleek darting orange creature, arrow-shaped, fast, ${CTHULHU}"
  [tank]="a bulky heavy purple armored brute, slow and tough, ${CTHULHU}"
  [flying]="a winged blue flying creature with membranous wings, ${CTHULHU}"
  [invisible]="a translucent ghostly grey wraith, semi-transparent, ${CTHULHU}"
  [regenerating]="a green regenerating slime with a pulsing healing core, ${CTHULHU}"
  [armored]="a grey hexagonal armored shell creature, plated, ${CTHULHU}"
  [swarmer]="a tiny orange swarm bug, small and numerous looking, ${CTHULHU}"
  [boss]="a huge menacing dark-red boss horror with a crown of spikes and many eyes, ${CTHULHU}"
  [shielded]="a blue creature surrounded by a glowing energy shield bubble, ${CTHULHU}"
  [splitter]="a violet creature visibly cracking apart into smaller blobs, ${CTHULHU}"
  [healer]="a green creature with a radiant red healing cross aura, ${CTHULHU}"
  [charger]="a yellow charging beast with speed streaks, hunched to dash, ${CTHULHU}"
  [climber]="a brown wall-climbing ghoul with hooked claws, built to attack defense towers, ${CTHULHU}"
  [silencer]="a purple silent wraith with a stitched mouth and anti-magic aura, ${CTHULHU}"
  [moss]="MOSS tower-eating boss: green-black tentacled mass with one huge jaw, ${CTHULHU}"
)

mode="${1:-all}"
case "$mode" in
  one) gen "$2" "$3" "$4" ;;
  towers) for k in "${!TOWERS[@]}"; do gen towers "$k" "${TOWERS[$k]}"; done ;;
  enemies) for k in "${!ENEMIES[@]}"; do gen enemies "$k" "${ENEMIES[$k]}"; done ;;
  all)
    for k in "${!TOWERS[@]}"; do gen towers "$k" "${TOWERS[$k]}"; done
    for k in "${!ENEMIES[@]}"; do gen enemies "$k" "${ENEMIES[$k]}"; done ;;
  *) echo "unknown mode: $mode"; exit 1 ;;
esac
echo ">> all done. Now rebuild: ./build-web.sh dev"
