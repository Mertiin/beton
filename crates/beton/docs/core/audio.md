# `src/core/audio/mod.rs`

## Purpose

Register sound files or bytes, decode them when first played (or pinned), and play through the default output device. Game code only sees `SoundId`, `SoundHandle`, and `VoiceId` — never rodio types.

## Register vs resident

```
boot:   load / create  →  SoundId (source registered)
        pin(id)        →  SoundHandle (keeps PCM resident)
update: play / play_loop  →  VoiceId; decodes if needed
        unused for 2s     →  resolve drops PCM, keeps the source
```

Holding only a `SoundId` does not keep the clip decoded. A playing voice or a `SoundHandle` does. Voices clone samples at `play`, so unloading later does not cut off a voice that is already playing.

## Handles

| Handle | Meaning |
|--------|---------|
| `SoundId` | Generational id into the clip store |
| `SoundHandle` | Owning pin; `.id()` for `play` |
| `VoiceId` | One playing voice; invalid after `stop` or when a one-shot ends |

`stop` / `set_volume` on an unknown voice are silent no-ops.

## Loading

```rust
let flap = ctx.audio.load(Path::new("assets/flap.wav"))?;
let blip = ctx.audio.create(&wav_bytes)?;
let pin = ctx.audio.pin(flap).expect("live");
ctx.audio.play(flap, 0.4);
```

| Method | Behavior |
|--------|----------|
| `load(path)` | Prefers preloaded [`Game::assets`](app.md) bytes; else file must exist; same path → same id |
| `create(bytes)` | Validates by decoding once, stores bytes, PCM not kept until play/pin |
| `pin(id)` | Keeps the clip resident until the handle drops |

**Features:** `wav` and `vorbis` are on by default.

## Playback

```rust
let voice = ctx.audio.play(flap, 0.4);
let music = ctx.audio.play_loop(theme, 0.2);
ctx.audio.set_volume(voice, 0.2);
ctx.audio.stop(music);
```

On the web the device opens on the first key/click. Plays issued before that (for example BGM in `boot`) stay queued and start when the device opens; registered clips are not dropped.

## Resolve (engine)

After `reap_finished`, the app calls `audio.resolve(dt)`. Playing voices count as references. Unused clips unload after 2 seconds.

## Memory helpers

| Method | Meaning |
|--------|---------|
| `live_count()` | How many clips are decoded |
| `resident_bytes()` | Approx RAM: sum of PCM `f32` sample buffers |
| `retained_source_bytes()` | CPU bytes kept for re-decode (`create` sources; paths contribute 0) |

## Related docs

- [app.md](app.md) — frame order
- [render/texture.md](render/texture.md) — same lifetime rules for images
