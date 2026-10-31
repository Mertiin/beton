# `src/core/ui/`

## Purpose

`Ui` is the **screen-fixed immediate-mode UI**: boxes, images, and bitmap text laid out with [Yoga](https://www.yogalayout.dev/) (pure-Rust `yoga-rs`). It lives in framebuffer pixels (origin top-left, Y down — the same space as `Input::cursor_px`). The game camera does not move it.

There is **one document per frame**, not a stack of UI objects. A HUD and a menu are both nodes in that tree. Rebuild it every `update` the same way you fill `Scene`.

## Mental model

```text
previous frame's boxes  →  hit-test cursor  →  hovered / clicked
Game::update builds tree
Yoga layout → sprites in pixel space
world pass, then UI pass on top
```

Interaction queries describe the **previous** frame. That one-frame lag is what makes `hovered` / `clicked` available during `update`.

## Building

Stable [`UiId`](../lib.md) values. Read interaction first, then build with [`style!`](../lib.md):

```rust
if ctx.ui.clicked(BUTTON) { self.n += 1; }
let hot = ctx.ui.hovered(BUTTON);
ctx.ui.node(PANEL, style!(
    flex_direction: column,
    width: 280 pt,
    padding: 20 pt,
    gap: 12 pt,
    background: Some([0.16, 0.18, 0.24, 1.0]),
), |ui| {
    ui.image(ICON, tex, style!(width: 48 pt, height: 48 pt, fit: Fit::Contain));
    ui.text(LABEL, &font, [18.0, 24.0], style!(color: [1.0, 1.0, 1.0, 1.0]), "12");
    ui.node(BUTTON, style!(width: 120 pt, height: 44 pt, background: Some([0.3, 0.5, 0.9, 1.0])), |_| {});
});
```

Top-level calls are children of an implicit root the size of `ctx.fb_size`. Yoga runs with `use_web_defaults: false` (column, stretch cross-axis, flex-shrink 0, border-box). `boot` does not keep a tree.

### Many panels, submenus

Call `node` more than once at the top level for separate panels. Later siblings draw on top and win the hit-test. Submenus are game state: store an enum, build only the branch you want next frame. Leaving a branch out removes it.

A full-screen dimmer is an absolute node:

```rust
style!(
    position: absolute,
    width: 100 %,
    height: 100 %,
    top: 0 pt,
    left: 0 pt,
    background: Some([0.0, 0.0, 0.0, 0.55]),
)
```

## Style

[`style!`](../lib.md) fills a [`Style`](../lib.md). Unset layout fields keep Yoga defaults. Lengths use `N pt`, `N %` (0–100), `ident pt`, `(expr) pt`, or `auto`.

| Key | Meaning |
|-----|---------|
| `width` / `height` | Border-box size |
| `flex` | Yoga flex shorthand |
| `flex_direction` | `row` / `column` / reverses |
| `justify_content` / `align_items` | Main / cross packing |
| `position` | `relative` / `absolute` / `static` |
| `top` / `right` / `bottom` / `left` | Insets |
| `padding` / `margin` (+ edge / `inline` / `vertical`) | Box edges |
| `gap` / `row_gap` / `column_gap` | Child spacing |
| `background` | Optional RGBA fill (engine white texture) |
| `color` | Text / image tint |
| `fit` | Image `Fill`, `Contain`, `Cover` |
| `uv` | Image UV rect; default `[0, 0, 1, 1]` (full texture). Sprite-sheet frames pass `anim.uv()` |

Text is one line using a `Font` (grid or TTF). For TTF fonts, `glyph_size[1]` is the em height in pixels. Images use `Gfx::texture_size` (times the UV span) for `Contain` / `Cover`.

## Pointer

`Input` tracks the left button: `pointer_down()` / `pointer_pressed()`. Before `update`, the engine hit-tests last frame’s border boxes. The topmost box under the cursor (and its ancestors) are hovered; they are clicked when `pointer_pressed` is also set. The implicit root has no id.

## Draw

UI sprites are drawn in a **second pass** with a pixel orthographic camera after the world pass. They are not Z-sorted with world sprites and always sit on top. Backgrounds use `Gfx::white_texture()`.

## Related docs

- [input.md](input.md) — cursor and pointer
- [scene.md](scene.md) — world-space sprites / fonts
- [anim.md](anim.md) — sheets + clips for `uv`
- [render/mod.md](render/mod.md) — second pass
- [app.md](app.md) — when begin / layout / draw run
