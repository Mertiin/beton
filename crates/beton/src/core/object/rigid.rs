//! Rigid body material and contact solver.
//!
//! Sprites rotate with [`Velocity::angular`], but contacts use the axis-aligned
//! bounds of each oriented box (stable resting). Hitboxes grow with tilt.

use crate::core::math::Vec3;

use super::game_object::GameObjectId;
use super::velocity::Velocity;
use super::world::World;

const SUBSTEP: f32 = 1.0 / 120.0;
const MAX_SUBSTEPS: u32 = 8;
const SOLVER_ITERS: u32 = 10;
const BAUMGARTE: f32 = 0.8;
const SLOP: f32 = 0.02;
const SLEEP_SPEED: f32 = 0.12;
const SLEEP_ANGULAR: f32 = 0.12;
const MAX_ANGULAR: f32 = 25.0;

/// Mass and contact material for rigid bodies.
///
/// Linear and angular speed live on [`Velocity`]. Attach both for a dynamic
/// body, or only this with [`RigidBody::fixed`] for a static collider. Call
/// [`step_rigid_bodies`] from your game `update`; the frame loop does not.
///
/// Collision uses the axis-aligned bounds of each box after rotation (so a
/// spinning sprite still rests stably; the hitbox grows when tilted).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RigidBody {
    /// Inverse mass. `0` means fixed (infinite mass).
    pub inv_mass: f32,
    /// Bounce factor. Solver clamps to `0..=1`. `0` is dead, `1` is elastic.
    pub restitution: f32,
    /// Tangential friction. Solver clamps to `>= 0`. `0` is ice.
    pub friction: f32,
}

impl Default for RigidBody {
    fn default() -> Self {
        Self::dynamic(1.0)
    }
}

impl RigidBody {
    /// Dynamic body with the given mass. Mass `<= 0` becomes fixed.
    pub fn dynamic(mass: f32) -> Self {
        if mass <= 0.0 {
            return Self::fixed();
        }
        Self {
            inv_mass: 1.0 / mass,
            restitution: 0.15,
            friction: 0.55,
        }
    }

    /// Fixed collider: infinite mass, no bounce preference beyond defaults.
    pub fn fixed() -> Self {
        Self {
            inv_mass: 0.0,
            restitution: 0.15,
            friction: 0.55,
        }
    }
}

struct BodySnap {
    id: GameObjectId,
    pos: Vec3,
    rotation: f32,
    size: [f32; 2],
    vel: Vec3,
    angular: f32,
    inv_mass: f32,
    inv_inertia: f32,
    restitution: f32,
    friction: f32,
    dynamic: bool,
    touched: bool,
}

struct Contact {
    nx: f32,
    ny: f32,
    penetration: f32,
    cx: f32,
    cy: f32,
}

/// Integrate gravity and resolve contacts for unparented [`RigidBody`]s.
///
/// Dynamic bodies need a [`Velocity`]. Fixed bodies (`inv_mass == 0`) act as
/// static colliders and do not move. Parenteds are skipped. Hit size is the
/// axis-aligned bounds of `size * scale` at the current `rotation`.
///
/// Split into substeps of `1/120` s, at most 8. `dt <= 0` is a no-op.
///
/// Objects stepped here should not also call [`super::velocity::integrate_velocities`].
pub fn step_rigid_bodies(world: &mut World, dt: f32, gravity: Vec3) {
    if dt <= 0.0 {
        return;
    }

    let mut ids = Vec::new();
    world.each_mut::<RigidBody>(|id, object, _body| {
        if object.parent.is_none() {
            ids.push(id);
        }
    });

    let mut snaps = Vec::with_capacity(ids.len());
    for id in ids {
        let Some(object) = world.get(id) else {
            continue;
        };
        let Some(body) = world.get_component::<RigidBody>(id) else {
            continue;
        };
        let dynamic = body.inv_mass > 0.0;
        if dynamic && world.get_component::<Velocity>(id).is_none() {
            continue;
        }
        let (vel, angular) = if dynamic {
            world
                .get_component::<Velocity>(id)
                .map(|v| (v.linear, v.angular))
                .unwrap_or((Vec3::ZERO, 0.0))
        } else {
            (Vec3::ZERO, 0.0)
        };
        let size = [
            object.size[0] * object.scale[0],
            object.size[1] * object.scale[1],
        ];
        snaps.push(BodySnap {
            id,
            pos: object.pos,
            rotation: object.rotation,
            size,
            vel,
            angular,
            inv_mass: body.inv_mass,
            inv_inertia: box_inv_inertia(body.inv_mass, size),
            restitution: body.restitution.clamp(0.0, 1.0),
            friction: body.friction.max(0.0),
            dynamic,
            touched: false,
        });
    }

    if snaps.is_empty() {
        return;
    }

    let mut remaining = dt.min(SUBSTEP * MAX_SUBSTEPS as f32);
    while remaining > 0.0 {
        let h = remaining.min(SUBSTEP);
        remaining -= h;
        substep(&mut snaps, h, gravity);
    }

    for snap in snaps {
        if let Some(object) = world.get_mut(snap.id) {
            object.pos = snap.pos;
            object.rotation = snap.rotation;
        }
        if snap.dynamic {
            if let Some(vel) = world.get_component_mut::<Velocity>(snap.id) {
                vel.linear = snap.vel;
                vel.angular = snap.angular;
            }
        }
    }
}

fn box_inv_inertia(inv_mass: f32, size: [f32; 2]) -> f32 {
    if inv_mass <= 0.0 {
        return 0.0;
    }
    let w2 = size[0] * size[0];
    let h2 = size[1] * size[1];
    let denom = w2 + h2;
    if denom <= 1e-8 {
        return 0.0;
    }
    // I = m (w² + h²) / 12  →  inv_I = 12 * inv_mass / (w² + h²)
    12.0 * inv_mass / denom
}

fn substep(snaps: &mut [BodySnap], h: f32, gravity: Vec3) {
    for body in snaps.iter_mut() {
        body.touched = false;
        if body.dynamic {
            body.vel += gravity * h;
            body.pos += body.vel * h;
            body.rotation += body.angular * h;
            body.angular = body.angular.clamp(-MAX_ANGULAR, MAX_ANGULAR);
        }
    }

    for _ in 0..SOLVER_ITERS {
        let n = snaps.len();
        for i in 0..n {
            for j in (i + 1)..n {
                resolve_pair(snaps, i, j);
            }
        }
    }

    for body in snaps.iter_mut() {
        if body.dynamic && body.touched {
            // Kill solver noise that otherwise pumps spin on resting contacts.
            body.angular *= 0.92;
            body.vel.x *= 0.98;
            body.vel.y *= 0.98;

            // Soft pull toward the nearest face-down pose so AABB-of-OBB leans
            // do not freeze at a tip against a neighbor. Only when already slow.
            if body.vel.length() < 1.5 && body.angular.abs() < 2.0 {
                let target = snap_rotation_to_axis(body.rotation);
                let err = shortest_angle(target - body.rotation);
                body.angular += err * 12.0;
            }
        }
        // Only sleep when faces are nearly axis-aligned. Sleeping a tipped box
        // freezes it on a corner and looks broken.
        if body.dynamic
            && body.touched
            && body.vel.length() < SLEEP_SPEED
            && body.angular.abs() < SLEEP_ANGULAR
            && faces_nearly_axis_aligned(body.rotation)
        {
            body.vel = Vec3::ZERO;
            body.angular = 0.0;
            body.rotation = snap_rotation_to_axis(body.rotation);
        }
    }
}

fn shortest_angle(mut radians: f32) -> f32 {
    let pi = std::f32::consts::PI;
    while radians > pi {
        radians -= 2.0 * pi;
    }
    while radians < -pi {
        radians += 2.0 * pi;
    }
    radians
}

/// True when a rectangle's faces are nearly parallel to the world axes.
fn faces_nearly_axis_aligned(rotation: f32) -> bool {
    // sin(2θ) is 0 at k·π/2 (faces flush with axes).
    (rotation * 2.0).sin().abs() < 0.2
}

fn snap_rotation_to_axis(rotation: f32) -> f32 {
    let quarter = std::f32::consts::FRAC_PI_2;
    (rotation / quarter).round() * quarter
}

fn resolve_pair(snaps: &mut [BodySnap], i: usize, j: usize) {
    let (left, right) = snaps.split_at_mut(j);
    let a = &mut left[i];
    let b = &mut right[0];

    let Some(contact) = aabb_contact(a, b) else {
        return;
    };

    let inv_sum = a.inv_mass + b.inv_mass;
    if inv_sum <= 0.0 {
        return;
    }

    a.touched = true;
    b.touched = true;

    let (nx, ny, penetration) = (contact.nx, contact.ny, contact.penetration);
    // Offsets from current centers (before Baumgarte). Moving first then
    // measuring r against the old contact point pumps angular energy.
    let rax = contact.cx - a.pos.x;
    let ray = contact.cy - a.pos.y;
    let rbx = contact.cx - b.pos.x;
    let rby = contact.cy - b.pos.y;

    // Linear relative velocity for the normal (matches linear-only kn).
    let vn = (b.vel.x - a.vel.x) * nx + (b.vel.y - a.vel.y) * ny;

    if vn < 0.0 {
        let kn = a.inv_mass + b.inv_mass;
        if kn > 1e-8 {
            let e = if vn > -1.0 {
                0.0
            } else {
                a.restitution.max(b.restitution)
            };
            let jn = -(1.0 + e) * vn / kn;
            apply_impulse(a, b, jn * nx, jn * ny, rax, ray, rbx, rby);

            // Friction uses contact velocity including spin.
            let avx = a.vel.x - a.angular * ray;
            let avy = a.vel.y + a.angular * rax;
            let bvx = b.vel.x - b.angular * rby;
            let bvy = b.vel.y + b.angular * rbx;
            let rvx = bvx - avx;
            let rvy = bvy - avy;
            let vt_x = rvx - (rvx * nx + rvy * ny) * nx;
            let vt_y = rvy - (rvx * nx + rvy * ny) * ny;
            let vt_len = (vt_x * vt_x + vt_y * vt_y).sqrt();
            if vt_len > 1e-8 {
                let tx = vt_x / vt_len;
                let ty = vt_y / vt_len;
                let kt = a.inv_mass + b.inv_mass;
                if kt > 1e-8 {
                    let mu = (a.friction * b.friction).sqrt();
                    let mut jt = vt_len / kt;
                    let max_f = jn.abs() * mu;
                    if jt > max_f {
                        jt = max_f;
                    }
                    apply_impulse(a, b, -jt * tx, -jt * ty, rax, ray, rbx, rby);
                }
            }
        }
    }

    let correction = (penetration - SLOP).max(0.0) * BAUMGARTE / inv_sum;
    a.pos.x -= nx * correction * a.inv_mass;
    a.pos.y -= ny * correction * a.inv_mass;
    b.pos.x += nx * correction * b.inv_mass;
    b.pos.y += ny * correction * b.inv_mass;
}

fn apply_impulse(
    a: &mut BodySnap,
    b: &mut BodySnap,
    jx: f32,
    jy: f32,
    rax: f32,
    ray: f32,
    rbx: f32,
    rby: f32,
) {
    a.vel.x -= jx * a.inv_mass;
    a.vel.y -= jy * a.inv_mass;
    b.vel.x += jx * b.inv_mass;
    b.vel.y += jy * b.inv_mass;
    const TORQUE_SCALE: f32 = 0.5;
    a.angular -= (rax * jy - ray * jx) * a.inv_inertia * TORQUE_SCALE;
    b.angular += (rbx * jy - rby * jx) * b.inv_inertia * TORQUE_SCALE;
}

/// AABB of each oriented box, then classic AABB contact. Normal from A toward B.
fn aabb_contact(a: &BodySnap, b: &BodySnap) -> Option<Contact> {
    let (aw, ah) = aabb_extents(a.size, a.rotation);
    let (bw, bh) = aabb_extents(b.size, b.rotation);
    let ox = overlap_1d(a.pos.x, aw, b.pos.x, bw);
    let oy = overlap_1d(a.pos.y, ah, b.pos.y, bh);
    if ox <= 0.0 || oy <= 0.0 {
        return None;
    }

    let (nx, ny, penetration) = if ox < oy {
        let nx = if a.pos.x < b.pos.x { 1.0 } else { -1.0 };
        (nx, 0.0, ox)
    } else {
        let ny = if a.pos.y < b.pos.y { 1.0 } else { -1.0 };
        (0.0, ny, oy)
    };

    let ax0 = a.pos.x - aw * 0.5;
    let ax1 = a.pos.x + aw * 0.5;
    let ay0 = a.pos.y - ah * 0.5;
    let ay1 = a.pos.y + ah * 0.5;
    let bx0 = b.pos.x - bw * 0.5;
    let bx1 = b.pos.x + bw * 0.5;
    let by0 = b.pos.y - bh * 0.5;
    let by1 = b.pos.y + bh * 0.5;
    let ox0 = ax0.max(bx0);
    let ox1 = ax1.min(bx1);
    let oy0 = ay0.max(by0);
    let oy1 = ay1.min(by1);

    Some(Contact {
        nx,
        ny,
        penetration,
        cx: (ox0 + ox1) * 0.5,
        cy: (oy0 + oy1) * 0.5,
    })
}

/// World-axis width/height of a box with local `size` spun by `rotation`.
fn aabb_extents(size: [f32; 2], rotation: f32) -> (f32, f32) {
    let c = rotation.cos().abs();
    let s = rotation.sin().abs();
    let w = size[0] * c + size[1] * s;
    let h = size[0] * s + size[1] * c;
    (w, h)
}

fn overlap_1d(center_a: f32, size_a: f32, center_b: f32, size_b: f32) -> f32 {
    let half_a = size_a * 0.5;
    let half_b = size_b * 0.5;
    (half_a + half_b) - (center_a - center_b).abs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::object::game_object::GameObject;
    use crate::core::object::velocity::{integrate_velocities, Velocity};

    fn box_at(pos: Vec3, size: [f32; 2]) -> GameObject {
        GameObject {
            pos,
            rotation: 0.0,
            scale: [1.0, 1.0],
            parent: None,
            size,
            sprite: None,
            tag: 0,
        }
    }

    #[test]
    fn dynamic_falls_under_gravity() {
        let mut world = World::new();
        let id = world.spawn(box_at(Vec3::new(0.0, 5.0, 0.0), [1.0, 1.0]));
        world.insert(id, Velocity::ZERO);
        world.insert(id, RigidBody::dynamic(1.0));
        step_rigid_bodies(&mut world, 0.1, Vec3::new(0.0, -32.0, 0.0));
        let vel = world.get_component::<Velocity>(id).unwrap();
        assert!(vel.linear.y < 0.0);
        assert!(world.get(id).unwrap().pos.y < 5.0);
    }

    #[test]
    fn fixed_ignores_gravity_and_stuck_velocity() {
        let mut world = World::new();
        let id = world.spawn(box_at(Vec3::new(0.0, 0.0, 0.0), [4.0, 1.0]));
        world.insert(id, Velocity::linear(Vec3::new(0.0, -100.0, 0.0)));
        world.insert(id, RigidBody::fixed());
        step_rigid_bodies(&mut world, 0.5, Vec3::new(0.0, -32.0, 0.0));
        assert_eq!(world.get(id).unwrap().pos, Vec3::new(0.0, 0.0, 0.0));
        assert_eq!(
            world.get_component::<Velocity>(id).unwrap().linear,
            Vec3::new(0.0, -100.0, 0.0)
        );
    }

    #[test]
    fn overlapping_dynamics_separate() {
        let mut world = World::new();
        let a = world.spawn(box_at(Vec3::new(0.0, 0.0, 0.0), [2.0, 2.0]));
        let b = world.spawn(box_at(Vec3::new(0.5, 0.0, 0.0), [2.0, 2.0]));
        world.insert(a, Velocity::ZERO);
        world.insert(b, Velocity::ZERO);
        world.insert(a, RigidBody::dynamic(1.0));
        world.insert(b, RigidBody::dynamic(1.0));
        step_rigid_bodies(&mut world, SUBSTEP, Vec3::ZERO);
        let pa = world.get(a).unwrap().pos;
        let pb = world.get(b).unwrap().pos;
        let ox = overlap_1d(pa.x, 2.0, pb.x, 2.0);
        assert!(ox < 0.15, "overlap left {ox}");
    }

    #[test]
    fn falling_box_lands_on_floor() {
        let mut world = World::new();
        let floor = world.spawn(box_at(Vec3::new(0.0, -1.0, 0.0), [20.0, 1.0]));
        world.insert(floor, RigidBody::fixed());

        let box_id = world.spawn(box_at(Vec3::new(0.0, 2.0, 0.0), [1.0, 1.0]));
        world.insert(box_id, Velocity::ZERO);
        world.insert(box_id, RigidBody::dynamic(1.0));

        for _ in 0..120 {
            step_rigid_bodies(&mut world, SUBSTEP, Vec3::new(0.0, -32.0, 0.0));
        }

        let pos = world.get(box_id).unwrap().pos;
        assert!(
            (pos.y - 0.0).abs() < 0.2,
            "expected near floor top, got y={}",
            pos.y
        );
        let oy = overlap_1d(pos.y, 1.0, -1.0, 1.0);
        assert!(oy <= SLOP + 0.05, "box still sunk into floor: oy={oy}");
    }

    #[test]
    fn offset_impact_spins_box() {
        let mut world = World::new();
        let floor = world.spawn(box_at(Vec3::new(0.0, -1.0, 0.0), [20.0, 1.0]));
        world.insert(floor, RigidBody::fixed());

        let box_id = world.spawn(box_at(Vec3::new(0.0, 1.0, 0.0), [2.0, 1.0]));
        world.insert(
            box_id,
            Velocity {
                linear: Vec3::new(6.0, -4.0, 0.0),
                angular: 0.0,
            },
        );
        world.insert(box_id, RigidBody::dynamic(1.0));

        let mut saw_spin = false;
        for _ in 0..60 {
            step_rigid_bodies(&mut world, SUBSTEP, Vec3::new(0.0, -32.0, 0.0));
            let vel = world.get_component::<Velocity>(box_id).unwrap();
            let rot = world.get(box_id).unwrap().rotation;
            if vel.angular.abs() > 0.05 || rot.abs() > 0.05 {
                saw_spin = true;
            }
        }
        assert!(saw_spin, "expected spin from offset contact at some point");
    }

    #[test]
    fn angular_integrates_into_rotation() {
        let mut world = World::new();
        let id = world.spawn(box_at(Vec3::new(0.0, 5.0, 0.0), [1.0, 1.0]));
        world.insert(
            id,
            Velocity {
                linear: Vec3::ZERO,
                angular: 2.0,
            },
        );
        world.insert(id, RigidBody::dynamic(1.0));
        for _ in 0..10 {
            step_rigid_bodies(&mut world, SUBSTEP, Vec3::ZERO);
        }
        let expected = 2.0 * SUBSTEP * 10.0;
        assert!((world.get(id).unwrap().rotation - expected).abs() < 1e-4);
        assert!((world.get_component::<Velocity>(id).unwrap().angular - 2.0).abs() < 1e-4);
    }

    #[test]
    fn rotated_box_collides_with_floor() {
        let mut world = World::new();
        let floor = world.spawn(box_at(Vec3::new(0.0, -1.0, 0.0), [20.0, 1.0]));
        world.insert(floor, RigidBody::fixed());

        let mut falling = box_at(Vec3::new(0.0, 1.5, 0.0), [1.0, 1.0]);
        falling.rotation = std::f32::consts::FRAC_PI_4;
        let box_id = world.spawn(falling);
        world.insert(box_id, Velocity::ZERO);
        world.insert(box_id, RigidBody::dynamic(1.0));

        for _ in 0..180 {
            step_rigid_bodies(&mut world, SUBSTEP, Vec3::new(0.0, -32.0, 0.0));
        }

        let obj = world.get(box_id).unwrap();
        // Diamond / square resting: center should sit above the floor, not through it.
        assert!(
            obj.pos.y > -0.2,
            "rotated box fell through floor: y={}",
            obj.pos.y
        );
        // Should not float high after settling.
        assert!(
            obj.pos.y < 1.2,
            "rotated box floating too high: y={}",
            obj.pos.y
        );
    }

    #[test]
    fn tipped_box_stays_above_floor() {
        let mut world = World::new();
        let floor = world.spawn(box_at(Vec3::new(0.0, -1.0, 0.0), [20.0, 1.0]));
        world.insert(floor, RigidBody::fixed());

        let mut falling = box_at(Vec3::new(0.0, 1.0, 0.0), [1.2, 1.2]);
        falling.rotation = 0.4;
        let box_id = world.spawn(falling);
        world.insert(
            box_id,
            Velocity {
                linear: Vec3::ZERO,
                angular: -1.0,
            },
        );
        world.insert(box_id, RigidBody::dynamic(1.0));

        for _ in 0..240 {
            step_rigid_bodies(&mut world, SUBSTEP, Vec3::new(0.0, -32.0, 0.0));
        }

        let obj = world.get(box_id).unwrap();
        assert!(
            obj.pos.y > -0.2,
            "tipped box fell through floor: y={}",
            obj.pos.y
        );
        assert!(
            obj.pos.y < 2.0,
            "tipped box floated away: y={}",
            obj.pos.y
        );
    }

    #[test]
    fn parented_body_is_not_moved() {
        let mut world = World::new();
        let parent = world.spawn(box_at(Vec3::ZERO, [1.0, 1.0]));
        let mut child = box_at(Vec3::new(0.0, 3.0, 0.0), [1.0, 1.0]);
        child.parent = Some(parent);
        let id = world.spawn(child);
        world.insert(id, Velocity::ZERO);
        world.insert(id, RigidBody::dynamic(1.0));
        step_rigid_bodies(&mut world, 0.5, Vec3::new(0.0, -32.0, 0.0));
        assert_eq!(world.get(id).unwrap().pos, Vec3::new(0.0, 3.0, 0.0));
        integrate_velocities(&mut world, 0.5);
        assert_eq!(world.get(id).unwrap().pos, Vec3::new(0.0, 3.0, 0.0));
    }
}
