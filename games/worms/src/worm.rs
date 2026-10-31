//! Upright worm body. Walks, jumps, and falls through holes in the solid mask.

use crate::terrain::{Terrain, MAP_W};
use beton::Vec3;

pub const WORM_W: f32 = 6.0;
pub const WORM_H: f32 = 10.0;
pub const GRAVITY: f32 = 320.0;
pub const WALK_SPEED: f32 = 36.0;
pub const JUMP_SPEED: f32 = 115.0;
pub const MAX_FALL: f32 = 220.0;
/// Impact speed above this hurts. A normal jump lands slower.
pub const FALL_HURT: f32 = 160.0;

#[derive(Clone, Debug)]
pub struct Body {
    pub pos: Vec3,
    pub vel: Vec3,
    pub grounded: bool,
    pub facing: f32,
    pub travel: f32,
    pub hp: f32,
    pub alive: bool,
    pub drowning: bool,
}

impl Body {
    pub fn at(x: f32, y: f32) -> Self {
        Self {
            pos: Vec3::new(x, y, 0.0),
            vel: Vec3::ZERO,
            grounded: true,
            facing: 1.0,
            travel: 0.0,
            hp: 100.0,
            alive: true,
            drowning: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct StepOut {
    pub fall_damage: f32,
    pub drowned: bool,
}

pub fn step_body(
    body: &mut Body,
    terrain: &Terrain,
    water_y: f32,
    wish_x: f32,
    jump: bool,
    allow_control: bool,
    dt: f32,
) -> StepOut {
    let mut out = StepOut::default();
    if !body.alive && body.grounded && !body.drowning {
        body.vel = Vec3::ZERO;
        return out;
    }

    let can = allow_control && body.alive && !body.drowning;
    if can && body.grounded && jump {
        body.vel.y = JUMP_SPEED;
        body.grounded = false;
    }
    if can && wish_x != 0.0 && body.grounded {
        let dir = wish_x.signum();
        body.facing = dir;
        let dx = dir * WALK_SPEED * dt;
        let nx = body.pos.x + dx;
        body.vel.x = dir * WALK_SPEED;
        if !hits(terrain, nx, body.pos.y) {
            body.pos.x = nx;
            body.travel += dx.abs();
        } else if !hits(terrain, nx, body.pos.y + 2.0) {
            body.pos.x = nx;
            body.pos.y += 2.0;
            body.travel += dx.abs();
        }
    } else if body.grounded {
        body.vel.x = 0.0;
    }

    if body.grounded {
        body.vel.y = 0.0;
    } else {
        body.vel.y = (body.vel.y - GRAVITY * dt).max(-MAX_FALL);
    }
    let prev_vy = body.vel.y;
    body.pos.y += body.vel.y * dt;

    if hits(terrain, body.pos.x, body.pos.y) {
        if prev_vy <= 0.0 {
            let mut n = 0;
            while hits(terrain, body.pos.x, body.pos.y) && n < 24 {
                body.pos.y += 0.5;
                n += 1;
            }
            let impact = -prev_vy;
            body.vel.y = 0.0;
            body.grounded = true;
            if impact > FALL_HURT && body.alive {
                let dmg = (impact - FALL_HURT) * 0.35;
                body.hp = (body.hp - dmg).max(0.0);
                out.fall_damage = dmg;
                if body.hp <= 0.0 {
                    body.alive = false;
                }
            }
        } else {
            body.pos.y -= body.vel.y * dt;
            body.vel.y = 0.0;
        }
    } else {
        body.grounded = feet_on_ground(terrain, body.pos.x, body.pos.y);
        if body.grounded {
            body.vel.y = 0.0;
        }
    }

    body.pos.x = body.pos.x.clamp(4.0, MAP_W as f32 - 4.0);

    if body.alive && (body.pos.y < water_y || body.pos.y < -WORM_H) {
        body.drowning = true;
        body.alive = false;
        body.hp = 0.0;
        body.grounded = false;
        body.vel.y = -40.0;
        out.drowned = true;
    } else if body.drowning {
        body.vel.y = -40.0;
        body.grounded = false;
    }
    if body.hp <= 0.0 {
        body.alive = false;
    }
    out
}

fn hits(terrain: &Terrain, x: f32, y: f32) -> bool {
    let left = x - WORM_W * 0.5 + 0.2;
    let bottom = y - WORM_H * 0.5 + 0.2;
    let mut oy = 0.0;
    while oy < WORM_H - 0.4 {
        let mut ox = 0.0;
        while ox < WORM_W - 0.4 {
            let (c, r) = Terrain::world_to_cell(left + ox, bottom + oy);
            if terrain.solid_at(c, r) {
                return true;
            }
            ox += 1.5;
        }
        oy += 1.5;
    }
    false
}

pub fn feet_on_ground(terrain: &Terrain, x: f32, y: f32) -> bool {
    let feet = y - WORM_H * 0.5 - 0.8;
    let hw = WORM_W * 0.3;
    for ox in [-hw, 0.0, hw] {
        let (c, r) = Terrain::world_to_cell(x + ox, feet);
        if terrain.solid_at(c, r) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn floor_at(y: f32) -> Terrain {
        let mut t = Terrain::empty();
        t.fill_below(y);
        t
    }

    #[test]
    fn walk_onto_a_ledge_and_fall() {
        let mut t = floor_at(40.0);
        t.clear_rect(120.0, 0.0, 220.0, 80.0);
        let mut body = Body::at(100.0, 45.0);
        assert!(feet_on_ground(&t, body.pos.x, body.pos.y));
        for _ in 0..180 {
            step_body(&mut body, &t, 0.0, 1.0, false, true, 1.0 / 60.0);
        }
        assert!(
            body.pos.y < 40.0,
            "expected a fall, y={}",
            body.pos.y
        );
        assert!(!body.grounded || body.pos.y < 30.0);
    }

    #[test]
    fn jump_arc_leaves_the_ground_and_returns() {
        let t = floor_at(40.0);
        let mut body = Body::at(80.0, 45.0);
        let start = body.pos.y;
        step_body(&mut body, &t, 0.0, 0.0, true, true, 1.0 / 60.0);
        assert!(body.vel.y > 0.0 || body.pos.y > start);
        let mut peak = body.pos.y;
        for _ in 0..90 {
            step_body(&mut body, &t, 0.0, 0.0, false, true, 1.0 / 60.0);
            peak = peak.max(body.pos.y);
        }
        assert!(peak > start + 4.0, "peak {peak} start {start}");
        assert!(body.grounded, "should land");
        assert!(body.hp > 99.0, "a hop should not hurt");
    }

    #[test]
    fn a_long_fall_costs_hp() {
        let t = floor_at(40.0);
        let mut body = Body::at(80.0, 220.0);
        body.grounded = false;
        for _ in 0..300 {
            step_body(&mut body, &t, 0.0, 0.0, false, true, 1.0 / 60.0);
        }
        assert!(body.grounded, "should reach the floor");
        assert!(body.hp < 90.0, "hp {}", body.hp);
    }

    #[test]
    fn drown_when_center_crosses_water() {
        let t = Terrain::empty();
        let mut body = Body::at(40.0, 20.0);
        let out = step_body(&mut body, &t, 30.0, 0.0, false, true, 1.0 / 60.0);
        assert!(out.drowned);
        assert_eq!(body.hp, 0.0);
        assert!(!body.alive);
    }
}
