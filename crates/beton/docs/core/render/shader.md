# `src/core/render/shader.wgsl`

## Purpose

This is the program that runs on the **GPU**. It is written in **WGSL** (WebGPU Shading Language), the language `wgpu` accepts.

CPU code decides *what* to draw (lists of sprites as **instances**). The shader decides *how each corner and each pixel* turns into a color on screen.

You can make entire games without editing this file. Read it when you want the missing link between matrices / UVs and actual pixels.

## Two stages

Graphics pipelines traditionally split work:

```text
┌──────────────────┐         ┌───────────────────┐
│  Vertex shader   │  ───►   │ Fragment shader   │
│  (per corner ×   │         │ (per pixel)       │
│   per instance)  │         │                   │
└──────────────────┘         └───────────────────┘
   place the quad               color the quad
```

### Vertex shader — `vs`

Runs once per unit-quad corner per instance (4 × N sprites).

**Vertex attrs** (from the static unit quad):

| Field | Meaning |
|-------|---------|
| `corner` | Local xy in ±0.5 |
| `corner_uv` | 0/1 corner UV (v=0 is top) |

**Instance attrs** (one per sprite):

| Field | Meaning |
|-------|---------|
| `center` | World-space sprite center |
| `size` | Full width/height |
| `rotation` | Radians |
| `uv_rect` | `(u0, v0, u1, v1)` atlas rect |
| `color` | RGBA tint |

Output (`VsOut`):

| Field | Meaning |
|-------|---------|
| `clip` | Position in clip space (`@builtin(position)`) — required |
| `uv`, `color` | Passed through to the fragment stage |

Placement:

1. Scale local corner by `size`
2. Rotate by `rotation`
3. Add `center.xy`
4. Map `corner_uv` through `uv_rect` into atlas UVs
5. Multiply by `camera.view_proj`

```wgsl
out.clip = camera.view_proj * vec4<f32>(world_xy, in.center.z, 1.0);
```

The `1.0` in `w` marks it as a **position** (directions would use `0.0` so translation does not apply).

### Fragment shader — `fs`

Runs **once per pixel** covered by the rasterized triangles (thousands of times per sprite).

```wgsl
return textureSample(image_tex, image_sampler, in.uv) * in.color;
```

1. Sample the bound texture at the interpolated UV  
2. Multiply by the interpolated vertex color (tint)  
3. That RGBA is written with alpha blending into the framebuffer  

Interpolation: UVs and colors at the three triangle vertices are blended across the triangle’s interior automatically — that is why a quad can show a smooth region of a texture.

## Bind groups — “what the shader can see”

WGSL declares resources with `@group` and `@binding`. Those must match the layouts created in `render/mod.rs`.

### Group 0 — camera (vertex stage)

```wgsl
@group(0) @binding(0) var<uniform> camera: Camera;
```

One matrix shared by all draws in the frame. Set once per frame from CPU.

### Group 1 — image (fragment stage)

```wgsl
@group(1) @binding(0) var image_tex: texture_2d<f32>;
@group(1) @binding(1) var image_sampler: sampler;
```

Changes whenever the renderer switches to a sprite with a different `TextureId`.

If CPU and WGSL bindings disagree, pipeline creation fails at startup — loud and early, which is good.

## Data flow for one pixel of the bird

1. CPU uploads one instance (center, size, rotation, UV rect, color)  
2. Vertex shader places each unit-quad corner in world space, then applies `view_proj`  
3. Rasterizer figures out which pixels the two triangles cover  
4. For each pixel, fragment shader samples `bird-sheet.png` at the interpolated UV and multiplies by white tint  
5. Alpha blend composites over the cleared background / previously drawn sprites  

## Why the shader is tiny

This engine only draws **tinted textured quads**. No lights, no normal maps, no skinning, no post-process. Fewer concepts → shorter API → easier to learn.

When you add features later, this file grows first (e.g. a second texture for masks, a flash uniform, etc.), and `render/mod.rs` gains matching layouts.

## Editing checklist

If you change the shader:

1. Keep `@location` indices aligned with `VertexAttribute` in `render/mod.rs` (vertex 0–1, instance 2–6)  
2. Keep bind group indices aligned with `set_bind_group` calls  
3. Rebuild — WGSL is compiled at pipeline creation time  
4. Expect either a clean window or a clear creation error; silent “wrong colors” usually means a math bug, not a failed compile  

## Experiment ideas (safe learning)

- Return `vec4(1, 0, 0, 1)` from `fs` — everything solid red (proves fragments run)  
- Return `vec4(in.uv, 0, 1)` — visualize UVs as colors  
- Skip multiplying by `in.color` — tints stop working  

Revert when done; Flappy expects normal sampling.

## Related docs

- [mod.md](mod.md) — CPU pipeline & draw loop  
- [texture.md](texture.md) — what `image_tex` is bound to  
- [math/mat4.md](../math/mat4.md) — what `view_proj` contains  
- [scene.md](../scene.md) — where UV and color originate  
