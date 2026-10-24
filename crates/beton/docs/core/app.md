# `src/core/app/`

## Purpose

This file is the **heartbeat of the engine**: create a window, talk to the OS event loop, keep time, call your game code, and ask the renderer to present a frame.

If you understand this file, you understand *when* everything else runs.

## The big picture

Desktop games do not get a `while true { update(); draw(); }` main function for free. The operating system owns the message pump (mouse, keyboard, resize, close). On Windows / macOS / Linux that pump is wrapped by **`winit`**.

`beton` implements winit’s `ApplicationHandler` on an internal `EngineApp`. Your job is only to implement the tiny `Game` trait; `run` wires the rest.

```
run(config, game)
    │
    ▼
EventLoop ──► resumed: create Window + Renderer + Gfx
         ──► RedrawRequested: frame()
         ──► KeyboardInput: update Input
         ──► about_to_wait: maybe request_redraw / sleep
```

## Types you implement or configure

### `Config`

| Field | Meaning |
|-------|---------|
| `title` | Window title bar |
| `width`, `height` | **Logical** size (DPI-aware). On a 2× display the OS may allocate more physical pixels. |
| `max_fps` | Soft cap. `Some(60)` (default) sleeps between frames. `None` polls as fast as the loop allows (still usually capped by vsync). |

Logical vs physical size: you say “480×720 points.” On a Retina / high-DPI screen the swapchain might be 960×1440 pixels. The camera uses the **aspect ratio** of the drawable, so gameplay coordinates stay the same.

### `Game`

```rust
trait Game {
    fn boot(&mut self, ctx: &mut Context);
    fn update(&mut self, ctx: &mut Context);
}
```

| Method | When | Typical work |
|--------|------|--------------|
| `assets` | Optional; default empty | Paths to preload before `boot` (disk / `fetch`). Required on the web for `load_texture` / `audio.load` |
| `boot` | Once, after the GPU and `assets` are ready | Load textures and sounds, set `ctx.camera`, spawn starting objects, store handles in your struct |
| `update` | Every frame | Read input, change velocities / spawn / despawn, push backdrop & UI into `scene`, tick animations, play SFX |

`boot` receives `dt = 0` and `time = 0`. Do not put ongoing gameplay here.

Your game struct owns **your** state (score, enums like Ready/Playing/Over, animation clips). The engine owns the window, world storage, and GPU.

### `Context`

A bundle of borrows valid for one `boot` / `update` call:

| Field | Type | Notes |
|-------|------|-------|
| `dt` | `f32` | Seconds since last frame, **capped at 0.1**. If the game hitch-stalls for a second, objects only jump as if 100 ms passed — they do not teleport. |
| `time` | `f32` | Seconds since boot finished / clock reset. Useful for oscillators, pipe spawn schedules (`if time >= next_pipe_at`). |
| `input` | `&Input` | Keys, plus cursor position (see [input.md](input.md)). |
| `fb_size` | `[f32; 2]` | Framebuffer **physical** pixels `[width, height]`. Pair with `cursor_px` and `Camera::screen_to_world`. |
| `gfx` | `&mut Gfx` | Load / create textures. Prefer loading in `boot`. |
| `audio` | `&mut Audio` | Load / play sounds. Prefer loading in `boot`. |
| `world` | `&mut World` | Retained entities. |
| `scene` | `&mut Scene` | Cleared every frame *before* `update`. Push anything that is not on a `GameObject`. |
| `camera` | `&mut Camera` | Change whenever you want (usually once in `boot`). |

`Context` is not `Send` across threads and does not outlive the frame. Do not store references from it in your game struct — store **ids** and **TextureIds** instead.

### `run`

```rust
pub fn run(config: Config, game: impl Game) -> Result<(), String>
```

On desktop, blocks until the window closes. Returns `Err` if window/GPU setup fails or the surface is lost. The close button exits with `Ok(())`. Escape does too, unless you are on the web — there `run` returns as soon as the loop is scheduled, and Escape is a normal key ([platform/web.md](platform/web.md)).

## Exact frame order

This order is intentional. Knowing it prevents “why did my collision see last frame’s position?” bugs.

1. **Measure time** — `now`, `dt = min(now - last, 0.1)`, `time = now - start`. The clock is reset when `boot` returns, so the first `update` starts near `time = 0`.
2. **`scene.clear()`** — throw away last frame’s immediate draws.
3. **`game.update(ctx)`** — your logic. Move game objects by writing local `pos` / `rotation` / `scale`, or by calling `integrate_velocities` / `step_rigid_bodies` (the frame loop does not call those for you).
4. **Reap finished one-shot voices** — drop empty players so repeated `play` calls do not leak.
5. **`audio.resolve(dt)`** — decode clips a voice still references; unload PCM that has been unused for 2 seconds.
6. **Copy game-object sprites into the scene** — `World::draw_pose` (local fields when unparented; full parent walk otherwise), then set `center`, add world rotation to `sprite.rotation`, and multiply `sprite.size` by world scale.
7. **`take_sorted()`** — sort all scene sprites by ascending Z, and collect the `TextureId`s those sprites use.
8. **`camera.view_proj(aspect)`** — build the matrix for this window size.
9. **`gfx.ensure_resident`** — upload any used or pinned texture that is not on the GPU yet.
10. **`renderer.render(...)`** — upload + draw + present (see [render/mod.md](render/mod.md)).
11. **`gfx.reap`** — after present, advance the unused timer and drop GPU textures past the 2 second grace. In-flight draws already sampled them.
12. **`input.begin_frame()`** — clear the “pressed this frame” set so edges do not stick.
13. **Schedule next wake** — if `max_fps` is set, `next_frame_at = now + 1/fps`.

### Motion belongs to the game

```rust
if ctx.input.pressed(Key::Space) {
    bird_vy = FLAP;
}
bird_vy += GRAVITY * ctx.dt;
bird.pos.y += bird_vy * ctx.dt;
```

Speeds can live on your `Game` struct, or on the built-in [`Velocity`](object/velocity.md) / [`RigidBody`](object/rigid.md) components. The frame loop still does not step them — call the free functions from `update` when you want that. `GameObject` itself is a transform + optional sprite.

### Why clear scene before update?

So you cannot accidentally “leak” last frame’s score glyphs or backdrop. Everything immediate must be re-issued. World sprites are re-copied automatically after `update`.

## Frame pacing (`max_fps`)

Two different limiters exist:

| Limiter | Where | Effect |
|---------|-------|--------|
| `PresentMode::Fifo` | Renderer | Vsync — typically ≤ monitor Hz, reduces tearing |
| `Config.max_fps` | Event loop | Soft sleep via `ControlFlow::WaitUntil` |

With `max_fps: Some(60)` the loop **does not busy-spin**. When the previous frame finishes, it schedules a wake one period later and only then calls `request_redraw`.

With `max_fps: None` it uses `ControlFlow::Poll` and requests a redraw whenever the loop is about to wait — maximum responsiveness, more CPU wakeups. GPU work is still usually vsync-limited.

After a slow frame, the next deadline is `Instant::now() + period` (not “catch up from a backlog”), so the engine does **not** try to burn multiple updates back-to-back to repay debt. That keeps gameplay from “fast-forwarding” after a hitch; `dt` already absorbed the hitch (up to 0.1 s).

## Window / OS events worth knowing

| Event | Engine reaction |
|-------|-----------------|
| `resumed` | Create the window, then `schedule` GPU init and `Game::assets` preload. `boot` is not called yet |
| user event `GpuReady` | Install `Renderer` + `Gfx`, give both the preloaded bytes, open audio on desktop, then `boot` and reset the clock |
| `RedrawRequested` | Run `frame` |
| `KeyboardInput` | Desktop: Escape exits. Otherwise update `Input`. A press may open audio on the web |
| `MouseInput` | Left press may open audio on the web. Buttons are not stored on `Input` |
| `CursorMoved` / `CursorLeft` | Update `Input` cursor position and inside flag |
| `Resized` / `ScaleFactorChanged` | Resize swapchain + depth buffer, request redraw |
| `CloseRequested` | Exit |
| `about_to_wait` | If FPS-capped and early, sleep; else request redraw |

On some platforms the window is only really ready in `resumed`. That is why `boot` is not called from `run` immediately — there is no GPU yet.

## Minimal game skeleton

```rust
struct MyGame { white: Option<TextureId> }

impl Game for MyGame {
    fn boot(&mut self, ctx: &mut Context) {
        self.white = Some(
            ctx.gfx.create_texture(1, 1, &[255, 255, 255, 255]).unwrap()
        );
    }

    fn update(&mut self, ctx: &mut Context) {
        let white = self.white.unwrap();
        let mut s = Sprite::new(white, Vec3::ZERO, [2.0, 2.0]);
        s.color = [1.0, 0.3, 0.2, 1.0];
        ctx.scene.push(s);
    }
}
```

## Common pitfalls

- **Loading textures or sounds every frame** — do it in `boot` (or cache on first use). Decode / upload is expensive.
- **Storing `&Context` or `&World` in your struct** — lifetimes will not allow it; store `GameObjectId` / `TextureId` / `SoundId`.
- **Expecting `sprite.center` on a `GameObject` to move it** — draw path overwrites center from `world_pos`. Write `pos`.
- **Using `dt` uncapped for menus** — the engine already caps at 0.1; if you need fixed-timestep physics, accumulate time yourself inside the game.
- **Forgetting Escape on desktop** — already handled. On the web it is delivered to `update` like any other key.

## Related docs

- [object/world.md](object/world.md) — what lives across frames
- [scene.md](scene.md) — what you draw once
- [input.md](input.md) — keys and cursor
- [audio.md](audio.md) — load clips and play / stop
- [asset.md](asset.md) — the 2 second residency rule
- [platform/mod.md](platform/mod.md) — desktop vs web around this loop
- [render/mod.md](render/mod.md) — what `render()` does with your sprites  
