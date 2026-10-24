# `src/core/math/rng.rs`

## Purpose

A tiny **deterministic** random number generator for gameplay (pipe heights, spawn jitter, shuffle bags). Same seed → same sequence forever.

This is **not** for cryptography, account tokens, or anything security-sensitive.

## Why deterministic?

Debugging random games is miserable when every run differs. With a fixed seed you can:

1. Reproduce a crash
2. Record “seed 42” in a bug report
3. Share challenge levels that always spawn the same patterns

Flappy uses `Rng::new(1)` so pipe layouts are stable across runs (until you change the call pattern).

## Algorithm

64-bit **xorshift**: scramble the state with xor and shifts, emit a new `u64` each time. Extremely small code, surprisingly good for games, not perfect statistically — fine for gaps and loot tables.

Seed `0` is replaced with a fixed non-zero constant so the generator cannot get stuck in an all-zero state.

## API

```rust
let mut rng = Rng::new(seed);
let y = rng.range(-4.0, 4.0); // half-open interval [lo, hi)
```

`range` maps the next random bits into a float uniformly (for practical purposes) between `lo` and `hi`.

### Half-open interval

`[lo, hi)` means `lo` can appear, `hi` cannot. That matches many language conventions (`Random.range` styles) and makes tiling ranges easier (`0..1`, `1..2`, …) without double-hitting boundaries.

## Patterns

### Own the rng on your game struct

```rust
struct Flappy { rng: Rng, /* … */ }
```

Pass `&mut self.rng` into spawn helpers.

### Reseed on new run (optional)

```rust
self.rng = Rng::new(player_chosen_seed);
```

Or derive from `std::time` if you *want* variety each launch — still store the seed so you can print it when the player dies.

### Random integer in a range

Not built-in; derive from floats or raw bits:

```rust
let i = rng.range(0.0, 5.0) as i32; // 0..4
```

Be aware of float edge cases if you need perfect uniformity over large integer ranges — for small gameplay ranges this is fine.

## Common pitfalls

- **Creating `Rng::new(1)` every call** — resets the sequence; pipes become identical every spawn. Keep one long-lived instance.
- **Using for secrets** — wrong tool.
- **Assuming `range(a, a)` works** — degenerate; keep `hi > lo`.

## Related docs

- Flappy’s pipe spawner in `games/flappy/src/main.rs` — practical usage  
