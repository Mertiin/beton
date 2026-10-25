# `src/core/net/`

## Purpose

A **polled WebSocket message channel** for LAN multiplayer. Games own the payload bytes. The engine does not replicate `GameObject`s, predict motion, or encrypt traffic.

Use it like [`Input`](input.md): each frame the engine drains the socket into `ctx.net.events()`, your `update` reads and sends, then unread events are discarded.

## Roles

| Role | Desktop | Browser |
|------|---------|---------|
| Host (`Net::host`) | Listen on `0.0.0.0:port` | Always errors — tabs cannot listen |
| Client (`Net::connect`) | Dial `ws://host:port` | Same, via `web_sys::WebSocket` |

A match is therefore a **desktop host** plus desktop or browser clients. Only `ws://` (no TLS). `wss://` is out of scope for this slice.

**Mixed content:** an `https://` page may only open `ws://` to `localhost` / `127.0.0.1`. Browser testing uses `trunk serve` (http) or a host on the same machine. Do not expect GitHub Pages to reach a LAN host over plain `ws://`.

## Types

| Type | Role |
|------|------|
| `PeerId` | Stable peer id. Host is always `PeerId::HOST` (`0`). Host assigns `1, 2, …` |
| `NetRole` | `Offline`, `Hosting`, or `Client` |
| `NetEvent` | One notification for this frame |
| `Net` | The handle on [`Context`](app.md) |

### `NetEvent`

| Variant | Meaning |
|---------|---------|
| `Listening { port }` | Host bound (useful when you passed port `0`) |
| `Connected` | Client finished the handshake |
| `PeerJoined(id)` | Remote peer ready (host sees new clients) |
| `PeerLeft(id)` | Remote peer gone |
| `Message { from, bytes }` | Binary payload |
| `Disconnected` | Local socket gone |
| `Overflow` | Inbound queue dropped at least one `Message` |

### Caps

| Constant | Value | Rule |
|----------|------:|------|
| `MAX_PAYLOAD` | 16 KiB | Larger `send` / `broadcast` fail |
| `MAX_PEERS` | 8 | Further host accepts are closed |
| Inbound queue | 256 messages | Oldest `Message` dropped; one `Overflow` kept |

## API on `Net`

```rust
ctx.net.host(7777)?;
ctx.net.connect("ws://127.0.0.1:7777")?;
ctx.net.send(PeerId::HOST, &bytes)?;   // client → host
ctx.net.broadcast(&bytes)?;            // host → all remotes
for event in ctx.net.events() { /* … */ }
let peers = ctx.net.peers();
let role = ctx.net.role();
ctx.net.close();
```

- A second `host` / `connect` while open returns an error until `close`.
- Clients may only `send` to `PeerId::HOST`. Client `broadcast` is an error.
- Bytes are opaque. Define your own tags in the game (see `games/netplay`).

## Frame order

1. Before `update`: `net.begin_frame()` moves driver events onto this frame’s slice.
2. `Game::update` — read `events()`, send / broadcast.
3. After `update`: `net.end_frame()` clears the slice.

Unread events are **gone**. Read every frame. (Same trade-off as edge-triggered keys: skip a menu frame and you miss that frame’s `PeerJoined`.)

## Where sockets live

`net` never uses `#[cfg(target_arch = "wasm32")]`. OS work is [`NetDriver`](platform/mod.md) in the platform backends:

- **Desktop** — `tungstenite` (handshake, no TLS), one I/O thread per peer, accept thread on the host. `Drop` shuts sockets and joins threads (wake accept via `127.0.0.1`, not `0.0.0.0`).
- **Browser** — `WebSocket` with binary frames. `host` returns an error string.

## Common pitfalls

- **Expecting the engine to move sprites** — send positions yourself; apply them in `update`.
- **Hosting from wasm** — always fails; run a desktop host.
- **`https` page → `ws://` LAN host** — blocked by the browser; use http Trunk or localhost.
- **Skipping `events()`** — one-frame lifetime; joins and messages vanish if unread.
- **No auth** — anyone on the LAN who can reach the port can join. Fine for samples, not for the public internet.

## Related docs

- [app.md](app.md) — `Context::net` and the frame loop
- [platform/mod.md](platform/mod.md) — `NetDriver` behind the facade
- [platform/native.md](platform/native.md) — desktop threads
- [platform/web.md](platform/web.md) — browser client
- [input.md](input.md) — same one-frame event style
