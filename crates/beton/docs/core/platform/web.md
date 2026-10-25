# `src/core/platform/web.rs`

## Purpose

Browser bodies for the functions declared in [`mod.rs`](mod.md). This file is compiled only for `wasm32-unknown-unknown`.

The page keeps running after `run` returns. The game lives on a canvas and on the browser’s event loop, not on a thread you can block.

## Logging and panics

`install_panic_hook` calls `console_error_panic_hook::set_once()` so a Rust panic shows up in the devtools console instead of a blank wasm trap.

`warn` calls `console.warn`. GPU init failures, missing files, and a lost surface are reported there. They do **not** come back as the `Result` from `run`, because `run` has already returned.

## `read_bytes`

[`Game::assets`](../app.md) paths are fetched with `window.fetch` relative to the page URL.

```text
fetch(path) → Response → arrayBuffer → Vec<u8>
```

A missing `window`, a failed promise, a non-OK HTTP status, or a body that is not an array buffer becomes `Err` with the path in the message. Trunk (or whatever hosts the page) must actually serve those files — usually by copying `assets/` into the dist.

There is no filesystem. A path that works in `cargo run` still 404s on the web if it was not packaged.

## Window

`window_attributes` sets the same title and logical size as desktop, then `.with_append(true)`. winit creates a `<canvas>` and appends it to the document. That canvas is the swapchain.

## `schedule`

```text
wasm_bindgen_futures::spawn_local(fut)
```

GPU init and asset fetch are spawned on the browser executor and return immediately. The future must not hold a non-`'static` borrow; `app` moves the window and the asset path list into the async block, then sends `GpuReady` back through an event-loop proxy.

Do not `block_on` here. The browser thread is the page; blocking it hangs the tab.

## `run_loop`

```text
event_loop.spawn_app(app);
Ok(())
```

`spawn_app` registers the handler and returns. `run` then returns `Ok(())` to `main`. The loop keeps running until the page is closed. Calling `event_loop.exit()` would stop the loop and leave a canvas that no longer draws, which is why Escape does not exit.

## Policy flags

| Function | Returns | Why |
|----------|---------|-----|
| `open_audio_at_boot` | `false` | Browsers block audio until a user gesture. `Audio` stays silent through `boot`. |
| `is_audio_gesture` | the press flag | A key-down or left click may call `try_open_device`. Plays queued while silent start then. |
| `exit_on_escape` | `false` | Escape is delivered to the game like any other key. |

## Networking (`web_net.rs`)

`NetDriver::host` always returns an error — a tab cannot listen. `connect` opens `web_sys::WebSocket` with `BinaryType::Arraybuffer` (`ws://` or `wss://`). Callbacks push into a queue the frame drains.

Mixed content: from an `https://` page, `ws://` only works to localhost; use `wss://` for a remote host (TLS must terminate in front of the plain desktop listener). Prefer `trunk serve` (http) for LAN `ws://` testing.

## Related docs

- [mod.md](mod.md) — shared API
- [native.md](native.md) — desktop bodies
- [../net.md](../net.md) — public API
- [../app.md](../app.md) — scheduling and the gesture check
- [../audio.md](../audio.md) — queued plays before the device exists
