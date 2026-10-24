# `src/core/platform/mod.rs`

## Purpose

The **only** file in the engine that branches on the target. `app`, `render`, `audio`, and game code call the functions below and never write `#[cfg(target_arch = "wasm32")]`.

This file does not implement those functions. It picks a backend and forwards:

```text
#[cfg(not(wasm32))]  mod native;  use native as backend;
#[cfg(wasm32)]       mod web;     use web as backend;
```

One of `native.rs` / `web.rs` is compiled. The other is not in the build. Callers see one API either way.

## Why it exists

`winit` and `wgpu` work on both desktop and the browser, but a few OS details differ:

| Concern | Desktop ([native.md](native.md)) | Browser ([web.md](web.md)) |
|---------|----------------------------------|----------------------------|
| Event loop | `run_app` owns the thread | `spawn_app` returns immediately |
| GPU init | block with `pollster` | `spawn_local` a future |
| Window | OS window | `<canvas>` appended to the page |
| Clock | `web_time` (same type as std here) | `web_time` (`std::time::Instant` panics on wasm) |
| Warnings | `eprintln!` | `console.warn` |
| Audio device | open at boot | open on first key/click (autoplay policy) |
| File bytes | `std::fs::read` (+ beside the exe) | `fetch` relative to the page |
| Escape | exits the loop | ignored (exit would leave a dead canvas) |

`Duration` and `Instant` are re-exported from `web_time` in this file for **both** targets, so `app` can keep one clock type. On desktop that crate is a std-compatible clock. On wasm it is the one that does not panic.

## Leaf rule

`platform` does not import `Renderer`, `Gfx`, `Audio`, or `EngineApp`. Callers pass futures in (`schedule`) so there is no module cycle: app depends on platform, platform does not depend on app.

Cargo target tables in `Cargo.toml` are the other allowed place for target-specific **dependencies** (`pollster` vs `wasm-bindgen`). Behavior still goes through the functions here.

## Surface (crate-internal)

| Function | Role |
|----------|------|
| `install_panic_hook` | Browser console panics; no-op on desktop |
| `warn` | Log a warning |
| `window_attributes` | Title, size, and (on web) canvas append |
| `schedule` | Run or spawn an async GPU-init future |
| `run_loop` | Drive or schedule the winit event loop |
| `open_audio_at_boot` | Whether `Audio::new` runs during GPU install |
| `is_audio_gesture` | Whether this press should open audio |
| `exit_on_escape` | Whether Escape calls `event_loop.exit()` |
| `read_bytes` | Async file or HTTP load for [`Game::assets`](../app.md) |

`run_loop`’s contract: on the web it returns `Ok(())` as soon as the loop is scheduled. Later failures go through `warn`, not through the `Result` from `run`.

## Rule for new features

If a feature touches the OS (files, fetch, gamepad, networking, threads), add a function here and implement it in **both** backends. Sprite, scene, world, and math code never import this module.

## Related docs

- [native.md](native.md) — desktop bodies
- [web.md](web.md) — browser bodies
- [../app.md](../app.md) — the only caller that uses every function
- [../audio.md](../audio.md) — device open timing
