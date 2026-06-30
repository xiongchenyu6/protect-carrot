#!/usr/bin/env bash
# Generate game sound effects with ElevenLabs' text-to-sound-effects API.
# The API key is read from the environment — NEVER hardcode it here.
#
#   export ELEVENLABS_API_KEY=sk_...
#   ./tools/gen_sfx.sh
#
# Output: assets/audio/<name>.wav  (loaded by src/audio.rs)
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

echo ">> generating SFX into assets/audio/"
gen build      1.2 "placing a defense turret: mechanical clunk with a short power-up chime, game sfx"
gen shoot      0.7 "short punchy arcade tower shot, quick energetic zap"
gen hit        0.5 "small soft impact hitting a creature, short wet thud"
gen explosion  1.5 "punchy explosion blast with debris, arcade game sfx"
gen death      0.9 "eldritch alien creature dying, squishy splat with a creepy gurgle"
gen meteor     2.0 "massive flaming meteor crashing down, deep powerful boom"
gen freeze     1.4 "magical ice freeze, crystalline shatter and frost whoosh"
gen gold       1.0 "bright cascade of gold coins, cheerful reward jingle"
gen wave       2.0 "ominous deep war horn announcing an incoming monster wave, eerie"
gen victory    3.0 "triumphant short victory fanfare stinger, heroic"
gen defeat     3.0 "dark ominous game-over sting, low dread and despair"
gen click      0.4 "soft clean UI button click"
echo ">> done."
