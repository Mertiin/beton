# `src/lib.rs`

## Purpose

This is the crate’s **front door**. It does three jobs:

1. Declares the `core` module tree (`mod core;`).
2. Re-exports the types games are supposed to use (`pub use core::app::{run, Config, …}`).
3. Sets crate-wide lint / safety policy.

When you write:

```rust
use beton::{run, Config, Context, Game, Sprite, World};
```

every name in that list is exported from here. If a type is **not** listed in `lib.rs`, it is internal — games should not reach for it (for example `Renderer` in `core/render/mod.rs`).

## Why a short public API matters

Without this file, callers would import deep paths like `beton::core::render::texture::Gfx`. That couples your game to the engine’s folder layout and makes refactors painful. A single re-export list is the contract: **if it is here, it is supported; if it is not, it may change**.

## Safety and docs policy

```rust
#![forbid(unsafe_code)]
#![warn(missing_docs)]
```

- **`forbid(unsafe_code)`** — the engine refuses `unsafe { … }` blocks.
- **`warn(missing_docs)`** — public items should have rustdoc comments.

## Module map

| Module | Responsibility | Re-exported? |
|--------|----------------|--------------|
| `core::app` | Window, clock, `Game` loop | yes — `run`, `Config`, `Context`, `Game` |
| `core::object` | Retained game objects + components | yes — `GameObject`, `GameObjectId`, `World`, `Query`, `Velocity`, `RigidBody`, steppers |
| `core::scene` | One-frame sprites + text | yes — `Sprite`, `Font`, `Scene` |
| `core::camera` | View into the world | yes — `Camera`, `Projection` |
| `core::input` | Keyboard snapshot | yes — `Input`, `Key` |
| `core::net` | Host / client messages | yes — `Net`, `NetEvent`, `NetRole`, `PeerId`, caps |
| `core::anim` | Sheets + clips | yes — `SpriteSheet`, `Animation` |
| `core::audio` | Sound register + playback | yes — `Audio`, `SoundId`, `SoundHandle`, `VoiceId` |
| `core::math` | Vectors, matrices, RNG | yes — `Vec3`, `Mat4`, `Rng` |
| `core::render` | GPU textures + internal renderer | partially — `Gfx`, `TextureId`, `TextureHandle` |
| `core::asset` | Shared load/unload lifetime rules | no (internal) |
| `core::platform` | Desktop vs web | no (internal) |

## Full public surface

| Import | Role in one sentence |
|--------|----------------------|
| `run` | Open a window and drive your `Game` until exit |
| `Config` | Title, size, optional FPS cap |
| `Context` | Per-frame handles (`dt`, `world`, `scene`, …) |
| `Game` | Trait you implement (`boot` + `update`) |
| `Camera`, `Projection` | Where you look from / how it projects |
| `Input`, `Key` | Held keys and “just pressed” edges |
| `Net`, `NetEvent`, `NetRole`, `PeerId` | LAN WebSocket channel (opaque bytes) |
| `MAX_PAYLOAD`, `MAX_PEERS` | Net send / accept caps |
| `Vec3`, `Mat4`, `Rng` | Math helpers |
| `Gfx`, `TextureId`, `TextureHandle` | Register / pin textures |
| `Audio`, `SoundId`, `SoundHandle`, `VoiceId` | Register / pin / play clips |
| `Sprite`, `Font`, `Scene` | Drawing primitives |
| `GameObject`, `GameObjectId`, `World`, `Query` | Entities and components |
| `Velocity`, `integrate_velocities` | Shared linear + angular speed; plain integrate |
| `RigidBody`, `step_rigid_bodies` | Mass / material; AABB gravity and contacts |
| `SpriteSheet`, `Animation` | Atlas UV math + timed frame playback |

## How to extend the API later

If you add a new public type:

1. Implement it under `src/core/…`.
2. Document it with `///` comments.
3. Add `pub use …` here so games can find it.
4. Add a constructibility check in `tests/api.rs` if it is meant to build without a GPU.
