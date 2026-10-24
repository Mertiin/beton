# `src/core/render/mod.rs`

## Purpose

This is the **internal renderer** — the code that talks to the GPU through [`wgpu`](https://wgpu.rs). Games never import it. `app.rs` owns a `Renderer` and calls `render()` after your `update`.

If you have never written graphics code, treat this file as a guided tour of what happens between “I pushed sprites” and “I see pixels.”

## The problem it solves

You have a list of sprites (center, size, UV, color, texture). The GPU does not understand sprites. It understands:

- **Buffers** of numbers (vertex positions, UVs, colors, indices)
- **Textures** and **samplers**
- **Pipelines** (which shader, how to blend, how to interpret vertex layouts)
- **Render passes** (clear the screen, draw, finish)
- **Present** (show the image in the window)

`Renderer` translates sprites → those concepts every frame.

## Major objects created at startup (`Renderer::new`)

| Object | Plain meaning |
|--------|----------------|
| `Instance` | Connection to graphics APIs available on this machine |
| `Surface` | “This window’s drawable images” (swapchain) |
| `Adapter` | The chosen GPU |
| `Device` / `Queue` | Submit work (`Device` creates resources; `Queue` uploads & submits commands) |
| `SurfaceConfiguration` | Size, format, **present mode `Fifo` (vsync)** |
| `ShaderModule` | Compiled WGSL from `shader.wgsl` |
| `BindGroupLayout`s | Schemas: group 0 = camera; group 1 = texture+sampler |
| `RenderPipeline` | Fixed state: topology, blend, depth, vertex layout |
| `camera_buffer` | 64-byte uniform for `view_proj` |
| Static unit-quad vertex + index buffers | One quad reused for every sprite via GPU instancing |
| Growing instance buffer | Per-sprite center, size, rotation, UV, color |
| Depth texture | Companion depth buffer matching the window size |
| `Gfx` | Texture store returned alongside the renderer |

If adapter request fails, the error message mentions needing a working Vulkan (etc.) driver — common on fresh Linux installs or VMs without GPU passthrough.

## What one frame does (`render`)

### 1. Acquire the next swapchain image

```text
surface.get_current_texture()
```

Outcomes map to `FrameError`:

| Result | Meaning | App reaction |
|--------|---------|--------------|
| Success / Suboptimal | Got a frame | Draw |
| Timeout / Occluded | Not ready (minimized, etc.) | Retry next chance |
| Outdated | Size/format mismatch | Reconfigure surface |
| Lost / Validation | Fatal surface problem | Exit |

### 2. Upload the camera matrix

Writes `view_proj` into `camera_buffer` — 16 floats the vertex shader reads as `camera.view_proj`.

### 3. Pack sprites into instances

Each sprite becomes **one instance** of **14 floats**: center3 + size2 + rotation + uv4 + color4.

The GPU already has a **static unit quad** (corners ±0.5, corner UVs 0/1). The vertex shader scales, rotates, and translates those corners per instance.

Unit-quad corner order (Y-up, v=0 at top):

```text
0 --- 1     0 = top-left, 1 = top-right
|   / |     2 = bottom-right, 3 = bottom-left
| /   |
3 --- 2
```

Indices: `0,1,2` and `0,2,3` (fixed; never rebuilt).

`pack_instances` builds **runs of the same texture** so the draw loop can bind once and `draw_indexed(0..6, …, instance_range)`. Sorting by Z (done earlier in `Scene`) can break perfect batching if textures interleave — correctness of order wins over batching here.

### 4. Grow the instance buffer if needed

The instance buffer starts small and resizes to the next power of two when a crowded frame appears. The unit quad buffers stay fixed.

### 5. Begin a render pass

- Clear color to a dark gray `(0.08, 0.08, 0.09)`
- Clear depth to `1.0`
- Set pipeline, camera bind group, unit-quad vertex/index buffers, instance buffer

### 6. Draw each texture run

```text
set_bind_group(1, texture)
draw_indexed(0..6, 0, start..start+count)   // 1 quad × N instances
```

Missing textures are skipped.

### 7. Submit + present

```text
queue.submit(command_buffer)
queue.present(frame)
```

`Fifo` waits for vsync — typically caps FPS near the monitor refresh and prevents tearing.

## Pipeline choices (why the screen looks like this)

| Setting | Choice | Why |
|---------|--------|-----|
| Topology | Triangle list | Two tris per instance |
| Vertex step | Unit quad + instance attrs | GPU instancing; no CPU quad expansion |
| Blend | Alpha blending | Transparent PNGs / fades |
| Cull mode | None | 2D quads from either winding still draw |
| Depth compare | `Always` | Do not discard by depth |
| Depth write | Off | Painter’s algorithm via CPU Z-sort instead |

So Z-sorting in `Scene` is what actually controls overlap. The depth buffer is created and cleared, but not used to reject pixels. That keeps overlapping transparent sprites predictable (true depth testing + alpha is a harder problem).

## Resize path

On window resize, `resize` reconfigures the surface and recreates the depth buffer to match. Drawing with a stale size causes `Outdated` frames.

## How this connects to your game

```text
Game::update
    → pushes Sprites (scene + world copies)
Scene::take_sorted (by Z)
    → Renderer::render
        → pack_instances → instance buffer
        → shader.wgsl places unit-quad corners & samples
        → present to window
```

You never call `render` yourself.

## Learning path if you want to go deeper

1. Read [shader.md](shader.md) — what the GPU runs per vertex/pixel  
2. Read `push_instance` / `pack_instances` in this file — CPU side of instance data  
3. Skim wgpu’s “hello triangle” tutorial — same concepts, fewer abstractions  
4. Change clear color or blend mode once to see the effect  

## Common failure modes

| Symptom | Likely cause |
|---------|--------------|
| Black / empty window | Camera looking wrong way; all sprites off-screen; textures failed to load |
| Crash on startup about adapter | GPU driver / backend missing |
| Stretch wrong after resize | Aspect updated via `aspect()` — if you hardcode aspect in game code, stop |
| Flicker when minimized | `Occluded` → Retry; normal |
| Extreme GPU use | Uncapped poll loop — see `Config.max_fps` in [app.md](../app.md) |

## Related docs

- [texture.md](texture.md) — `Gfx` side  
- [shader.md](shader.md) — WGSL program  
- [app.md](../app.md) — when `render` is invoked  
- [camera.md](../camera.md) — source of `view_proj`  
