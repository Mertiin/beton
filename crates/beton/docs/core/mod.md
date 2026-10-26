# `src/core/mod.rs`

## Purpose

This file is the **table of contents** for the engine. It does not contain gameplay or GPU code. It declares every child module and decides which ones the rest of the crate (and, through `lib.rs`, games) can name.

If you are looking for “where does X live?”, start here, then open that child’s doc.

## What “declaring a module” means

In Rust, a folder is not automatically part of the crate. A parent file must write `mod name;`. This file is that parent for everything under `src/core/`.

`lib.rs` only does `mod core;`. Everything else is reachable because this file lists it.

## Visibility

Two levels show up here:

| Keyword | Who can `use` it |
|---------|------------------|
| `pub mod` | Other modules in the crate, and — if `lib.rs` re-exports — games |
| `pub(crate) mod` | Other modules in this crate only. Games cannot name it. |

`asset` and `platform` are `pub(crate)`. They are real engine pieces, but they are not part of the supported game API. `lib.rs` never re-exports them.

The other modules are `pub` so `lib.rs` can `pub use` their types. A game still should import from `beton::…`, not from `beton::core::…`. The `core` module itself is private (`mod core` in `lib.rs` has no `pub`), so the deep path is not even available outside the crate.

## Children

| Module | Source | Doc | Role |
|--------|--------|-----|------|
| `app` | `app/mod.rs` | [app.md](app.md) | Window, clock, `Game` / `run` |
| `object` | `object/` | [object/mod.md](object/mod.md) | Retained objects, parents, components |
| `scene` | `scene/mod.rs` | [scene.md](scene.md) | One-frame sprites and text |
| `ui` | `ui/mod.rs` | [ui.md](ui.md) | Screen-fixed layout UI |
| `camera` | `camera/mod.rs` | [camera.md](camera.md) | View and projection |
| `input` | `input/mod.rs` | [input.md](input.md) | Keys, cursor, pointer |
| `net` | `net/mod.rs` | [net.md](net.md) | Host / client message channel |
| `anim` | `anim/mod.rs` | [anim.md](anim.md) | Sheets and clips |
| `audio` | `audio/mod.rs` | [audio.md](audio.md) | Clips and voices |
| `math` | `math/` | [math/mod.md](math/mod.md) | `Vec3`, `Mat4`, `Rng` |
| `render` | `render/` | [render/mod.md](render/mod.md) | GPU textures and the hidden renderer |
| `asset` | `asset.rs` | [asset.md](asset.md) | Shared load / grace / unload rules (internal) |
| `platform` | `platform/` | [platform/mod.md](platform/mod.md) | Desktop vs web (internal) |

## Why the split is here and not in `lib.rs`

`lib.rs` is the **contract** (names games may use). This file is the **map** (how the crate is divided). Keeping the map in one place means a new internal module does not have to touch the public prelude unless you also want to export it.

## Related docs

- [../lib.md](../lib.md) — which of these names games actually import
- [../README.md](../README.md) — suggested reading order
