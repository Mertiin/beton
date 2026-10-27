# beton

A small game engine (`crates/beton`) and sample games (`games/`).

## Requirements

Rust 1.88 or newer:

```bash
rustup update stable
```

On Linux the build needs `pkg-config` and the X11, xkbcommon, Wayland, and ALSA development packages (`libasound2-dev` or the distro’s `alsa-lib` development package). Running also needs `libxkbcommon-x11` and a Vulkan driver. Windows uses WASAPI for audio and needs nothing extra for sound.

## Run samples

From the repository root:

```bash
cargo run --release -p flappy
cargo run --release -p zombies
cargo run --release -p twenty48
cargo run --release -p physics
cargo run --release -p netplay
cargo run --release -p tetris
cargo run --release -p ui
cargo run --release -p brutal-pixels
```

Web (from `games/brutal-pixels`):

```bash
trunk serve --release
```

- **flappy** — Start / restart from the on-screen menus (or Space). Space flaps while playing.
- **zombies** — top-down; zombies stream in at 50 000/minute and chase the mouse (sprite count stress test).
- **twenty48** — classic 2048; arrows or WASD slide, R restarts.
- **physics** — rigid-body sandbox; Space drops a box, arrows/A/D kick the newest, R resets.
- **netplay** — two-player Pong over WebSocket. Host is left paddle (simulates the ball); first client is right. `H` hosts port 7777, `C` connects. Or `netplay host [PORT]` / `netplay connect HOST:PORT`. W/S or arrows (mouse Y also works). Needs a desktop host (browser can only dial out).
- **tetris** — classic Tetris (7-bag, hold, ghost). Arrows/WASD move, Up/W/X rotate, Z CCW, Space hard drop, C/Shift hold, R restarts. Board logic is isolated for a later battle-royale mode.
- **ui** — screen UI demo: HUD with image/count hover, a button opens a dimmed settings panel, Back closes it.
- **brutal-pixels** — side-view tower auto-battler. Buy modules from the bottom bar onto your tower, FIGHT for auto combat (guns + walking units). HP resets each round; loadout persists. Click a mounted module twice to sell. R restarts after a win or loss.

Escape or the window close button exits.

## Run samples in the browser

Needs the `wasm32-unknown-unknown` target and [Trunk](https://trunkrs.dev/):

```bash
rustup target add wasm32-unknown-unknown
cargo install trunk
```

Quick compile check (no browser):

```bash
cargo check --target wasm32-unknown-unknown -p twenty48
cargo check --target wasm32-unknown-unknown -p flappy
```

Serve and open in Chrome or Edge (WebGPU):

```bash
cd games/twenty48   # or games/flappy
trunk serve --release
```

Release static output:

```bash
cd games/twenty48   # or games/flappy
trunk build --release
```

Artifacts land in `target/twenty48-web/` or `games/flappy/dist/`.

Pushes to `main` publish flappy, 2048, physics, netplay, tetris, and brutal-pixels to [GitHub Pages](https://mertiin.github.io/beton/). The repository Pages source must be **GitHub Actions**. Netplay on Pages is a client only — run a desktop host locally. Zombies and gallery stay desktop-only; they load files from beside the executable.

Click the canvas so it has keyboard focus. Escape does not quit on the web (it would leave a dead page). On flappy, click first so audio can start (browser gesture rule), then Space flaps. File assets listed in `Game::assets` are fetched over HTTP (Trunk copies `games/flappy/assets/` into the dist).

Desktop and web share the same engine. Target-specific code lives only in [`crates/beton/src/core/platform/`](crates/beton/src/core/platform/).

## Use the engine in another project

Add a path or version dependency on `beton`, implement `Game`, and call `run`. See [`crates/beton/README.md`](crates/beton/README.md).

Beginner walkthrough of each engine source file: [`crates/beton/docs/`](crates/beton/docs/README.md).

## License

MIT OR Apache-2.0
