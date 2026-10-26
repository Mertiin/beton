# `src/core/object/velocity.rs`

## Purpose

Built-in **velocity** component (linear + angular) and a free-function integrator. This is the only engine type that holds units/second and radians/second. A pipe, a bullet, and a crate all use it; only the crate also needs [`RigidBody`](rigid.md), and only when the `physics` feature is on.

`World` does not know about this type. Attach it with `world.insert` like any other component.

## `Velocity`

```rust
pub struct Velocity {
    pub linear: Vec3,  // units / second
    pub angular: f32,  // radians / second around Z
}
```

Helpers: `Velocity::ZERO`, `Velocity::linear(v)` (angular `0`).

```rust
world.insert(id, Velocity::linear(Vec3::new(-6.0, 0.0, 0.0)));
```

## `integrate_velocities(world, dt)`

Writes `pos += linear * dt` and `rotation += angular * dt` for **unparented** objects that have `Velocity`. With the `physics` feature, objects that also have a `RigidBody` are skipped.

| Object has… | Result |
|-------------|--------|
| `Velocity` only | Moved and spun |
| `Velocity` + `RigidBody` | Skipped (owned by [`step_rigid_bodies`](rigid.md), or ignored if fixed) |
| Parent set | Skipped (local `pos` is not world space) |

`dt <= 0` is a no-op. The frame loop never calls this — your `Game::update` does.

```rust
integrate_velocities(ctx.world, ctx.dt);
```

Do not also write `pos += vel * dt` on an object that [`step_rigid_bodies`](rigid.md) already moved.

## Common pitfalls

- **Inventing a second speed field** — use `Velocity` so physics and plain movers share one type.
- **Calling this and `step_rigid_bodies` on the same dynamic body** — the rigid step already integrates; this function skips those objects, but writing `pos` yourself would double-move.
- **Parented movers** — neither stepper walks the hierarchy. Unparent, or write local `pos` yourself.

## Related docs

- [rigid.md](rigid.md) — mass, contacts, gravity
- [component.md](component.md) — how `insert` / `each` store this
- [game_object.md](game_object.md) — write `pos`, not `sprite.center`
