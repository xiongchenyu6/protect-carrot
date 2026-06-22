#!/usr/bin/env python3
"""Flux.1-Kontext image edit via the tunneled ComfyUI: keep the SAME character from an
input image and change its pose/action per the prompt (used to make hero walk frames).
Usage: comfy_kontext.py <input.png> <out.png> <seed> "<edit prompt>"
"""
import json, sys, time, urllib.request, urllib.parse, os, uuid

HOST = "http://127.0.0.1:8188"
inp, out, seed, prompt = sys.argv[1], sys.argv[2], int(sys.argv[3]), sys.argv[4]
guidance = float(sys.argv[5]) if len(sys.argv) > 5 else 2.5


def upload(path):
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
    "1": {"class_type": "UNETLoader", "inputs": {"unet_name": "flux1-dev-kontext_fp8_scaled.safetensors", "weight_dtype": "default"}},
    "2": {"class_type": "DualCLIPLoader", "inputs": {"clip_name1": "clip_l.safetensors", "clip_name2": "t5xxl_fp8_e4m3fn.safetensors", "type": "flux"}},
    "3": {"class_type": "VAELoader", "inputs": {"vae_name": "ae.safetensors"}},
    "10": {"class_type": "LoadImage", "inputs": {"image": name}},
    "11": {"class_type": "FluxKontextImageScale", "inputs": {"image": ["10", 0]}},
    "12": {"class_type": "VAEEncode", "inputs": {"pixels": ["11", 0], "vae": ["3", 0]}},
    "4": {"class_type": "CLIPTextEncode", "inputs": {"text": prompt, "clip": ["2", 0]}},
    "13": {"class_type": "ReferenceLatent", "inputs": {"conditioning": ["4", 0], "latent": ["12", 0]}},
    "5": {"class_type": "FluxGuidance", "inputs": {"guidance": guidance, "conditioning": ["13", 0]}},
    "6": {"class_type": "ConditioningZeroOut", "inputs": {"conditioning": ["4", 0]}},
    "7": {"class_type": "KSampler", "inputs": {"seed": seed, "steps": 20, "cfg": 1.0,
          "sampler_name": "euler", "scheduler": "simple", "denoise": 1.0,
          "model": ["1", 0], "positive": ["5", 0], "negative": ["6", 0], "latent_image": ["12", 0]}},
    "8": {"class_type": "VAEDecode", "inputs": {"samples": ["7", 0], "vae": ["3", 0]}},
    "9": {"class_type": "SaveImage", "inputs": {"filename_prefix": "kontext", "images": ["8", 0]}},
}
req = urllib.request.Request(HOST + "/prompt", data=json.dumps({"prompt": g}).encode(),
                            headers={"Content-Type": "application/json"})
pid = json.load(urllib.request.urlopen(req, timeout=30))["prompt_id"]
for _ in range(900):
    time.sleep(2)
    h = json.load(urllib.request.urlopen(f"{HOST}/history/{pid}", timeout=30))
    if pid in h:
        st = h[pid].get("status", {})
        if st.get("status_str") == "error":
            print("ERROR", json.dumps(st)[:400], flush=True); sys.exit(3)
        for node in h[pid]["outputs"].values():
            for img in node.get("images", []):
                u = f"{HOST}/view?filename={urllib.parse.quote(img['filename'])}&subfolder={urllib.parse.quote(img.get('subfolder',''))}&type={img.get('type','output')}"
                data = urllib.request.urlopen(u, timeout=120).read()
                open(out, "wb").write(data)
                print("SAVED", out, len(data), flush=True); sys.exit(0)
        sys.exit(1)
print("TIMEOUT", flush=True); sys.exit(2)
