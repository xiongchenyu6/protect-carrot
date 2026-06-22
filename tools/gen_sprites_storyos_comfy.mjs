#!/usr/bin/env node
// Batch-generate Protect Carrot sprites through StoryOS's ComfyUI workflow.
//
// Requires a reachable ComfyUI compatible with StoryOS's FLUX2 Klein workflow:
//   COMFY_BASE_URL=https://comfy.example/api/comfy/v1 \
//   COMFY_API_TOKEN=optional-bearer-token \
//   node tools/gen_sprites_storyos_comfy.mjs all       # 19 towers + 16 archetype enemies + 20 equipment icons
//   node tools/gen_sprites_storyos_comfy.mjs species   # 100 monster species portraits
//   node tools/gen_sprites_storyos_comfy.mjs equipment # 20 persistent relic icons
//   node tools/gen_sprites_storyos_comfy.mjs full      # all of the above
//   node tools/gen_sprites_storyos_comfy.mjs manifest full  # write prompt manifest only
//   node tools/gen_sprites_storyos_comfy.mjs models    # inspect required Comfy models
//   COMFY_MODEL_ROOT=/path/to/ComfyUI/models \
//     COMFY_MODEL_URL_MANIFEST=tmp/comfy/model_urls.json \
//     node tools/gen_sprites_storyos_comfy.mjs prepare-models
//
// The script loads StoryOS's workflow JSON from:
//   ../Documents/github/xiongchenyu6/storyos/crates/storyos-image/src/workflows/flux2_klein.json
// Override with STORYOS_ROOT=/path/to/storyos.

import fs from 'node:fs/promises'
import { createWriteStream } from 'node:fs'
import { spawnSync } from 'node:child_process'
import path from 'node:path'
import process from 'node:process'
import { pipeline } from 'node:stream/promises'

const PROJECT_ROOT = path.resolve(import.meta.dirname, '..')
const STORYOS_ROOT =
  process.env.STORYOS_ROOT ||
  '/home/freeman.xiong/Documents/github/xiongchenyu6/storyos'
const WORKFLOW_PATH = path.join(
  STORYOS_ROOT,
  'crates/storyos-image/src/workflows/flux2_klein.json',
)
const STORYOS_MODEL_MANIFEST_PATH =
  process.env.STORYOS_MODEL_MANIFEST ||
  path.join(STORYOS_ROOT, 'docs/ops/comfy-model-manifest.json')

const MODEL_KEYS = ['unet_name', 'lora_name', 'vae_name', 'clip_name', 'ckpt_name']
const MODEL_KEY_CATEGORY = {
  unet_name: 'unet',
  lora_name: 'lora',
  vae_name: 'vae',
  clip_name: 'clip',
  ckpt_name: 'checkpoint',
}
const MODEL_CATEGORY_LOADER = {
  unet: 'UNETLoader',
  lora: 'LoraLoaderModelOnly',
  vae: 'VAELoader',
  clip: 'CLIPLoader',
  checkpoint: 'CheckpointLoaderSimple',
}
const MODEL_CATEGORY_DIR = {
  unet: 'unet',
  lora: 'loras',
  vae: 'vae',
  clip: 'clip',
  checkpoint: 'checkpoints',
}

const STYLE = [
  'top-down 2D tower-defense game sprite',
  'commercial mobile game quality',
  'Lovecraftian Cthulhu dark-fantasy theme',
  'bold readable silhouette',
  'clean outline',
  'soft painterly shading',
  'centered full body or full tower',
  'single isolated subject',
  'one pose',
  'transparent background',
  'no text',
  'no watermark',
  'no ground shadow',
  'square icon composition',
].join(', ')

const CHROMA_STYLE = [
  'top-down 2D tower-defense game sprite',
  'commercial mobile game quality',
  'Lovecraftian Cthulhu dark-fantasy theme',
  'bold readable silhouette',
  'clean outline',
  'soft painterly shading',
  'centered full body or full tower',
  'single isolated subject',
  'one pose',
  'perfectly flat solid bright magenta chroma-key background filling every background pixel',
  'no shadow',
  'no floor',
  'no background texture',
  'do not use magenta anywhere on the subject',
  'no text',
  'no watermark',
  'not a sprite sheet',
  'not a character turnaround',
  'square icon composition',
].join(', ')

const REMBG_STYLE = [
  'top-down 2D tower-defense game sprite',
  'commercial mobile game quality',
  'Lovecraftian Cthulhu dark-fantasy theme',
  'bold readable silhouette',
  'clean outline',
  'soft painterly shading',
  'centered full body or full tower',
  'single isolated subject',
  'one pose',
  'plain white studio background',
  'no scenery',
  'no atmosphere behind the subject',
  'no shadow',
  'no floor',
  'no text',
  'no watermark',
  'not a sprite sheet',
  'not a character turnaround',
  'square icon composition',
].join(', ')

const EQUIPMENT_STYLE = [
  '2D inventory item sprite for a commercial tower-defense game',
  'Lovecraftian Cthulhu dark-fantasy relic',
  'single physical equipment item only',
  'centered cutout object',
  'readable silhouette at small UI size',
  'painterly material detail',
  'subtle rim light',
  'transparent background',
  'no UI frame',
  'no square app icon',
  'no rounded rectangle border',
  'no flat vector symbol',
  'no text',
  'no watermark',
  'no ground shadow',
  'square icon composition',
].join(', ')

const EQUIPMENT_CHROMA_STYLE = [
  '2D inventory item sprite for a commercial tower-defense game',
  'Lovecraftian Cthulhu dark-fantasy relic',
  'single physical equipment item only',
  'centered cutout object',
  'readable silhouette at small UI size',
  'painterly material detail',
  'subtle rim light',
  'perfectly flat solid bright magenta chroma-key background filling every background pixel',
  'no UI frame',
  'no square app icon',
  'no rounded rectangle border',
  'no flat vector symbol',
  'no shadow',
  'no floor',
  'do not use magenta anywhere on the item',
  'no text',
  'no watermark',
  'not a sprite sheet',
  'square icon composition',
].join(', ')

const EQUIPMENT_REMBG_STYLE = [
  '2D inventory item sprite for a commercial tower-defense game',
  'Lovecraftian Cthulhu dark-fantasy relic',
  'single physical equipment item only',
  'centered cutout object',
  'readable silhouette at small UI size',
  'painterly material detail',
  'subtle rim light',
  'plain white studio background',
  'no UI frame',
  'no square app icon',
  'no rounded rectangle border',
  'no flat vector symbol',
  'no scenery',
  'no shadow',
  'no floor',
  'no text',
  'no watermark',
  'not a sprite sheet',
  'square icon composition',
].join(', ')

const NEGATIVE_PROMPT = [
  'text',
  'watermark',
  'logo',
  'signature',
  'UI frame',
  'sprite sheet',
  'character sheet',
  'turnaround',
  'multiple poses',
  'multiple views',
  'grid',
  'panels',
  'comic layout',
  'room layout',
  'floorplan',
  'building',
  'architecture',
  'window',
  'wall',
  'cage',
  'fence',
  'columns',
  'doors',
  'cropped subject',
  'multiple characters',
  'scene background',
  'ground plane',
  'cast shadow',
  'blurry',
  'low quality',
  'photorealistic photo',
].join(', ')

const TARGETS = {
  towers: {
    arrow: 'crossbow arrow tower, red lacquer, bone charms, practical early-game turret',
    cannon: 'iron cannon tower, orange firebox, riveted stone base, explosive artillery',
    magic: 'purple arcane tower with a floating crystal eye, occult runes',
    sniper: 'green long-range watchtower with a precision ballista, hunter optics',
    thunder: 'yellow storm tower with tesla coils and forked lightning',
    laser: 'pink eldritch laser obelisk with a focused lens aperture',
    missile: 'heavy 2x2 missile bunker with multiple rockets and warning paint',
    fortress: 'massive 2x2 fortress cannon, stone bastion, brass recoil rails',
    ice: 'blue ice tower, frozen crystal barrel, frost mist',
    wind: 'turquoise wind turbine tower, cyclone fins, ritual feathers',
    frostnova: 'ice nova obelisk, pale blue shockwave crystal crown',
    shadow: 'black obsidian shadow tower, purple smoke, sealed forbidden glyphs',
    holy: 'gold holy light tower, reliquary spire, radiant halo',
    detection: 'lavender detection tower with one large watchful mystic eye',
    poison: 'toxic alchemy tower with green vials, pipes, dripping venom',
    fire: 'flame tower with brazier core and dragon-mouth nozzle',
    summon: 'summoner totem tower with spectral guardian aura',
    prism: 'grand 3x3 cyan prism laser tower, crystalline beam splitter',
    necromancer: 'necromancer bone tower, skull lantern, green soul flame',
  },
  enemies: {
    normal: 'one squat red slug-like cult-mutated monster with eyes and small claws, organic creature only, no building, no grid',
    fast: 'thin orange skittering runner, many legs, fast silhouette',
    tank: 'one bulky purple armored beetle brute, shell plates, slow heavy monster, single creature only',
    flying: 'blue winged eye horror, bat wings, airborne creature',
    invisible: 'translucent grey wraith, fading edges, stealth monster',
    regenerating: 'one green slime blob horror with a pulsing regeneration core, single creature only',
    armored: 'grey plated skeleton knight creature, heavy armor shell',
    swarmer: 'tiny orange swarm insect horror, nimble and numerous',
    boss: 'huge dark-red old god boss, crown of spikes, many eyes',
    shielded: 'one blue mushroom-like monster inside one glowing bubble shield membrane, single creature only',
    splitter: 'one violet gelatin blob creature with cracks suggesting it will split later, single body only',
    healer: 'green robed cult healer monster with a sickly healing aura',
    charger: 'yellow charging beast with horns and speed streak posture',
    climber: 'one brown hooked-claw ghoul in a climbing pose, organic creature only, no wall, no tower, no cage',
    silencer: 'one purple hooded silent wraith with stitched mouth and anti-magic aura, single creature only',
    moss: 'single green-black tentacled eldritch boss monster, huge circular jaw, mossy fungal hide, no tower visible',
  },
  equipment: {
    rusty_sight: 'rusty brass gun sight relic, cracked glass reticle, common equipment icon',
    carrot_sigil: 'sealed carrot sigil amulet, wax stamp and green warding cord, common defensive relic icon',
    bone_fletching: 'ghoul bone arrow fletching bundle, carved white feathers, common ranged relic icon',
    saltpeter_keg: 'small saltpeter powder keg with black iron bands, uncommon artillery relic icon',
    prism_shard: 'fractured arcane prism shard, cyan and violet refraction, uncommon relic icon',
    frost_lens: 'frosted glass lens in silver occult frame, blue mist, uncommon relic icon',
    ember_core: 'glowing ember core crystal, orange flame heart, rare fire relic icon',
    venom_vial: 'deep abyss venom vial, green toxic liquid and cork, rare poison relic icon',
    thunder_coil: 'brass thunder coil with yellow lightning arcs, rare storm relic icon',
    shadow_seal: 'black wax shadow seal, purple forbidden glyph, rare relic icon',
    bulwark_plate: 'heavy fortress armor plate, riveted steel and occult scratches, epic relic icon',
    clockwork_trigger: 'mad inventor clockwork trigger assembly, exposed gears, epic relic icon',
    witch_salt: 'white witch salt crystal cluster in a small ritual pouch, epic relic icon',
    deep_one_scale:
      'single large iridescent deep one fish scale charm, cold blue-green shimmer, pierced with a small cord, not armor clothing, not chainmail, epic relic icon',
    cultist_manual:
      'closed yellow leather-bound cultist manual book, brass corner caps, forbidden bookmark, no readable letters, no shield badge, legendary relic icon',
    star_metal_barrel: 'star metal cannon barrel, polished meteor iron, legendary weapon relic icon',
    void_capacitor: 'void capacitor canister, violet energy between brass terminals, legendary relic icon',
    sainted_gear: 'sainted golden gear reliquary, white glow, legendary defensive relic icon',
    kraken_heart: 'pulsing kraken heart, toxic green veins and tentacle roots, mythic relic icon',
    azathoth_eye: 'azathoth eye relic, cosmic iris, gold frame and forbidden aura, mythic relic icon',
  },
}

const KIND_HINTS = {
  Normal: 'blood-red crawling lesser horror',
  Fast: 'fast skittering runner horror',
  Tank: 'heavy slow brute horror',
  Flying: 'winged airborne eye horror',
  Invisible: 'translucent stealth wraith horror',
  Regenerating: 'regenerating slime flesh horror',
  Armored: 'armored plated undead horror',
  Swarmer: 'tiny swarm creature horror',
  Boss: 'large old-god boss horror',
  Shielded: 'shielded monster with glowing barrier',
  Splitter: 'monster cracking into smaller bodies',
  Healer: 'cult healer monster with sickly aura',
  Charger: 'charging horned siege beast',
  Climber: 'wall-climbing tower-attacking ghoul',
  Silencer: 'silent anti-magic wraith with stitched mouth',
  Moss: 'tower-eating tentacled MOSS boss',
}

const SPECIES_CREATURE_GUARD = [
  'living eldritch monster creature',
  'organic anatomy with a readable head or core and limbs, claws, wings, tendrils, or body segments',
  'full-body creature sprite, not an object',
  'not architecture',
  'not an actual tower',
  'not a wall section',
  'not a statue',
  'not a shield emblem',
  'not a portal',
  'not an abstract symbol',
].join(', ')

const SPECIES_PROMPT_OVERRIDES = {
  '002':
    'clustered salt-marsh larval horror swarm, many tiny wet bodies forming one readable creature mass, no pattern tile',
  '010':
    'hook-clawed moss-covered ghoul in a climbing pose, visible arms and legs, siege monster that attacks towers, no wall or building',
  '021':
    'red tide explosive flesh-pod monster with cracked organic shell and small legs, no ring icon',
  '031':
    'shielded bone-plated turtle horror, living body behind a curved mithril bone carapace, no flat emblem',
  '032':
    'beacon-eyed cult horror with a lighthouse-like glowing head, humanoid monster body, no actual lighthouse or tower',
  '038':
    'long hooked-claw wall-leaper ghoul, hunched body with scythe arms, no tower or wall',
  '039':
    'blue glass insect swarm condensed into one crystalline bug silhouette, no snowflake or mandala',
  '047':
    'moss-marked siege ghoul with stone-breaking claws and a heavy body, no tower or pillar',
  '054':
    'thorny centipede aristocrat crawling monster, many hooked legs, no vertical thorn wall object',
  '056':
    'plague-crowned flying parasite with wings and a living skull body, no crown-only object',
  '058':
    'bulky ice cave shield brute carrying an icy carapace, living monster body, no shield icon',
  '062':
    'storm-charged wingless airborne eel horror with lightning fins and a face, no tower or antenna object',
  '065':
    'armored iron-hook siege ghoul with oversized hook claws, full creature body, no weapon-only object',
  '075':
    'hunched wall-eating brick gnawer monster with claws and a huge jaw, no wall section or tower',
  '079':
    'deep-sea white medic horror, pale abyssal healer creature with fins, mask-like face, and tentacle hands, no foggy rectangle or background panel',
  '084':
    'sleepless stalactite swarm monster, hanging bat-insect cluster with eyes and teeth, no mineral column',
  '085':
    'rotting star regenerating mutant with pulsing core, limbs, and slime growths, no stacked object',
  '087':
    'old-wall gnawing noble ghoul with tattered mantle, claws, and hunched body, no masonry tower',
  '094':
    'fiery red-star furnace boss as a living brute with furnace chest and arms, no furnace object',
  '099':
    'final sleeping god beneath the seal, huge tentacled eldritch monster with eyes and maw, no totem or pillar',
}

async function loadSpeciesTargets() {
  const src = await fs.readFile(path.join(PROJECT_ROOT, 'src/monster.rs'), 'utf8')
  const lines = src.split(/\r?\n/)
  const out = {}
  let block = null
  for (const line of lines) {
    if (line.trim() === 'sp!(') {
      block = [line]
      continue
    }
    if (!block) continue
    block.push(line)
    if (line.trim() === '),') {
      const cleaned = block.map((l) => l.trim().replace(/,$/, ''))
      const id = Number(cleaned[1])
      const quoted = cleaned
        .filter((l) => /^".*"$/.test(l))
        .map((l) => l.slice(1, -1))
      const name = quoted[0]
      const tags = quoted.at(-1) || ''
      const kind = cleaned[3]
      const key = String(id).padStart(3, '0')
      const hint = KIND_HINTS[kind] || 'Lovecraftian monster'
      const subject = SPECIES_PROMPT_OVERRIDES[key] || `${hint}, unique monster species portrait`
      out[key] = `${name}, ${tags}, ${subject}, ${SPECIES_CREATURE_GUARD}`
      block = null
    }
  }
  if (Object.keys(out).length !== 100) {
    throw new Error(`Expected 100 monster species, parsed ${Object.keys(out).length}`)
  }
  return out
}

const mode = process.argv[2] || 'all'
const modelMode = mode === 'models' || mode === 'prepare-models'
const downloadModels = mode === 'prepare-models' || process.env.COMFY_DOWNLOAD_MODELS === '1'
const dryRun = mode === 'manifest' || process.env.COMFY_DRY_RUN === '1'
const targetMode = mode === 'manifest' ? process.argv[3] || 'full' : mode
const onlyName = mode === 'manifest' ? process.argv[4] || null : process.argv[3] || null
const baseUrl = (process.env.COMFY_BASE_URL || '').replace(/\/+$/, '')
const token = process.env.COMFY_API_TOKEN || null
const workflowMode = process.env.COMFY_WORKFLOW || 'storyos'
const checkpointName = process.env.COMFY_CHECKPOINT || 'sd_xl_base_1.0.safetensors'
const bgRemove = process.env.COMFY_BG_REMOVE || (workflowMode === 'checkpoint' ? 'rembg' : 'none')
const chromaKey = bgRemove === 'chroma'
const rembg = bgRemove === 'rembg'
const imageSize = Number(process.env.COMFY_SIZE || (workflowMode === 'checkpoint' ? 512 : 1024))
const steps = Number(process.env.COMFY_STEPS || (workflowMode === 'checkpoint' ? 18 : 20))
const cfg = Number(process.env.COMFY_CFG || (workflowMode === 'checkpoint' ? 7 : 5))

if (!baseUrl && !dryRun && !modelMode) {
  console.error('Set COMFY_BASE_URL first.')
  process.exit(2)
}

const workflowTemplate =
  dryRun || modelMode
    ? null
    : workflowMode === 'storyos'
      ? JSON.parse(await fs.readFile(WORKFLOW_PATH, 'utf8'))
    : makeCheckpointWorkflow()

if (!['storyos', 'checkpoint'].includes(workflowMode)) {
  console.error('COMFY_WORKFLOW must be storyos or checkpoint.')
  process.exit(2)
}

function headers(extra = {}) {
  const h = { ...extra }
  if (token) h.Authorization = `Bearer ${token}`
  return h
}

function cloneWorkflow() {
  return JSON.parse(JSON.stringify(workflowTemplate))
}

function makeCheckpointWorkflow() {
  return {
    1: {
      class_type: 'CheckpointLoaderSimple',
      inputs: {
        ckpt_name: checkpointName,
      },
    },
    2: {
      class_type: 'CLIPTextEncode',
      inputs: {
        text: '',
        clip: ['1', 1],
      },
    },
    3: {
      class_type: 'CLIPTextEncode',
      inputs: {
        text: '__NEGATIVE__',
        clip: ['1', 1],
      },
    },
    4: {
      class_type: 'EmptyLatentImage',
      inputs: {
        width: imageSize,
        height: imageSize,
        batch_size: 1,
      },
    },
    5: {
      class_type: 'KSampler',
      inputs: {
        seed: 0,
        steps,
        cfg,
        sampler_name: 'euler',
        scheduler: 'normal',
        denoise: 1,
        model: ['1', 0],
        positive: ['2', 0],
        negative: ['3', 0],
        latent_image: ['4', 0],
      },
    },
    6: {
      class_type: 'VAEDecode',
      inputs: {
        samples: ['5', 0],
        vae: ['1', 2],
      },
    },
    8: {
      class_type: 'SaveImage',
      inputs: {
        filename_prefix: 'protect_carrot_sprite',
        images: ['6', 0],
      },
    },
  }
}

function applyParams(workflow, prompt, seed, width, height) {
  for (const node of Object.values(workflow)) {
    if (!node || typeof node !== 'object' || !node.inputs) continue
    switch (node.class_type) {
      case 'CLIPTextEncode':
        node.inputs.text = node.inputs.text === '__NEGATIVE__' ? NEGATIVE_PROMPT : prompt
        break
      case 'RandomNoise':
        node.inputs.noise_seed = seed
        break
      case 'KSampler':
        node.inputs.seed = seed
        node.inputs.steps = steps
        node.inputs.cfg = cfg
        break
      case 'Flux2Scheduler':
      case 'EmptyFlux2LatentImage':
      case 'EmptyLatentImage':
        if ('width' in node.inputs) node.inputs.width = width
        if ('height' in node.inputs) node.inputs.height = height
        break
      case 'SaveImage':
        node.inputs.filename_prefix = 'protect_carrot_sprite'
        break
    }
  }
  return workflow
}

async function comfyJson(subpath, init = {}) {
  const resp = await fetch(`${baseUrl}/${subpath.replace(/^\/+/, '')}`, {
    ...init,
    headers: headers({
      ...(init.headers || {}),
      ...(init.body ? { 'Content-Type': 'application/json' } : {}),
    }),
  })
  if (!resp.ok) {
    throw new Error(`${subpath} ${resp.status}: ${await resp.text()}`)
  }
  return resp.json()
}

async function comfyBytes(view) {
  const qs = new URLSearchParams({
    filename: view.filename,
    subfolder: view.subfolder || '',
    type: view.type || 'output',
  })
  const resp = await fetch(`${baseUrl}/view?${qs}`, { headers: headers() })
  if (!resp.ok) throw new Error(`/view ${resp.status}: ${await resp.text()}`)
  return Buffer.from(await resp.arrayBuffer())
}

function firstImageOutput(history) {
  for (const node of Object.values(history.outputs || {})) {
    if (node?.images?.length) return node.images[0]
  }
  return null
}

async function waitForImage(promptId) {
  const deadline = Date.now() + Number(process.env.COMFY_TIMEOUT_MS || 180000)
  while (Date.now() < deadline) {
    const history = await comfyJson(`history/${promptId}`)
    const item = history[promptId]
    if (item) {
      const image = firstImageOutput(item)
      if (image) return image
      if (item.status?.status_str === 'error') {
        throw new Error(`Comfy workflow failed: ${JSON.stringify(item.status)}`)
      }
    }
    await new Promise((resolve) => setTimeout(resolve, 1000))
  }
  throw new Error(`Timed out waiting for Comfy prompt ${promptId}`)
}

function seedFor(key) {
  let h = 2166136261
  for (const ch of key) {
    h ^= ch.charCodeAt(0)
    h = Math.imul(h, 16777619)
  }
  return h >>> 0
}

async function generate(kind, name, subject) {
  const prompt = promptFor(kind, subject)
  const workflow = applyParams(
    cloneWorkflow(),
    prompt,
    seedFor(`${kind}/${name}`),
    imageSize,
    imageSize,
  )
  const queued = await comfyJson('prompt', {
    method: 'POST',
    body: JSON.stringify({ prompt: workflow, client_id: `protect-carrot-${Date.now()}` }),
  })
  const image = await waitForImage(queued.prompt_id)
  const bytes = await comfyBytes(image)
  const out = path.join(PROJECT_ROOT, 'assets/sprites', kind, `${name}.png`)
  await fs.mkdir(path.dirname(out), { recursive: true })
  if (chromaKey || rembg) {
    const raw = path.join(PROJECT_ROOT, 'tmp/comfy/generated_raw', kind, `${name}.png`)
    await fs.mkdir(path.dirname(raw), { recursive: true })
    await fs.writeFile(raw, bytes)
    if (rembg) {
      removeBackground(raw, out)
    } else {
      removeChroma(raw, out)
    }
  } else {
    await fs.writeFile(out, bytes)
  }
  console.log(`ok ${kind}/${name} -> ${path.relative(PROJECT_ROOT, out)}`)
}

function styleFor(kind) {
  if (kind === 'equipment') {
    return rembg ? EQUIPMENT_REMBG_STYLE : chromaKey ? EQUIPMENT_CHROMA_STYLE : EQUIPMENT_STYLE
  }
  const style = rembg ? REMBG_STYLE : chromaKey ? CHROMA_STYLE : STYLE
  if (kind !== 'species') return style
  return style
    .replace('centered full body or full tower', 'centered full body creature')
    .replace('single isolated subject', 'single isolated living creature')
    .replace('centered full body or full tower', 'centered full body creature')
}

function promptFor(kind, subject) {
  const style = styleFor(kind)
  return `${subject}. ${style}`
}

function promptReport(mode, entries) {
  const lines = [
    '# Comfy Prompt Manifest',
    '',
    `Mode: ${mode}`,
    `Total prompts: ${entries.length}`,
    '',
  ]
  for (const kind of ['towers', 'enemies', 'equipment', 'species']) {
    const group = entries.filter((entry) => entry.kind === kind)
    if (group.length === 0) continue
    lines.push(`## ${kind}`, '')
    for (const entry of group) {
      lines.push(`### ${entry.name}`)
      lines.push(`- Output: \`${entry.output}\``)
      lines.push(`- Seed: \`${entry.seed}\``)
      lines.push(`- Prompt: ${entry.prompt}`)
      lines.push('')
    }
  }
  return `${lines.join('\n')}\n`
}

function removeBackground(input, out) {
  const candidates = [
    process.env.PYTHON,
    path.join(PROJECT_ROOT, 'tmp/comfy/.venv/bin/python'),
    path.join(PROJECT_ROOT, '.venv/bin/python'),
    'python3',
  ].filter(Boolean)
  const code = `
from io import BytesIO
from pathlib import Path
import numpy as np
from PIL import Image
from scipy import ndimage
from rembg import remove, new_session
src = Path(__import__('sys').argv[1])
out = Path(__import__('sys').argv[2])
out.parent.mkdir(parents=True, exist_ok=True)
session = new_session('u2netp')
data = remove(src.read_bytes(), session=session)
img = Image.open(BytesIO(data)).convert('RGBA')
arr = np.array(img)
mask = arr[:, :, 3] > 8
labels, count = ndimage.label(mask)
if count > 1:
    sizes = np.bincount(labels.ravel())
    sizes[0] = 0
    keep = int(sizes.argmax())
    arr[:, :, 3] = np.where(labels == keep, arr[:, :, 3], 0).astype(np.uint8)
    img = Image.fromarray(arr, 'RGBA')
bbox = img.getbbox()
if bbox:
    crop = img.crop(bbox)
    size = max(img.size)
    pad = int(size * 0.08)
    max_subject = max(1, size - pad * 2)
    scale = min(max_subject / crop.width, max_subject / crop.height)
    resized = crop.resize(
        (max(1, int(crop.width * scale)), max(1, int(crop.height * scale))),
        Image.Resampling.LANCZOS,
    )
    canvas = Image.new('RGBA', (size, size), (0, 0, 0, 0))
    canvas.alpha_composite(resized, ((size - resized.width) // 2, (size - resized.height) // 2))
    img = canvas
buf = BytesIO()
img.save(buf, format='PNG')
out.write_bytes(buf.getvalue())
print(out)
`
  let last = null
  for (const python of candidates) {
    const res = spawnSync(python, ['-c', code, input, out], { stdio: 'inherit' })
    if (res.status === 0) return
    last = res
  }
  throw new Error(`Failed to remove background from ${input}: ${last?.status ?? 'unknown'}`)
}

function removeChroma(input, out) {
  const home = process.env.HOME || '/home/freeman.xiong'
  const codexHome = process.env.CODEX_HOME || path.join(home, '.codex')
  const helper = path.join(
    codexHome,
    'skills/.system/imagegen/scripts/remove_chroma_key.py',
  )
  const candidates = [
    process.env.PYTHON,
    path.join(PROJECT_ROOT, '.venv/bin/python'),
    'python3',
  ].filter(Boolean)
  let last = null
  for (const python of candidates) {
    const res = spawnSync(
      python,
      [
        helper,
        '--input',
        input,
        '--out',
        out,
        '--key-color',
        '#ff00ff',
        '--auto-key',
        'border',
        '--soft-matte',
        '--transparent-threshold',
        '18',
        '--opaque-threshold',
        '180',
        '--edge-contract',
        '1',
        '--despill',
        '--force',
      ],
      { stdio: 'inherit' },
    )
    if (res.status === 0) return
    last = res
  }
  throw new Error(`Failed to remove chroma key from ${input}: ${last?.status ?? 'unknown'}`)
}

async function workflowForModelAudit() {
  if (workflowMode === 'storyos') {
    return JSON.parse(await fs.readFile(WORKFLOW_PATH, 'utf8'))
  }
  return makeCheckpointWorkflow()
}

function collectModelRefs(workflow) {
  const refs = []
  const seen = new Set()
  for (const [nodeId, node] of Object.entries(workflow)) {
    if (!node?.inputs) continue
    for (const key of MODEL_KEYS) {
      const name = node.inputs[key]
      if (typeof name !== 'string' || !name) continue
      const category = MODEL_KEY_CATEGORY[key]
      const id = `${category}:${name}`
      if (seen.has(id)) continue
      seen.add(id)
      refs.push({
        category,
        key,
        name,
        loader: MODEL_CATEGORY_LOADER[category],
        dir: MODEL_CATEGORY_DIR[category],
        nodeId,
        nodeType: node.class_type,
      })
    }
  }
  return refs
}

async function loadStoryosModelManifest() {
  try {
    return JSON.parse(await fs.readFile(STORYOS_MODEL_MANIFEST_PATH, 'utf8'))
  } catch {
    return null
  }
}

function manifestCoversModel(manifest, ref) {
  if (!manifest) return false
  return Array.isArray(manifest[ref.category]) && manifest[ref.category].includes(ref.name)
}

function sanitizeModelEnvName(name) {
  return name.toUpperCase().replace(/[^A-Z0-9]+/g, '_').replace(/^_+|_+$/g, '')
}

async function loadModelUrlMap() {
  const out = new Map()
  const inline = process.env.COMFY_MODEL_URLS
  if (inline) {
    addModelUrls(out, JSON.parse(inline))
  }
  const manifestPath = process.env.COMFY_MODEL_URL_MANIFEST
  if (manifestPath) {
    addModelUrls(out, JSON.parse(await fs.readFile(manifestPath, 'utf8')))
  }
  return out
}

function addModelUrls(out, data) {
  if (!data || typeof data !== 'object') return
  if (Array.isArray(data.models)) {
    for (const item of data.models) {
      if (item?.name && item?.url) out.set(item.name, item.url)
    }
  }
  for (const [key, value] of Object.entries(data)) {
    if (key === 'models') continue
    if (typeof value === 'string') {
      out.set(key, value)
    } else if (value && typeof value === 'object') {
      for (const [name, url] of Object.entries(value)) {
        if (typeof url === 'string') out.set(name, url)
      }
    }
  }
}

function modelUrlFor(ref, urlMap) {
  return (
    process.env[`COMFY_MODEL_URL_${sanitizeModelEnvName(ref.name)}`] ||
    urlMap.get(ref.name) ||
    null
  )
}

function modelRoots() {
  const home = process.env.HOME || '/home/freeman.xiong'
  const roots = [
    process.env.COMFY_MODEL_ROOT,
    process.env.COMFYUI_MODEL_ROOT,
    path.join(home, 'ComfyUI/models'),
    path.join(home, 'Documents/ComfyUI/models'),
    path.join(home, 'Documents/github/ComfyUI/models'),
    path.join(home, 'ComfyUI_windows_portable/ComfyUI/models'),
  ].filter(Boolean)
  return [...new Set(roots.map((root) => path.resolve(root)))]
}

async function pathExists(file) {
  try {
    await fs.access(file)
    return true
  } catch {
    return false
  }
}

async function findLocalModel(ref, roots) {
  for (const root of roots) {
    const candidates = [
      path.join(root, ref.dir, ref.name),
      path.join(root, ref.category, ref.name),
      path.join(root, ref.name),
    ]
    for (const file of candidates) {
      if (await pathExists(file)) return file
    }
  }
  return null
}

async function queryComfyAvailability(refs) {
  const status = new Map()
  if (!baseUrl) return status
  for (const [category, loader] of Object.entries(MODEL_CATEGORY_LOADER)) {
    const wanted = refs.filter((ref) => ref.category === category)
    if (wanted.length === 0) continue
    try {
      const resp = await fetch(`${baseUrl}/object_info/${loader}`, { headers: headers() })
      if (!resp.ok) {
        for (const ref of wanted) status.set(ref.name, { state: 'unknown', detail: `HTTP ${resp.status}` })
        continue
      }
      const text = await resp.text()
      for (const ref of wanted) {
        status.set(ref.name, { state: text.includes(ref.name) ? 'present' : 'missing' })
      }
    } catch (error) {
      for (const ref of wanted) status.set(ref.name, { state: 'unknown', detail: String(error) })
    }
  }
  return status
}

function firstWritableModelRoot(roots) {
  if (process.env.COMFY_MODEL_ROOT) return path.resolve(process.env.COMFY_MODEL_ROOT)
  if (process.env.COMFYUI_MODEL_ROOT) return path.resolve(process.env.COMFYUI_MODEL_ROOT)
  return roots[0] || null
}

async function downloadModel(ref, url, roots) {
  const root = firstWritableModelRoot(roots)
  if (!root) {
    throw new Error('Set COMFY_MODEL_ROOT=/path/to/ComfyUI/models before downloading models.')
  }
  const dest = path.join(root, ref.dir, ref.name)
  if (await pathExists(dest)) return dest
  await fs.mkdir(path.dirname(dest), { recursive: true })
  const tmp = `${dest}.part`
  const headers = {}
  const downloadToken = process.env.COMFY_MODEL_DOWNLOAD_TOKEN || process.env.HF_TOKEN
  if (downloadToken) headers.Authorization = `Bearer ${downloadToken}`
  console.log(`download ${ref.category}/${ref.name}`)
  const resp = await fetch(url, { headers })
  if (!resp.ok || !resp.body) {
    throw new Error(`Download failed for ${ref.name}: HTTP ${resp.status} ${await resp.text()}`)
  }
  await pipeline(resp.body, createWriteStream(tmp))
  await fs.rename(tmp, dest)
  return dest
}

function modelReportMarkdown(report) {
  const lines = [
    '# Comfy Model Report',
    '',
    `Workflow: \`${workflowMode}\``,
    `StoryOS manifest: \`${path.relative(PROJECT_ROOT, STORYOS_MODEL_MANIFEST_PATH)}\``,
    `Comfy base URL: ${baseUrl ? `\`${baseUrl}\`` : '_not set_'}`,
    `Local model roots: ${report.roots.length ? report.roots.map((root) => `\`${root}\``).join(', ') : '_none_'}`,
    '',
    '| Category | Model | Manifest | Server | Local | URL |',
    '|---|---|---:|---:|---:|---:|',
  ]
  for (const item of report.models) {
    lines.push(
      `| ${item.category} | \`${item.name}\` | ${item.manifest ? 'yes' : 'no'} | ${item.server} | ${item.local ? `\`${item.local}\`` : 'missing'} | ${item.url ? 'yes' : 'no'} |`,
    )
  }
  return `${lines.join('\n')}\n`
}

async function handleModelMode() {
  const workflow = await workflowForModelAudit()
  const refs = collectModelRefs(workflow)
  const manifest = await loadStoryosModelManifest()
  const roots = modelRoots()
  const server = await queryComfyAvailability(refs)
  const urls = await loadModelUrlMap()
  const models = []
  for (const ref of refs) {
    let local = await findLocalModel(ref, roots)
    const url = modelUrlFor(ref, urls)
    if (downloadModels && !local && url) {
      local = await downloadModel(ref, url, roots)
    }
    const serverStatus = server.get(ref.name)
    models.push({
      category: ref.category,
      name: ref.name,
      loader: ref.loader,
      nodeId: ref.nodeId,
      nodeType: ref.nodeType,
      manifest: manifestCoversModel(manifest, ref),
      server: serverStatus ? serverStatus.state : 'not-checked',
      server_detail: serverStatus?.detail || null,
      local,
      url: Boolean(url),
      target_dir: ref.dir,
    })
  }
  const report = {
    workflow: workflowMode,
    workflow_path: workflowMode === 'storyos' ? WORKFLOW_PATH : null,
    storyos_manifest_path: STORYOS_MODEL_MANIFEST_PATH,
    base_url: baseUrl || null,
    roots,
    downloaded: downloadModels,
    models,
  }
  const out = path.join(PROJECT_ROOT, 'tmp/comfy/model_report.json')
  const md = path.join(PROJECT_ROOT, 'tmp/comfy/model_report.md')
  await fs.mkdir(path.dirname(out), { recursive: true })
  await fs.writeFile(out, JSON.stringify(report, null, 2))
  await fs.writeFile(md, modelReportMarkdown(report))
  console.log(path.relative(PROJECT_ROOT, out))
  console.log(path.relative(PROJECT_ROOT, md))

  const missingManifest = models.filter((item) => !item.manifest)
  const missingServer = models.filter((item) => item.server === 'missing')
  const missingLocalForDownload = downloadModels && models.filter((item) => !item.local)
  if (missingManifest.length || missingServer.length || missingLocalForDownload.length) {
    if (missingManifest.length) {
      console.error(`Missing from StoryOS model manifest: ${missingManifest.map((m) => m.name).join(', ')}`)
    }
    if (missingServer.length) {
      console.error(`Missing on Comfy server: ${missingServer.map((m) => m.name).join(', ')}`)
    }
    if (missingLocalForDownload.length) {
      console.error(`Missing local files after prepare: ${missingLocalForDownload.map((m) => m.name).join(', ')}`)
    }
    process.exitCode = 1
  }
}

if (modelMode) {
  await handleModelMode()
  process.exit(process.exitCode ?? 0)
}

const speciesTargets =
  targetMode === 'species' || targetMode === 'full' ? await loadSpeciesTargets() : null

const batches =
  targetMode === 'all'
    ? ['towers', 'enemies', 'equipment']
    : targetMode === 'full'
      ? ['towers', 'enemies', 'equipment', 'species']
      : targetMode === 'towers' ||
          targetMode === 'enemies' ||
          targetMode === 'equipment' ||
          targetMode === 'species'
        ? [targetMode]
        : null

if (!batches && targetMode !== 'one') {
  console.error('Usage: node tools/gen_sprites_storyos_comfy.mjs [all|full|towers|enemies|equipment|species|one towers arrow|manifest full|models|prepare-models]')
  process.exit(2)
}

if (dryRun) {
  const entries = []
  for (const kind of batches) {
    const targets = kind === 'species' ? speciesTargets : TARGETS[kind]
    for (const [name, subject] of Object.entries(targets)) {
      if (onlyName && name !== onlyName) continue
      entries.push({
        kind,
        name,
        output: `assets/sprites/${kind}/${name}.png`,
        seed: seedFor(`${kind}/${name}`),
        prompt: promptFor(kind, subject),
      })
    }
  }
  const out = path.join(PROJECT_ROOT, 'tmp/comfy/prompt_manifest.json')
  const report = path.join(PROJECT_ROOT, 'tmp/comfy/prompt_report.md')
  await fs.mkdir(path.dirname(out), { recursive: true })
  await fs.writeFile(out, JSON.stringify({ mode: targetMode, total: entries.length, entries }, null, 2))
  await fs.writeFile(report, promptReport(targetMode, entries))
  console.log(`${path.relative(PROJECT_ROOT, out)} (${entries.length} prompts)`)
  console.log(path.relative(PROJECT_ROOT, report))
} else if (targetMode === 'one') {
  const kind = process.argv[3]
  const name = process.argv[4]
  if (!TARGETS[kind]?.[name]) {
    throw new Error(`Unknown target ${kind}/${name}`)
  }
  await generate(kind, name, TARGETS[kind][name])
} else {
  for (const kind of batches) {
    const targets = kind === 'species' ? speciesTargets : TARGETS[kind]
    for (const [name, subject] of Object.entries(targets)) {
      if (onlyName && name !== onlyName) continue
      await generate(kind, name, subject)
    }
  }
}
