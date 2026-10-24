# `src/core/math/mat4.rs`

## Purpose

`Mat4` is a **4×4 matrix** stored **column-major** (same layout `wgpu` expects in uniform buffers). The camera builds one of these every frame; the vertex shader multiplies every sprite corner by it.

Most game code never constructs a `Mat4` by hand. Read this page when you want to understand *what the camera is doing*, or when debugging “why is nothing visible?”

## Beginner analogy

A 4×4 matrix is a packed recipe: “take a point and move / rotate / scale / project it.” Multiplying matrices **chains** recipes. The engine uses:

```text
view_proj = projection × view
```

Apply view first (put the world in camera space), then projection (squash into the GPU’s clip cube).

## Storage

```rust
pub struct Mat4(pub [[f32; 4]; 4]); // column-major: m[col][row]
```

`as_bytes()` returns 64 bytes (`16 × f32`) for uploading to the camera uniform buffer.

## `IDENTITY`

The “do nothing” matrix. Multiplying by it leaves points unchanged. Useful as a default before you have a real camera.

## `look_at_rh(eye, target, up)` — the view matrix

Builds a right-handed view transform:

1. Forward `f = normalize(target - eye)` — where the camera looks
2. Right `s = normalize(cross(f, up))`
3. True up `u = cross(s, f)`
4. Pack rotation + translation so the eye moves to the origin and looks down **−Z** in view space

If `eye ≈ target`, or `up` is parallel to forward, axes become degenerate (zero-length cross products) and the matrix can implode into NaNs / black screen. Keep a healthy distance and a sensible up vector.

## `orthographic_rh(height, aspect, near, far)`

Maps a box in view space into clip space with **Z in [0, 1]** (wgpu style).

- Visible height = `height` world units
- Visible width = `height * aspect`
- Centered on the view origin

No perspective divide weirdness for XY — parallel lines stay parallel. Ideal for 2D.

## `perspective_rh(fov_y, aspect, near, far)`

Classic perspective matrix. `fov_y` is vertical field of view in **radians**. Distant objects shrink. `near` must be `> 0` and `< far`.

## `mul(self, other)`

Matrix product `self * other`. Remember order: `proj.mul(view)` means view runs first on a column vector (standard GL-style column vectors).

## `project_point(p)`

Transforms a point with `w = 1`, then divides by `w` (perspective divide). Returns a point roughly in NDC / clip-ish space. Used in unit tests to assert “world origin should land near the center of the screen.”

Handy mental check:

```rust
let clip = camera.view_proj(aspect).project_point(Vec3::ZERO);
// expect clip.x ≈ 0, clip.y ≈ 0 for the default camera
```

## Do gameplay programmers need this?

Usually **no**. Use `Camera`. Reach for `Mat4` when:

- Writing camera tests
- Learning how view/projection works
- Extending the engine with custom transforms

## Common pitfalls

- **Row-major vs column-major confusion** when pasting formulas from random websites — this file is column-major.
- **Swapping mul order** — `view.mul(proj)` is wrong for this pipeline.
- **Degrees vs radians** in perspective FOV — API wants radians (`FRAC_PI_2`, not `90.0`).

## Related docs

- [camera.md](../camera.md) — public camera API that calls these helpers  
- [render/shader.md](../render/shader.md) — GPU side of `view_proj * position`  
