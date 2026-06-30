#!/usr/bin/env bash
# Generate the ADDITIONAL game sound effects (beyond gen_sfx.sh's base set) with
# ElevenLabs' text-to-sound-effects API. Key is read from the environment only.
#
#   export ELEVENLABS_API_KEY=sk_...
#   ./tools/gen_sfx_extra.sh
#
# Output: assets/audio/<name>.wav  (wired into src/audio.rs Sound enum)
set -euo pipefail
cd "$(dirname "$0")/.."

: "${ELEVENLABS_API_KEY:?set ELEVENLABS_API_KEY in env first}"
command -v ffmpeg >/dev/null || { echo "ffmpeg is required to convert ElevenLabs MP3 output to WAV"; exit 1; }
mkdir -p assets/audio
URL="https://api.elevenlabs.io/v1/sound-generation?output_format=mp3_44100_128"

gen() { # name  duration  prompt
  local name="$1" dur="$2" prompt="$3"
  local tmp="assets/audio/${name}.mp3.tmp"
  local out="assets/audio/${name}.wav"
  local body
  body=$(printf '{"text":%s,"duration_seconds":%s,"prompt_influence":0.5}' \
    "$(printf '%s' "$prompt" | python3 -c 'import json,sys;print(json.dumps(sys.stdin.read()))')" "$dur")
  local code
  code=$(curl -s -X POST "$URL" -H "xi-api-key: $ELEVENLABS_API_KEY" \
    -H "Content-Type: application/json" -d "$body" -o "$tmp" -w "%{http_code}")
  if [ "$code" = "200" ]; then
    ffmpeg -y -v error -i "$tmp" -ac 2 -ar 44100 "$out"
    rm -f "$tmp"
    echo "  ok  $out"
  else
    echo "  FAIL($code) $out"
    cat "$tmp"
    echo
    rm -f "$tmp"
  fi
}

echo ">> generating EXTRA SFX into assets/audio/"
gen upgrade   1.1 "magical tower upgrade power-up, ascending bright shimmer with a metallic reinforce clank, game sfx"
gen sell      0.9 "selling a building, coins clatter with a soft dismantle poof, arcade sfx"
gen summon    1.3 "necromantic summon of a skeleton minion, bony rattle with a dark magic whoosh"
gen raise     1.4 "raising the dead, eerie ghostly groan with a rising unholy chime"
gen laser     0.8 "sustained sci-fi laser beam zap, focused energy hum, arcade sfx"
gen poison    1.0 "toxic acid sizzle and bubbling poison hiss, nasty wet corrosion"
gen chain     0.8 "crackling chain lightning arc jumping between targets, electric zap"
gen boss      2.4 "gigantic eldritch cosmic horror roar, deep guttural dread, boss appears"
gen combo     0.7 "rewarding rising combo chime, sparkly arcade kill-streak ping"
gen nogold    0.5 "soft negative error buzz, not enough resources, gentle denied blip"
gen waveclear 2.0 "wave cleared success chime, short uplifting magical sparkle resolve"
gen curse     1.0 "dark hex curse cast, ominous low whisper with a sinister bell"
echo ">> done."
