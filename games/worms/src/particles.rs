//! Blast ring, white impact spheres, and the shell's orange sparks.

use beton::{Particle, Particles, ShaderId, TextureId, Vec3};

pub fn emit_hit(
    particles: &mut Particles,
    white: TextureId,
    ring: ShaderId,
    pos: Vec3,
    radius: f32,
    fragments: u32,
    fragment_speed: f32,
    fragment_life: f32,
) {
    let end = (radius.max(6.0)) * 2.0;
    particles.emit(Particle::blast(
        white,
        ring,
        pos,
        [1.0, 1.0, 1.0, 0.9],
        0.4,
        end * 0.2,
        end,
    ));
    let n = fragments.clamp(1, 16);
    let speed = fragment_speed.max(8.0);
    for i in 0..n {
        let angle = i as f32 * (6.28318 / n as f32);
        let vel = Vec3::new(angle.cos() * speed * 0.35, angle.sin().abs() * speed * 0.45 + 10.0, 0.0);
        particles.emit(Particle::fragment(
            white,
            pos,
            vel,
            [0.95, 0.95, 1.0, 1.0],
            fragment_life.max(0.15),
            3.2,
            -30.0,
        ));
    }
}

pub fn emit_spark(particles: &mut Particles, white: TextureId, pos: Vec3) {
    particles.emit(Particle::trail(
        white,
        ShaderId::SPRITE,
        pos,
        [1.0, 0.55, 0.15, 1.0],
        0.3,
        2.8,
    ));
}

pub fn emit_leaf(particles: &mut Particles, white: TextureId, pos: Vec3) {
    particles.emit(Particle::fragment(
        white,
        pos,
        Vec3::new(8.0, -6.0, 0.0),
        [0.35, 0.75, 0.28, 0.9],
        2.5,
        1.6,
        4.0,
    ));
}
