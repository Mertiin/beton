//! A small game engine with a short public API.
//!
//! Depend on this crate from another package, implement [`Game`], and call [`run`].
//!
//! Keyboard codes come from `winit` ([`Key`]). This crate pins the `winit` major version.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod core;

pub use core::anim::{Animation, SpriteSheet};
pub use core::app::{run, Config, Context, Game};
pub use core::audio::{Audio, SoundHandle, SoundId, VoiceId};
pub use core::camera::{Camera, Projection};
pub use core::input::{Input, Key};
pub use core::math::{Mat4, Rng, Vec3};
pub use core::object::{
    integrate_velocities, step_rigid_bodies, GameObject, GameObjectId, Query, RigidBody, Velocity,
    World,
};
pub use core::render::{Gfx, TextureHandle, TextureId};
pub use core::scene::{Font, Scene, Sprite};
