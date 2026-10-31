"""Install ComfyUI portable and the Klein sprite weights if they are missing.

Uses only the Python standard library plus curl.exe and 7zr. Safe to re-run:
files that are already large enough are left alone, and a live server on
port 8188 is not started a second time.
"""

import json
import subprocess
import sys
import time
import urllib.request
from pathlib import Path

ROOT = Path(r"D:/ComfyUI")
PORTABLE = ROOT / "ComfyUI_windows_portable"
MODELS = PORTABLE / "ComfyUI" / "models"
DOWNLOADS = ROOT / "downloads"
API = "http://127.0.0.1:8188"
SEVEN = DOWNLOADS / "7zr.exe"

# Min sizes reject an HTML error page saved under a weight's filename.
WEIGHTS = [
    (
        "https://huggingface.co/black-forest-labs/FLUX.2-klein-base-4b-fp8/resolve/main/flux-2-klein-base-4b-fp8.safetensors",
        MODELS / "diffusion_models" / "flux-2-klein-base-4b-fp8.safetensors",
        3_500_000_000,
    ),
    (
        "https://huggingface.co/Comfy-Org/Flux-2-Klein-4B/resolve/main/split_files/text_encoders/qwen_3_4b.safetensors",
        MODELS / "text_encoders" / "qwen_3_4b.safetensors",
        7_000_000_000,
    ),
    (
        "https://huggingface.co/Comfy-Org/Flux-2-Klein-4B/resolve/main/split_files/vae/flux2-vae.safetensors",
        MODELS / "vae" / "flux2-vae.safetensors",
        300_000_000,
    ),
    (
        "https://huggingface.co/svntax-dev/pixel_spritesheet_4walk_small_lora_v1/resolve/main/pixel_4walk_small_flux2_klein_base_4b_v1.safetensors",
        MODELS / "loras" / "pixel_4walk_small_flux2_klein_base_4b_v1.safetensors",
        80_000_000,
    ),
]


def http_ok(path):
    try:
        with urllib.request.urlopen(API + path, timeout=3) as resp:
            return resp.status == 200
    except Exception:
        return False


def download(url, dest, min_bytes):
    dest.parent.mkdir(parents=True, exist_ok=True)
    if dest.exists() and dest.stat().st_size >= min_bytes:
        print("have", dest.name, flush=True)
        return
    if dest.exists() and dest.stat().st_size < 100_000:
        dest.unlink()
    print("get", dest.name, flush=True)
    subprocess.check_call(
        [
            "curl.exe",
            "-L",
            "--fail",
            "--retry",
            "5",
            "--retry-delay",
            "3",
            "-A",
            "Mozilla/5.0",
            "-C",
            "-",
            "--output",
            str(dest),
            url,
        ]
    )
    size = dest.stat().st_size
    if size < min_bytes:
        raise SystemExit(f"{dest.name} is {size} bytes, expected at least {min_bytes}")


def latest_nvidia_archive():
    req = urllib.request.Request(
        "https://api.github.com/repos/Comfy-Org/ComfyUI/releases/latest",
        headers={
            "User-Agent": "brutal-gfx-setup",
            "Accept": "application/vnd.github+json",
        },
    )
    with urllib.request.urlopen(req) as resp:
        data = json.load(resp)
    for asset in data.get("assets", []):
        if asset.get("name") == "ComfyUI_windows_portable_nvidia.7z":
            return asset["browser_download_url"], int(asset["size"])
    raise SystemExit("latest ComfyUI release has no ComfyUI_windows_portable_nvidia.7z")


def install_portable():
    if (PORTABLE / "run_nvidia_gpu.bat").exists():
        print("have ComfyUI portable", flush=True)
        return
    DOWNLOADS.mkdir(parents=True, exist_ok=True)
    if not SEVEN.exists() or SEVEN.stat().st_size < 100_000:
        download("https://www.7-zip.org/a/7zr.exe", SEVEN, 100_000)
    url, size = latest_nvidia_archive()
    archive = DOWNLOADS / "ComfyUI_windows_portable_nvidia.7z"
    download(url, archive, size - 1024)
    print("extract", archive.name, flush=True)
    subprocess.check_call([str(SEVEN), "x", str(archive), f"-o{ROOT}", "-y"])
    if not (PORTABLE / "run_nvidia_gpu.bat").exists():
        raise SystemExit(f"extract finished but {PORTABLE} has no run_nvidia_gpu.bat")


def start_server():
    if http_ok("/system_stats"):
        print("server up", API, flush=True)
        return
    print("start", PORTABLE / "run_nvidia_gpu.bat", flush=True)
    subprocess.Popen(
        ["cmd", "/c", "start", "ComfyUI", "run_nvidia_gpu.bat"],
        cwd=str(PORTABLE),
    )
    deadline = time.time() + 240
    while time.time() < deadline:
        if http_ok("/system_stats"):
            print("server up", API, flush=True)
            return
        time.sleep(3)
    raise SystemExit("ComfyUI did not open port 8188. Read the ComfyUI window.")


def main():
    install_portable()
    for url, dest, min_bytes in WEIGHTS:
        download(url, dest, min_bytes)
    start_server()
    python = PORTABLE / "python_embeded" / "python.exe"
    print("python", python, flush=True)
    for folder in ("diffusion_models", "loras", "text_encoders", "vae"):
        names = sorted(p.name for p in (MODELS / folder).glob("*.safetensors"))
        print(folder, " ".join(names), flush=True)


if __name__ == "__main__":
    try:
        main()
    except subprocess.CalledProcessError as exc:
        sys.exit(exc.returncode or 1)
