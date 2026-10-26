# `src/core/ui/`

## Purpose

`Ui` is the **screen-fixed immediate-mode UI**: boxes, images, and bitmap text laid out with a small CSS-like box model. It lives in framebuffer pixels (origin top-left, Y down — the same space as `Input::cursor_px`). The game camera does not move it.

There is **one document per frame**, not a stack of UI objects. A HUD and a menu are both nodes in that tree. Rebuild it every `update` the same way you fill `Scene`.

## Mental model

```text
previous frame's boxes  →  hit-test cursor  →  hovered / clicked
Game::update builds tree
layout → sprites in pixel space
world pass, then UI pass on top
```

Interaction queries describe the **previous** frame. That one-frame lag is what makes `hovered` / `clicked` available during `update`.

## Building

Stable [`UiId`](../lib.md) values. Read interaction first, then build:

```rust
if ctx.ui.clicked(BUTTON) { self.n += 1; }
let hot = ctx.ui.hovered(BUTTON);
ctx.ui.node(PANEL, style, |ui| {
    ui.image(ICON, tex, image_style);
    ui.text(LABEL, &font, [18.0, 24.0], text_style, "12");
    ui.node(BUTTON, button_style, |_| {});
});
```

Top-level calls are children of an implicit root the size of `ctx.fb_size`. `boot` does not keep a tree.

### Many panels, submenus

Call `node` more than once at the top level for separate panels. Later siblings draw on top and win the hit-test. Submenus are game state: store an enum, build only the branch you want next frame. Leaving a branch out removes it.

A full-screen dimmer is an ordinary node with a background — not a separate modal API.

## Style

One struct, not selectors:

| Field | Meaning |
|-------|---------|
| `display` | `Block` (default) or `Flex` |
| `direction` | `Row` / `Column` (flex) |
| `width` / `height` | `Auto`, `Px`, `Pct` (of parent content), `Fill` |
| `margin` / `padding` | `[top, right, bottom, left]`; `Style::edges(v)` |
| `gap`, `justify`, `align` | Flex packing (default align `Stretch`) |
| `background` | Optional RGBA fill (engine white texture) |
| `color` | Text / image tint |
| `fit` | Image `Fill`, `Contain`, `Cover` |

**Border-box**: width and height include padding. Margins do not collapse. `Pct` on an auto-sized parent is treated as `Auto`.

Block stacks children downward and stretches an `Auto` width to the parent. Flex is one line, no wrap. `Fill` children split positive free space equally and do not shrink.

Text is one line using a `Font` (grid or TTF). For TTF fonts, `glyph_size[1]` is the em height in pixels. Images use `Gfx::texture_size` for `Contain` / `Cover`.

## Pointer

`Input` tracks the left button: `pointer_down()` / `pointer_pressed()`. Before `update`, the engine hit-tests last frame’s border boxes. The topmost box under the cursor (and its ancestors) are hovered; they are clicked when `pointer_pressed` is also set. The implicit root has no id.

## Draw

UI sprites are drawn in a **second pass** with a pixel orthographic camera after the world pass. They are not Z-sorted with world sprites and always sit on top. Backgrounds use `Gfx::white_texture()`.

## Related docs

- [input.md](input.md) — cursor and pointer
- [scene.md](scene.md) — world-space sprites / fonts
- [render/mod.md](render/mod.md) — second pass
- [app.md](app.md) — when begin / layout / draw run
