#!/usr/bin/env python3
"""Generate story voiceover WAV files.

This uses the edge-tts Python package because it does not need a project secret.
Install it into a temporary venv before running when the system Python is managed
by Nix:

    python3 -m venv tmp/tts-venv
    tmp/tts-venv/bin/pip install edge-tts
    tmp/tts-venv/bin/python tools/gen_story_voice.py

Outputs are loaded by src/ui.rs from assets/audio/story/.
"""

from __future__ import annotations

import asyncio
import subprocess
from pathlib import Path

try:
    import edge_tts
except ImportError as exc:
    raise SystemExit(
        "Missing edge-tts. Run:\n"
        "  python3 -m venv tmp/tts-venv\n"
        "  tmp/tts-venv/bin/pip install edge-tts\n"
        "  tmp/tts-venv/bin/python tools/gen_story_voice.py"
    ) from exc


OUT_DIR = Path("assets/audio/story")

LINES = [
    {
        "file": "prologue_narrator.wav",
        "voice": "zh-CN-YunyangNeural",
        "rate": "+8%",
        "text": "月雾落下，边境塔阵全灭。古园中心，最后一枚萝卜核心还在发光。它不是粮食，是最后的虚空封印。",
    },
    {
        "file": "prologue_guardian.wav",
        "voice": "zh-CN-XiaoxiaoNeural",
        "rate": "+0%",
        "text": "你们想拔萝卜，也得先过我的塔阵。",
    },
    {
        "file": "prologue_warlord.wav",
        "voice": "zh-CN-YunxiNeural",
        "rate": "-8%",
        "text": "把它拔出来，世界就会安静。",
    },
    {
        "file": "endless_narrator.wav",
        "voice": "zh-CN-YunyangNeural",
        "rate": "+6%",
        "text": "封印没有被拔出来，但虚空已经学会绕路。敌潮不再排队，只会一波比一波更硬。",
    },
    {
        "file": "endless_guardian.wav",
        "voice": "zh-CN-XiaoxiaoNeural",
        "rate": "+0%",
        "text": "把塔阵接到核心上，能撑几波就撑几波。",
    },
    {
        "file": "endless_warlord.wav",
        "voice": "zh-CN-YunxiNeural",
        "rate": "-8%",
        "text": "甜味会散尽，守夜人也会。",
    },
]


async def generate_one(line: dict[str, str]) -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = OUT_DIR / line["file"]
    tmp = out.with_suffix(".mp3.tmp")
    communicate = edge_tts.Communicate(
        line["text"],
        line["voice"],
        rate=line["rate"],
        volume="+0%",
    )
    await communicate.save(str(tmp))
    subprocess.run(
        ["ffmpeg", "-y", "-v", "error", "-i", str(tmp), "-ac", "2", "-ar", "44100", str(out)],
        check=True,
    )
    tmp.unlink(missing_ok=True)
    print(f"ok {out}")


async def main() -> None:
    for line in LINES:
        await generate_one(line)


if __name__ == "__main__":
    asyncio.run(main())
