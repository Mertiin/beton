//! Rigid body material and a Rapier cuboid step.
//!
//! Sprites rotate with the body. Contacts use an oriented box of `size * scale`,
//! not the axis-aligned bounds of that box.

use std::collections::HashSet;

use rapier2d::prelude::{
    ColliderBuilder, PhysicsWorld, RigidBodyBuilder, RigidBodyHandle, Rotation, Vector,
};

use crate::core::math::Vec3;

use super::game_object::GameObjectId;
use super::velocity::Velocity;
use super::world::World;

const SUBSTEP: f32 = 1.0 / 120.0;
const MAX_SUBSTEPS: u32 = 8;
const DIVERGE: f32 = 1.0e-5;

/// Rapier state owned by [`World`] so bodies survive from step to step.
///
/// Games never touch this. [`RigidBody`] is the component they insert;
/// [`SimBody`] is the private handle the stepper keeps beside it.
#[derive(Default)]
pub(super) struct PhysicsSim {
    rapier: PhysicsWorld,
}

/// Mass and contact material for rigid bodies.
///
/// Linear and angular speed live on [`Velocity`]. Attach both for a dynamic
/// body, or only this with [`RigidBody::fixed`] for a static collider. Call
/// [`step_rigid_bodies`] from your game `update`; the frame loop does not.
///
/// Collision is an oriented cuboid of `size * scale`.
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

/// Rapier handles for one [`RigidBody`]. The stepper inserts and removes this.
#[derive(Clone, Copy)]
struct SimBody {
    body: RigidBodyHandle,
    spec: BodySpec,
    synced_x: f32,
    synced_y: f32,
    synced_rot: f32,
    synced_vx: f32,
    synced_vy: f32,
    synced_ang: f32,
}

#[derive(Clone, Copy, PartialEq)]
struct BodySpec {
    hx: f32,
    hy: f32,
    inv_mass: f32,
    restitution: f32,
    friction: f32,
    dynamic: bool,
}

struct Participant {
    id: GameObjectId,
    spec: BodySpec,
    pos: Vec3,
    rotation: f32,
    lin: Vec3,
    ang: f32,
    dynamic: bool,
}

/// Integrate gravity and resolve contacts for unparented [`RigidBody`]s.
///
/// Dynamic bodies need a [`Velocity`]. Fixed bodies (`inv_mass == 0`) act as
/// static colliders and do not move. Parented objects are skipped. The hit
/// shape is an oriented cuboid of `size * scale`.
///
/// The step is split into slices of `1/120` s, at most 8. `dt <= 0` is a no-op.
/// Writes to `pos`, `rotation`, or [`Velocity`] since the previous step are
/// copied onto the Rapier body before it advances.
///
/// Objects stepped here should not also call [`super::velocity::integrate_velocities`].
pub fn step_rigid_bodies(world: &mut World, dt: f32, gravity: Vec3) {
    if dt <= 0.0 {
        return;
    }

    let participants = collect_participants(world);
    let live: HashSet<GameObjectId> = participants.iter().map(|p| p.id).collect();
    drop_orphan_links(world, &live);

    let mut sim = std::mem::take(&mut world.physics);
    remove_stale_bodies(&mut sim, &live);
    for participant in &participants {
        ensure_body(world, &mut sim, participant);
    }

    let mut z_state: Vec<(f32, f32)> = participants.iter().map(|p| (p.pos.z, p.lin.z)).collect();
    if participants.iter().any(|p| p.dynamic) {
        let mut remaining = dt.min(SUBSTEP * MAX_SUBSTEPS as f32);
        while remaining > 1.0e-8 {
            let h = remaining.min(SUBSTEP);
            remaining -= h;
            sim.rapier.integration_parameters.dt = h;
            sim.rapier.gravity = vec2(gravity.x, gravity.y);
            sim.rapier.step();
            for (i, participant) in participants.iter().enumerate() {
                if participant.dynamic {
                    z_state[i].1 += gravity.z * h;
                    z_state[i].0 += z_state[i].1 * h;
                }
            }
        }
    }

    for (i, participant) in participants.iter().enumerate() {
        write_back(world, &sim, participant, z_state[i].0, z_state[i].1);
    }
    world.physics = sim;
}

fn collect_participants(world: &mut World) -> Vec<Participant> {
    let mut ids = Vec::new();
    world.each_mut::<RigidBody>(|id, object, _body| {
        if object.parent.is_none() {
            ids.push(id);
        }
    });

    let mut participants = Vec::with_capacity(ids.len());
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
        let (lin, ang) = if dynamic {
            world
                .get_component::<Velocity>(id)
                .map(|vel| (vel.linear, vel.angular))
                .unwrap_or((Vec3::ZERO, 0.0))
        } else {
            (Vec3::ZERO, 0.0)
        };
        participants.push(Participant {
            id,
            spec: BodySpec {
                hx: half_extent(object.size[0] * object.scale[0]),
                hy: half_extent(object.size[1] * object.scale[1]),
                inv_mass: body.inv_mass,
                restitution: body.restitution.clamp(0.0, 1.0),
                friction: body.friction.max(0.0),
                dynamic,
            },
            pos: object.pos,
            rotation: object.rotation,
            lin,
            ang,
            dynamic,
        });
    }
    participants
}

fn half_extent(full: f32) -> f32 {
    (full.abs() * 0.5).max(1.0e-4)
}

fn drop_orphan_links(world: &mut World, live: &HashSet<GameObjectId>) {
    let mut ids = Vec::new();
    world.each_mut::<SimBody>(|id, _object, _link| ids.push(id));
    for id in ids {
        if !live.contains(&id) {
            world.remove_component::<SimBody>(id);
        }
    }
}

fn remove_stale_bodies(sim: &mut PhysicsSim, live: &HashSet<GameObjectId>) {
    let stale: Vec<_> = sim
        .rapier
        .bodies
        .iter()
        .filter_map(|(handle, body)| {
            if live.contains(&unpack_id(body.user_data)) {
                None
            } else {
                Some(handle)
            }
        })
        .collect();
    for handle in stale {
        sim.rapier.remove_body(handle);
    }
}

fn ensure_body(world: &mut World, sim: &mut PhysicsSim, participant: &Participant) {
    let current = world.get_component::<SimBody>(participant.id).copied();
    let rebuild = current.is_none_or(|link| {
        link.spec != participant.spec || sim.rapier.bodies.get(link.body).is_none()
    });
    if rebuild {
        if let Some(link) = current {
            sim.rapier.remove_body(link.body);
            world.remove_component::<SimBody>(participant.id);
        }
        let (body, _collider) = sim
            .rapier
            .insert(rigid_builder(participant), collider_builder(participant));
        world.insert(
            participant.id,
            SimBody {
                body,
                spec: participant.spec,
                synced_x: participant.pos.x,
                synced_y: participant.pos.y,
                synced_rot: participant.rotation,
                synced_vx: participant.lin.x,
                synced_vy: participant.lin.y,
                synced_ang: participant.ang,
            },
        );
        return;
    }

    let link = current.expect("existing link");
    push_changes(sim, &link, participant);
}

fn rigid_builder(participant: &Participant) -> RigidBodyBuilder {
    let builder = if participant.dynamic {
        RigidBodyBuilder::dynamic()
            .ccd_enabled(true)
            .linvel(vec2(participant.lin.x, participant.lin.y))
            .angvel(participant.ang)
    } else {
        RigidBodyBuilder::fixed()
    };
    builder
        .translation(vec2(participant.pos.x, participant.pos.y))
        .rotation(participant.rotation)
        .user_data(pack_id(participant.id))
}

fn collider_builder(participant: &Participant) -> ColliderBuilder {
    let mut collider = ColliderBuilder::cuboid(participant.spec.hx, participant.spec.hy)
        .friction(participant.spec.friction)
        .restitution(participant.spec.restitution);
    if participant.dynamic {
        let mass = 1.0 / participant.spec.inv_mass;
        let area = (participant.spec.hx * 2.0) * (participant.spec.hy * 2.0);
        collider = collider.density(mass / area.max(1.0e-8));
    }
    collider
}

fn push_changes(sim: &mut PhysicsSim, link: &SimBody, participant: &Participant) {
    let pose = diverged(link.synced_x, participant.pos.x)
        || diverged(link.synced_y, participant.pos.y)
        || diverged(link.synced_rot, participant.rotation);
    let vel = participant.dynamic
        && (diverged(link.synced_vx, participant.lin.x)
            || diverged(link.synced_vy, participant.lin.y)
            || diverged(link.synced_ang, participant.ang));
    if !pose && !vel {
        return;
    }
    let Some(body) = sim.rapier.bodies.get_mut(link.body) else {
        return;
    };
    if pose {
        body.set_translation(vec2(participant.pos.x, participant.pos.y), true);
        body.set_rotation(Rotation::new(participant.rotation), true);
    }
    if vel {
        body.set_linvel(vec2(participant.lin.x, participant.lin.y), true);
        body.set_angvel(participant.ang, true);
    }
}

fn write_back(world: &mut World, sim: &PhysicsSim, participant: &Participant, z: f32, vz: f32) {
    let Some(link) = world.get_component::<SimBody>(participant.id).copied() else {
        return;
    };
    let Some(body) = sim.rapier.bodies.get(link.body) else {
        return;
    };
    let x = body.translation().x;
    let y = body.translation().y;
    let rot = continue_angle(participant.rotation, body.rotation().angle());
    let (vx, vy, ang) = if participant.dynamic {
        let vel = body.linvel();
        (vel.x, vel.y, body.angvel())
    } else {
        (0.0, 0.0, 0.0)
    };

    if let Some(object) = world.get_mut(participant.id) {
        object.pos = Vec3::new(x, y, z);
        object.rotation = rot;
    }
    if participant.dynamic {
        if let Some(vel) = world.get_component_mut::<Velocity>(participant.id) {
            vel.linear = Vec3::new(vx, vy, vz);
            vel.angular = ang;
        }
    }
    if let Some(link) = world.get_component_mut::<SimBody>(participant.id) {
        link.synced_x = x;
        link.synced_y = y;
        link.synced_rot = rot;
        if participant.dynamic {
            link.synced_vx = vx;
            link.synced_vy = vy;
            link.synced_ang = ang;
        }
    }
}

fn vec2(x: f32, y: f32) -> Vector {
    Vector::new(x, y)
}

fn diverged(stored: f32, current: f32) -> bool {
    (stored - current).abs() > DIVERGE
}

fn continue_angle(previous: f32, next: f32) -> f32 {
    let pi = std::f32::consts::PI;
    let mut delta = next - previous;
    while delta > pi {
        delta -= 2.0 * pi;
    }
    while delta < -pi {
        delta += 2.0 * pi;
    }
    previous + delta
}

fn pack_id(id: GameObjectId) -> u128 {
    id.0 as u128 | ((id.1 as u128) << 32)
}

fn unpack_id(data: u128) -> GameObjectId {
    GameObjectId(data as u32, (data >> 32) as u32)
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

    fn overlap_1d(center_a: f32, size_a: f32, center_b: f32, size_b: f32) -> f32 {
        (size_a * 0.5 + size_b * 0.5) - (center_a - center_b).abs()
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
        for _ in 0..240 {
            step_rigid_bodies(&mut world, SUBSTEP, Vec3::ZERO);
        }
        let pa = world.get(a).unwrap().pos;
        let pb = world.get(b).unwrap().pos;
        let dx = pa.x - pb.x;
        let dy = pa.y - pb.y;
        let dist = (dx * dx + dy * dy).sqrt();
        assert!(dist > 1.8, "expected boxes to separate, got {pa:?} {pb:?}");
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
        assert!(oy <= 0.08, "box still sunk into floor: oy={oy}");
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
        assert!((world.get(id).unwrap().rotation - expected).abs() < 1e-3);
        assert!((world.get_component::<Velocity>(id).unwrap().angular - 2.0).abs() < 1e-3);
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
        assert!(
            obj.pos.y > -0.2,
            "rotated box fell through floor: y={}",
            obj.pos.y
        );
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
        assert!(obj.pos.y < 2.0, "tipped box floated away: y={}", obj.pos.y);
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

    #[test]
    fn external_pose_and_velocity_are_applied() {
        let mut world = World::new();
        let id = world.spawn(box_at(Vec3::ZERO, [1.0, 1.0]));
        world.insert(id, Velocity::ZERO);
        world.insert(id, RigidBody::dynamic(1.0));
        step_rigid_bodies(&mut world, SUBSTEP, Vec3::ZERO);

        world.get_mut(id).unwrap().pos.x = 4.0;
        world.get_component_mut::<Velocity>(id).unwrap().linear.x = 3.0;
        step_rigid_bodies(&mut world, SUBSTEP, Vec3::ZERO);

        let x = world.get(id).unwrap().pos.x;
        assert!(
            x > 4.01 && x < 4.1,
            "expected the written pose to move one substep, got x={x}"
        );
    }

    #[test]
    fn clear_drops_simulation() {
        let mut world = World::new();
        let id = world.spawn(box_at(Vec3::ZERO, [2.0, 2.0]));
        world.insert(id, RigidBody::fixed());
        step_rigid_bodies(&mut world, SUBSTEP, Vec3::ZERO);
        world.clear();

        let id = world.spawn(box_at(Vec3::ZERO, [1.0, 1.0]));
        world.insert(id, Velocity::linear(Vec3::new(10.0, 0.0, 0.0)));
        world.insert(id, RigidBody::dynamic(1.0));
        step_rigid_bodies(&mut world, 0.1, Vec3::ZERO);
        assert!(
            world.get(id).unwrap().pos.x > 0.5,
            "a cleared collider should not block the new body"
        );
    }
}
