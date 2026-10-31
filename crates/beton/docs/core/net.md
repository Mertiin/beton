# `src/core/net/`

## Purpose

A **polled LAN multiplayer channel**. Signaling uses a WebSocket; game bytes ride **WebRTC data channels** (reliable ordered + unreliable unordered). Games own the payload bytes. The engine does not replicate `GameObject`s, predict motion, or encrypt traffic beyond DTLS.

Use it like [`Input`](input.md): each frame the engine drains into `ctx.net.events()`, your `update` reads and sends, then unread events are discarded.

## Roles

| Role | Desktop | Browser |
|------|---------|---------|
| Host (`Net::host`) | Listen on `0.0.0.0:port` (signaling TCP) + WebRTC UDP | Always errors — tabs cannot listen |
| Client (`Net::connect`) | Dial `ws://host:port`, then WebRTC | Same, via `WebSocket` + `RTCPeerConnection` |

A match is therefore a **desktop host** plus desktop or browser clients. Only `ws://` for signaling (no TLS). `wss://` is out of scope for this slice.

**Mixed content:** an `https://` page (e.g. GitHub Pages) may open `ws://` only to `localhost` / `127.0.0.1`, or `wss://` to any host. Plain `ws://` to a public/LAN IP is blocked by the browser. For remote play from Pages, put TLS in front of the desktop host (Caddy/nginx) and connect with `wss://…`. For LAN testing without TLS, use `trunk serve` (http) instead of Pages.

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
| `Connected` | Client: both WebRTC data channels open |
| `PeerJoined(id)` | Host: remote peer ready (both channels open) |
| `PeerLeft(id)` | Remote peer gone |
| `Message { from, bytes }` | Reliable ordered payload |
| `Datagram { from, bytes }` | Unreliable unordered payload (loss is silent) |
| `Disconnected` | Local socket gone |
| `Overflow` | Inbound queue dropped at least one `Message` / `Datagram` |

### Caps

| Constant | Value | Rule |
|----------|------:|------|
| `MAX_PAYLOAD` | 16 KiB | Larger `send` / `broadcast` (and unreliable variants) fail |
| `MAX_PEERS` | 8 | Further host accepts are closed |
| Inbound queue | 256 payloads | Oldest `Message`/`Datagram` dropped; one `Overflow` kept |

## API on `Net`

```rust
ctx.net.host(7777)?;
ctx.net.connect("ws://127.0.0.1:7777")?;
ctx.net.send(PeerId::HOST, &bytes)?;              // reliable
ctx.net.send_unreliable(PeerId::HOST, &bytes)?;   // datagram
ctx.net.broadcast(&bytes)?;                       // host → all (reliable)
ctx.net.broadcast_unreliable(&bytes)?;            // host → all (datagram)
for event in ctx.net.events() { /* … */ }
let peers = ctx.net.peers();
let role = ctx.net.role();
ctx.net.close();
```

- A second `host` / `connect` while open returns an error until `close`.
- Clients may only `send` / `send_unreliable` to `PeerId::HOST`. Client `broadcast*` is an error.
- Bytes are opaque. Define your own tags in the game (see `games/netplay`).
- Pose snapshots belong on the unreliable path; joins and one-shot events on the reliable path.

## Signaling (internal)

WebSocket frames are binary tags, not game traffic:

| Tag | Payload |
|-----|---------|
| `1` | SDP offer (UTF-8) |
| `2` | SDP answer (UTF-8) |
| `3` | ICE candidate attribute string (UTF-8) |

Each frame is `[tag u8][len u32 LE][payload]`. Channels: `"rel"` (reliable ordered) and `"dg"` (`maxRetransmits: 0`, unordered).

## Frame order

1. Before `update`: `net.begin_frame()` moves driver events onto this frame’s slice.
2. `Game::update` — read `events()`, send / broadcast.
3. After `update`: `net.end_frame()` clears the slice.

Unread events are **gone**. Read every frame.

## Where sockets live

`net` never uses `#[cfg(target_arch = "wasm32")]`. OS work is [`NetDriver`](platform/mod.md) in the platform backends:

- **Desktop** — `tungstenite` signaling + `str0m` WebRTC (UDP). One I/O thread per peer; accept thread on the host. First bind may prompt the Windows firewall for TCP and UDP.
- **Browser** — `WebSocket` + `RTCPeerConnection`. `host` returns an error string. No public STUN server (LAN / localhost only).

## Common pitfalls

- **Expecting the engine to move sprites** — send poses yourself; apply them in `update`.
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
