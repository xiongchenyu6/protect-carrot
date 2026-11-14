#!/usr/bin/env node
// Run in the Nix shell. Pass a checkout of patch-gg/pixelfx at the pinned revision.
import { mkdirSync, writeFileSync, copyFileSync } from 'node:fs';
import { resolve, dirname, join } from 'node:path';
import { pathToFileURL, fileURLToPath } from 'node:url';
import { execFileSync, spawnSync } from 'node:child_process';

const revision = '044e12a45dc9cc312cec50ee606dfc98afe5adc2';
const source = resolve(process.argv[2] ?? '');
if (!process.argv[2] || execFileSync('git', ['-C', source, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim() !== revision) {
  throw new Error(`Pass a PixelFX checkout at ${revision}`);
}
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const output = join(root, 'assets', 'pixelfx');
const { allIds, loadEffect, renderFrames, spriteSheet } = await import(pathToFileURL(join(source, 'tools/lib/fxlib.mjs')));
mkdirSync(output, { recursive: true });
const effects = [];
for (const id of allIds()) {
  const fx = await loadEffect(id);
  const rendered = renderFrames(fx, { bg: 'none', actor: false, seed: 7 });
  const sheet = spriteSheet(fx, rendered, id.split('/')[1]);
  const destination = join(output, `${id}.webp`);
  mkdirSync(dirname(destination), { recursive: true });
  const converted = spawnSync('python3', ['-c',
    'import io,sys; from PIL import Image; Image.open(io.BytesIO(sys.stdin.buffer.read())).save(sys.argv[1], format="WEBP", lossless=True, exact=True, method=6)',
    destination], { input: sheet.png, maxBuffer: 16 * 1024 * 1024 });
  if (converted.status !== 0) throw new Error(`${id}: ${converted.stderr}`);
  const meta = sheet.json.meta.pixelfx;
  const contact = fx.mod.act?.hits?.find(hit => hit.t >= 0)?.t ?? 0;
  effects.push({ id, size: meta.frameSize, pivot: meta.pivot, fps: meta.fps,
    frames: meta.frames, columns: meta.columns, looping: meta.loop, contact });
  console.log(`${id} ${meta.frames} frames`);
}
writeFileSync(join(output, 'catalog.json'), JSON.stringify({ revision, effects }, null, 2) + '\n');
copyFileSync(join(source, 'LICENSE'), join(output, 'LICENSE'));
copyFileSync(join(source, 'NOTICE.md'), join(output, 'NOTICE.md'));
console.log(`Imported ${effects.length} effects at ${revision}`);
