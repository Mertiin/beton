# `src/render/texture.rs`

## Purpose

Move images from **disk / CPU memory** into **GPU memory**, and give game code a cheap handle (`TextureId`) so it never touches raw `wgpu` texture objects.

This is the only part of `render/` that games call directly (`ctx.gfx.load_texture` / `create_texture`).

## Why textures are a separate step

CPUs and GPUs have different memories. A PNG on disk is useless to a shader until you:

1. Decode it into a grid of pixels in RAM (RGBA bytes)
2. Allocate a GPU texture of that size
3. Copy (`upload`) the bytes to the GPU
4. Create a **sampler** (rules for how to read between pixels when the image is scaled)
5. Create a **bind group** (a small object that says “shader binding 0 = this texture, binding 1 = this sampler”)

`Gfx` does all of that and returns `TextureId(index)` into its internal `Vec`.

## `TextureId`

```rust
pub struct TextureId(pub(crate) u32);
```

- Cheap: `Copy`, compare with `==`
- Opaque: you cannot invent meaningful ids (except `from_raw` for tests)
- Stable for the lifetime of the `Gfx` / window — there is no “unload” API yet, so ids remain valid until the app exits

Invalid ids make the renderer **skip** draws that reference them (missing bind group), so you may see invisible sprites rather than a crash.

## `Gfx::load_texture(path)`

```rust
let bird = ctx.gfx.load_texture(Path::new("assets/bird-sheet.png"))?;
```

Steps inside:

1. `image::open` decodes PNG/JPEG (feature-gated)
2. Convert to RGBA8
3. Delegate to `create_texture`

**Paths:** Flappy's build script copies `assets/` next to the executable, and the game loads from that directory. A relative path such as `assets/bird-sheet.png` is resolved from the process working directory instead.

**Features:** `png` and `jpeg` are on by default. A headless tool that only uses `create_texture` can disable them to shrink deps.

## `Gfx::create_texture(width, height, rgba)`

Upload raw pixels you built yourself. Requirements:

- `width > 0`, `height > 0`
- `rgba.len() == width * height * 4`
- Each pixel is `R, G, B, A` bytes in that order, row-major, top row first (as produced by the `image` crate)

Use cases:

- 1×1 white pixel for tinted rectangles
- Procedural digit atlases (Flappy)
- Runtime-generated maps

### Size limits

The GPU advertises `max_texture_dimension_2d`. Oversized images return a clear error instead of failing obscurely later.

## Filtering — why “nearest”?

When a 34×24 bird is drawn across many screen pixels, the GPU must invent intermediate colors.

| Filter | Look |
|--------|------|
| **Nearest** (this engine) | Hard pixels — crisp pixel art, readable tiny fonts |
| Linear | Soft blur — nicer for photos, mushy for 4×5 digit glyphs |

Samplers also **clamp** UVs to the edge. UVs slightly outside 0…1 stretch the border texel instead of wrapping (wrapping would smear atlas neighbors).

## Color formats / sRGB

`Gfx` picks `Rgba8UnormSrgb` or `Rgba8Unorm` to match the window surface. sRGB textures let the GPU handle gamma more correctly when blending. You normally do not configure this — if colors look “too dark” or “too bright” after a surface format change, this is the area to investigate.

## Alpha

Channels are uploaded as-is. The render pipeline uses **alpha blending**, so:

- `A = 0` → invisible
- `A = 255` → opaque
- Soft edges in PNGs work

Art with an opaque background painted into the PNG will show that background. Clear it in an image editor (Flappy’s pipe had this issue early on).

## `texture_size(id)`

Returns `(width, height)` in pixels, or `(0, 0)` if the id is unknown. Useful if you want world size to match aspect ratio:

```rust
let (pw, ph) = ctx.gfx.texture_size(tex);
let aspect = pw as f32 / ph as f32;
let size = [2.0 * aspect, 2.0];
```

## Lifetime / ownership mental model

```text
Game code          Gfx                  GPU
─────────          ───                  ───
TextureId  ──idx─► Vec<GpuTexture> ──► wgpu::Texture + View + Sampler + BindGroup
```

Dropping a `TextureId` does nothing — it is just an integer. The GPU resources live as long as `Gfx` (tied to the window / renderer).

## Common pitfalls

- **Loading every frame** — upload cost will dominate; load in `boot`.
- **Wrong path / CWD** — `load_texture` fails with a readable error. Flappy expects `assets/` beside the executable.
- **RGB without alpha** — decoder expands to RGBA; ensure your math uses 4 bytes per pixel for `create_texture`.
- **Premultiplied vs straight alpha** — this pipeline expects **straight** alpha (standard PNG). Premultiplied assets can look dark at edges.
- **Expecting mipmaps** — not generated; fine for 2D UI/sprites, less ideal for heavily minified 3D textures.

## Related docs

- [mod.md](mod.md) — how bind groups are used at draw time  
- [shader.md](shader.md) — `textureSample` in the fragment shader  
- [scene.md](../scene.md) — sprites reference `TextureId`  
