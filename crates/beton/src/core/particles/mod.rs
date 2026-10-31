//! Short-lived tinted quads: trails, blasts, debris.

use crate::core::math::Vec3;
use crate::core::render::{ShaderId, TextureId};
use crate::core::scene::{Scene, Sprite};

/// Default cap on live particles. Emitting past this drops the oldest.
pub const MAX_PARTICLES: usize = 200;

/// One live particle. Emit into [`Particles`]; call [`step_particles`] from `update`.
#[derive(Clone, Debug)]
pub struct Particle {
    /// World-space center.
    pub pos: Vec3,
    /// Linear velocity in world units per second.
    pub vel: Vec3,
    /// Seconds remaining.
    pub life: f32,
    /// Initial life (for fade / size lerp).
    pub max_life: f32,
    /// Size at spawn.
    pub size: f32,
    /// Size at death (`life == 0`).
    pub end_size: f32,
    /// RGBA tint. Alpha is multiplied by `life / max_life`.
    pub color: [f32; 4],
    /// Texture to sample (often the 1×1 white).
    pub texture: TextureId,
    /// Pipeline. Defaults to the built-in sprite shader.
    pub shader: ShaderId,
    /// Gravity applied to `vel.y` each second (negative pulls down).
    pub gravity: f32,
    /// When true, [`step_particles`] kills the particle if `solid` returns true at `pos`.
    pub collide: bool,
}

impl Particle {
    /// Soft puff that fades out in place (trail / smoke).
    pub fn trail(
        texture: TextureId,
        shader: ShaderId,
        pos: Vec3,
        color: [f32; 4],
        life: f32,
        size: f32,
    ) -> Self {
        Self {
            pos,
            vel: Vec3::ZERO,
            life,
            max_life: life.max(0.001),
            size,
            end_size: size * 0.5,
            color,
            texture,
            shader,
            gravity: 0.0,
            collide: false,
        }
    }

    /// Expanding disc / ring that grows toward `end_size` while fading.
    pub fn blast(
        texture: TextureId,
        shader: ShaderId,
        pos: Vec3,
        color: [f32; 4],
        life: f32,
        start_size: f32,
        end_size: f32,
    ) -> Self {
        Self {
            pos,
            vel: Vec3::ZERO,
            life,
            max_life: life.max(0.001),
            size: start_size,
            end_size,
            color,
            texture,
            shader,
            gravity: 0.0,
            collide: false,
        }
    }

    /// Hard shard under gravity; dies on solid when `collide` is true.
    pub fn fragment(
        texture: TextureId,
        pos: Vec3,
        vel: Vec3,
        color: [f32; 4],
        life: f32,
        size: f32,
        gravity: f32,
    ) -> Self {
        Self {
            pos,
            vel,
            life,
            max_life: life.max(0.001),
            size,
            end_size: size,
            color,
            texture,
            shader: ShaderId::SPRITE,
            gravity,
            collide: true,
        }
    }
}

/// Immediate-mode particle list for one game. Cleared only when you call [`Particles::clear`].
#[derive(Clone, Debug)]
pub struct Particles {
    items: Vec<Particle>,
    max: usize,
}

impl Default for Particles {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            max: MAX_PARTICLES,
        }
    }
}

impl Particles {
    /// Empty list with the default cap.
    pub fn new() -> Self {
        Self::default()
    }

    /// How many particles are alive.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// True when nothing is alive.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Drop every particle.
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Spawn a particle. If over capacity, the oldest (lowest remaining life) is removed.
    pub fn emit(&mut self, particle: Particle) {
        if particle.life <= 0.0 {
            return;
        }
        if self.items.len() >= self.max {
            let mut oldest = 0usize;
            let mut oldest_life = f32::MAX;
            for (i, p) in self.items.iter().enumerate() {
                if p.life < oldest_life {
                    oldest_life = p.life;
                    oldest = i;
                }
            }
            self.items.swap_remove(oldest);
        }
        self.items.push(particle);
    }

    pub(crate) fn push_to_scene(&self, scene: &mut Scene) {
        for p in &self.items {
            let t = (p.life / p.max_life).clamp(0.0, 1.0);
            // t=1 at spawn → `size`; t=0 at death → `end_size`.
            let size = p.size + (p.end_size - p.size) * (1.0 - t);
            let mut sprite = Sprite::new(p.texture, p.pos, [size, size]);
            sprite.shader = p.shader;
            sprite.color = [p.color[0], p.color[1], p.color[2], p.color[3] * t];
            scene.push(sprite);
        }
    }
}

/// Integrate particles for `dt` seconds.
///
/// `solid` is called for particles with [`Particle::collide`]. Return true to kill them.
pub fn step_particles(particles: &mut Particles, dt: f32, solid: impl Fn(Vec3) -> bool) {
    let mut i = 0;
    while i < particles.items.len() {
        let p = &mut particles.items[i];
        p.life -= dt;
        if p.life <= 0.0 {
            particles.items.swap_remove(i);
            continue;
        }
        p.vel.y += p.gravity * dt;
        p.pos.x += p.vel.x * dt;
        p.pos.y += p.vel.y * dt;
        if p.collide && solid(p.pos) {
            particles.items.swap_remove(i);
            continue;
        }
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::render::TextureId;

    #[test]
    fn trail_dies_on_life() {
        let mut particles = Particles::new();
        particles.emit(Particle::trail(
            TextureId::from_raw(0),
            ShaderId::SPRITE,
            Vec3::ZERO,
            [1.0, 1.0, 1.0, 1.0],
            0.2,
            4.0,
        ));
        step_particles(&mut particles, 1.0, |_| false);
        assert!(particles.is_empty());
    }

    #[test]
    fn collide_kills_fragment() {
        let mut particles = Particles::new();
        particles.emit(Particle::fragment(
            TextureId::from_raw(0),
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::ZERO,
            [1.0, 1.0, 1.0, 1.0],
            1.0,
            2.0,
            0.0,
        ));
        step_particles(&mut particles, 0.01, |p| p.x > 0.5);
        assert!(particles.is_empty());
    }

    #[test]
    fn emit_caps_at_max() {
        let mut particles = Particles::new();
        for i in 0..MAX_PARTICLES + 10 {
            particles.emit(Particle::trail(
                TextureId::from_raw(0),
                ShaderId::SPRITE,
                Vec3::ZERO,
                [1.0, 1.0, 1.0, 1.0],
                1.0 + i as f32 * 0.01,
                2.0,
            ));
        }
        assert_eq!(particles.len(), MAX_PARTICLES);
    }
}
