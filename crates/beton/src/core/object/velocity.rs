//! Linear and angular velocity component, plus a plain integrator.

use crate::core::math::Vec3;

use super::rigid::RigidBody;
use super::world::World;

/// Velocity in world space: linear units/second, angular radians/second.
///
/// Attach with [`World::insert`]. Call [`integrate_velocities`] for objects that
/// have this and no [`RigidBody`](super::rigid::RigidBody). Objects with both
/// are stepped by [`super::rigid::step_rigid_bodies`] instead.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Velocity {
    /// Linear velocity (units / second).
    pub linear: Vec3,
    /// Angular velocity around Z (radians / second). Positive tips a right-facing edge up.
    pub angular: f32,
}

impl Default for Velocity {
    fn default() -> Self {
        Self::ZERO
    }
}

impl Velocity {
    /// Zero linear and angular velocity.
    pub const ZERO: Self = Self {
        linear: Vec3::ZERO,
        angular: 0.0,
    };

    /// Linear only; angular is zero.
    pub const fn linear(linear: Vec3) -> Self {
        Self {
            linear,
            angular: 0.0,
        }
    }
}

/// Write `pos += linear * dt` and `rotation += angular * dt` for unparented
/// objects that have [`Velocity`] and do not have a [`RigidBody`].
///
/// Skips parented objects and anything with a `RigidBody` (dynamic bodies are
/// owned by the rigid step; fixed bodies ignore a stuck `Velocity`).
///
/// `dt <= 0` is a no-op.
pub fn integrate_velocities(world: &mut World, dt: f32) {
    if dt <= 0.0 {
        return;
    }

    let mut ids = Vec::new();
    world.each_mut::<Velocity>(|id, object, _vel| {
        if object.parent.is_none() {
            ids.push(id);
        }
    });

    for id in ids {
        if world.contains_component::<RigidBody>(id) {
            continue;
        }
        if let Some((object, vel)) = world.get_with_mut::<Velocity>(id) {
            object.pos += vel.linear * dt;
            object.rotation += vel.angular * dt;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::object::game_object::GameObject;
    use crate::core::object::rigid::RigidBody;

    fn bare(pos: Vec3) -> GameObject {
        GameObject {
            pos,
            rotation: 0.0,
            scale: [1.0, 1.0],
            parent: None,
            size: [1.0, 1.0],
            sprite: None,
            tag: 0,
        }
    }

    #[test]
    fn integrate_moves_lone_velocity() {
        let mut world = World::new();
        let id = world.spawn(bare(Vec3::ZERO));
        world.insert(id, Velocity::linear(Vec3::new(2.0, 0.0, 0.0)));
        integrate_velocities(&mut world, 0.5);
        assert!((world.get(id).unwrap().pos.x - 1.0).abs() < 1e-5);
    }

    #[test]
    fn integrate_applies_angular() {
        let mut world = World::new();
        let id = world.spawn(bare(Vec3::ZERO));
        world.insert(
            id,
            Velocity {
                linear: Vec3::ZERO,
                angular: 2.0,
            },
        );
        integrate_velocities(&mut world, 0.5);
        assert!((world.get(id).unwrap().rotation - 1.0).abs() < 1e-5);
    }

    #[test]
    fn integrate_skips_dynamic_rigid_body() {
        let mut world = World::new();
        let id = world.spawn(bare(Vec3::ZERO));
        world.insert(id, Velocity::linear(Vec3::new(10.0, 0.0, 0.0)));
        world.insert(id, RigidBody::dynamic(1.0));
        integrate_velocities(&mut world, 1.0);
        assert_eq!(world.get(id).unwrap().pos, Vec3::ZERO);
    }

    #[test]
    fn integrate_skips_fixed_body_with_stuck_velocity() {
        let mut world = World::new();
        let id = world.spawn(bare(Vec3::ZERO));
        world.insert(id, Velocity::linear(Vec3::new(10.0, 0.0, 0.0)));
        world.insert(id, RigidBody::fixed());
        integrate_velocities(&mut world, 1.0);
        assert_eq!(world.get(id).unwrap().pos, Vec3::ZERO);
    }

    #[test]
    fn integrate_skips_parented() {
        let mut world = World::new();
        let parent = world.spawn(bare(Vec3::ZERO));
        let mut child = bare(Vec3::ZERO);
        child.parent = Some(parent);
        let id = world.spawn(child);
        world.insert(id, Velocity::linear(Vec3::new(10.0, 0.0, 0.0)));
        integrate_velocities(&mut world, 1.0);
        assert_eq!(world.get(id).unwrap().pos, Vec3::ZERO);
    }
}
