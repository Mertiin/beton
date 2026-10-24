# `src/core/asset.rs`

## Purpose

One small state machine that both **textures** and **sounds** follow: stay registered forever, become resident on first use, and drop the heavy payload after it has been unused for a short grace period.

This file does not talk to the GPU or to the audio device. It only answers “should I load, keep, or unload?”. `Gfx` and `Audio` own the real pixels and PCM and apply that answer.

Games never import this module. You feel it as `pin`, `live_count`, and “it reloaded after I stopped drawing it.”

## The two layers

| Layer | What it is | When it goes away |
|-------|------------|-------------------|
| **Registration** | A `TextureId` / `SoundId` plus the original path or bytes | When the store is cleared (not during normal play) |
| **Residency** | GPU texture or decoded PCM | After 2 seconds with no reference and no pin |

`UNUSED_GRACE_SECS` is `2.0`. A one-frame gap does not unload. A texture that leaves the screen and comes back within two seconds is still uploaded.

## `AssetLifetime`

Each slot holds one of these:

| Field | Meaning |
|-------|---------|
| `pin` | An `Arc<()>` shared with every `TextureHandle` / `SoundHandle` |
| `resident` | Whether the heavy payload is supposed to be in memory |
| `unused_for` | Seconds accumulated while nothing references it |

**Pinned** means `Arc::strong_count > 1`: the slot keeps one clone, and each handle keeps another. Dropping the last handle brings the count back to 1, so the grace timer is allowed to run.

`pin()` clones that `Arc` into a `Pin`. Callers turn it into the `Arc` stored on the public handle (`into_arc`).

## `resolve(referenced, dt)` → `ResolveAction`

`referenced` is “something used it this step” (a sprite on screen, a playing voice) **or** the slot is pinned. The function ORs the pin in itself, so callers can pass “drawn / playing” only.

| Situation | Action | Side effect |
|-----------|--------|-------------|
| Referenced, not resident yet | `Load` | Marks resident, zeros the timer |
| Referenced, already resident | `Keep` | Zeros the timer |
| Not referenced, not resident | `Keep` | Nothing to unload |
| Not referenced, resident, timer still under 2s | `Keep` | Adds `dt` |
| Not referenced, resident, timer reaches 2s | `Unload` | Clears resident and the timer |

A later reference during the grace period **resets** `unused_for` to 0. You do not “finish the remaining second” after a flicker of use.

`mark_resident` / `mark_unloaded` exist so a caller can correct the flag when the real upload or decode fails, or when it already knows the payload is present.

## How the two stores use it

They share the rules and differ in **when** unload is allowed to happen.

| Store | Load | Unload |
|-------|------|--------|
| `Audio` | `resolve(dt)` in one pass: `Load` decodes PCM immediately | Same pass: `Unload` drops PCM. A voice already playing copied its samples, so it keeps sounding. |
| `Gfx` | `ensure_resident` **before** draw uploads anything used or pinned | `reap` **after** present runs `resolve` and drops the GPU texture. In-flight draws still have a bind group. |

`reap` ignores a `Load` result from that second call. Upload already happened in `ensure_resident`. The post-present pass is only there to advance the unused timer and drop textures.

## Tests in this file

The unit tests drive `AssetLifetime` with no GPU and no audio device: first reference loads, grace keeps then unloads, a reference resets the timer, a pin blocks unload, dropping the pin allows the grace unload.

If you change the grace length or the pin rule, these tests are the place that should fail first. Texture and sound tests then check that their stores honor the same actions.

## Common pitfalls

- **Treating a `TextureId` as a pin** — the id does not keep the payload. Pin, or keep drawing / playing.
- **Expecting unload the frame something disappears** — two seconds of grace is intentional.
- **Unloading a GPU texture before `present`** — that is why `Gfx` splits ensure and reap. Do not fold them into one call.

## Related docs

- [render/texture.md](render/texture.md) — images
- [audio.md](audio.md) — clips
- [app.md](app.md) — where ensure / reap / resolve run in the frame
