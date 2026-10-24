# `src/core/camera/`

## Purpose

The camera answers: **from where am I looking, and how does that view fold onto the rectangular window?**

Gameplay positions live in infinite world space. The monitor is a finite rectangle of pixels. The camera builds a single matrix (`view_proj`) the GPU multiplies with every sprite corner to land in the right place on screen.

## Beginner analogy

You stand in a warehouse (the world) holding a picture frame (the window).

- **Eye** — where your face is
- **Target** — the point you stare at
- **Up** — which way the top of the frame points (usually sky = +Y)
- **Projection** — the “lens”:
  - **Orthographic** = technical drawing / 2D game (parallel rays, no shrink with distance)
  - **Perspective** = real camera (far things look smaller)

## Fields

```rust
pub struct Camera {
    pub eye: Vec3,
    pub target: Vec3,
    pub up: Vec3,
    pub projection: Projection,
}
```

### Default camera

| Field | Default | Effect |
|-------|---------|--------|
| `eye` | `(0, 0, 10)` | Sit on +Z |
| `target` | `(0, 0, 0)` | Look at origin |
| `up` | `(0, 1, 0)` | Y is up |
| `projection` | Ortho height `20`, near `0.1`, far `100` | 20 world units tall view |

With that setup, world `(0, 0)` is the center of the screen. `y = +10` is near the top edge; `y = -10` near the bottom (when height is 20). Width depends on aspect: a tall Flappy window shows less world-width than a wide desktop window.

## Orthographic projection (2D games)

```rust
Projection::Orthographic { height, near, far }
```

| Parameter | Meaning |
|-----------|---------|
| `height` | Visible world units from bottom to top of the window |
| `near`, `far` | Depth range along the view forward. Keep sprites’ Z between them |

**Width is not specified.** It is `height * aspect`, where `aspect = framebuffer_width / framebuffer_height`. Resize the window → aspect changes → horizontal field of view changes. Vertical gameplay distances stay stable — a good property for portrait mobile-style games.

There is **no perspective foreshortening**. A sprite at Z = 0 and Z = 5 is the same size on screen (Z still affects draw sort).

### Choosing `height`

| Height | Feel |
|--------|------|
| Small (e.g. 8) | Zoomed in — sprites look large |
| Medium (20) | Flappy’s choice — comfortable playfield |
| Large (50) | Zoomed out — tiny sprites, huge world visible |

Pick one convention and size all art relative to it.

## Perspective projection (3D-ready)

```rust
Projection::Perspective { fov_y, near, far }
```

| Parameter | Meaning |
|-----------|---------|
| `fov_y` | Vertical field of view in **radians** (π/2 ≈ 90°) |
| `near`, `far` | Clip planes; geometry outside is discarded |

Use this when you want real depth. Sprites are still flat quads, but their screen size depends on distance from the eye. The math types are already 3D so you can grow into meshes later without replacing the camera.

## `view_proj(aspect)` — what it computes

```text
view_proj = projection_matrix × view_matrix
```

1. **View (`look_at_rh`)** — rotate/translate the world so the camera sits at the origin looking down −Z.
2. **Projection** — squash the view volume into clip space (roughly a cube the GPU understands).
3. **Multiply** — one matrix uploaded to the shader each frame.

You rarely call this yourself; `app` does after `update`. It is public for tests and tooling (`project_point` on `Mat4` helps verify “origin should land at screen center”).

## Moving / shaking the camera

Change `eye` and `target` together to pan:

```rust
ctx.camera.eye.x = bird_x;
ctx.camera.target.x = bird_x;
```

For screen shake, offset `eye` and `target` by the same small random vector for a few frames.

Do **not** forget to move both — if only `eye` moves, the camera looks toward a fixed target and orbits oddly.

## Near / far pitfalls

- Objects with Z outside `[near, far]` (in view space) can disappear or clip.
- `near` too small and `far` too large hurts depth precision in true 3D; for ortho 2D with few layers it rarely matters.
- Default `near: 0.1`, `far: 100` with eye at Z=10 looking at 0 is comfortable for gameplay Z around 0.

## Aspect ratio gotchas

- Letterboxing is **not** built in. A super-wide window simply shows more world horizontally.
- If you need a fixed world width *and* height, you must pick one to derive from aspect yourself (or add letterbox bars via scene sprites).

## `screen_to_world(px, fb_w, fb_h)`

Maps a framebuffer pixel (origin top-left, Y down — the same space as `Input::cursor_px` and `Context::fb_size`) onto the camera’s look plane.

Orthographic: NDC from the pixel, then

```text
world.x = target.x + ndc_x * (height * aspect / 2)
world.y = target.y + ndc_y * (height / 2)
world.z = target.z
```

So a cursor in the middle of the window lands on `target`’s XY. Panning `target` pans the result. This assumes the default style of camera (looking along −Z at the XY plane). It does not do a full inverse of `view_proj`.

Perspective returns `target` and ignores the pixel. Picking in 3D is not implemented.

`fb_w` and `fb_h` are clamped to at least 1 so a zero-sized framebuffer does not divide by zero.

```rust
let world = ctx.camera.screen_to_world(
    ctx.input.cursor_px(),
    ctx.fb_size[0],
    ctx.fb_size[1],
);
```

## Worked 2D setup (Flappy-style)

```rust
*ctx.camera = Camera {
    eye: Vec3::new(0.0, 0.0, 10.0),
    target: Vec3::ZERO,
    up: Vec3::Y,
    projection: Projection::Orthographic {
        height: 20.0,
        near: 0.1,
        far: 100.0,
    },
};
```

Then design level geometry in roughly `x ∈ [-10, 10]`, `y ∈ [-10, 10]` for a square window; on a tall window, usable X shrinks.

## Related docs

- [math/mat4.md](math/mat4.md) — the matrices behind `view_proj`  
- [scene.md](scene.md) — sprites live in the space the camera views
- [input.md](input.md) — `cursor_px` is the pixel you pass in  
- [render/shader.md](render/shader.md) — vertex shader multiplies by this matrix  
