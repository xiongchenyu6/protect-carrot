#!/usr/bin/env python3
"""Flux img2img via the tunneled ComfyUI: nudge an input image into a new pose while
keeping the same character (Kontext-style consistency via low denoise).
Usage: comfy_img2img.py <input.png> <out.png> <seed> <denoise> "<prompt>"
"""
import json, sys, time, urllib.request, urllib.parse, os

HOST = "http://127.0.0.1:8188"
inp, out, seed, denoise, prompt = (
    sys.argv[1], sys.argv[2], int(sys.argv[3]), float(sys.argv[4]), sys.argv[5]
)

# 1) upload the input image to ComfyUI's input dir.
def upload(path):
    import mimetypes, uuid
    boundary = "----cg" + uuid.uuid4().hex
    fn = os.path.basename(path)
    body = b""
    body += f"--{boundary}\r\n".encode()
    body += f'Content-Disposition: form-data; name="image"; filename="{fn}"\r\n'.encode()
    body += b"Content-Type: image/png\r\n\r\n"
    body += open(path, "rb").read() + b"\r\n"
    body += f"--{boundary}\r\n".encode()
    body += b'Content-Disposition: form-data; name="overwrite"\r\n\r\ntrue\r\n'
    body += f"--{boundary}--\r\n".encode()
    req = urllib.request.Request(
        HOST + "/upload/image", data=body,
        headers={"Content-Type": f"multipart/form-data; boundary={boundary}"},
    )
    return json.load(urllib.request.urlopen(req, timeout=60))["name"]

name = upload(inp)
g = {
    "1": {"class_type": "CheckpointLoaderSimple", "inputs": {"ckpt_name": "flux1-dev-fp8.safetensors"}},
    "2": {"class_type": "CLIPTextEncode", "inputs": {"text": prompt, "clip": ["1", 1]}},
    "3": {"class_type": "FluxGuidance", "inputs": {"guidance": 2.8, "conditioning": ["2", 0]}},
    "4": {"class_type": "CLIPTextEncode", "inputs": {"text": "", "clip": ["1", 1]}},
    "10": {"class_type": "LoadImage", "inputs": {"image": name}},
    "11": {"class_type": "VAEEncode", "inputs": {"pixels": ["10", 0], "vae": ["1", 2]}},
    "6": {"class_type": "KSampler", "inputs": {"seed": seed, "steps": 22, "cfg": 1.0,
          "sampler_name": "euler", "scheduler": "simple", "denoise": denoise,
          "model": ["1", 0], "positive": ["3", 0], "negative": ["4", 0], "latent_image": ["11", 0]}},
    "7": {"class_type": "VAEDecode", "inputs": {"samples": ["6", 0], "vae": ["1", 2]}},
    "8": {"class_type": "SaveImage", "inputs": {"filename_prefix": "walk", "images": ["7", 0]}},
}
req = urllib.request.Request(HOST + "/prompt", data=json.dumps({"prompt": g}).encode(),
                            headers={"Content-Type": "application/json"})
pid = json.load(urllib.request.urlopen(req, timeout=30))["prompt_id"]
for _ in range(600):
    time.sleep(2)
    h = json.load(urllib.request.urlopen(f"{HOST}/history/{pid}", timeout=30))
    if pid in h:
        for node in h[pid]["outputs"].values():
            for img in node.get("images", []):
                u = f"{HOST}/view?filename={urllib.parse.quote(img['filename'])}&subfolder={urllib.parse.quote(img.get('subfolder',''))}&type={img.get('type','output')}"
                data = urllib.request.urlopen(u, timeout=60).read()
                open(out, "wb").write(data)
                print("SAVED", out, len(data), flush=True); sys.exit(0)
        sys.exit(1)
print("TIMEOUT", flush=True); sys.exit(2)
