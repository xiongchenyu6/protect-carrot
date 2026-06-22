#!/usr/bin/env python3
"""Generate cohesive pixel-art TOWER sprites via the PixelLab REST API (pixflux).

Bypasses the (outdated) pixellab python client, which rejects the free-tier
`usage.type == 'generations'` response. Reads the token from env:

    export PIXELLAB_SECRET=...
    .venv/bin/python tools/gen_pixellab.py

Saves assets/sprites/towers/<name>.png (overwrites the gpt-image ones so towers
match the pixel-art enemies). Stops cleanly when the free tier is exhausted.
"""
import base64
import os
import sys
import urllib.request
import json

SECRET = os.environ["PIXELLAB_SECRET"]
URL = "https://api.pixellab.ai/v1/generate-image-pixflux"
STYLE = ("detailed pixel art, game asset, single small defense tower building, "
         "front view, centered, clean outline, transparent background")

TOWERS = {
    "arrow": "a stone archer tower with a crossbow on top, red roof",
    "cannon": "a stone tower with a black iron cannon, orange trim",
    "magic": "a purple wizard spire with a glowing magic crystal orb",
    "sniper": "a tall green watchtower with a long sniper ballista",
    "thunder": "a tesla coil tower crackling with yellow lightning",
    "laser": "a sleek tech tower with a glowing pink laser lens",
    "missile": "a large fortified rocket battery with missiles, orange",
    "fortress": "a massive fortified bunker with a huge brown artillery cannon",
    "ice": "a blue crystalline ice tower radiating frost",
    "wind": "a turquoise tower with spinning turbine cyclone blades",
    "frostnova": "a light-blue crystal obelisk pulsing an icy aura",
    "shadow": "a dark obsidian tower leaking purple shadow mist",
    "holy": "a golden ornate holy spire with a radiant halo",
    "detection": "a lavender tower with a large mystical watching eye",
    "poison": "a tower with bubbling green toxic vials, dripping ooze",
    "fire": "a tower with a flame nozzle spewing orange fire",
    "summon": "a grey wooden totem tower with a spectral wolf aura",
    "prism": "a tall cyan crystalline prism tower splitting light beams",
}


def gen(name: str, subject: str, size: int = 64) -> str:
    body = json.dumps({
        "description": f"{subject}, {STYLE}",
        "image_size": {"width": size, "height": size},
    }).encode()
    req = urllib.request.Request(
        URL, data=body,
        headers={"Authorization": f"Bearer {SECRET}", "Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=180) as resp:
        d = json.loads(resp.read())
    out = f"assets/sprites/towers/{name}.png"
    open(out, "wb").write(base64.b64decode(d["image"]["base64"]))
    return f"ok {out}  (usage={d.get('usage')})"


def main():
    done = 0
    for name, subject in TOWERS.items():
        try:
            print(gen(name, subject), flush=True)
            done += 1
        except urllib.error.HTTPError as e:
            msg = e.read().decode()[:200]
            print(f"FAIL {name}: HTTP {e.code} {msg}", flush=True)
            if e.code in (402, 403, 429):
                print(">> free tier likely exhausted — stopping.", flush=True)
                break
        except Exception as e:
            print(f"FAIL {name}: {type(e).__name__} {e}", flush=True)
    print(f">> generated {done}/{len(TOWERS)} towers")


if __name__ == "__main__":
    main()
