# `src/core/particles/mod.rs`

## Purpose

Short-lived tinted quads for trails, blasts, and debris. Games emit into `ctx.particles`, call [`step_particles`](../lib.md) each `update`, and the engine draws them after world sprites.

## Emit

```rust
ctx.particles.emit(Particle::trail(white, shader, pos, color, 0.35, 4.0));
ctx.particles.emit(Particle::blast(white, shader, pos, color, 0.35, 8.0, 40.0));
ctx.particles.emit(Particle::fragment(white, pos, vel, color, 0.5, 2.0, -180.0));
```

Over [`MAX_PARTICLES`](../lib.md) (200), the oldest particle is dropped.

## Step

```rust
step_particles(ctx.particles, ctx.dt, |pos| {
    // return true to kill particles with `collide: true`
    false
});
```

Life, gravity, and motion are integrated here. Fade and size lerp happen at draw time from `life / max_life`.

## Related

- [scene.md](scene.md) — where particle sprites land
- [render/shader.md](render/shader.md) — custom shaders on a particle
