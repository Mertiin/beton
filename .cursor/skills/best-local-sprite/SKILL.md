---
name: best-local-sprite
description: >-
  Sets up the local ComfyUI portable install and FLUX.2 Klein sprite weights,
  picks the best installed model, and generates a game sprite or sprite sheet.
  Use when the user asks to install or set up ComfyUI, Klein, or local image
  generation, or to generate, draw, or replace a sprite, sprite sheet,
  pixel-art character, tank, or other game image on this machine.
---

# Best local sprite

Set the stack up if it is missing, pick the best model that is installed, then generate. Do not attach a LoRA just because it exists.

Windows PowerShell 5.1: do not chain commands with `&&` or `||`.

## Set up

Run [scripts/setup_comfy.py](scripts/setup_comfy.py) with any Python. It is standard library only. Re-running it skips files that are already present.

```bash
python .cursor/skills/best-local-sprite/scripts/setup_comfy.py
```

What that script does:

1. If `D:/ComfyUI/ComfyUI_windows_portable/run_nvidia_gpu.bat` is missing, download the latest `ComfyUI_windows_portable_nvidia.7z` from the Comfy-Org/ComfyUI GitHub release and extract it with `7zr`. Use that NVIDIA archive, not the `cu126` or CPU builds. A 50-series GPU needs the current CUDA build. Do not install ComfyUI into a venv on the system Python; that Python is too new.
2. Download any missing Klein sprite weights into `ComfyUI/models/`: `flux-2-klein-base-4b-fp8.safetensors` (diffusion), `qwen_3_4b.safetensors` (text encoder), `flux2-vae.safetensors` (VAE), and `pixel_4walk_small_flux2_klein_base_4b_v1.safetensors` (LoRA, top-down people only).
3. If `http://127.0.0.1:8188/system_stats` is down, start `run_nvidia_gpu.bat` in its own window and wait until the port answers. Leave that window open. A log line that mentions `ImportError` for triton is not a failed start.

Do not download Klein 9B. Those weights are gated behind the FLUX non-commercial license. Download them only after the user has accepted that license.

## Choose the model

Read the script's printed file lists, or `GET /models/diffusion_models` and `/models/loras`.

| Job | Use |
| --- | --- |
| Side-view vehicle, prop, or anything that must match across frames (tank, gun, tread cycle) | Largest installed **FLUX.2 Klein base** (filename contains `klein-base`). No walk-cycle LoRA. One reference image, then build the sheet in code. |
| Top-down character walk, jump, and facing rows | Klein **4B base** plus `pixel_4walk_small_flux2_klein_base_4b_v1` if that LoRA is installed. 512×512, then downscale ×4. |
| Front/side/back turnaround of a person | `CharacterSheet` LoRA only when both that LoRA and Klein 9B base are installed. |
| One picture, no sheet | Largest installed Klein **base**. |

Rules:

- Prefer `klein-base` over `klein` distilled. Distilled checkpoints smear pixel sheets.
- A 16 GB GPU runs Klein 4B base. Use Klein 9B base only when that file is already on disk and a generation succeeds.
- Do not use the 4-walk LoRA for vehicles, guns, or side-view sheets. It was trained on tiny top-down people.

## Generate

Run [scripts/klein_t2i.py](scripts/klein_t2i.py) with the embedded Python the setup script printed. Pass `--unet`. Add `--lora` only when the table says to.

```bash
D:/ComfyUI/ComfyUI_windows_portable/python_embeded/python.exe .cursor/skills/best-local-sprite/scripts/klein_t2i.py --unet flux-2-klein-base-4b-fp8.safetensors --prefix ref --seed 1101 --prompt "..."
```

Default graph: Euler, Flux2 scheduler, CFG 5, 512×512, 20 steps, text encoder `qwen_3_4b.safetensors`, VAE `flux2-vae.safetensors`. Look at the PNGs in `ComfyUI/output` and keep the one that matches the request. Generate two more seeds if the first is wrong.

## Turn a reference into a beton sheet

The engine samples one equal-cell grid. `SpriteSheet` insets each cell by 2%, so for a tight pixel sheet set UV yourself and do not use that inset.

For a vehicle with a rotating gun:

1. Key the flat background to alpha. Klein magenta is not pure `#FF00FF`; key `r > 180`, `g < 90`, `b > 70`.
2. Crop to the hull. Remove any gun drawn on the hull.
3. Downscale with nearest-neighbor so one cell is the on-screen size (this game is about 2 screen pixels per world unit). A block vote eats thin barrels.
4. Build four drive frames by shifting track pixels. Do not re-roll the hull.
5. Recolor armor for the second player. Leave tracks and the gun dark.
6. Put the gun in its own cell, breech at the cell center, tube pointing right.
7. Layout: row 0 player A frames, row 1 player B frames, row 2 column 0 the gun. Save one PNG.

Wire one texture. Body UV is `(frame, player)`. Gun UV is `(0, 2)` and is not flipped; aim rotation points it. Muzzle distance is `gun_pixels / cell_width * body_width`.
