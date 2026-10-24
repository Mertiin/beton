# `src/core/platform/native.rs`

## Purpose

Desktop bodies for the functions declared in [`mod.rs`](mod.md). This file is compiled only when the target is **not** `wasm32` (Windows, macOS, Linux).

Nothing here is public outside `platform`. `app` calls `platform::read_bytes`, and `mod.rs` forwards to these functions.

## Logging and panics

`install_panic_hook` is empty. A desktop panic already prints a stack trace.

`warn` writes to stderr with `eprintln!`. GPU and audio failures during play show up in the terminal that launched the game.

## `read_bytes`

Used by [`Game::assets`](../app.md) before `boot`. The path is whatever the game listed (`"assets/bird-sheet.png"`).

1. Try `std::fs::read(path)` as given (relative to the process working directory — usually the folder you ran `cargo run` from).
2. If that fails, try the same relative path **beside the executable**. `build.rs` copies in sample games land assets next to the exe, and `cargo run` is not always started from that folder.
3. If both fail, the error names the original path and the beside-exe path.

The function is `async` so it matches the web `fetch` signature. On desktop the read itself is synchronous; `schedule` blocks until the whole GPU-init future (including this read) finishes.

## Window

`window_attributes` sets the title and a **logical** inner size. No canvas is attached. The OS creates a normal window.

## `schedule`

```text
pollster::block_on(fut)
```

`Renderer::new` is async because wgpu’s adapter/device requests are async. On desktop there is no browser executor, so the engine blocks the thread until the GPU is ready and asset bytes are in memory. `run` does not return to the game until that work has been posted back through a winit user event (the event loop is already running; `block_on` happens inside `resumed`).

## `run_loop`

```text
event_loop.run_app(&mut app)
```

This **does not return** until the window closes or `event_loop.exit()` runs. The `Result` is `Err` only if winit stops the loop with an error. Escape and the close button are clean exits (`Ok` from `run`, unless a later `fail` stored an error).

## Policy flags

| Function | Returns | Why |
|----------|---------|-----|
| `open_audio_at_boot` | `true` | Open the output device during GPU install. No click is required. |
| `is_audio_gesture` | `false` | Key and mouse presses do not try to open audio again. |
| `exit_on_escape` | `true` | Escape leaves the process. |

## Related docs

- [mod.md](mod.md) — why these functions exist
- [web.md](web.md) — the other backend
- [../app.md](../app.md) — call sites
