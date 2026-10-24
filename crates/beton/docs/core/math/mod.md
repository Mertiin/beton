# `src/core/math/mod.rs`

## Purpose

Tiny module root for math helpers. It declares three child modules and re-exports their public types so the rest of the engine can write:

```rust
use beton::{Mat4, Rng, Vec3};
```

Inside the crate the same names live at `crate::core::math`. There is no gameplay logic here — only organization.

## Children

| File | Doc | Role |
|------|-----|------|
| `vec3.rs` | [vec3.md](vec3.md) | Points, directions, velocities |
| `mat4.rs` | [mat4.md](mat4.md) | Camera / projection matrices |
| `rng.rs` | [rng.md](rng.md) | Deterministic random floats |

## Why a custom math module?

Many engines pull in `glam` or `nalgebra`. This crate keeps a **minimal** subset so:

- Dependencies stay small
- The public API stays easy to read
- You can see every formula (helpful when learning)

When you outgrow it (quaternions, SIMD, dual quaternions…), swapping in a crate is possible — but for sprites + cameras, these three files are enough.

## Related docs

- Index: [../README.md](../README.md)  
