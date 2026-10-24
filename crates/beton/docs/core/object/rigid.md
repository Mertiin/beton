# `src/core/object/rigid.rs`

## Purpose

Built-in **rigid body** material and a free-function oriented-box contact solver. Mass and bounce live here. Linear and angular speed live on [`Velocity`](velocity.md).

`World` does not know about this type. The frame loop does not call the stepper. Your game attaches components and calls `step_rigid_bodies`.

## `RigidBody`

| Field | Meaning |
|-------|---------|
| `inv_mass` | Inverse mass. `0` = fixed (infinite mass) |
| `restitution` | Bounce. Solver clamps to `0..=1` |
| `friction` | Tangential friction. Solver clamps to `>= 0` |

Helpers:

| Constructor | Effect |
|-------------|--------|
| `RigidBody::dynamic(mass)` | `inv_mass = 1/mass`. Mass `<= 0` becomes fixed |
| `RigidBody::fixed()` | `inv_mass = 0` |
| `Default` | Dynamic mass 1, restitution 0.2, friction 0.4 |

A fixed floor is only a `RigidBody`. A falling crate is `RigidBody` **and** `Velocity`.

## Who moves

| Components | What runs |
|------------|-----------|
| `Velocity` only | [`integrate_velocities`](velocity.md) |
| Fixed `RigidBody`, no `Velocity` | Static collider. Solver pushes others off it |
| Dynamic `RigidBody` + `Velocity` | `step_rigid_bodies` (gravity, integrate, impulses) |
| Either with a parent | Left alone |

A `Velocity` stuck on a fixed body is ignored by both steppers.

## `step_rigid_bodies(world, dt, gravity)`

```rust
step_rigid_bodies(ctx.world, ctx.dt, Vec3::new(0.0, -32.0, 0.0));
```

`gravity` is per call, not stored on `World`. Unparented objects only. The hitbox is the **axis-aligned bounds** of `size * scale` at the current `rotation` (tilted boxes get a larger AABB). Sprites still spin with `rotation`.

Because `each` pulls a component store out of the world, the step cannot pair bodies inside a query. It records ids, then reads and writes through the public getters after `each` returns.

### Substeps

`dt <= 0` is a no-op. Otherwise the call is split into chunks of `1/120` s, at most 8, so a hitch does not tunnel through the floor in one jump.

Each substep:

1. Dynamic bodies: `linear += gravity * h`, then `pos += linear * h` and `rotation += angular * h`. Fixed bodies skip all three.
2. Ten iterations of every pair: AABB-of-OBB overlap, separate on the smaller axis (Baumgarte `0.8`, slop `0.02`), then normal/friction impulses with light torque from box inertia.
3. Contact damping, a soft upright bias when nearly resting, then sleep only when nearly axis-aligned and slow.

Z is integrated and never collided.

## Common pitfalls

- **Expecting the frame loop to step** — it does not. Call this from `update`.
- **Dynamic without `Velocity`** — skipped. Attach both.
- **Also calling `integrate_velocities` on the same crate** — that function skips dynamic rigid bodies, so it is safe, but do not also write `pos += vel * dt` yourself.
- **Parented crates** — not moved. Unparent first.
- **Expecting exact OBB edges** — contacts use the AABB of the rotated box, so a diamond rests as if it had a larger upright hitbox.
- **Huge angular speeds** — angular is clamped; hard kicks can still look wild for a frame.

## Related docs

- [velocity.md](velocity.md) — shared linear speed
- [game_object.md](game_object.md) — `size` and AABB
- [world.md](world.md) — `insert`, `each`, poses
- [component.md](component.md) — storage
