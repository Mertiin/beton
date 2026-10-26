# `src/core/object/world.rs`

## Purpose

The **cast list**: every live [`GameObject`](game_object.md), its generational id, its parent pointer, and the component map from [`component.rs`](component.md).

Objects stay until you `despawn` or `clear` them. Contrast with [`Scene`](../scene.md), which forgets every sprite at the start of the next frame.

`World` does not integrate motion and does not name gameplay component types. Games write local `pos` / `rotation` / `scale`, drive them from `each`, or call [`integrate_velocities`](velocity.md) / [`step_rigid_bodies`](rigid.md).

## Slots

```text
slots: Vec<Slot { generation, object: Option<GameObject> }>
free:  Vec<slot index>
```

`spawn` pops a free index if there is one, otherwise it pushes a new slot at generation 0. `despawn_one` sets `object` to `None`, bumps `generation` with wrapping add, pushes the index onto `free`, and `clear_slot`s every component.

`get` / `get_mut` require both the index and the generation to match, and the slot to still hold an object. A stale id returns `None` even when that slot was reused.

## `despawn` removes descendants

`despawn` collects current children and despawns them first, then the object itself. Grandchildren go with them. Components on every removed id are dropped.

`clear` drops every slot, the free list, every component store, and the rigid-body simulation. The next spawn starts at generation 0, so the simulation has to go too or a new object would reuse a stale Rapier body.

`children(id)` is “live objects whose `parent` is this id,” not a stored child array. Order follows slot order.

## Parent / child

World pose walks the parent chain. Components are **not** inherited.

### `set_parent(child, parent, world_position_stays)`

Returns `false` (and changes nothing) when:

- `child` or `parent` is not live
- `parent` is `child`
- parenting `child` under `parent` would cycle (walk up from the new parent, depth guard 64)

`world_position_stays: true` (the usual choice) rewrites local `pos`, `rotation`, and `scale` so the **world** pose does not jump.

`world_position_stays: false` keeps the current local numbers and only changes `parent`. The object will pop to a new world pose.

Local position under a parent is scaled by the parent’s world scale, then rotated by the parent’s world rotation, then added to the parent’s world position. Z is added and is not rotated. Rotation accumulates. Scale multiplies per axis.

If a parent scale axis is almost zero (`abs <= 1e-8`), that axis’s local position becomes 0 and local scale keeps the child’s world scale instead of dividing.

### Pose helpers

| Method | Result |
|--------|--------|
| `world_pos` / `world_rotation` / `world_scale` | Full parent walk. `None` if any id in the chain is dead |
| `draw_pose` | Local fields when `parent` is `None` (no walk). Otherwise the three helpers above |
| `overlaps(a, b)` | World-space AABB. Each `size` is multiplied by that object’s world scale. Edge contact is not a hit (see [game_object.md](game_object.md)) |

The frame loop uses `draw_pose`, then writes `sprite.center` from the position, **adds** world rotation to `sprite.rotation`, and multiplies `sprite.size` by world scale. A sprite authored with its own rotation still spins relative to the object.

## Component API on `World`

The methods are here; the storage is in [`component.rs`](component.md).

| Method | Behavior |
|--------|----------|
| `insert(id, value)` | Attach `T`. Returns the previous `T` if there was one. A dead id drops `value` and returns `None` |
| `get_component` / `get_component_mut` | `None` if the id is dead or has no `T` |
| `remove_component` | Take `T` off a live id |
| `contains_component` | Live id and a `T` present |
| `count_component::<T>` | How many live `T` values exist |
| `get_with_mut` | Object and one component together |
| `each_mut::<T>` | Every live object that has `T` |
| `each::<(A, B, …)>` | Every live object that has **all** of one to four types |

Iteration order is unspecified.

`iter` / `iter_mut` visit every live object, with or without components. Do not `spawn` or `despawn` while an `iter_mut` borrow is still held; collect ids first.

## World vs scene

| Use `World` when… | Use `Scene` when… |
|-------------------|-------------------|
| Needs an id, collision, parent, or components | Purely decorative this frame |
| Exists for many frames | Backdrop, score text, one-shot FX you rebuild yourself |

## Common pitfalls

- **Moving by writing `sprite.center`** — draw overwrites it from `draw_pose`. Write `pos`.
- **Parenting by assigning `object.parent`** — skips the cycle check and does not bake the pose.
- **Holding an id across `despawn`** — generation mismatch. Spawn returns a new id.
- **Expecting `each` in spawn order** — sparse-set swap-remove does not preserve it.
- **Despawning inside `each` the id you are borrowing** — record ids and despawn after the closure returns.

## Related docs

- [game_object.md](game_object.md) — fields on the struct
- [component.md](component.md) — sparse sets and `Query`
- [mod.md](mod.md) — re-exports
- [../scene.md](../scene.md) — immediate drawing
- [../app.md](../app.md) — when world sprites are copied into the scene
- [../anim.md](../anim.md) — clips you own, applied onto `sprite`
