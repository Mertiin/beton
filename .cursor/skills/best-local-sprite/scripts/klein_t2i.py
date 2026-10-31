"""Queue one FLUX.2 Klein text-to-image job and wait for the PNG."""

import argparse
import json
import sys
import time
import urllib.request

API = "http://127.0.0.1:8188"


def graph(args):
    prompt = {
        "1": {
            "class_type": "UNETLoader",
            "inputs": {"unet_name": args.unet, "weight_dtype": "default"},
        },
        "2": {
            "class_type": "CLIPLoader",
            "inputs": {
                "clip_name": args.clip,
                "type": "flux2",
                "device": "default",
            },
        },
        "3": {
            "class_type": "VAELoader",
            "inputs": {"vae_name": args.vae},
        },
        "4": {
            "class_type": "CLIPTextEncode",
            "inputs": {"clip": ["2", 0], "text": args.prompt},
        },
        "5": {
            "class_type": "CLIPTextEncode",
            "inputs": {"clip": ["2", 0], "text": args.negative},
        },
        "6": {
            "class_type": "EmptyFlux2LatentImage",
            "inputs": {"width": args.width, "height": args.height, "batch_size": 1},
        },
        "7": {
            "class_type": "KSamplerSelect",
            "inputs": {"sampler_name": "euler"},
        },
        "8": {
            "class_type": "Flux2Scheduler",
            "inputs": {"steps": args.steps, "width": args.width, "height": args.height},
        },
        "10": {
            "class_type": "RandomNoise",
            "inputs": {"noise_seed": args.seed},
        },
        "12": {
            "class_type": "VAEDecode",
            "inputs": {"samples": ["11", 0], "vae": ["3", 0]},
        },
        "13": {
            "class_type": "SaveImage",
            "inputs": {"images": ["12", 0], "filename_prefix": args.prefix},
        },
    }
    model = ["1", 0]
    if args.lora:
        prompt["14"] = {
            "class_type": "LoraLoaderModelOnly",
            "inputs": {"model": ["1", 0], "lora_name": args.lora, "strength_model": args.strength},
        }
        model = ["14", 0]
    prompt["9"] = {
        "class_type": "CFGGuider",
        "inputs": {
            "model": model,
            "positive": ["4", 0],
            "negative": ["5", 0],
            "cfg": args.cfg,
        },
    }
    prompt["11"] = {
        "class_type": "SamplerCustomAdvanced",
        "inputs": {
            "noise": ["10", 0],
            "guider": ["9", 0],
            "sampler": ["7", 0],
            "sigmas": ["8", 0],
            "latent_image": ["6", 0],
        },
    }
    return prompt


def post(path, payload=None):
    data = None if payload is None else json.dumps(payload).encode()
    req = urllib.request.Request(
        API + path,
        data=data,
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req) as resp:
        return json.load(resp)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--unet", required=True)
    parser.add_argument("--prompt", required=True)
    parser.add_argument("--prefix", default="sprite")
    parser.add_argument("--seed", type=int, default=1)
    parser.add_argument("--steps", type=int, default=20)
    parser.add_argument("--cfg", type=float, default=5.0)
    parser.add_argument("--width", type=int, default=512)
    parser.add_argument("--height", type=int, default=512)
    parser.add_argument("--clip", default="qwen_3_4b.safetensors")
    parser.add_argument("--vae", default="flux2-vae.safetensors")
    parser.add_argument("--lora", default="")
    parser.add_argument("--strength", type=float, default=1.0)
    parser.add_argument(
        "--negative",
        default="photorealistic, blurry, text, watermark, scenery, shadow, top-down",
    )
    args = parser.parse_args()
    queued = post("/prompt", {"prompt": graph(args)})
    if queued.get("node_errors"):
        print(json.dumps(queued["node_errors"], indent=2))
        sys.exit(1)
    pid = queued["prompt_id"]
    print("queued", pid, flush=True)
    while True:
        hist = post("/history/" + pid)
        if pid in hist:
            images = []
            for node in hist[pid].get("outputs", {}).values():
                for img in node.get("images", []):
                    images.append(img.get("filename"))
            status = hist[pid].get("status", {}).get("status_str")
            print(status, " ".join(images))
            sys.exit(0 if status == "success" and images else 1)
        time.sleep(2)


if __name__ == "__main__":
    main()
