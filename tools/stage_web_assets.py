#!/usr/bin/env python3
"""Publish runtime content only: referenced PixelFX clips, compressed music."""
from pathlib import Path
import re
import shutil

def stage(root=Path('.')):
    referenced=set()
    for source in (root/'src').rglob('*.rs'):
        referenced.update(re.findall(r'"([a-z-]+/[a-z-]+)"',source.read_text()))
    destination=root/'web/assets'
    shutil.rmtree(destination,ignore_errors=True)
    count=0
    for file in (root/'assets').rglob('*'):
        if not file.is_file(): continue
        path=file.relative_to(root/'assets')
        if path.as_posix()=='audio/bgm.wav' or path.parts[0]=='fonts': continue
        if path.parts[0]=='pixelfx' and path.suffix=='.webp' and path.with_suffix('').relative_to('pixelfx').as_posix() not in referenced: continue
        target=destination/path
        target.parent.mkdir(parents=True,exist_ok=True)
        shutil.copyfile(file,target);count+=1
    print(f'Staged {count} runtime assets')
if __name__=='__main__': stage()
