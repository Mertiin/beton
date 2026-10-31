//! Tank match state: fuel, aim, shell flight, damage, turns.

use crate::terrain::{Terrain, MAP_H, MAP_W};
use crate::weapons::Weapon;
use beton::{GameObjectId, Vec3};

pub const TANK_W: f32 = 14.0;
pub const TANK_H: f32 = 10.0;
pub const FUEL_BUDGET: f32 = 135.0;
pub const MAX_HP: f32 = 100.0;
pub const MOVE_SPEED: f32 = 55.0;
pub const AIM_SPEED: f32 = 1.6;
pub const CHARGE_TIME: f32 = 0.85;
pub const GRAVITY: f32 = 140.0;
pub const SETTLE_SPEED: f32 = 120.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurnPhase {
    Aim,
    Flying,
    Settle,
    Over { winner: usize },
}

#[derive(Clone, Debug)]
pub struct Tank {
    pub id: GameObjectId,
    pub hp: f32,
    pub fuel: f32,
    pub aim: f32,
    pub facing: f32,
}

#[derive(Clone, Debug)]
pub struct Shell {
    pub vel: Vec3,
    pub weapon_index: usize,
    pub trail_acc: f32,
}

#[derive(Clone, Debug)]
pub struct MatchState {
    pub tanks: [Tank; 2],
    pub active: usize,
    pub phase: TurnPhase,
    pub charge: f32,
    pub charging: bool,
    pub weapon_index: usize,
    pub shell_id: Option<GameObjectId>,
}

impl MatchState {
    pub fn spawn_tanks(_terrain: &Terrain, left: GameObjectId, right: GameObjectId) -> Self {
        Self {
            tanks: [
                Tank {
                    id: left,
                    hp: MAX_HP,
                    fuel: FUEL_BUDGET,
                    aim: 0.55,
                    facing: 1.0,
                },
                Tank {
                    id: right,
                    hp: MAX_HP,
                    fuel: FUEL_BUDGET,
                    aim: std::f32::consts::PI - 0.55,
                    facing: -1.0,
                },
            ],
            active: 0,
            phase: TurnPhase::Aim,
            charge: 0.0,
            charging: false,
            weapon_index: 0,
            shell_id: None,
        }
    }

    pub fn tank_spawn_y(terrain: &Terrain, side: usize) -> (f32, f32) {
        let x = if side == 0 {
            MAP_W as f32 * 0.18
        } else {
            MAP_W as f32 * 0.82
        };
        let y = terrain.surface_y(x as i32).unwrap_or(40.0) + TANK_H * 0.5;
        (x, y)
    }

    pub fn begin_turn(&mut self) {
        self.phase = TurnPhase::Aim;
        self.charge = 0.0;
        self.charging = false;
        self.tanks[self.active].fuel = FUEL_BUDGET;
        self.shell_id = None;
    }

    pub fn next_player(&mut self) {
        self.active = 1 - self.active;
        self.begin_turn();
    }

    pub fn try_move(
        &mut self,
        terrain: &Terrain,
        positions: &mut [Vec3; 2],
        dir: f32,
        dt: f32,
    ) -> bool {
        if self.phase != TurnPhase::Aim {
            return false;
        }
        let tank = &mut self.tanks[self.active];
        if tank.fuel <= 0.0 || dir == 0.0 {
            return false;
        }
        tank.facing = dir.signum();
        let step = dir.signum() * MOVE_SPEED * dt;
        let want = step.abs().min(tank.fuel) * dir.signum();
        let mut pos = positions[self.active];
        let next_x = (pos.x + want).clamp(TANK_W * 0.5, MAP_W as f32 - TANK_W * 0.5);
        let traveled = (next_x - pos.x).abs();
        if traveled < 0.001 {
            return false;
        }
        // Block if the destination footprint overlaps solid dirt above the feet.
        if tank_blocked(terrain, next_x, pos.y) {
            return false;
        }
        pos.x = next_x;
        if let Some(surface) = terrain.surface_y(pos.x as i32) {
            pos.y = surface + TANK_H * 0.5;
        }
        tank.fuel = (tank.fuel - traveled).max(0.0);
        positions[self.active] = pos;
        true
    }

    pub fn aim(&mut self, delta: f32) {
        if self.phase != TurnPhase::Aim {
            return;
        }
        let tank = &mut self.tanks[self.active];
        tank.aim = (tank.aim + delta).clamp(0.05, std::f32::consts::PI - 0.05);
    }

    pub fn fire(&mut self, weapons: &[Weapon], muzzle: Vec3) -> Option<(Vec3, Shell)> {
        if self.phase != TurnPhase::Aim || weapons.is_empty() {
            return None;
        }
        let tank = &self.tanks[self.active];
        let weapon = &weapons[self.weapon_index.min(weapons.len() - 1)];
        let power = 0.45 + 0.55 * (self.charge / CHARGE_TIME).clamp(0.0, 1.0);
        let speed = weapon.speed * power;
        let vel = Vec3::new(tank.aim.cos() * speed, tank.aim.sin() * speed, 0.0);
        self.phase = TurnPhase::Flying;
        self.charging = false;
        self.charge = 0.0;
        Some((
            muzzle,
            Shell {
                vel,
                weapon_index: self.weapon_index.min(weapons.len() - 1),
                trail_acc: 0.0,
            },
        ))
    }
}

/// Wall / overhang test. Samples the mid and upper body only so resting feet on dirt
/// do not freeze the tank in place.
pub fn tank_blocked(terrain: &Terrain, x: f32, y: f32) -> bool {
    let half_w = TANK_W * 0.45;
    let half_h = TANK_H * 0.45;
    for ox in [-half_w, 0.0, half_w] {
        for oy in [0.0, half_h * 0.55] {
            let (c, r) = Terrain::world_to_cell(x + ox, y + oy);
            if terrain.solid_at(c, r) {
                return true;
            }
        }
    }
    false
}

pub fn step_shell(
    pos: &mut Vec3,
    shell: &mut Shell,
    weapon: &Weapon,
    terrain: &Terrain,
    dt: f32,
) -> Option<Vec3> {
    let steps = ((shell.vel.length() * dt).max(1.0) as i32).clamp(1, 24);
    let sub = dt / steps as f32;
    for _ in 0..steps {
        shell.vel.y -= GRAVITY * weapon.gravity * sub;
        pos.x += shell.vel.x * sub;
        pos.y += shell.vel.y * sub;
        if pos.x < 0.0 || pos.x >= MAP_W as f32 || pos.y < -20.0 || pos.y >= MAP_H as f32 + 40.0
        {
            return Some(*pos);
        }
        let (c, r) = Terrain::world_to_cell(pos.x, pos.y);
        if terrain.solid_at(c, r) {
            return Some(*pos);
        }
    }
    None
}

pub fn apply_blast(
    terrain: &mut Terrain,
    positions: &mut [Vec3; 2],
    tanks: &mut [Tank; 2],
    hit: Vec3,
    weapon: &Weapon,
) {
    terrain.stamp_circle(hit.x, hit.y, weapon.blast_radius);
    for i in 0..2 {
        let d = (positions[i] - hit).length();
        if d < weapon.blast_radius + TANK_W * 0.5 {
            let falloff = 1.0 - (d / (weapon.blast_radius + TANK_W * 0.5)).clamp(0.0, 1.0);
            tanks[i].hp = (tanks[i].hp - weapon.damage * falloff).max(0.0);
        }
    }
}

pub fn settle_tanks(terrain: &Terrain, positions: &mut [Vec3; 2], dt: f32) -> bool {
    let mut moving = false;
    for pos in positions.iter_mut() {
        // Fell off the map — stop integrating so the turn can advance.
        if pos.y < -TANK_H {
            pos.y = -TANK_H - 1.0;
            continue;
        }
        let Some(surface) = terrain.surface_y(pos.x as i32) else {
            moving = true;
            pos.y -= SETTLE_SPEED * dt;
            continue;
        };
        let target = surface + TANK_H * 0.5;
        // Compare against the heightfield, not solid_at(feet): world_to_cell is 1px
        // off the surface top, which made resting tanks look airborne forever and
        // soft-locked the match after the first shot.
        if pos.y <= target + 0.1 {
            if (pos.y - target).abs() > 0.05 {
                pos.y = target;
            }
            continue;
        }
        moving = true;
        pos.y = (pos.y - SETTLE_SPEED * dt).max(target);
    }
    !moving
}

/// Visible cannon length from the breech (inside the turret) to the muzzle.
///
/// The gun image is 160px and the tube is 68px. The hull is 80 world units
/// wide for a 160px cell, so one pixel is half a world unit.
pub const BARREL_LEN: f32 = 80.0 * 68.0 / 160.0;

pub fn muzzle_point(pos: Vec3, aim: f32) -> Vec3 {
    Vec3::new(
        pos.x + aim.cos() * BARREL_LEN,
        pos.y + aim.sin() * BARREL_LEN,
        0.2,
    )
}

/// Place the barrel so its breech (the texture center) sits on `pivot`.
pub fn barrel_sprite_pose(pivot: Vec3, aim: f32) -> (Vec3, f32) {
    (Vec3::new(pivot.x, pivot.y, 0.05), aim)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::Terrain;
    use crate::weapons::Weapon;
    use beton::{Blend, GameObject, ShaderId, World};

    fn empty_object() -> GameObject {
        GameObject {
            pos: Vec3::ZERO,
            rotation: 0.0,
            scale: [1.0, 1.0],
            parent: None,
            size: [1.0, 1.0],
            sprite: None,
            tag: 0,
        }
    }

    fn dummy_weapon(radius: f32, damage: f32) -> Weapon {
        Weapon {
            name: "t".into(),
            blast_radius: radius,
            damage,
            speed: 200.0,
            gravity: 1.0,
            trail: 10.0,
            fragments: 4,
            fragment_speed: 50.0,
            fragment_life: 0.4,
            effect: "glow".into(),
            blend: Blend::Add,
            shader: ShaderId::SPRITE,
        }
    }

    fn dummy_ids(world: &mut World) -> (GameObjectId, GameObjectId) {
        let a = world.spawn(empty_object());
        let b = world.spawn(empty_object());
        (a, b)
    }

    #[test]
    fn fuel_does_not_drain_when_blocked() {
        let terrain = Terrain::generate(2);
        let mut world = World::new();
        let (a, b) = dummy_ids(&mut world);
        let mut state = MatchState {
            tanks: [
                Tank {
                    id: a,
                    hp: 100.0,
                    fuel: FUEL_BUDGET,
                    aim: 0.5,
                    facing: 1.0,
                },
                Tank {
                    id: b,
                    hp: 100.0,
                    fuel: FUEL_BUDGET,
                    aim: 2.5,
                    facing: -1.0,
                },
            ],
            active: 0,
            phase: TurnPhase::Aim,
            charge: 0.0,
            charging: false,
            weapon_index: 0,
            shell_id: None,
        };
        let x = MAP_W as f32 * 0.5;
        let surface = terrain.surface_y(x as i32).unwrap();
        // Place the tank deep in dirt so any horizontal move is blocked.
        let mut positions = [
            Vec3::new(x, surface - TANK_H * 2.0, 0.0),
            Vec3::new(MAP_W as f32 * 0.8, 50.0, 0.0),
        ];
        let before = state.tanks[0].fuel;
        assert!(tank_blocked(&terrain, positions[0].x, positions[0].y));
        let moved = state.try_move(&terrain, &mut positions, 1.0, 0.05);
        assert!(!moved);
        assert_eq!(state.tanks[0].fuel, before);
    }

    #[test]
    fn blast_reduces_hp() {
        let mut terrain = Terrain::generate(3);
        let mut world = World::new();
        let (a, b) = dummy_ids(&mut world);
        let mut tanks = [
            Tank {
                id: a,
                hp: 100.0,
                fuel: 0.0,
                aim: 0.5,
                facing: 1.0,
            },
            Tank {
                id: b,
                hp: 100.0,
                fuel: 0.0,
                aim: 2.5,
                facing: -1.0,
            },
        ];
        let mut positions = [Vec3::new(100.0, 80.0, 0.0), Vec3::new(200.0, 80.0, 0.0)];
        let hit = positions[0];
        let w = dummy_weapon(40.0, 50.0);
        apply_blast(&mut terrain, &mut positions, &mut tanks, hit, &w);
        assert!(tanks[0].hp < 100.0);
    }

    #[test]
    fn settle_drops_after_hole() {
        let mut terrain = Terrain::generate(4);
        let x = MAP_W as f32 * 0.5;
        let surface = terrain.surface_y(x as i32).unwrap();
        terrain.stamp_circle(x, surface - 5.0, 25.0);
        let mut positions = [Vec3::new(x, surface + TANK_H * 0.5, 0.0); 2];
        let start_y = positions[0].y;
        for _ in 0..90 {
            settle_tanks(&terrain, &mut positions, 1.0 / 60.0);
        }
        assert!(positions[0].y < start_y - 1.0 || positions[0].y < -TANK_H);
    }

    #[test]
    fn settle_finishes_after_falling_off_map() {
        let terrain = Terrain::generate(5);
        let mut positions = [
            Vec3::new(40.0, -TANK_H - 50.0, 0.0),
            Vec3::new(400.0, 80.0, 0.0),
        ];
        if let Some(y) = terrain.surface_y(400) {
            positions[1].y = y + TANK_H * 0.5;
        }
        let mut done = false;
        for _ in 0..120 {
            if settle_tanks(&terrain, &mut positions, 1.0 / 60.0) {
                done = true;
                break;
            }
        }
        assert!(done, "fallen tank must not block settle forever");
    }

    #[test]
    fn second_fire_allowed_after_next_player() {
        let mut world = World::new();
        let (a, b) = dummy_ids(&mut world);
        let mut state = MatchState::spawn_tanks(&Terrain::generate(1), a, b);
        state.begin_turn();
        let weapons = [dummy_weapon(10.0, 10.0)];
        assert!(state
            .fire(&weapons, Vec3::new(10.0, 10.0, 0.0))
            .is_some());
        assert_eq!(state.phase, TurnPhase::Flying);
        state.next_player();
        assert_eq!(state.phase, TurnPhase::Aim);
        assert!(state
            .fire(&weapons, Vec3::new(20.0, 10.0, 0.0))
            .is_some());
    }
}
