# `src/core/math/vec3.rs`

## Purpose

`Vec3` is a three-component floating-point vector. In this engine it represents:

- **Positions** (`object.pos`)
- **Directions / velocities** (game-owned speeds you apply to `GameObject::pos`)
- **Camera eye / target / up**

You do not need linear algebra fluency to use it — treat it as a named triple of numbers with helpful operations.

## Coordinate conventions

| Axis | Direction (default camera) |
|------|----------------------------|
| **+X** | Right |
| **+Y** | Up |
| **+Z** | Toward the camera (eye sits on +Z looking at the origin) |

For flat 2D games you mostly touch X and Y. Z still matters for **draw order** (see [scene.md](../scene.md)).

```
        +Y
         |
         |
         +----- +X
        /
      +Z (out of screen, toward default eye)
```

## Creating values

```rust
Vec3::ZERO;           // (0, 0, 0)
Vec3::X;              // (1, 0, 0)
Vec3::Y;              // (0, 1, 0)
Vec3::Z;              // (0, 0, 1)
Vec3::new(1.5, -2.0, 0.0);
```

Fields are public: `v.x`, `v.y`, `v.z`.

## Arithmetic

| Operation | Meaning |
|-----------|---------|
| `a + b` | Component-wise add (e.g. move a point by an offset) |
| `a - b` | Component-wise subtract (vector from b to a is `a - b`) |
| `a * 3.0` | Scale |
| `-a` | Flip direction |
| `+=`, `-=`, `*=` | In-place versions |

Typical game motion:

```rust
object.pos += velocity * dt;
```

(“move along velocity for `dt` seconds.”) The engine does not auto-integrate.

## Geometric helpers

### `dot(a, b)`

Sum of component products: `a.x*b.x + a.y*b.y + a.z*b.z`.

Intuition:

- If both are unit length, `dot` is `cos(angle)` between them
- `dot > 0` → same general hemisphere
- Used inside matrix look-at math

### `cross(a, b)`

A vector perpendicular to both `a` and `b` (right-hand rule). Length relates to the parallelogram area. Used to build camera basis axes in `Mat4::look_at_rh`. Rarely needed in 2D gameplay code.

### `length` / `normalize`

```rust
let speed = velocity.length();
let dir = velocity.normalize(); // unit length, or ZERO if length was 0
```

Normalize carefully: normalizing a near-zero vector returns `ZERO` here (no NaNs from divide-by-zero), but then you must not assume it is unit length.

## Gameplay patterns

### Move toward a point at fixed speed

```rust
let to = target - pos;
let dist = to.length();
if dist > 0.01 {
    pos += to.normalize() * SPEED * dt;
}
```

### Separate axes

```rust
pos.x += scroll * dt;
// gravity only on y
velocity.y += GRAVITY * dt;
```

### Keep Z for layering

```rust
pos.z = 0.0;   // gameplay
ui_origin.z = 1.0;
```

## Copy semantics

`Vec3` is `Copy`. Assignments duplicate the three floats — no borrow issues. That makes it pleasant in game loops.

## Common pitfalls

- **Confusing position and velocity** — both are `Vec3`, but units differ (units vs units/second).
- **Adding velocities without `dt`** — motion becomes frame-rate dependent.
- **Assuming screen pixels** — these are world units; see [camera.md](../camera.md).
- **Normalizing then multiplying by huge dt** — still fine; the issue is usually forgetting normalize when you wanted a direction.

## Related docs

- [mat4.md](mat4.md) — transforms built from vectors  
- [camera.md](../camera.md) — eye/target/up are `Vec3`s  
- [object/game_object.md](../object/game_object.md) — `pos` on game objects  
