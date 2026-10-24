# gallery

Demo of texture and sound **register → load on use → unload after 2s unused**, with **RAM reporting**.

## Controls

| Key | Action |
|-----|--------|
| `1` | Red room (`room-red.jpg`) |
| `2` | Blue room (`room-blue.jpg`) |
| `3` | Green room (`room-green.jpg`) |
| `0` | Lobby (drop room pins) |
| `Esc` | Quit |

## What to watch

- **HUD** — two digits: textures live, audio live (capped at 9).
- **Console** — every second and on room change:

```text
tick room=red tex=3 (7.92 MB)  audio=1 (10.6 KB)  retained_src tex=12.5 KB audio=16.0 KB  total_res=7.93 MB
```

| Field | Meaning |
|-------|---------|
| `tex=N (X MB)` | Resident textures on the GPU (RGBA8 ≈ width×height×4) |
| `audio=N (X KB)` | Decoded PCM in RAM |
| `retained_src` | CPU bytes kept for re-upload/re-decode (`create_texture` / `Audio::create` sources; path-backed photos are 0 here) |
| `total_res` | Resident texture + audio bytes |

Each room photo is **1920×1080**. Once pinned/drawn it costs about **7.9 MB** on the GPU. Leave the room (`0`) and wait ~2 seconds: that ~7.9 MB drops off `total_res`.

Photos are from [Lorem Picsum](https://picsum.photos); see `assets/README.md`.

```powershell
cargo run -p gallery
```
