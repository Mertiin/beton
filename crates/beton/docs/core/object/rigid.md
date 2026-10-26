# `src/core/object/rigid.rs`

Compiled only with the `physics` feature (`beton = { features = ["physics"] }`). Off by default.

## Purpose

Built-in **rigid body** material and a free-function step that drives [Rapier 2D](https://rapier.rs/). Mass and bounce live here. Linear and angular speed live on [`Velocity`](velocity.md).

`World` does not name `RigidBody` in its public API. The frame loop does not call the stepper. Your game attaches components and calls `step_rigid_bodies`. The stepper keeps a private Rapier world on `World`, and a private `SimBody` component (body handle plus the last pose it wrote) so the next step can see what the game changed.

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
| `Default` | Dynamic mass 1, restitution 0.15, friction 0.55 |

A fixed floor is only a `RigidBody`. A falling crate is `RigidBody` **and** `Velocity`.

## Who moves

| Components | What runs |
|------------|-----------|
| `Velocity` only | [`integrate_velocities`](velocity.md) |
| Fixed `RigidBody`, no `Velocity` | Static collider. Rapier pushes others off it |
| Dynamic `RigidBody` + `Velocity` | `step_rigid_bodies` (gravity, integrate, contacts) |
| Either with a parent | Left alone. A body that gains a parent is removed from Rapier |

A `Velocity` stuck on a fixed body is ignored by both steppers.

## `step_rigid_bodies(world, dt, gravity)`

```rust
step_rigid_bodies(ctx.world, ctx.dt, Vec3::new(0.0, -32.0, 0.0));
```

`gravity` is per call, not stored on `World`. Unparented objects only. The hitbox is an **oriented cuboid** of `size * scale` (half-extents go to Rapier). Sprites spin with `rotation`. Z is integrated from `Velocity` and gravity, and never collided — Rapier is 2D.

The first step builds a Rapier body per participant and stores `SimBody` on that object. Later steps copy `pos`, `rotation`, and `Velocity` into Rapier only when they differ from the last values this function wrote, so a sleeping stack is not woken by the sync. Changing mass, friction, restitution, `size`, or `scale` rebuilds that body.

`clear` drops the Rapier world together with every object. `despawn` drops `SimBody` immediately; the Rapier body is removed on the next step. Ids are generational, and `clear` restarts them at 0, which is why the simulation cannot outlive `clear`.

### Substeps

`dt <= 0` is a no-op. Otherwise the call is split into slices of `1/120` s, at most 8, so a hitch does not tunnel through the floor in one jump. Dynamic bodies use continuous collision. Fixed-only scenes skip the Rapier step.

Each slice steps Rapier with that slice of `dt` and the XY gravity. Z uses the same slices: `vz += gravity.z * h`, then `z += vz * h`.

## Common pitfalls

- **Expecting the frame loop to step** — it does not. Call this from `update`.
- **Dynamic without `Velocity`** — skipped. Attach both.
- **Also calling `integrate_velocities` on the same crate** — that function skips rigid bodies, so it is safe, but do not also write `pos += vel * dt` yourself.
- **Parented crates** — not moved, and not colliders. Unparent first.
- **Writing `pos` or `Velocity` between steps** — the next step copies those writes onto the Rapier body. That is how a kick works.
- **Expecting an axis-aligned hitbox** — a diamond rests on a corner. `World::overlaps` is still a plain AABB and ignores rotation.

## Related docs

- [velocity.md](velocity.md) — shared linear speed
- [game_object.md](game_object.md) — `size` and AABB
- [world.md](world.md) — `insert`, `each`, poses
- [component.md](component.md) — storage
