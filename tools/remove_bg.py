#!/usr/bin/env python3
"""Make a solid backdrop transparent via flood-fill from the image corners.

    python3 tools/remove_bg.py assets/sprites/abilities assets/sprites/talents ...

Flood-fills from all four corners, clearing pixels whose color is within
`TOL` of the seed color. Leaves interior art untouched. Idempotent (skips
images that are already mostly transparent).
"""
import sys
import os
from collections import deque
from PIL import Image

TOL = 42  # per-channel color tolerance


def close(a, b):
    return all(abs(a[i] - b[i]) <= TOL for i in range(3))


def strip(path):
    im = Image.open(path).convert("RGBA")
    px = im.load()
    w, h = im.size
    # already transparent? skip.
    if im.getchannel("A").getextrema()[0] == 0:
        return f"skip (already transparent) {path}"
    seeds = [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)]
    bg = px[0, 0]
    seen = set()
    q = deque(seeds)
    cleared = 0
    while q:
        x, y = q.popleft()
        if (x, y) in seen or not (0 <= x < w and 0 <= y < h):
            continue
        seen.add((x, y))
        r, g, b, a = px[x, y]
        if a == 0 or close((r, g, b), bg):
            if a != 0:
                px[x, y] = (r, g, b, 0)
                cleared += 1
            q.extend([(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)])
    im.save(path)
    return f"ok cleared={cleared} {path}"


def main():
    for d in sys.argv[1:]:
        if not os.path.isdir(d):
            continue
        for name in sorted(os.listdir(d)):
            if name.endswith(".png"):
                print(strip(os.path.join(d, name)), flush=True)


if __name__ == "__main__":
    main()
