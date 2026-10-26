# `src/core/input/`

## Purpose

Provide a simple, game-friendly view of the keyboard for the current frame: which keys are **held**, and which keys **went down this frame**.

The OS delivers irregular key events. Games want polling-style questions inside `update`. This file bridges the two.

## Two questions games ask

| API | Question | Use case |
|-----|----------|----------|
| `input.down(Key::Space)` | Is Space currently held? | Move while held, charge a shot |
| `input.pressed(Key::Space)` | Did Space transition up→down **this frame**? | Jump, flap, confirm, pause toggle |

Using `down` for a flap makes the bird rocket skyward for as long as you hold the key. Using `pressed` fires once per keystroke — what Flappy wants.

## How state is tracked

Internally:

```text
down:    HashSet of keys currently held
pressed: HashSet of keys that became held since last begin_frame
```

On a keyboard event:

- **Press:** if the key was not already in `down`, insert into both `down` and `pressed` (so auto-repeat from the OS does not spam `pressed`).
- **Release:** remove from `down`.

After your `update` and the draw, the engine calls `begin_frame()`, which **clears `pressed` only**. `down` stays accurate across frames.

### Why clear pressed after the frame?

Keyboard events can arrive *before* `RedrawRequested` in the same turn of the event loop. Clearing pressed *before* `update` would drop those edges. Clearing *after* draw keeps them visible for the frame that is about to run, then forgets them.

## `Key` type

```rust
pub use winit::keyboard::KeyCode as Key;
```

These are **physical** keys (the key in that position on a US-QWERTY-ish layout), not typed characters. `Key::KeyW` is the W key even if the user’s layout maps that position differently for text entry.

Common codes:

- `Key::Space`, `Key::Escape` (Escape also quits the app in `app.rs`)
- `Key::KeyA` … `Key::KeyZ`
- `Key::ArrowUp`, `ArrowDown`, `ArrowLeft`, `ArrowRight`
- `Key::Digit0` … `Digit9`
- `Key::ShiftLeft`, `ControlLeft`, …

See winit’s `KeyCode` docs for the full list. The engine **pins** the winit major version in the workspace so your game and the engine share one `Key` type.

## Cursor

The same snapshot tracks the pointer. Positions are **physical framebuffer pixels**, origin top-left, Y down — not world units.

| API | Meaning |
|-----|---------|
| `cursor_px()` | Last position from `CursorMoved`, as `[x, y]` |
| `cursor_inside()` | `true` after a move over the window, `false` after `CursorLeft` |

Turn pixels into world XY with `ctx.camera.screen_to_world(ctx.input.cursor_px(), ctx.fb_size[0], ctx.fb_size[1])` (orthographic cameras). See [camera.md](camera.md).

## Left pointer button

| API | Meaning |
|-----|---------|
| `pointer_down()` | Left mouse button currently held |
| `pointer_pressed()` | Left button went down **this frame** |

Same edge rules as keys: OS repeats do not spam `pointer_pressed`. Cleared in `begin_frame` after the frame. Used by [ui.md](ui.md) for clicks; a left press still unlocks audio on the web.

## What this module does not handle

- Right / middle mouse buttons, or touch points
- Gamepad
- Text input (for typing your name) — that needs characters, not key codes
- Key binding remapping UI

## Patterns

### Toggle pause

```rust
if ctx.input.pressed(Key::KeyP) {
    paused = !paused;
}
```

### Hold to move

```rust
let mut dir = 0.0;
if ctx.input.down(Key::ArrowLeft) { dir -= 1.0; }
if ctx.input.down(Key::ArrowRight) { dir += 1.0; }
player.pos.x += dir * SPEED * ctx.dt;
```

### Chord

```rust
if ctx.input.down(Key::ShiftLeft) && ctx.input.pressed(Key::KeyR) {
    restart();
}
```

## Common pitfalls

- **Using `pressed` for continuous movement** — motion stops between OS key-repeat events; use `down`.
- **Using `down` for single-shot actions** — action repeats every frame; use `pressed`.
- **Expecting Escape in game logic on desktop** — the engine exits first; your `update` may not see it. On the web it is a normal key.
- **Comparing character `'a'`** — wrong layer; use `Key::KeyA`.
- **Treating `cursor_px` as world units** — it is framebuffer pixels. Convert with the camera.

## Related docs

- [app.md](app.md) — when events are applied vs when `begin_frame` runs
- [camera.md](camera.md) — `screen_to_world`
- [ui.md](ui.md) — screen UI hit-testing from the pointer

