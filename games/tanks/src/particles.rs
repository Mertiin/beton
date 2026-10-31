//! Thin helpers that emit into beton’s particle system.

use crate::weapons::Weapon;
use beton::{Particle, Particles, Rng, ShaderId, TextureId, Vec3};

pub fn emit_trail(particles: &mut Particles, white: TextureId, shader: ShaderId, pos: Vec3, color: [f32; 4]) {
    particles.emit(Particle::trail(white, shader, pos, color, 0.35, 4.0));
}

pub fn emit_blast(particles: &mut Particles, white: TextureId, weapon: &Weapon, pos: Vec3) {
    let end = weapon.blast_radius * 2.0;
    particles.emit(Particle::blast(
        white,
        weapon.shader,
        pos,
        [1.0, 0.75, 0.25, 1.0],
        0.35,
        end * 0.25,
        end,
    ));
}

pub fn emit_fragments(
    particles: &mut Particles,
    white: TextureId,
    weapon: &Weapon,
    pos: Vec3,
    rng: &mut Rng,
) {
    for _ in 0..weapon.fragments {
        let angle = rng.range(0.0, std::f32::consts::TAU);
        let speed = rng.range(weapon.fragment_speed * 0.4, weapon.fragment_speed);
        let vel = Vec3::new(angle.cos() * speed, angle.sin() * speed, 0.0);
        let size = rng.range(1.5, 3.5);
        particles.emit(Particle::fragment(
            white,
            pos,
            vel,
            [0.85, 0.7, 0.45, 1.0],
            weapon.fragment_life,
            size,
            -180.0,
        ));
    }
}
