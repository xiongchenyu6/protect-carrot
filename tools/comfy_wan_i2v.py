#!/usr/bin/env python3
"""WAN 2.2 image-to-video via the tunneled ComfyUI: animate a still illustration into
a short clip (two-stage high/low-noise MoE). Downloads every output frame.
Usage: comfy_wan_i2v.py <start.png> <out_dir> <seed> <length> <W> <H> "<prompt>"
"""
import json, sys, time, urllib.request, urllib.parse, os, uuid

HOST = "http://127.0.0.1:8188"
inp, out_dir, seed, length, W, H, prompt = (
    sys.argv[1], sys.argv[2], int(sys.argv[3]), int(sys.argv[4]),
    int(sys.argv[5]), int(sys.argv[6]), sys.argv[7],
)
os.makedirs(out_dir, exist_ok=True)

def upload(path):
    boundary = "----cg" + uuid.uuid4().hex
    fn = os.path.basename(path)
    body = b"".join([
        f"--{boundary}\r\n".encode(),
        f'Content-Disposition: form-data; name="image"; filename="{fn}"\r\n'.encode(),
        b"Content-Type: image/png\r\n\r\n", open(path, "rb").read(), b"\r\n",
        f"--{boundary}\r\n".encode(),
        b'Content-Disposition: form-data; name="overwrite"\r\n\r\ntrue\r\n',
        f"--{boundary}--\r\n".encode(),
    ])
    req = urllib.request.Request(HOST + "/upload/image", data=body,
        headers={"Content-Type": f"multipart/form-data; boundary={boundary}"})
    return json.load(urllib.request.urlopen(req, timeout=60))["name"]

name = upload(inp)
NEG = "static, still, frozen, blurry, low quality, watermark, text, distorted, flicker"
g = {
    "1": {"class_type": "UNETLoader", "inputs": {"unet_name": "wan2.2_i2v_high_noise_14B_fp8_scaled.safetensors", "weight_dtype": "fp8_e4m3fn"}},
    "2": {"class_type": "UNETLoader", "inputs": {"unet_name": "wan2.2_i2v_low_noise_14B_fp8_scaled.safetensors", "weight_dtype": "fp8_e4m3fn"}},
    "3": {"class_type": "CLIPLoader", "inputs": {"clip_name": "umt5_xxl_fp8_e4m3fn_scaled.safetensors", "type": "wan"}},
    "4": {"class_type": "VAELoader", "inputs": {"vae_name": "wan_2.1_vae.safetensors"}},
    "5": {"class_type": "CLIPTextEncode", "inputs": {"text": prompt, "clip": ["3", 0]}},
    "6": {"class_type": "CLIPTextEncode", "inputs": {"text": NEG, "clip": ["3", 0]}},
    "7": {"class_type": "LoadImage", "inputs": {"image": name}},
    "8": {"class_type": "WanImageToVideo", "inputs": {"positive": ["5", 0], "negative": ["6", 0], "vae": ["4", 0], "width": W, "height": H, "length": length, "batch_size": 1, "start_image": ["7", 0]}},
    "9": {"class_type": "KSamplerAdvanced", "inputs": {"model": ["1", 0], "add_noise": "enable", "noise_seed": seed, "steps": 20, "cfg": 3.5, "sampler_name": "euler", "scheduler": "simple", "positive": ["8", 0], "negative": ["8", 1], "latent_image": ["8", 2], "start_at_step": 0, "end_at_step": 10, "return_with_leftover_noise": "enable"}},
    "10": {"class_type": "KSamplerAdvanced", "inputs": {"model": ["2", 0], "add_noise": "disable", "noise_seed": seed, "steps": 20, "cfg": 3.5, "sampler_name": "euler", "scheduler": "simple", "positive": ["8", 0], "negative": ["8", 1], "latent_image": ["9", 0], "start_at_step": 10, "end_at_step": 20, "return_with_leftover_noise": "disable"}},
    "11": {"class_type": "VAEDecode", "inputs": {"samples": ["10", 0], "vae": ["4", 0]}},
    "12": {"class_type": "SaveImage", "inputs": {"filename_prefix": "intro", "images": ["11", 0]}},
}
req = urllib.request.Request(HOST + "/prompt", data=json.dumps({"prompt": g}).encode(),
    headers={"Content-Type": "application/json"})
pid = json.load(urllib.request.urlopen(req, timeout=30))["prompt_id"]
print("queued", pid, flush=True)
for _ in range(1200):  # up to ~40 min
    time.sleep(2)
    h = json.load(urllib.request.urlopen(f"{HOST}/history/{pid}", timeout=30))
    if pid in h:
        st = h[pid].get("status", {})
        if st.get("status_str") == "error":
            print("ERROR", json.dumps(st)[:600], flush=True); sys.exit(3)
        n = 0
        for node in h[pid]["outputs"].values():
            for img in node.get("images", []):
                u = f"{HOST}/view?filename={urllib.parse.quote(img['filename'])}&subfolder={urllib.parse.quote(img.get('subfolder',''))}&type={img.get('type','output')}"
                data = urllib.request.urlopen(u, timeout=120).read()
                open(os.path.join(out_dir, f"frame_{n:03d}.png"), "wb").write(data)
                n += 1
        print("SAVED", n, "frames ->", out_dir, flush=True)
        sys.exit(0 if n > 0 else 1)
print("TIMEOUT", flush=True); sys.exit(2)
