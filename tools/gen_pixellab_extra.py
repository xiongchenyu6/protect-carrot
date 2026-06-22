#!/usr/bin/env python3
"""Generate extra pixel-art icons via the PixelLab REST API (pixflux):
ability icons, talent icons, and boss portraits. Token from env only:

    export PIXELLAB_SECRET=...
    python3 tools/gen_pixellab_extra.py

Stops cleanly when the free tier is exhausted (HTTP 402/403/429).
"""
import base64
import os
import urllib.request
import urllib.error
import json

SECRET = os.environ["PIXELLAB_SECRET"]
URL = "https://api.pixellab.ai/v1/generate-image-pixflux"
ICON_STYLE = ("detailed pixel art game UI icon, single centered emblem, bold "
              "readable silhouette, clean outline, transparent background")
BOSS_STYLE = ("detailed pixel art, eldritch cosmic horror boss monster portrait, "
              "front view bust, menacing, centered, transparent background")

# (subdir, name, prompt, size)
JOBS = [
    # --- ability icons ---
    ("abilities", "meteor", f"a flaming meteor crashing down with fire trail, {ICON_STYLE}", 64),
    ("abilities", "freeze", f"a blue snowflake encased in ice crystal, frost aura, {ICON_STYLE}", 64),
    ("abilities", "goldrush", f"an overflowing pile of shiny gold coins, sparkle, {ICON_STYLE}", 64),
    # --- talent icons ---
    ("talents", "damage", f"a fiery red sword with an upward arrow, power up, {ICON_STYLE}", 64),
    ("talents", "range", f"a green target reticle with a ranged arrow, {ICON_STYLE}", 64),
    ("talents", "speed", f"a yellow lightning bolt with motion lines, fast, {ICON_STYLE}", 64),
    # --- boss portraits (themed by boss skill) ---
    ("bosses", "serpent", f"a colossal sea serpent father, {BOSS_STYLE}", 96),
    ("bosses", "abyssal", f"a deep-lake abyssal guardian with a glowing shield, {BOSS_STYLE}", 96),
    ("bosses", "yellow", f"the yellow sign king in tattered robes, {BOSS_STYLE}", 96),
    ("bosses", "storm", f"a crackling storm titan of lightning, {BOSS_STYLE}", 96),
    ("bosses", "furnace", f"a red molten furnace star demon, {BOSS_STYLE}", 96),
    ("bosses", "brood", f"a bloated brood mother spawning larvae, {BOSS_STYLE}", 96),
    ("bosses", "void", f"a phasing void horror, partly invisible, {BOSS_STYLE}", 96),
    ("bosses", "starforged", f"a starforged bulwark golem of cosmic metal, {BOSS_STYLE}", 96),
    ("bosses", "moss", f"a creeping moss-carpet fungal leviathan, {BOSS_STYLE}", 96),
    ("bosses", "dream", f"a dream-eclipse nightmare entity, eerie, {BOSS_STYLE}", 96),
]


def gen(subdir: str, name: str, prompt: str, size: int) -> str:
    body = json.dumps({
        "description": prompt,
        "image_size": {"width": size, "height": size},
    }).encode()
    req = urllib.request.Request(
        URL, data=body,
        headers={"Authorization": f"Bearer {SECRET}", "Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=180) as resp:
        d = json.loads(resp.read())
    os.makedirs(f"assets/sprites/{subdir}", exist_ok=True)
    out = f"assets/sprites/{subdir}/{name}.png"
    open(out, "wb").write(base64.b64decode(d["image"]["base64"]))
    return f"ok {out}  (usage={d.get('usage')})"


def main():
    done = 0
    for subdir, name, prompt, size in JOBS:
        try:
            print(gen(subdir, name, prompt, size), flush=True)
            done += 1
        except urllib.error.HTTPError as e:
            msg = e.read().decode()[:200]
            print(f"FAIL {name}: HTTP {e.code} {msg}", flush=True)
            if e.code in (402, 403, 429):
                print(">> free tier likely exhausted — stopping.", flush=True)
                break
        except Exception as e:
            print(f"FAIL {name}: {type(e).__name__} {e}", flush=True)
    print(f">> generated {done}/{len(JOBS)} icons")


if __name__ == "__main__":
    main()
