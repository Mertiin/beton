# Engine source guide

These notes walk through every file under `src/` for someone who has **not** worked on low-level graphics before. There is one page per source file. Each page is meant to stand alone: you can open one file’s doc, read it top to bottom, and come away knowing what that file is for, how it fits the rest of the engine, and what gotchas to watch for.

You do **not** need to understand the GPU pages to ship a game. Start with the loop and game types. Come back to `render/` when you want to know *why* something appears on screen the way it does.

## Mental model

Your game talks in **world units** — abstract numbers you invent (`bird.y += 1.0`). The engine turns those into **pixels on a monitor**.

That translation has three layers:

| Layer | You think in… | Engine types |
|-------|---------------|--------------|
| Gameplay | “the bird is here, pipes move left” | `World`, `GameObject`, components, `Input`, `Animation` |
| Presentation | “draw this image here, this big, this tint” | `Scene`, `Sprite`, `Font`, `Camera` |
| Audio | “play this clip once / looped” | `Audio`, `SoundId`, `SoundHandle`, `VoiceId` |
| GPU | “upload buffers, run shaders, present” | `Gfx`, `Renderer`, `shader.wgsl` (mostly hidden) |

Each frame roughly does:

1. Read keyboard into `Input`
2. Clear the one-frame draw list (`Scene`)
3. Drain network events into `Net`
4. Call your `Game::update` — you move objects, push sprites/text, read/send net bytes
5. Drop unread net events
6. Resolve each game object’s world pose and copy its sprite into the scene
7. Resolve textures/sounds (load on use, unload after a short unused pause)
8. Sort by Z, upload geometry, draw, present

```
┌─────────────┐     ┌──────────────────┐     ┌─────────────────┐
│  Your Game  │ ──► │  World + Scene   │ ──► │  Renderer/wgpu  │
│ boot/update │     │  Camera, Gfx     │     │  shader.wgsl    │
└─────────────┘     └──────────────────┘     └─────────────────┘
     logic               “what to show”         “how to show it”
```

## Coordinate systems (read once)

Graphics stacks juggle several spaces. In this engine you mostly care about two:

1. **World space** — where you place objects. Origin `(0,0,0)` is usually the middle of the playfield. +X right, +Y up, +Z toward the default camera.
2. **Clip / NDC space** — what the GPU uses after the camera matrix (−1…1-ish). You never set this yourself; `Camera::view_proj` produces the matrix that gets you there.

**Pixels** only appear when you pick a window size (`Config`) or load a texture (`width × height`). A sprite’s `size` is in **world units**, not pixels. With an orthographic camera of height `20`, a sprite of size `[2, 2]` covers 10% of the screen height — regardless of whether the window is 480 or 1080 pixels tall.

## Read in this order

One markdown file per source file. A module that is only `mod.rs` keeps a single page (`app.md`). A folder with several files gets a page for each (`object/world.md`, `platform/native.md`).

| Order | Doc | Source | What it is |
|------:|-----|--------|------------|
| 1 | [lib.md](lib.md) | `src/lib.rs` | Public exports — the whole API surface |
| 2 | [core/mod.md](core/mod.md) | `src/core/mod.rs` | Module map under `core` |
| 3 | [core/app.md](core/app.md) | `src/core/app/mod.rs` | Window, clock, `Game` / `Context` / `run` |
| 4 | [core/platform/mod.md](core/platform/mod.md) | `src/core/platform/mod.rs` | Desktop vs web facade (only `cfg`) |
| 5 | [core/platform/native.md](core/platform/native.md) | `src/core/platform/native.rs` | Desktop backends |
| 6 | [core/platform/web.md](core/platform/web.md) | `src/core/platform/web.rs` | Browser backends |
| 7 | [core/object/mod.md](core/object/mod.md) | `src/core/object/mod.rs` | Object module glue |
| 8 | [core/object/game_object.md](core/object/game_object.md) | `src/core/object/game_object.rs` | `GameObject` fields and AABB |
| 9 | [core/object/world.md](core/object/world.md) | `src/core/object/world.rs` | Spawn, parents, poses |
| 10 | [core/object/component.md](core/object/component.md) | `src/core/object/component.rs` | Sparse sets and `Query` |
| 11 | [core/object/velocity.md](core/object/velocity.md) | `src/core/object/velocity.rs` | Shared linear speed + integrator |
| 12 | [core/object/rigid.md](core/object/rigid.md) | `src/core/object/rigid.rs` | Rigid body material + AABB step |
| 13 | [core/scene.md](core/scene.md) | `src/core/scene/mod.rs` | Sprites, fonts, one-frame draw list |
| 14 | [core/camera.md](core/camera.md) | `src/core/camera/mod.rs` | How world space maps into the window |
| 15 | [core/input.md](core/input.md) | `src/core/input/mod.rs` | Keys held / pressed, cursor |
| 16 | [core/net.md](core/net.md) | `src/core/net/mod.rs` | Host / client WebSocket messages |
| 17 | [core/anim.md](core/anim.md) | `src/core/anim/mod.rs` | Sprite sheets and frame clips |
| 18 | [core/audio.md](core/audio.md) | `src/core/audio/mod.rs` | Register clips, play / pin / resolve |
| 19 | [core/asset.md](core/asset.md) | `src/core/asset.rs` | Shared load / grace / unload rules |
| 20 | [core/math/vec3.md](core/math/vec3.md) | `src/core/math/vec3.rs` | Positions and directions |
| 21 | [core/math/mat4.md](core/math/mat4.md) | `src/core/math/mat4.rs` | Camera matrices |
| 22 | [core/math/rng.md](core/math/rng.md) | `src/core/math/rng.rs` | Tiny random numbers |
| 23 | [core/math/mod.md](core/math/mod.md) | `src/core/math/mod.rs` | Math module glue |
| 24 | [core/render/texture.md](core/render/texture.md) | `src/core/render/texture.rs` | Register / pin / resolve textures |
| 25 | [core/render/mod.md](core/render/mod.md) | `src/core/render/mod.rs` | The renderer (internal) |
| 26 | [core/render/shader.md](core/render/shader.md) | `src/core/render/shader.wgsl` | GPU vertex + fragment programs |

## Glossary

| Term | Plain meaning |
|------|----------------|
| **Texture** | An image that can live in GPU memory. You get a `TextureId`; pin with `TextureHandle`. |
| **Clip / SoundId** | Registered audio source. Decoded PCM appears on first play or pin. |
| **Voice / VoiceId** | One playing instance of a clip. Stop or change its volume. |
| **Component** | Extra typed data on a game object (not a field on `GameObject`). |
| **Sprite** | “Draw this texture as a rectangle here, this big.” |
| **Quad** | Two triangles forming a rectangle — how every sprite is drawn. |
| **UV** | Coordinates into a texture, usually `0…1`. Used to show one cell of an atlas. |
| **Atlas / sheet** | One image packing many frames or glyphs side by side. |
| **Orthographic** | Flat camera: no perspective shrink. Ideal for 2D. |
| **Perspective** | 3D camera: farther things look smaller. |
| **dt** | Delta time — seconds since the last frame. Multiply speeds by `dt`. |
| **Vsync / Fifo** | Present mode that waits for the monitor refresh so you do not tear. |
| **wgpu** | Rust library that talks to Vulkan / Metal / DX12. Lives under `render/`. |
| **Bind group** | A GPU “folder” of resources (camera matrix, or a texture+sampler) the shader can read. |
| **Pipeline** | Fixed recipe: which shader, how to blend, how to interpret vertices. |
| **Generational id** | Handle that becomes invalid after despawn, even if the slot is reused. |

## How this repo is laid out

```
crates/beton/     ← the engine (this docs folder lives here)
  src/lib.rs           ← public exports
  src/core/            ← app, object, render, audio, …
games/flappy/          ← sample game that only uses the public API
games/gallery/         ← demo of texture/sound pin + unload-after-pause
```

The Flappy sample is the best “worked example.” When a doc says “see Flappy,” open `games/flappy/src/main.rs`. For asset residency, run `gallery`.
