//! Retained game objects, hierarchy, and optional type-keyed components.

mod component;
mod game_object;
#[cfg(feature = "physics")]
mod rigid;
mod velocity;
mod world;

pub use component::Query;
pub use game_object::{GameObject, GameObjectId};
#[cfg(feature = "physics")]
pub use rigid::{step_rigid_bodies, RigidBody};
pub use velocity::{integrate_velocities, Velocity};
pub use world::World;
