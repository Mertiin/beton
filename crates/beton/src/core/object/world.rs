//! Storage for [`GameObject`]s with generational ids and optional components.

use crate::core::math::Vec3;

use super::component::{ComponentMap, Query};
use super::game_object::{GameObject, GameObjectId};
use super::rigid::PhysicsSim;

#[derive(Clone)]
pub(super) struct Slot {
    pub(super) generation: u32,
    pub(super) object: Option<GameObject>,
}

/// Storage for [`GameObject`]s with generational ids.
#[derive(Default)]
pub struct World {
    pub(super) slots: Vec<Slot>,
    free: Vec<u32>,
    pub(super) components: ComponentMap,
    /// Rapier simulation used by [`super::rigid::step_rigid_bodies`].
    pub(super) physics: PhysicsSim,
}

impl World {
    /// Empty world.
    pub fn new() -> Self {
        Self::default()
    }

    pub(super) fn slot_is_live(&self, index: u32) -> bool {
        self.slots
            .get(index as usize)
            .is_some_and(|s| s.object.is_some())
    }

    pub(super) fn slot_generation(&self, index: u32) -> u32 {
        self.slots[index as usize].generation
    }

    pub(super) fn object_mut(&mut self, index: u32) -> Option<&mut GameObject> {
        self.slots
            .get_mut(index as usize)
            .and_then(|s| s.object.as_mut())
    }

    /// Insert a game object and return its id.
    pub fn spawn(&mut self, object: GameObject) -> GameObjectId {
        if let Some(index) = self.free.pop() {
            let slot = &mut self.slots[index as usize];
            slot.object = Some(object);
            GameObjectId(index, slot.generation)
        } else {
            let index = self.slots.len() as u32;
            self.slots.push(Slot {
                generation: 0,
                object: Some(object),
            });
            GameObjectId(index, 0)
        }
    }

    /// Remove a game object and all of its descendants (Unity-style).
    pub fn despawn(&mut self, id: GameObjectId) {
        let kids: Vec<_> = self.children(id).collect();
        for kid in kids {
            self.despawn(kid);
        }
        self.despawn_one(id);
    }

    fn despawn_one(&mut self, id: GameObjectId) {
        let Some(slot) = self.slots.get_mut(id.0 as usize) else {
            return;
        };
        if slot.generation != id.1 || slot.object.is_none() {
            return;
        }
        slot.object = None;
        slot.generation = slot.generation.wrapping_add(1);
        self.free.push(id.0);
        self.components.clear_slot(id.0);
    }

    /// Borrow a game object if the id is live.
    pub fn get(&self, id: GameObjectId) -> Option<&GameObject> {
        let slot = self.slots.get(id.0 as usize)?;
        if slot.generation != id.1 {
            return None;
        }
        slot.object.as_ref()
    }

    /// Mutably borrow a game object if the id is live.
    pub fn get_mut(&mut self, id: GameObjectId) -> Option<&mut GameObject> {
        let slot = self.slots.get_mut(id.0 as usize)?;
        if slot.generation != id.1 {
            return None;
        }
        slot.object.as_mut()
    }

    /// Attach a component of type `T`. Returns the previous value if one existed.
    ///
    /// A dead id drops `value` and returns `None`.
    pub fn insert<T: 'static>(&mut self, id: GameObjectId, value: T) -> Option<T> {
        if self.get(id).is_none() {
            return None;
        }
        self.components.insert(id.0, value)
    }

    /// Borrow a component if the id is live and has `T`.
    pub fn get_component<T: 'static>(&self, id: GameObjectId) -> Option<&T> {
        if self.get(id).is_none() {
            return None;
        }
        self.components.get::<T>(id.0)
    }

    /// Mutably borrow a component if the id is live and has `T`.
    pub fn get_component_mut<T: 'static>(&mut self, id: GameObjectId) -> Option<&mut T> {
        if self.get(id).is_none() {
            return None;
        }
        self.components.get_mut::<T>(id.0)
    }

    /// Remove a component. Returns the value if it was present.
    pub fn remove_component<T: 'static>(&mut self, id: GameObjectId) -> Option<T> {
        if self.get(id).is_none() {
            return None;
        }
        self.components.remove::<T>(id.0)
    }

    /// Whether a live object has a component of type `T`.
    pub fn contains_component<T: 'static>(&self, id: GameObjectId) -> bool {
        self.get(id).is_some() && self.components.contains::<T>(id.0)
    }

    /// Number of live components of type `T` (for tests and debugging).
    pub fn count_component<T: 'static>(&self) -> usize {
        self.components.count::<T>()
    }

    /// Borrow the object and a component together.
    pub fn get_with_mut<T: 'static>(
        &mut self,
        id: GameObjectId,
    ) -> Option<(&mut GameObject, &mut T)> {
        let World {
            slots, components, ..
        } = self;
        let slot = slots.get_mut(id.0 as usize)?;
        if slot.generation != id.1 {
            return None;
        }
        let object = slot.object.as_mut()?;
        let component = components.get_mut::<T>(id.0)?;
        Some((object, component))
    }

    /// Visit every live object that has component `T`.
    ///
    /// Iteration order is unspecified.
    pub fn each_mut<T: 'static>(
        &mut self,
        mut f: impl FnMut(GameObjectId, &mut GameObject, &mut T),
    ) {
        self.each::<(T,)>(|id, object, (component,)| f(id, object, component));
    }

    /// Visit every live object that has all components in `Q`.
    ///
    /// `Q` is a tuple of one to four component types, for example `(Body, Collider)`.
    /// Iteration order is unspecified.
    pub fn each<Q: Query>(
        &mut self,
        f: impl for<'a> FnMut(GameObjectId, &'a mut GameObject, Q::Item<'a>),
    ) {
        Q::run(self, f);
    }

    /// Iterate live game objects.
    pub fn iter(&self) -> impl Iterator<Item = (GameObjectId, &GameObject)> {
        self.slots.iter().enumerate().filter_map(|(index, slot)| {
            slot.object
                .as_ref()
                .map(|object| (GameObjectId(index as u32, slot.generation), object))
        })
    }

    /// Mutably iterate live game objects.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (GameObjectId, &mut GameObject)> {
        self.slots
            .iter_mut()
            .enumerate()
            .filter_map(|(index, slot)| {
                let generation = slot.generation;
                slot.object
                    .as_mut()
                    .map(|object| (GameObjectId(index as u32, generation), object))
            })
    }

    /// Remove every game object, every component, and the rigid-body simulation.
    pub fn clear(&mut self) {
        self.slots.clear();
        self.free.clear();
        self.components.clear_all();
        self.physics = PhysicsSim::default();
    }

    /// Direct children of `id`.
    pub fn children(&self, id: GameObjectId) -> impl Iterator<Item = GameObjectId> + '_ {
        self.iter()
            .filter(move |(_, object)| object.parent == Some(id))
            .map(|(child, _)| child)
    }

    /// Set `child`'s parent. Rejects missing ids, self-parenting, and cycles.
    ///
    /// When `world_position_stays` is true (Unity default), rewrites the child's local
    /// pose so its **world** pose is unchanged. When false, the child's current
    /// `pos` / `rotation` / `scale` are kept as local values under the new parent.
    pub fn set_parent(
        &mut self,
        child: GameObjectId,
        parent: Option<GameObjectId>,
        world_position_stays: bool,
    ) -> bool {
        if self.get(child).is_none() {
            return false;
        }
        if let Some(p) = parent {
            if p == child || self.get(p).is_none() || self.would_cycle(child, p) {
                return false;
            }
        }

        if !world_position_stays {
            if let Some(object) = self.get_mut(child) {
                object.parent = parent;
                return true;
            }
            return false;
        }

        let Some(world_pos) = self.world_pos(child) else {
            return false;
        };
        let Some(world_rot) = self.world_rotation(child) else {
            return false;
        };
        let Some(world_scale) = self.world_scale(child) else {
            return false;
        };

        let (local_pos, local_rot, local_scale) = match parent {
            None => (world_pos, world_rot, world_scale),
            Some(p) => {
                let parent_pos = self.world_pos(p).unwrap();
                let parent_rot = self.world_rotation(p).unwrap();
                let parent_scale = self.world_scale(p).unwrap();
                let offset = world_pos - parent_pos;
                let (ux, uy) = rotate2(-parent_rot, offset.x, offset.y);
                let local_x = if parent_scale[0].abs() > 1e-8 {
                    ux / parent_scale[0]
                } else {
                    0.0
                };
                let local_y = if parent_scale[1].abs() > 1e-8 {
                    uy / parent_scale[1]
                } else {
                    0.0
                };
                let local_scale = [
                    if parent_scale[0].abs() > 1e-8 {
                        world_scale[0] / parent_scale[0]
                    } else {
                        world_scale[0]
                    },
                    if parent_scale[1].abs() > 1e-8 {
                        world_scale[1] / parent_scale[1]
                    } else {
                        world_scale[1]
                    },
                ];
                (
                    Vec3::new(local_x, local_y, offset.z),
                    world_rot - parent_rot,
                    local_scale,
                )
            }
        };

        if let Some(object) = self.get_mut(child) {
            object.parent = parent;
            object.pos = local_pos;
            object.rotation = local_rot;
            object.scale = local_scale;
            true
        } else {
            false
        }
    }

    fn would_cycle(&self, child: GameObjectId, parent: GameObjectId) -> bool {
        let mut walk = Some(parent);
        let mut guard = 0u32;
        while let Some(id) = walk {
            if id == child {
                return true;
            }
            walk = self.get(id).and_then(|o| o.parent);
            guard += 1;
            if guard > 64 {
                return true;
            }
        }
        false
    }

    /// Resolved world position.
    pub fn world_pos(&self, id: GameObjectId) -> Option<Vec3> {
        let object = self.get(id)?;
        match object.parent {
            None => Some(object.pos),
            Some(parent) => {
                let parent_pos = self.world_pos(parent)?;
                let parent_rot = self.world_rotation(parent)?;
                let parent_scale = self.world_scale(parent)?;
                let lx = object.pos.x * parent_scale[0];
                let ly = object.pos.y * parent_scale[1];
                let (rx, ry) = rotate2(parent_rot, lx, ly);
                Some(Vec3::new(
                    parent_pos.x + rx,
                    parent_pos.y + ry,
                    parent_pos.z + object.pos.z,
                ))
            }
        }
    }

    /// Resolved world rotation in radians.
    pub fn world_rotation(&self, id: GameObjectId) -> Option<f32> {
        let object = self.get(id)?;
        match object.parent {
            None => Some(object.rotation),
            Some(parent) => Some(self.world_rotation(parent)? + object.rotation),
        }
    }

    /// Resolved world XY scale.
    pub fn world_scale(&self, id: GameObjectId) -> Option<[f32; 2]> {
        let object = self.get(id)?;
        match object.parent {
            None => Some(object.scale),
            Some(parent) => {
                let ps = self.world_scale(parent)?;
                Some([ps[0] * object.scale[0], ps[1] * object.scale[1]])
            }
        }
    }

    /// Pose used when drawing: local fields when unparented, else full hierarchy resolve.
    pub fn draw_pose(&self, id: GameObjectId) -> Option<(Vec3, f32, [f32; 2])> {
        let object = self.get(id)?;
        if object.parent.is_none() {
            return Some((object.pos, object.rotation, object.scale));
        }
        Some((
            self.world_pos(id)?,
            self.world_rotation(id)?,
            self.world_scale(id)?,
        ))
    }

    /// World-space AABB overlap using each object's `size * world_scale`.
    pub fn overlaps(&self, a: GameObjectId, b: GameObjectId) -> bool {
        let (Some(pa), Some(sa), Some(oa)) =
            (self.world_pos(a), self.world_scale(a), self.get(a))
        else {
            return false;
        };
        let (Some(pb), Some(sb), Some(ob)) =
            (self.world_pos(b), self.world_scale(b), self.get(b))
        else {
            return false;
        };
        let size_a = [oa.size[0] * sa[0], oa.size[1] * sa[1]];
        let size_b = [ob.size[0] * sb[0], ob.size[1] * sb[1]];
        GameObject::aabb_overlaps(pa, size_a, pb, size_b)
    }
}

fn rotate2(radians: f32, x: f32, y: f32) -> (f32, f32) {
    let (sin, cos) = radians.sin_cos();
    (x * cos - y * sin, x * sin + y * cos)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::math::Vec3;
    use std::panic::{self, AssertUnwindSafe};

    fn bare(x: f32, y: f32, w: f32, h: f32) -> GameObject {
        GameObject {
            pos: Vec3::new(x, y, 0.0),
            rotation: 0.0,
            scale: [1.0, 1.0],
            parent: None,
            size: [w, h],
            sprite: None,
            tag: 0,
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    struct Body {
        vel: Vec3,
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    struct Collider {
        radius: f32,
    }

    #[test]
    fn overlaps_touching_and_separated() {
        let a = bare(0.0, 0.0, 2.0, 2.0);
        let touching = bare(2.0, 0.0, 2.0, 2.0);
        let separated = bare(3.0, 0.0, 2.0, 2.0);
        let overlapping = bare(1.0, 0.0, 2.0, 2.0);
        assert!(GameObject::aabb_overlaps(
            a.pos, a.size, overlapping.pos, overlapping.size
        ));
        assert!(!GameObject::aabb_overlaps(
            a.pos, a.size, separated.pos, separated.size
        ));
        assert!(!GameObject::aabb_overlaps(
            a.pos, a.size, touching.pos, touching.size
        ));
    }

    #[test]
    fn despawned_id_returns_none() {
        let mut world = World::new();
        let id = world.spawn(bare(0.0, 0.0, 1.0, 1.0));
        assert!(world.get(id).is_some());
        world.despawn(id);
        assert!(world.get(id).is_none());
        let id2 = world.spawn(bare(1.0, 1.0, 1.0, 1.0));
        assert!(world.get(id).is_none());
        assert!(world.get(id2).is_some());
        assert_ne!(id, id2);
    }

    #[test]
    fn child_follows_parent_translation() {
        let mut world = World::new();
        let parent = world.spawn(bare(2.0, 3.0, 1.0, 1.0));
        let child = world.spawn(bare(1.0, 0.0, 1.0, 1.0));
        assert!(world.set_parent(child, Some(parent), false));
        let pos = world.world_pos(child).unwrap();
        assert!((pos.x - 3.0).abs() < 1e-4);
        assert!((pos.y - 3.0).abs() < 1e-4);

        world.get_mut(parent).unwrap().pos.x = 5.0;
        let pos = world.world_pos(child).unwrap();
        assert!((pos.x - 6.0).abs() < 1e-4);
    }

    #[test]
    fn parent_rotation_orbits_child() {
        let mut world = World::new();
        let parent = world.spawn(bare(0.0, 0.0, 1.0, 1.0));
        let child = world.spawn(bare(2.0, 0.0, 1.0, 1.0));
        assert!(world.set_parent(child, Some(parent), false));
        world.get_mut(parent).unwrap().rotation = std::f32::consts::FRAC_PI_2;
        let pos = world.world_pos(child).unwrap();
        assert!(pos.x.abs() < 1e-4, "x={}", pos.x);
        assert!((pos.y - 2.0).abs() < 1e-4, "y={}", pos.y);
        let rot = world.world_rotation(child).unwrap();
        assert!((rot - std::f32::consts::FRAC_PI_2).abs() < 1e-4);
    }

    #[test]
    fn parent_scale_scales_offset_and_hit() {
        let mut world = World::new();
        let parent = world.spawn(bare(0.0, 0.0, 1.0, 1.0));
        world.get_mut(parent).unwrap().scale = [2.0, 2.0];
        let child = world.spawn(bare(1.0, 0.0, 1.0, 1.0));
        assert!(world.set_parent(child, Some(parent), false));
        let pos = world.world_pos(child).unwrap();
        assert!((pos.x - 2.0).abs() < 1e-4);
        assert!(world.world_scale(child).unwrap()[0] > 1.9);
    }

    #[test]
    fn reparent_bake_keeps_world_pose() {
        let mut world = World::new();
        let a = world.spawn(bare(0.0, 0.0, 1.0, 1.0));
        let b = world.spawn(bare(5.0, 0.0, 1.0, 1.0));
        let child = world.spawn(bare(1.0, 2.0, 1.0, 1.0));
        assert!(world.set_parent(child, Some(a), false));
        let before = world.world_pos(child).unwrap();
        assert!(world.set_parent(child, Some(b), true));
        let after = world.world_pos(child).unwrap();
        assert!((before.x - after.x).abs() < 1e-4);
        assert!((before.y - after.y).abs() < 1e-4);
    }

    #[test]
    fn cycle_rejected() {
        let mut world = World::new();
        let a = world.spawn(bare(0.0, 0.0, 1.0, 1.0));
        let b = world.spawn(bare(1.0, 0.0, 1.0, 1.0));
        assert!(world.set_parent(b, Some(a), false));
        assert!(!world.set_parent(a, Some(b), false));
        assert!(!world.set_parent(a, Some(a), false));
    }

    #[test]
    fn cascade_despawn() {
        let mut world = World::new();
        let parent = world.spawn(bare(0.0, 0.0, 1.0, 1.0));
        let child = world.spawn(bare(1.0, 0.0, 1.0, 1.0));
        let grand = world.spawn(bare(1.0, 0.0, 1.0, 1.0));
        assert!(world.set_parent(child, Some(parent), false));
        assert!(world.set_parent(grand, Some(child), false));
        world.despawn(parent);
        assert!(world.get(parent).is_none());
        assert!(world.get(child).is_none());
        assert!(world.get(grand).is_none());
    }

    #[test]
    fn draw_pose_fast_path_matches_local_for_roots() {
        let mut world = World::new();
        let id = world.spawn(bare(3.0, -2.0, 1.0, 1.0));
        world.get_mut(id).unwrap().rotation = 0.5;
        world.get_mut(id).unwrap().scale = [2.0, 3.0];
        let (pos, rot, scale) = world.draw_pose(id).unwrap();
        assert_eq!(pos, Vec3::new(3.0, -2.0, 0.0));
        assert!((rot - 0.5).abs() < 1e-6);
        assert_eq!(scale, [2.0, 3.0]);
    }

    #[test]
    fn draw_pose_matches_full_resolve_for_children() {
        let mut world = World::new();
        let parent = world.spawn(bare(1.0, 0.0, 1.0, 1.0));
        world.get_mut(parent).unwrap().rotation = 0.25;
        world.get_mut(parent).unwrap().scale = [2.0, 2.0];
        let child = world.spawn(bare(1.0, 0.0, 1.0, 1.0));
        assert!(world.set_parent(child, Some(parent), false));
        let via_draw = world.draw_pose(child).unwrap();
        let via_full = (
            world.world_pos(child).unwrap(),
            world.world_rotation(child).unwrap(),
            world.world_scale(child).unwrap(),
        );
        assert!((via_draw.0.x - via_full.0.x).abs() < 1e-5);
        assert!((via_draw.0.y - via_full.0.y).abs() < 1e-5);
        assert!((via_draw.1 - via_full.1).abs() < 1e-5);
        assert!((via_draw.2[0] - via_full.2[0]).abs() < 1e-5);
    }

    #[test]
    fn insert_two_types_and_replace() {
        let mut world = World::new();
        let id = world.spawn(bare(0.0, 0.0, 1.0, 1.0));
        assert!(world
            .insert(id, Body { vel: Vec3::X })
            .is_none());
        assert!(world
            .insert(id, Collider { radius: 0.5 })
            .is_none());
        let old = world.insert(id, Body { vel: Vec3::Y });
        assert_eq!(old, Some(Body { vel: Vec3::X }));
        assert_eq!(
            world.get_component::<Body>(id).unwrap().vel,
            Vec3::Y
        );
        assert_eq!(
            world.get_component::<Collider>(id).unwrap().radius,
            0.5
        );
    }

    #[test]
    fn despawn_drops_components_and_reuse_is_clean() {
        let mut world = World::new();
        let id = world.spawn(bare(0.0, 0.0, 1.0, 1.0));
        world.insert(id, Body { vel: Vec3::X });
        world.despawn(id);
        assert!(!world.contains_component::<Body>(id));
        let id2 = world.spawn(bare(1.0, 1.0, 1.0, 1.0));
        assert!(!world.contains_component::<Body>(id2));
        assert_eq!(world.count_component::<Body>(), 0);
    }

    #[test]
    fn cascade_despawn_drops_child_components() {
        let mut world = World::new();
        let parent = world.spawn(bare(0.0, 0.0, 1.0, 1.0));
        let child = world.spawn(bare(1.0, 0.0, 1.0, 1.0));
        assert!(world.set_parent(child, Some(parent), false));
        world.insert(child, Body { vel: Vec3::X });
        world.despawn(parent);
        assert_eq!(world.count_component::<Body>(), 0);
    }

    #[test]
    fn each_body_moves_pos() {
        let mut world = World::new();
        let id = world.spawn(bare(0.0, 0.0, 1.0, 1.0));
        world.insert(
            id,
            Body {
                vel: Vec3::new(1.0, 2.0, 0.0),
            },
        );
        world.each_mut::<Body>(|_id, object, body| {
            object.pos += body.vel * 0.5;
        });
        let pos = world.get(id).unwrap().pos;
        assert!((pos.x - 0.5).abs() < 1e-5);
        assert!((pos.y - 1.0).abs() < 1e-5);
    }

    #[test]
    fn each_tuple_visits_only_objects_with_both() {
        let mut world = World::new();
        let both = world.spawn(bare(0.0, 0.0, 1.0, 1.0));
        let only_body = world.spawn(bare(1.0, 0.0, 1.0, 1.0));
        world.insert(both, Body { vel: Vec3::ZERO });
        world.insert(both, Collider { radius: 1.0 });
        world.insert(only_body, Body { vel: Vec3::X });
        let mut seen = 0;
        world.each::<(Body, Collider)>(|id, _object, (_body, _col)| {
            assert_eq!(id, both);
            seen += 1;
        });
        assert_eq!(seen, 1);
    }

    #[test]
    fn remove_swap_keeps_remaining_reachable() {
        let mut world = World::new();
        let a = world.spawn(bare(0.0, 0.0, 1.0, 1.0));
        let b = world.spawn(bare(1.0, 0.0, 1.0, 1.0));
        let c = world.spawn(bare(2.0, 0.0, 1.0, 1.0));
        world.insert(a, Body { vel: Vec3::X });
        world.insert(b, Body { vel: Vec3::Y });
        world.insert(c, Body { vel: Vec3::Z });
        assert!(world.remove_component::<Body>(a).is_some());
        assert_eq!(world.get_component::<Body>(b).unwrap().vel, Vec3::Y);
        assert_eq!(world.get_component::<Body>(c).unwrap().vel, Vec3::Z);
        assert_eq!(world.count_component::<Body>(), 2);
    }

    #[test]
    fn each_restores_stores_after_panic() {
        let mut world = World::new();
        let id = world.spawn(bare(0.0, 0.0, 1.0, 1.0));
        world.insert(id, Body { vel: Vec3::X });
        let panicked = panic::catch_unwind(AssertUnwindSafe(|| {
            world.each_mut::<Body>(|_id, _object, _body| {
                panic!("boom");
            });
        }));
        assert!(panicked.is_err());
        assert!(world.contains_component::<Body>(id));
        assert_eq!(
            world.get_component::<Body>(id).unwrap().vel,
            Vec3::X
        );
    }
}
