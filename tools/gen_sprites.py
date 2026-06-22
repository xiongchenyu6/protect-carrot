#!/usr/bin/env python3
"""Procedurally generate tower/enemy sprite PNGs for the Bevy port.

We have no hand-drawn art, so this draws clean top-down turret and creature
sprites from code (shaded discs, barrels, distinct silhouettes per type). Run via
the dev shell so Pillow is available:

    nix develop --command python3 tools/gen_sprites.py

Output: assets/sprites/towers/<kind>.png and assets/sprites/enemies/<kind>.png
Sprites face +x (right); the game rotates towers toward their target.
"""
import math
import os
from PIL import Image, ImageDraw

S = 64          # final sprite size
SS = 4          # supersample factor for anti-aliasing
Z = S * SS

OUT = os.path.join(os.path.dirname(__file__), "..", "assets", "sprites")


def rgb(h):
    return ((h >> 16) & 255, (h >> 8) & 255, h & 255)


def lighten(c, f):
    return tuple(min(255, int(v + (255 - v) * f)) for v in c)


def darken(c, f):
    return tuple(int(v * (1 - f)) for v in c)


def new():
    return Image.new("RGBA", (Z, Z), (0, 0, 0, 0))


def save(img, path):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    img.resize((S, S), Image.LANCZOS).save(path)


# ----------------------------- towers -----------------------------

TOWERS = {
    "arrow": 0xE74C3C, "cannon": 0xE67E22, "magic": 0x9B59B6, "sniper": 0x2ECC71,
    "thunder": 0xF1C40F, "laser": 0xE84393, "missile": 0xD35400, "ice": 0x3498DB,
    "wind": 0x00CEC9, "frostnova": 0x74B9FF, "shadow": 0x636E72, "holy": 0xFDCB6E,
    "detection": 0xA29BFE, "poison": 0x6C5CE7, "fire": 0xE17055, "summon": 0xB2BEC3,
    "fortress": 0x8E6E3C, "prism": 0x00D2FF, "necromancer": 0x4B3B5A,
}


def tower_sprite(color):
    img = new()
    d = ImageDraw.Draw(img)
    c = Z // 2
    col = rgb(color)
    # round base with shading (dark ring -> lighter center)
    base_r = int(Z * 0.42)
    d.ellipse([c - base_r, c - base_r, c + base_r, c + base_r],
              fill=(38, 42, 46, 255))
    ring_r = int(Z * 0.40)
    d.ellipse([c - ring_r, c - ring_r, c + ring_r, c + ring_r],
              outline=col + (255,), width=int(Z * 0.05))
    plate_r = int(Z * 0.30)
    d.ellipse([c - plate_r, c - plate_r, c + plate_r, c + plate_r],
              fill=darken(col, 0.35) + (255,))
    # barrel pointing +x
    bw, bh = int(Z * 0.42), int(Z * 0.14)
    d.rounded_rectangle([c, c - bh // 2, c + bw, c + bh // 2],
                        radius=bh // 2, fill=(28, 30, 34, 255))
    d.ellipse([c + bw - bh, c - bh // 2, c + bw, c + bh // 2],
              fill=darken(col, 0.1) + (255,))
    # bright gem in the center
    gem_r = int(Z * 0.15)
    d.ellipse([c - gem_r, c - gem_r, c + gem_r, c + gem_r],
              fill=lighten(col, 0.35) + (255,))
    hl = int(Z * 0.06)
    d.ellipse([c - gem_r // 2 - hl, c - gem_r // 2 - hl,
               c - gem_r // 2 + hl, c - gem_r // 2 + hl],
              fill=(255, 255, 255, 200))
    return img


# ----------------------------- enemies -----------------------------

ENEMIES = {
    "normal": 0xE74C3C, "fast": 0xF39C12, "tank": 0x8E44AD, "flying": 0x3498DB,
    "invisible": 0x95A5A6, "regenerating": 0x2ECC71, "armored": 0x7F8C8D,
    "swarmer": 0xE67E22, "boss": 0xC0392B,
    "shielded": 0x5DADE2, "splitter": 0xAF7AC5, "healer": 0x58D68D, "charger": 0xF5B041,
    "climber": 0xB9770E, "silencer": 0x6C5CE7, "moss": 0x145A32,
}


def poly(d, pts, fill, outline):
    d.polygon(pts, fill=fill + (255,), outline=outline + (255,))


def eyes(d, cx, cy, r):
    er = max(2, int(r * 0.14))
    for sx in (-1, 1):
        ex, ey = cx + sx * int(r * 0.32), cy - int(r * 0.12)
        d.ellipse([ex - er, ey - er, ex + er, ey + er], fill=(255, 255, 255, 255))
        d.ellipse([ex - er // 2, ey - er // 2, ex + er // 2, ey + er // 2],
                  fill=(20, 20, 20, 255))


def enemy_sprite(kind, color):
    img = new()
    d = ImageDraw.Draw(img)
    c = Z // 2
    col = rgb(color)
    out = darken(col, 0.5)
    r = int(Z * 0.40)
    ow = int(Z * 0.04)

    if kind in ("normal", "regenerating", "swarmer", "boss", "shielded", "splitter", "healer", "moss"):
        d.ellipse([c - r, c - r, c + r, c + r], fill=col + (255,),
                  outline=out + (255,), width=ow)
        if kind in ("regenerating", "healer"):
            g = int(r * 0.5)
            cross = (255, 255, 255, 230) if kind == "regenerating" else (220, 40, 40, 255)
            d.line([c - g, c, c + g, c], fill=cross, width=ow * 2)
            d.line([c, c - g, c, c + g], fill=cross, width=ow * 2)
        if kind == "shielded":  # glowing outer shield ring
            sr = int(r * 1.12)
            d.ellipse([c - sr, c - sr, c + sr, c + sr],
                      outline=lighten(col, 0.5) + (220,), width=ow)
        if kind == "splitter":  # cracks suggesting it splits
            for a in (30, 150, 270):
                ax, ay = math.cos(math.radians(a)), math.sin(math.radians(a))
                d.line([c, c, c + ax * r * 0.85, c + ay * r * 0.85],
                       fill=out + (255,), width=ow)
        if kind in ("boss", "moss"):
            for a in range(0, 360, 30):  # crown spikes
                ax, ay = math.cos(math.radians(a)), math.sin(math.radians(a))
                d.line([c + ax * r * 0.7, c + ay * r * 0.7,
                        c + ax * r * 1.15, c + ay * r * 1.15],
                       fill=out + (255,), width=ow * 2)
        if kind == "moss":
            for a in range(0, 360, 60):
                ax, ay = math.cos(math.radians(a)), math.sin(math.radians(a))
                d.line([c, c, c + ax * r * 1.2, c + ay * r * 1.2],
                       fill=lighten(col, 0.25) + (230,), width=ow)
        eyes(d, c, c, r)
    elif kind in ("fast", "charger", "climber"):  # triangle pointing right (charger = bigger)
        s = 1.0 if kind == "fast" else 1.1
        poly(d, [(c + r * s, c), (c - r * 0.7, c - r * 0.85),
                 (c - r * 0.7, c + r * 0.85)], col, out)
        if kind in ("charger", "climber"):  # speed/claw streaks behind
            for dy in (-0.4, 0.0, 0.4):
                d.line([c - r * 0.8, c + r * dy, c - r * 1.2, c + r * dy],
                       fill=lighten(col, 0.3) + (230,), width=ow)
        eyes(d, c - int(r * 0.1), c, r * 0.7)
    elif kind in ("tank", "armored"):  # heavy rounded square / hexagon
        if kind == "tank":
            d.rounded_rectangle([c - r, c - r, c + r, c + r], radius=int(r * 0.3),
                                fill=col + (255,), outline=out + (255,), width=ow)
        else:
            pts = [(c + r * math.cos(math.radians(a)),
                    c + r * math.sin(math.radians(a))) for a in range(0, 360, 60)]
            poly(d, pts, col, out)
            d.ellipse([c - r * 0.5, c - r * 0.5, c + r * 0.5, c + r * 0.5],
                      outline=lighten(col, 0.2) + (255,), width=ow)
        eyes(d, c, c, r)
    elif kind == "flying":  # diamond with wings
        d.polygon([(c, c - r * 0.4), (c + r * 0.5, c), (c, c + r * 0.4),
                   (c - r * 0.5, c)], fill=lighten(col, 0.15) + (255,),
                  outline=out + (255,))
        for sx in (-1, 1):
            d.polygon([(c, c), (c + sx * r, c - r * 0.7),
                       (c + sx * r * 0.6, c)], fill=col + (200,),
                      outline=out + (255,))
        eyes(d, c, c - int(r * 0.05), r * 0.5)
    elif kind in ("invisible", "silencer"):  # faint dashed circle
        d.ellipse([c - r, c - r, c + r, c + r], fill=col + (90,))
        for a in range(0, 360, 25):
            ax, ay = math.cos(math.radians(a)), math.sin(math.radians(a))
            d.line([c + ax * r, c + ay * r, c + ax * r * 0.78, c + ay * r * 0.78],
                   fill=out + (200,), width=ow)
        if kind == "silencer":
            d.line([c - r * 0.45, c + r * 0.35, c + r * 0.45, c + r * 0.35],
                   fill=out + (240,), width=ow * 2)
        eyes(d, c, c, r * 0.8)
    return img


def main():
    for name, col in TOWERS.items():
        save(tower_sprite(col), os.path.join(OUT, "towers", f"{name}.png"))
    for name, col in ENEMIES.items():
        save(enemy_sprite(name, col), os.path.join(OUT, "enemies", f"{name}.png"))
    print(f"generated {len(TOWERS)} towers + {len(ENEMIES)} enemies into {OUT}")


if __name__ == "__main__":
    main()
