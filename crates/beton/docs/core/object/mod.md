# `src/core/object/mod.rs`

## Purpose

Module root for retained game objects. Like [`math/mod.rs`](../math/mod.md), this file has no behavior. It declares the children and re-exports the names games use:

| Child | Re-export | Doc |
|-------|-----------|-----|
| `game_object.rs` | `GameObject`, `GameObjectId` | [game_object.md](game_object.md) |
| `world.rs` | `World` | [world.md](world.md) |
| `component.rs` | `Query` | [component.md](component.md) |
| `velocity.rs` | `Velocity`, `integrate_velocities` | [velocity.md](velocity.md) |
| `rigid.rs` | `RigidBody`, `step_rigid_bodies` | [rigid.md](rigid.md) |

`lib.rs` then exports those names from `beton`. `ComponentStore` and `ComponentMap` stay private to this module — games attach data through `World::insert` and `World::each`, not by building a store themselves.

`World` stays type-agnostic. `Velocity` and `RigidBody` are ordinary components; their steppers are free functions over `&mut World`, not methods on it.

## How the files split the work

```text
GameObject     what one actor looks like (transform, sprite, tag, hit size)
World          the list of actors, parents, and the component map
component.rs   how “every object that has Body and Collider” is stored and visited
velocity.rs    shared linear speed + plain integrator
rigid.rs       mass / material + AABB contact step
```

`World` is the only storage type you hold. It owns the slots and the component map. `Query` is a trait implemented for tuples of one to four component types so `world.each::<(Body, Collider)>(…)` can type-check.

## Related docs

- [game_object.md](game_object.md) — fields on one object
- [world.md](world.md) — spawn, parents, poses
- [component.md](component.md) — sparse sets and `each`
- [velocity.md](velocity.md) — `Velocity`
- [rigid.md](rigid.md) — `RigidBody`
- [../../lib.md](../../lib.md) — public export list
