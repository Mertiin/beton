# `src/core/anim/`

## Purpose

Turn a **sprite sheet** (many frames in one texture) into timed playback. You get UV rectangles for free and a small clip player (`Animation`) that advances with `dt`.

This is CPU-side bookkeeping. The GPU still just draws whatever UV you put on a `Sprite`.

## Why sheets instead of many textures?

| Many textures | One sheet |
|---------------|-----------|
| Swap `TextureId` every frame | Change `uv` on the same texture |
| More uploads / bind groups | One bind; good for batching |
| Easy to author as separate PNGs | Artist packs a strip/grid (or you stitch at build time) |

Flappy ships `bird-sheet.png` as three frames in a row and plays them with `Animation`.

## `SpriteSheet`

```rust
SpriteSheet {
    texture,   // atlas TextureId
    columns,   // cells across
    rows,      // cells down
}
```

### Indexing (row-major)

```
columns = 4, rows = 2

 0  1  2  3
 4  5  6  7
```

`uv(frame)`:

1. Wraps `frame` with modulo `columns * rows`
2. `col = frame % columns`, `row = frame / columns`
3. Builds `[u0, v0, u1, v1]` for that cell
4. Insets by 2% of the cell size on each edge (bleed guard)

You can call `uv` without an `Animation` if you want to drive frames yourself.

`apply(&mut sprite, frame)` writes `texture` + `uv` for a static cell — useful when many objects share one sheet without each needing a clip.

**Sharing:** `SpriteSheet` is a tiny handle (`TextureId` + grid size). Cloning it does not copy GPU pixels. One atlas, many objects/clips.

### Authoring constraints

Sheets assume **equal cell sizes** that divide the image evenly. A 102×24 image with `columns: 3` implies 34×24 cells. Uneven packing needs custom UV math.

## `Animation`

A clip is: “play these sheet indices, at this rate.”

| Field | Meaning |
|-------|---------|
| `sheet` | Atlas to sample |
| `frames` | Play list of cell indices — may repeat, e.g. `[0, 1, 2, 1]` |
| `fps` | Steps of that list per second |
| `looping` | Restart forever, or freeze on last |

Private state: `time`, `finished`.

### Constructors

- `Animation::new(sheet, frames, fps)` — looping
- `Animation::once(sheet, frames, fps)` — one-shot; then `finished()` is true and `tick` no-ops
- `Animation::instance(&self)` — same sheet/frames/fps/looping with a **fresh** clock (unlike `Clone`, which copies `time`)

### Time → frame

```text
index_in_list = floor(time * fps)   (mod length if looping)
cell = frames[index_in_list]
```

Example: `frames = [0, 1, 2, 1]`, `fps = 10`.

| time | list index | cell |
|------|------------|------|
| 0.00 | 0 | 0 |
| 0.10 | 1 | 1 |
| 0.20 | 2 | 2 |
| 0.30 | 3 | 1 |
| 0.40 | 0 | 0 (loop) |

### Methods

| Method | Role |
|--------|------|
| `tick(dt)` | Advance clock (no-op if finished / empty / fps≤0) |
| `frame_index()` | Current sheet cell |
| `uv()` / `texture()` | What to put on a sprite |
| `apply(&mut sprite)` | Writes `texture` + `uv` onto a sprite |
| `reset()` | `time = 0`, `finished = false` |
| `instance()` | Spawn another clip from this template |
| `finished()` | One-shot completed |

## Integration pattern (important)

The engine does **not** auto-tick animations. Your game owns them.

```rust
// in update
if state != State::Over {
    anim.tick(ctx.dt);
}
if let Some(bird) = ctx.world.get_mut(bird_id) {
    if let Some(sprite) = bird.sprite.as_mut() {
        anim.apply(sprite);
    }
}
```

That pattern lets you **freeze** the flap on death by skipping `tick`, or `reset()` on restart.

### Where to store the clip

Usually on your `Game` struct (`bird_anim: Option<Animation>`), not inside `GameObject`. Game objects stay generic; clips are gameplay policy. For many enemies sharing one atlas:

```rust
let mut a = template.instance();
let mut b = template.instance(); // independent time, same sheet
```

## One-shot example (explosion)

```rust
let mut boom = Animation::once(sheet, [0, 1, 2, 3, 4], 12.0);
// each frame:
boom.tick(ctx.dt);
boom.apply(sprite);
if boom.finished() {
    world.despawn(fx_id);
}
```

## Designing frame lists

| List | Feel |
|------|------|
| `[0, 1, 2]` | Linear |
| `[0, 1, 2, 1]` | Bounce (Flappy flap) |
| `[0, 0, 0, 1, 2]` | Hold on first pose |
| `[2]` | Static “use cell 2” via the same API |

## Common pitfalls

- **Forgetting `apply`** — `tick` alone does not change what is drawn.
- **Wrong columns/rows** — UVs sample the wrong slice; bird looks like a smear of neighbors.
- **Animating by swapping textures and also using a sheet** — pick one approach.
- **Very high fps with low `dt` variance** — fine; at 60 Hz display and fps=10 you still see each cell for multiple presents.
- **Expecting interpolation** — frames are discrete; no tweening between cells.

## Related docs

- [scene.md](scene.md) — what `uv` means on a sprite  
- [render/texture.md](render/texture.md) — loading the atlas image  
- [world.md](object/world.md) — putting the sprite on an object  
