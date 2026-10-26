# `src/core/object/component.rs`

## Purpose

Storage and iteration for **components**: extra typed values hanging off a [`GameObject`](game_object.md), without adding fields to that struct.

Games do not construct anything in this file. They call `world.insert` / `world.each`. This page is how that stays fast and how `each` can borrow the object and several components at once.

## Beginner picture

A component is a sticky note of one Rust type (`Body`, `Collider`, …). The engine also ships a built-in motion note — [`Velocity`](velocity.md) — and, with the `physics` feature, [`RigidBody`](rigid.md). Storage here is the same for those and for your own types.

- An object may have at most **one** value of each type.
- Most objects will not have most types. A bullet does not pay for a `Health` slot it never uses.
- Notes are not inherited through `parent`. A child does not see the parent’s `Body`.

## Sparse set (`ComponentStore<T>`)

One store per component type. Three arrays:

| Array | Role |
|-------|------|
| `dense` | The `T` values, packed with no holes |
| `owners` | Object slot index for each dense entry (same order as `dense`) |
| `sparse` | Slot index → dense index, or `ABSENT` (`u32::MAX`) |

```text
object slots:  0    1    2    3
sparse:        1  ABSENT  0    2

dense:   [T of slot 2, T of slot 0, T of slot 3]
owners:  [2, 0, 3]
```

Lookup is “read sparse, then read dense.” Objects without `T` cost one `ABSENT` entry once the sparse vec has grown to their slot, not a full `T`.

`insert` replaces in place when the slot already has `T`, and returns the old value. `remove` uses **swap-remove**: the last dense entry moves into the hole, and that owner’s sparse index is patched. Order of `each` is whatever order `owners` is in after those swaps — do not depend on spawn order.

`despawn` calls `clear_slot` on every store so a reused object id does not inherit the previous actor’s notes.

## `ComponentMap`

A `HashMap<TypeId, Box<dyn ErasedStore>>`. The map is type-erased so `World` can hold many component types without a generic parameter. Each `get` / `insert` downcasts back to `ComponentStore<T>`.

`take_store` / `put_store` pull one typed store out of the map for the duration of a query. That is what lets the callback borrow the store’s values while `World` still mutably borrows the object. Rust would otherwise see two mutable borrows of `World`.

## `Query`

```rust
pub trait Query {
    type Item<'a>;
    fn run<F>(world: &mut World, f: F) where /* callback */;
}
```

Implemented for tuples of **one to four** `'static` types: `(A,)`, `(A, B)`, `(A, B, C)`, `(A, B, C, D)`.

`World::each::<(Body, Collider)>` calls `<(Body, Collider) as Query>::run`. `each_mut::<Body>` is the one-type case written as a tuple of one.

The walk:

1. Refuse the query if the same type is listed twice (`assert_unique`). `(Body, Body)` panics — two mutable borrows of one store are not representable.
2. `take_store` each type. If any type has never been inserted, the callback is not called.
3. Iterate the **smallest** store’s `owners` (fewer candidates).
4. Skip slots that are despawned or missing any of the types.
5. `remove` each value, hand out `&mut` through a `Restore` guard, call the closure, then the guard’s `Drop` inserts the value back.

Removing before the callback is what makes the mutable borrows legal. `Restore` puts the value back on every exit path, including when you `return` early inside the closure.

The whole walk sits in `catch_unwind`. If the closure panics, stores are `put_store`’d back onto the world and the panic continues. A panic does not leave `Body` permanently detached. The unit test `each_restores_stores_after_panic` in `world.rs` locks that in.

## What you write

```rust
world.each::<(Body, Collider)>(|id, object, (body, col)| {
    object.pos += body.vel * dt;
    let _ = (id, col.radius);
});
```

The closure receives the live id, the object, and one `&mut` per tuple member. Types the object does not have are simply not visited. Iteration order is unspecified.

Four is the maximum tuple width implemented here. A fifth component means a second pass, or a small struct that groups the data.

## Common pitfalls

- **Assuming spawn order** — swap-remove scrambles dense order.
- **Inserting the same type twice and expecting a list** — the second `insert` replaces the first and returns it.
- **Querying `(T, T)`** — panics. Borrow `T` once.
- **Despawning the object you are visiting** — `object` is borrowed for this call; despawn other ids, or record ids and despawn after `each` returns.
- **Expecting the child to have the parent’s component** — hierarchy is transforms only.

## Related docs

- [world.md](world.md) — `insert`, `each`, `despawn`
- [game_object.md](game_object.md) — the object those notes attach to
- [mod.md](mod.md) — `Query` is the only name re-exported from this file
