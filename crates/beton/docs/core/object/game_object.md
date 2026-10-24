# `src/core/object/game_object.rs`

## Purpose

The data for **one** retained actor: who it is (`GameObjectId`), and the built-in fields every object has (`GameObject`).

Extra data (velocity, health, AI) does **not** go on this struct. Attach it with [`World::insert`](world.md). This file has no storage and no hierarchy walk — only the struct, the id, and a plain AABB test.

## `GameObjectId`

```text
GameObjectId(index, generation)
```

A generational handle. `World` checks both numbers before it returns an object. After `despawn`, the same slot can be reused with a new generation, and the old id still returns `None`.

The two fields are `pub(crate)`. Games treat the id as an opaque `Copy` value: store it, pass it to `world.get`, do not invent indices.

## `GameObject` fields

| Field | Meaning |
|-------|---------|
| `pos` | **Local** position relative to `parent` (world position if `parent` is `None`) |
| `rotation` | **Local** radians |
| `scale` | **Local** XY scale, default `[1.0, 1.0]` |
| `parent` | Optional parent id. Prefer [`World::set_parent`](world.md) so cycles are rejected |
| `size` | Unscaled hitbox width and height. Collision uses `size * world_scale` |
| `sprite` | If `Some`, drawn at the resolved **world** pose each frame |
| `tag` | Your classification integer (bird = 1, pipe = 2, …) |

There is no `velocity` field on this struct. The frame loop does not integrate motion, and `World` does not name motion types. Write `pos` / `rotation` / `scale` yourself, or attach the built-in components:

```rust
world.insert(id, Velocity::linear(Vec3::new(0.0, -2.0, 0.0)));
integrate_velocities(world, dt);

// Falling crate: mass + velocity, then the rigid step.
world.insert(crate_id, Velocity::ZERO);
world.insert(crate_id, RigidBody::dynamic(1.0));
step_rigid_bodies(world, dt, Vec3::new(0.0, -32.0, 0.0));
```

See [velocity.md](velocity.md) and [rigid.md](rigid.md).

### Hitbox vs sprite

`size` and `sprite.size` are independent. A bird can draw larger than the box `overlaps` uses. `size` is not scaled until a world query multiplies it by the resolved scale. The field itself stays in the object’s local units.

## `aabb_overlaps`

```rust
GameObject::aabb_overlaps(center_a, size_a, center_b, size_b) -> bool
```

Axis-aligned box test in XY. Z is ignored. Each size is a full width and height around the center (half-extents are `size * 0.5`).

The comparisons are strict (`<` / `>`). Two boxes that only share an edge return `false`. A pipe that sits exactly against the bird is not a hit; they must actually cross.

Most games should call [`World::overlaps`](world.md) instead. That helper resolves parented positions and multiplies each object’s `size` by world scale, then calls this function. Use `aabb_overlaps` directly when you already have centers and sizes (tests, a custom volume).

## Common pitfalls

- **Writing `sprite.center` to move an object** — the frame loop overwrites center from the resolved pose. Write `pos`.
- **Putting `parent` on the struct and expecting a cycle check** — assigning the field skips `set_parent`.
- **Expecting edge contact to count** — touching is not overlap.
- **Storing a despawned id** — it will not follow the object that reused the slot.

## Related docs

- [world.md](world.md) — spawn, parents, `overlaps`
- [component.md](component.md) — data that is not a field
- [velocity.md](velocity.md) — shared linear speed
- [rigid.md](rigid.md) — mass and AABB contacts
- [../scene.md](../scene.md) — what `sprite` points at
