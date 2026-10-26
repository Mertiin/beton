# beton

A small game engine with a short public API. Depend on this crate, implement [`Game`](docs/core/app.md), and call `run`.

```rust,ignore
use beton::{run, Config, Context, Game};

struct MyGame;

impl Game for MyGame {
    fn boot(&mut self, _ctx: &mut Context) {}
    fn update(&mut self, _ctx: &mut Context) {}
}

fn main() {
    run(Config {
        title: "my game".into(),
        ..Config::default()
    }, MyGame)
    .expect("run failed");
}
```

## New to low-level graphics?

There is a **beginner guide for every source file** under [`docs/`](docs/README.md) — frame loop, world vs scene, cameras, UVs/atlases, textures, and a walkthrough of the wgpu renderer + WGSL shader. Start at the docs index and read in the listed order.

## Features

- `png` / `jpeg` — decode those formats in `Gfx::load_texture` (both on by default). A game that only uses `Gfx::create_texture` can disable them.
- `wav` / `vorbis` — decode those formats in `Audio::load` / `Audio::create` (both on by default).
- `physics` — Rapier 2D rigid bodies (`RigidBody`, `step_rigid_bodies`). Off by default, so games that only move sprites do not link it. The physics sample enables it.

## Keyboard

`Key` is `winit::keyboard::KeyCode`. This crate pins the `winit` major version.

## Desktop and web

The same crate builds for native and `wasm32-unknown-unknown`. Target-specific code lives only in [`src/core/platform/`](src/core/platform/) (see [docs/core/platform/mod.md](docs/core/platform/mod.md)). On the web, `run` returns as soon as the event loop is scheduled; later failures go to the browser console.

## License

MIT OR Apache-2.0
