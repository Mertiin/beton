# `src/core/scene/`

## Purpose

`Scene` is the **immediate-mode draw list** for the current frame: textured quads (`Sprite`) and helpers to turn a bitmap font into many quads (`Font` + `text`).

The engine calls `scene.clear()` at the start of every frame, before your `update`. If you want something drawn, you must push it again (or attach a sprite to a `World` object, which the engine copies in after `update`).

## Why “immediate mode”?

Immediate mode means: **describe what you want this frame; nothing is remembered.** Retained mode (`World`) means: **keep an object until you despawn it.**

Immediate mode is perfect for:

- Full-screen backdrops
- Score / HUD text
- Temporary effects
- Debug overlays

You never forget to delete a HUD element — it vanishes unless you redraw it.

## `Sprite` — a textured rectangle

| Field | Meaning |
|-------|---------|
| `texture` | Which GPU image (`TextureId`) |
| `center` | World-space middle of the quad |
| `size` | `[width, height]` in **world units** |
| `rotation` | Radians around the center. Positive tips a right-facing sprite nose-up |
| `uv` | Sub-rectangle of the texture: `[min_u, min_v, max_u, max_v]` |
| `color` | RGBA tint multiplied with sampled texels |

### Constructor

`Sprite::new(texture, center, size)` fills UV with the full image `[0, 0, 1, 1]` and color with opaque white `[1, 1, 1, 1]`.

### World size vs texture pixels

A 34×24 pixel bird drawn at size `[1.6, 1.15]` is **scaled** by the camera. Pixel art stays sharp because textures use **nearest** filtering (see [render/texture.md](render/texture.md)). Stretching a tiny atlas glyph to a large world size is normal.

### Tint tricks

| Goal | Approach |
|------|----------|
| Solid color rect | 1×1 white texture + `color = [r, g, b, a]` |
| Darken a sprite | `color = [0.5, 0.5, 0.5, 1]` |
| Fade out | Lower alpha: `color[3] = 0.4` |
| Recolor white art | Tint freely — white texels take the tint color |

Alpha blending is on in the renderer, so transparent PNGs and faded tints composite over whatever was drawn earlier (sorted by Z).

## UV coordinates in depth

UV maps the rectangle’s corners onto the texture:

```
texture (u →)
v  (0,0) ---- (1,0)
↓    |  image  |
   (0,1) ---- (1,1)
```

In this engine, **v0 is the top** of the image when packing quads (see `push_quad` in the renderer). Full texture: `[0, 0, 1, 1]`.

### Atlases

If one PNG holds three equal frames side by side:

| Frame | Approx UV |
|-------|-----------|
| 0 (left) | `[0.00, 0, 0.33, 1]` |
| 1 (mid) | `[0.33, 0, 0.66, 1]` |
| 2 (right) | `[0.66, 0, 1.00, 1]` |

`SpriteSheet` and `Font` compute this with a small **inset padding** (~2% of a cell) so neighboring cells do not bleed when sampling. You can also flip a sprite by swapping UV ends — Flappy’s top pipe uses `[0, 1, 1, 0]` to flip the pipe image vertically.

### Flipping

| Effect | UV tweak |
|--------|----------|
| Flip vertical | swap v0 and v1: `[u0, v1, u1, v0]` |
| Flip horizontal | swap u0 and u1 |
| Mirror both | swap both pairs |

## `Font` and `Scene::text`

A `Font` is a grid atlas of glyphs:

| Field | Meaning |
|-------|---------|
| `texture` | Atlas image |
| `columns`, `rows` | Grid shape |
| `first_char` | Character that lives in cell 0 |

Flappy builds a 10×1 digit strip (`'0'`…`'9'`) in CPU memory and uploads it with `create_texture`. Then:

```rust
scene.text(&font, origin, glyph_size, color, "12");
```

For each character:

1. `index = ch as u32 - first_char as u32`
2. Skip if out of range (unknown chars leave a gap but still advance X)
3. Map index → column/row → UV with padding
4. Push a sprite centered at the glyph cell
5. Advance `origin.x` by `glyph_size[0]`

`origin` is the **left** of the first glyph (baseline-ish center Y). Glyph sprites are centered at `x + glyph_size[0]/2`.

Drop shadows are just two `text` calls — dark offset behind, white on top (see Flappy score).

## Draw order (Z)

After `update`, the engine sorts all sprites by **`center.z` ascending** (smaller Z first).

With the current depth state (`Always` compare, no depth write — see renderer docs), order is effectively **painter’s algorithm**: later draws win on ties of coverage. Use Z as your layer key:

| Layer | Example Z |
|-------|-----------|
| Sky | `-1.0` |
| Floor | `-0.5` |
| Gameplay | `0.0` |
| UI shadow | `0.9` |
| UI | `1.0` |

Game-object sprites use whatever Z is on the sprite template; remember draw copies `center` from **world** `pos`, so the resolved **`GameObject` world Z** becomes the draw Z.

## `Scene` methods

| Method | Use |
|--------|-----|
| `push(sprite)` | Queue one quad |
| `text(...)` | Queue one quad per glyph |
| `clear` / `take_sorted` | Engine-internal |

## Worked example — colored backdrop

```rust
let mut sky = Sprite::new(white, Vec3::new(0.0, 0.0, -1.0), [40.0, 20.0]);
sky.color = [0.45, 0.75, 0.95, 1.0];
ctx.scene.push(sky);
```

Here `white` is a 1×1 texture created in `boot`. The huge size covers the ortho view; tint makes it sky blue.

## Common pitfalls

- **Pushing in `boot` and expecting it to stay** — scene is cleared every frame.
- **UV outside 0…1** — usually clamped by the sampler (`ClampToEdge`), so you get edge pixels stretched, not a crash.
- **Text with the wrong `first_char`** — glyphs map to the wrong cells (digits look like junk).
- **Forgetting Z on UI** — UI can hide behind gameplay if Z is lower.
- **Huge numbers of `text` calls** — each character is a quad; fine for scores, not for a novel.

## Related docs

- [world.md](object/world.md) — retained sprites  
- [anim.md](anim.md) — driving `sprite.uv` from a sheet  
- [camera.md](camera.md) — how size maps to the screen  
- [render/shader.md](render/shader.md) — how tint × texture becomes a pixel  
