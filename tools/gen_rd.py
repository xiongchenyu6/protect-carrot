#!/usr/bin/env python3
"""Generate battlefield backgrounds via the Retro Diffusion API (RD_FAST).
Token from env only:

    export RD_API_KEY=rdpk-...
    python3 tools/gen_rd.py

POST https://api.retrodiffusion.ai/v1/inferences  (header X-RD-Token)
Response: {"base64_images":[...], "remaining_balance": N}
Stops cleanly when balance is exhausted.
"""
import base64
import os
import urllib.request
import urllib.error
import json

TOKEN = os.environ["RD_API_KEY"]
URL = "https://api.retrodiffusion.ai/v1/inferences"

# Atmospheric Cthulhu-themed top-down battlefield backdrops, one per level theme.
# 320x240 keeps the 4:3 board aspect; the grid renders on top.
BACKDROPS = [
    ("swamp", "top-down dark eldritch swamp battlefield, murky green water, twisted roots, eerie fog"),
    ("abyss", "top-down deep abyssal ocean trench floor, bioluminescent coral, dark blue void"),
    ("cosmic", "top-down cosmic void temple floor, purple nebula, ancient star runes glowing"),
    ("ruins", "top-down ruined stone temple courtyard, cracked tiles, moss, ominous"),
    ("snow", "top-down frozen tundra battlefield, cracked ice, pale blue snow, dead trees"),
    ("blood", "top-down cursed blood marsh, dark red soil, bone debris, sinister"),
]


def gen_bg(name: str, prompt: str) -> str:
    body = json.dumps({
        "prompt": prompt,
        "prompt_style": "rd_fast__game_asset",
        "width": 320,
        "height": 240,
        "num_images": 1,
    }).encode()
    req = urllib.request.Request(
        URL, data=body,
        headers={"X-RD-Token": TOKEN, "Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=180) as resp:
        d = json.loads(resp.read())
    os.makedirs("assets/sprites/backgrounds", exist_ok=True)
    out = f"assets/sprites/backgrounds/{name}.png"
    open(out, "wb").write(base64.b64decode(d["base64_images"][0]))
    return f"ok {out}  (remaining={d.get('remaining_balance')})"


def main():
    done = 0
    for name, prompt in BACKDROPS:
        try:
            print(gen_bg(name, prompt), flush=True)
            done += 1
        except urllib.error.HTTPError as e:
            msg = e.read().decode()[:200]
            print(f"FAIL {name}: HTTP {e.code} {msg}", flush=True)
            if e.code in (401, 402, 403, 429):
                print(">> balance likely exhausted / auth issue — stopping.", flush=True)
                break
        except Exception as e:
            print(f"FAIL {name}: {type(e).__name__} {e}", flush=True)
    print(f">> generated {done}/{len(BACKDROPS)} backgrounds")


if __name__ == "__main__":
    main()
