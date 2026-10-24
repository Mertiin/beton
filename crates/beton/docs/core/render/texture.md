# `src/core/render/texture.rs`

## Purpose

Register images for later GPU use, and give game code a cheap `TextureId` so it never touches raw `wgpu` objects. Decoded pixels live on the GPU only while something **references** the texture (or you pin it).

Games call `ctx.gfx.load_texture` / `load_texture_bytes` / `create_texture` / `pin`.

## Register vs resident

```
boot:   load_texture / create_texture  →  TextureId (source registered, no GPU upload yet)
        pin(id)                        →  TextureHandle (keeps it resident)
frame:  sprite uses id                 →  resolve uploads if needed
        unused for 2 seconds           →  resolve drops the GPU texture, keeps the source
```

Holding only a `TextureId` in your game struct does **not** keep the texture loaded. A live sprite, a scene draw this frame, or a `TextureHandle` does.

## `TextureId` / `TextureHandle`

| Type | Role |
|------|------|
| `TextureId` | `Copy` generational id for sprites / sheets / fonts |
| `TextureHandle` | Owning pin. `Clone` adds a reference; drop removes one. `.id()` for drawing |

`from_raw(index)` builds generation 0 (tests). Prefer ids from `Gfx`.

A stale or unloaded id makes the renderer **skip** that sprite (`bind_group` is `None`). `texture_size` returns `(0, 0)`.

## `Gfx::load_texture(path)`

Uses preloaded [`Game::assets`](../app.md) bytes when present; otherwise checks that the path exists on disk. The same path returns the same id. PNG/JPEG decode happens on first use (or immediately when served from preloaded bytes).

**Features:** `png` and `jpeg` are on by default.

## `Gfx::load_texture_bytes(bytes)`

Decodes PNG/JPEG from memory now, then stores RGBA like `create_texture`.

## `Gfx::create_texture(width, height, rgba)`

Validates and stores the bytes. GPU upload waits until first use. Requirements:

- `width > 0`, `height > 0`
- `rgba.len() == width * height * 4`
- Row-major RGBA8

## `Gfx::pin(id)`

```rust
let pipe = ctx.gfx.load_texture(&path)?;
let pipe_pin = ctx.gfx.pin(pipe).expect("live id");
// sprites still use `pipe` (the id)
```

Flappy and zombies pin boot assets so a one-frame gap never reloads them from disk.

## Resolve (engine)

Each frame the app calls `gfx.ensure_resident(used_ids)` **before** present, then `gfx.reap(used_ids, dt)` **after** present:

- Referenced and not resident → decode / upload
- Resident and unreferenced → add `dt`; after **2 seconds** unused, drop the GPU texture
- Path or bytes stay registered so the same id can load again

## Memory helpers

| Method | Meaning |
|--------|---------|
| `live_count()` | How many textures are on the GPU |
| `resident_bytes()` | Approx VRAM: sum of `width * height * 4` for residents |
| `retained_source_bytes()` | CPU bytes kept for `create_texture` re-upload (paths contribute 0) |

## Related docs

- [mod.md](mod.md) — renderer
- [../object/world.md](../object/world.md) — sprites on game objects
- [../audio.md](../audio.md) — same lifetime rules for sound
