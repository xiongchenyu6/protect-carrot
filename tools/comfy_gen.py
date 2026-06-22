#!/usr/bin/env python3
"""Generate one image via the tunneled ComfyUI (localhost:8188) Flux workflow.
Usage: comfy_gen.py <out_path.png> <seed> "<prompt>"  (env W/H/STEPS optional)"""
import json, sys, time, urllib.request, urllib.parse, os

HOST = "http://127.0.0.1:8188"
out, seed, prompt = sys.argv[1], int(sys.argv[2]), sys.argv[3]
W = int(os.environ.get("W", "512")); H = int(os.environ.get("H", "512")); STEPS = int(os.environ.get("STEPS", "16"))
g = {
    "1": {"class_type": "CheckpointLoaderSimple", "inputs": {"ckpt_name": "flux1-dev-fp8.safetensors"}},
    "2": {"class_type": "CLIPTextEncode", "inputs": {"text": prompt, "clip": ["1", 1]}},
    "3": {"class_type": "FluxGuidance", "inputs": {"guidance": 3.5, "conditioning": ["2", 0]}},
    "4": {"class_type": "CLIPTextEncode", "inputs": {"text": "", "clip": ["1", 1]}},
    "5": {"class_type": "EmptySD3LatentImage", "inputs": {"width": W, "height": H, "batch_size": 1}},
    "6": {"class_type": "KSampler", "inputs": {"seed": seed, "steps": STEPS, "cfg": 1.0, "sampler_name": "euler", "scheduler": "simple", "denoise": 1.0, "model": ["1", 0], "positive": ["3", 0], "negative": ["4", 0], "latent_image": ["5", 0]}},
    "7": {"class_type": "VAEDecode", "inputs": {"samples": ["6", 0], "vae": ["1", 2]}},
    "8": {"class_type": "SaveImage", "inputs": {"filename_prefix": "carrot", "images": ["7", 0]}},
}
req = urllib.request.Request(HOST + "/prompt", data=json.dumps({"prompt": g}).encode(), headers={"Content-Type": "application/json"})
pid = json.load(urllib.request.urlopen(req, timeout=30))["prompt_id"]
print("queued", pid, flush=True)
for _ in range(600):
    time.sleep(2)
    h = json.load(urllib.request.urlopen(f"{HOST}/history/{pid}", timeout=30))
    if pid in h:
        for node in h[pid]["outputs"].values():
            for img in node.get("images", []):
                u = f"{HOST}/view?filename={urllib.parse.quote(img['filename'])}&subfolder={urllib.parse.quote(img.get('subfolder',''))}&type={img.get('type','output')}"
                data = urllib.request.urlopen(u, timeout=60).read()
                os.makedirs(os.path.dirname(out) or ".", exist_ok=True)
                open(out, "wb").write(data)
                print("SAVED", out, len(data), flush=True); sys.exit(0)
        sys.exit(1)
print("TIMEOUT", flush=True); sys.exit(2)
