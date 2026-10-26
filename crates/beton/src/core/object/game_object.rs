//! Game object identity and the built-in transform fields.

use crate::core::math::Vec3;
use crate::core::scene::Sprite;

/// Generational handle. A stale id returns `None` from [`super::World`] lookups.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GameObjectId(pub(crate) u32, pub(crate) u32);

/// A game object with a local transform, optional sprite, and a game-defined tag.
///
/// The frame loop does not integrate motion. Write [`GameObject::pos`],
/// [`GameObject::rotation`], and [`GameObject::scale`] yourself, or attach
/// [`super::Velocity`] and call [`super::integrate_velocities`]. The `physics`
/// feature adds a rigid-body step for the same objects.
///
/// Extra data (health, AI, …) is attached with [`super::World::insert`], not
/// as fields on this struct.
#[derive(Clone, Debug)]
pub struct GameObject {
    /// Local position relative to [`GameObject::parent`] (world if no parent).
    pub pos: Vec3,
    /// Local rotation in radians.
    pub rotation: f32,
    /// Local XY scale. Default `[1.0, 1.0]`.
    pub scale: [f32; 2],
    /// Parent in the transform hierarchy.
    pub parent: Option<GameObjectId>,
    /// Unscaled width and height. With the `physics` feature, `size * scale` is an oriented cuboid.
    pub size: [f32; 2],
    /// Drawn at the resolved world pose each frame when `Some`.
    pub sprite: Option<Sprite>,
    /// Game-defined kind.
    pub tag: u32,
}

impl GameObject {
    /// Axis-aligned overlap of two boxes given centers and sizes (XY only).
    pub fn aabb_overlaps(center_a: Vec3, size_a: [f32; 2], center_b: Vec3, size_b: [f32; 2]) -> bool {
        let ax = size_a[0] * 0.5;
        let ay = size_a[1] * 0.5;
        let bx = size_b[0] * 0.5;
        let by = size_b[1] * 0.5;
        (center_a.x - ax) < (center_b.x + bx)
            && (center_a.x + ax) > (center_b.x - bx)
            && (center_a.y - ay) < (center_b.y + by)
            && (center_a.y + ay) > (center_b.y - by)
    }
}
