//! Teams, turns, shells, crates. Plain data so tests do not need a GPU.

use crate::terrain::{Terrain, MAP_H, MAP_W};
use crate::weapons::{Behavior, Weapon};
use crate::worm::{step_body, Body, WORM_H, WORM_W};
use beton::{Rng, Vec3};

pub const DT: f32 = 1.0 / 60.0;
pub const AIM_TIME: f32 = 30.0;
pub const RETREAT_TIME: f32 = 5.0;
pub const SETTLE_CAP: f32 = 3.0;
pub const CHARGE_TIME: f32 = 0.85;
pub const WATER_Y0: f32 = 36.0;
pub const SUDDEN_AFTER: u32 = 16;
pub const WATER_RISE: f32 = 4.0;
pub const MAX_PROJ: usize = 16;
pub const WORM_COUNT: usize = 8;
pub const AIM_SPEED: f32 = 1.6;

const NAMES: [&str; WORM_COUNT] = ["Nub", "Peb", "Wig", "Dot", "Zap", "Mox", "Kip", "Bun"];

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    Aim,
    Retreat,
    Settle,
    Over { winner: Option<u8> },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Shell,
    Fuse,
    Pellet,
    Cluster,
}

#[derive(Clone, Debug)]
pub struct Worm {
    pub team: u8,
    pub name: &'static str,
    pub aim: f32,
    pub weapon: usize,
    pub ammo: [u8; 8],
    pub body: Body,
}

#[derive(Clone, Debug)]
pub struct Projectile {
    pub pos: Vec3,
    pub vel: Vec3,
    pub kind: Kind,
    pub fuse: f32,
    pub ttl: f32,
    pub age: f32,
    pub owner: u8,
    pub radius: f32,
    pub damage: f32,
    pub gravity: f32,
    pub weapon: usize,
}

#[derive(Clone, Debug)]
pub struct Crate {
    pub pos: Vec3,
    pub vel: Vec3,
}

#[derive(Clone, Copy, Debug)]
pub struct Blast {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub damage: f32,
    pub weapon: u8,
}

/// One fixed step of input. Edges (`jump`, `charge_released`, `weapon`, `skip`)
/// are true for a single step. `charge_held` stays true while the key is down.
#[derive(Clone, Copy, Debug, Default)]
pub struct Control {
    pub dir: f32,
    pub jump: bool,
    pub aim: f32,
    pub charge_held: bool,
    pub charge_released: bool,
    pub weapon: Option<usize>,
    pub skip: bool,
}

impl Control {
    pub fn held_only(self) -> Self {
        Self {
            dir: self.dir,
            aim: self.aim,
            charge_held: self.charge_held,
            ..Self::default()
        }
    }

    pub fn any(self) -> bool {
        self.dir != 0.0
            || self.jump
            || self.aim != 0.0
            || self.charge_held
            || self.charge_released
            || self.weapon.is_some()
            || self.skip
    }
}

pub struct Match {
    pub worms: [Worm; WORM_COUNT],
    pub active: usize,
    pub cursor: [usize; 2],
    pub phase: Phase,
    pub timer: f32,
    pub charge: f32,
    pub charging: bool,
    pub wind: f32,
    pub water_y: f32,
    pub turns: u32,
    pub sudden: bool,
    pub projs: Vec<Projectile>,
    pub crate_box: Option<Crate>,
    pub blasts: Vec<Blast>,
    rng: Rng,
}

impl Match {
    pub fn new(terrain: &Terrain, seed: u64) -> Self {
        let worms = std::array::from_fn(|i| {
            let team = if i < 4 { 0u8 } else { 1 };
            let slot = i % 4;
            let x = if team == 0 {
                MAP_W as f32 * (0.10 + slot as f32 * 0.08)
            } else {
                MAP_W as f32 * (0.62 + slot as f32 * 0.08)
            };
            let surface = terrain.surface_y(x as i32).unwrap_or(80.0);
            let mut body = Body::at(x, surface + WORM_H * 0.5);
            body.facing = if team == 0 { 1.0 } else { -1.0 };
            Worm {
                team,
                name: NAMES[i],
                aim: if team == 0 {
                    0.9
                } else {
                    std::f32::consts::PI - 0.9
                },
                weapon: 0,
                ammo: [0; 8],
                body,
            }
        });
        Self {
            worms,
            active: 0,
            cursor: [1, 0],
            phase: Phase::Aim,
            timer: AIM_TIME,
            charge: 0.0,
            charging: false,
            wind: 0.0,
            water_y: WATER_Y0,
            turns: 0,
            sudden: false,
            projs: Vec::new(),
            crate_box: None,
            blasts: Vec::new(),
            rng: Rng::new(seed ^ 0xA11C_E5),
        }
    }

    pub fn grant_loadout(&mut self, weapons: &[Weapon]) {
        for worm in &mut self.worms {
            for (i, w) in weapons.iter().enumerate().take(8) {
                worm.ammo[i] = w.ammo;
            }
        }
    }

    pub fn team_alive(&self, team: u8) -> bool {
        self.worms.iter().any(|w| w.team == team && w.body.alive)
    }

    pub fn kill_team(&mut self, team: u8) {
        for w in &mut self.worms {
            if w.team == team {
                w.body.hp = 0.0;
                w.body.alive = false;
            }
        }
        self.finish_if_wiped();
    }

    pub fn step(&mut self, terrain: &mut Terrain, weapons: &[Weapon], mut control: Control, dt: f32) {
        if matches!(self.phase, Phase::Over { .. }) {
            self.integrate_bodies(terrain, Control::default(), false, dt);
            return;
        }

        let active = self.active;
        let aiming = matches!(self.phase, Phase::Aim);
        if aiming && self.worms[active].body.alive {
            if let Some(index) = control.weapon {
                if index < weapons.len() && self.worms[active].ammo[index] > 0 {
                    self.worms[active].weapon = index;
                }
            }
            let aim = self.worms[active].aim + control.aim;
            self.worms[active].aim = aim.clamp(0.15, std::f32::consts::PI - 0.15);
            let pellets = weapons
                .get(self.worms[active].weapon)
                .map(|w| w.behavior == Behavior::Pellets)
                .unwrap_or(false);
            if control.charge_held && !pellets {
                self.charging = true;
                self.charge = (self.charge + dt).min(CHARGE_TIME);
            }
            if pellets && control.charge_released {
                self.charge = CHARGE_TIME;
                self.charging = true;
            }
        } else if !aiming {
            control.charge_released = false;
            control.skip = false;
            control.weapon = None;
        }

        let allow = matches!(self.phase, Phase::Aim | Phase::Retreat);
        self.integrate_bodies(terrain, control, allow, dt);

        if aiming && control.charge_released {
            self.launch(terrain, weapons);
        } else if aiming && (control.skip || self.timer <= 0.0) {
            self.advance(terrain);
            return;
        }

        self.step_projs(terrain, dt);
        self.step_crate(terrain, dt);

        match self.phase {
            Phase::Aim => {
                self.timer -= dt;
                if self.timer <= 0.0 {
                    self.advance(terrain);
                }
            }
            Phase::Retreat => {
                self.timer -= dt;
                if self.timer <= 0.0 {
                    self.phase = Phase::Settle;
                    self.timer = SETTLE_CAP;
                }
            }
            Phase::Settle => {
                self.timer -= dt;
                if self.resting() || self.timer <= 0.0 {
                    self.advance(terrain);
                }
            }
            Phase::Over { .. } => {}
        }
        self.finish_if_wiped();
    }

    fn integrate_bodies(&mut self, terrain: &Terrain, control: Control, allow_active: bool, dt: f32) {
        let active = self.active;
        let water = self.water_y;
        for i in 0..WORM_COUNT {
            let drive = allow_active && i == active && self.worms[i].body.alive;
            let wish = if drive { control.dir } else { 0.0 };
            let jump = drive && control.jump;
            step_body(
                &mut self.worms[i].body,
                terrain,
                water,
                wish,
                jump,
                drive,
                dt,
            );
        }
    }

    fn launch(&mut self, terrain: &Terrain, weapons: &[Weapon]) {
        if weapons.is_empty() || !matches!(self.phase, Phase::Aim) {
            return;
        }
        let active = self.active;
        if !self.worms[active].body.alive {
            return;
        }
        let index = self.worms[active].weapon.min(weapons.len() - 1);
        if self.worms[active].ammo[index] == 0 {
            self.charging = false;
            self.charge = 0.0;
            return;
        }
        let weapon = &weapons[index];
        let power = if weapon.behavior == Behavior::Pellets {
            1.0
        } else {
            0.45 + 0.55 * (self.charge / CHARGE_TIME).clamp(0.0, 1.0)
        };
        let aim = self.worms[active].aim;
        let origin = self.worms[active].body.pos;
        // Walk the aim ray until the muzzle sits in open air so shells are not
        // deleted on the first solid test against the hillside under the worm.
        let mut muzzle = Vec3::new(origin.x, origin.y + 2.0, 0.2);
        for step in 4..=14 {
            let t = step as f32;
            let candidate = Vec3::new(
                origin.x + aim.cos() * t,
                origin.y + aim.sin() * t,
                0.2,
            );
            let (c, r) = Terrain::world_to_cell(candidate.x, candidate.y);
            if !terrain.solid_at(c, r) {
                muzzle = candidate;
                break;
            }
        }
        let speed = weapon.speed * power;
        self.worms[active].ammo[index] = self.worms[active].ammo[index].saturating_sub(1);
        match weapon.behavior {
            Behavior::Pellets => {
                let n = weapon.pellets.clamp(1, 8);
                let spread = weapon.spread;
                for i in 0..n {
                    if self.projs.len() >= MAX_PROJ {
                        break;
                    }
                    let t = if n == 1 {
                        0.0
                    } else {
                        (i as f32 / (n - 1) as f32) - 0.5
                    };
                    let angle = aim + t * spread;
                    self.projs.push(Projectile {
                        pos: muzzle,
                        vel: Vec3::new(angle.cos() * weapon.speed, angle.sin() * weapon.speed, 0.0),
                        kind: Kind::Pellet,
                        fuse: 0.0,
                        ttl: 0.18,
                        age: 0.0,
                        owner: active as u8,
                        radius: 0.0,
                        damage: weapon.damage,
                        gravity: weapon.gravity,
                        weapon: index,
                    });
                }
            }
            Behavior::Contact | Behavior::Fuse | Behavior::Cluster => {
                if self.projs.len() >= MAX_PROJ {
                    return;
                }
                let kind = match weapon.behavior {
                    Behavior::Fuse => Kind::Fuse,
                    Behavior::Cluster => Kind::Cluster,
                    _ => Kind::Shell,
                };
                let fuse = if kind == Kind::Shell { 99.0 } else { weapon.fuse.max(0.05) };
                let ttl = if kind == Kind::Cluster {
                    weapon.children.clamp(2, 8) as f32
                } else {
                    8.0
                };
                self.projs.push(Projectile {
                    pos: muzzle,
                    vel: Vec3::new(aim.cos() * speed, aim.sin() * speed, 0.0),
                    kind,
                    fuse,
                    ttl,
                    age: 0.0,
                    owner: active as u8,
                    radius: weapon.blast_radius,
                    damage: weapon.damage,
                    gravity: weapon.gravity,
                    weapon: index,
                });
            }
        }
        self.phase = Phase::Retreat;
        self.timer = RETREAT_TIME;
        self.charging = false;
        self.charge = 0.0;
    }

    fn step_projs(&mut self, terrain: &mut Terrain, dt: f32) {
        let wind = self.wind;
        let water = self.water_y;
        let mut i = 0;
        while i < self.projs.len() {
            let outcome = step_one(&mut self.projs[i], terrain, wind, water, dt);
            self.projs[i].age += dt;
            let pellet = if matches!(outcome, Fly::Keep) {
                pellet_touch(&self.projs[i], &self.worms)
            } else {
                None
            };
            match outcome {
                Fly::Gone => {
                    self.projs.remove(i);
                }
                Fly::Hit(pos) | Fly::Fuse(pos) => {
                    let proj = self.projs.remove(i);
                    self.detonate(terrain, proj, pos);
                }
                Fly::Keep if pellet.is_some() => {
                    let worm = pellet.unwrap();
                    let damage = self.projs[i].damage;
                    self.projs.remove(i);
                    if let Some(w) = self.worms.get_mut(worm) {
                        if w.body.alive {
                            w.body.hp = (w.body.hp - damage).max(0.0);
                            if w.body.hp <= 0.0 {
                                w.body.alive = false;
                            }
                        }
                    }
                }
                Fly::Keep => i += 1,
            }
        }
    }

    fn detonate(&mut self, terrain: &mut Terrain, proj: Projectile, pos: Vec3) {
        self.blast_at(terrain, pos, proj.radius, proj.damage, proj.weapon as u8);
        if proj.kind != Kind::Cluster {
            return;
        }
        let n = proj.ttl.round().clamp(2.0, 8.0) as u32;
        for k in 0..n {
            if self.projs.len() >= MAX_PROJ {
                break;
            }
            let angle = -0.4 + k as f32 * (0.8 / (n - 1) as f32);
            let speed = 70.0 + (k as f32) * 6.0;
            self.projs.push(Projectile {
                pos,
                vel: Vec3::new(angle.cos() * speed, angle.sin().abs() * speed + 40.0, 0.0),
                kind: Kind::Fuse,
                fuse: 0.55,
                ttl: 3.0,
                age: 0.0,
                owner: proj.owner,
                radius: proj.radius * 0.65,
                damage: proj.damage * 0.55,
                gravity: 1.0,
                weapon: proj.weapon,
            });
        }
    }

    fn blast_at(&mut self, terrain: &mut Terrain, pos: Vec3, radius: f32, damage: f32, weapon: u8) {
        if radius >= 1.0 {
            terrain.stamp_circle(pos.x, pos.y, radius);
        }
        let reach = radius + WORM_W * 0.5;
        if damage > 0.0 && reach > 0.0 {
            for worm in &mut self.worms {
                if !worm.body.alive {
                    continue;
                }
                let dx = worm.body.pos.x - pos.x;
                let dy = worm.body.pos.y - pos.y;
                let d = (dx * dx + dy * dy).sqrt();
                if d < reach {
                    let falloff = 1.0 - (d / reach).clamp(0.0, 1.0);
                    worm.body.hp = (worm.body.hp - damage * falloff).max(0.0);
                    if worm.body.hp <= 0.0 {
                        worm.body.alive = false;
                    }
                }
            }
        }
        if self.blasts.len() < 64 {
            self.blasts.push(Blast {
                x: pos.x,
                y: pos.y,
                radius,
                damage,
                weapon,
            });
        }
    }

    fn step_crate(&mut self, terrain: &Terrain, dt: f32) {
        let Some(crate_box) = self.crate_box.as_mut() else {
            return;
        };
        crate_box.vel.y = (crate_box.vel.y - 320.0 * dt).max(-180.0);
        crate_box.pos.y += crate_box.vel.y * dt;
        let (c, r) = Terrain::world_to_cell(crate_box.pos.x, crate_box.pos.y - 4.0);
        if terrain.solid_at(c, r) {
            crate_box.vel.y = 0.0;
            if let Some(y) = terrain.surface_y(crate_box.pos.x as i32) {
                crate_box.pos.y = y + 4.0;
            }
        }
        if crate_box.pos.y < self.water_y {
            self.crate_box = None;
            return;
        }
        let pos = self.crate_box.as_ref().map(|c| c.pos).unwrap_or(Vec3::ZERO);
        for worm in &mut self.worms {
            if !worm.body.alive {
                continue;
            }
            let dx = (worm.body.pos.x - pos.x).abs();
            let dy = (worm.body.pos.y - pos.y).abs();
            if dx < 8.0 && dy < 10.0 {
                for slot in &mut worm.ammo {
                    *slot = slot.saturating_add(1).min(9);
                }
                self.crate_box = None;
                return;
            }
        }
    }

    fn resting(&self) -> bool {
        if !self.projs.is_empty() {
            return false;
        }
        self.worms.iter().all(|w| {
            if w.body.drowning {
                return w.body.pos.y < self.water_y - 6.0;
            }
            w.body.grounded || !w.body.alive
        })
    }

    fn advance(&mut self, terrain: &Terrain) {
        if self.finish_if_wiped() {
            return;
        }
        self.turns += 1;
        if self.turns >= SUDDEN_AFTER {
            self.water_y += WATER_RISE;
            self.sudden = true;
            self.drown_low();
            if self.finish_if_wiped() {
                return;
            }
        }
        let next_team = 1 - self.worms[self.active].team;
        if let Some(index) = self.next_living(next_team) {
            self.active = index;
        } else if let Some(index) = self.next_living(1 - next_team) {
            self.active = index;
        } else {
            self.phase = Phase::Over { winner: None };
            return;
        }
        self.phase = Phase::Aim;
        self.timer = AIM_TIME;
        self.charge = 0.0;
        self.charging = false;
        self.wind = self.rng.range(-48.0, 48.0);
        self.projs.clear();
        if self.turns > 0 && self.turns % 3 == 0 && self.crate_box.is_none() {
            let x = self.rng.range(40.0, MAP_W as f32 - 40.0);
            self.crate_box = Some(Crate {
                pos: Vec3::new(x, MAP_H as f32 - 8.0, 0.2),
                vel: Vec3::ZERO,
            });
        }
        let _ = terrain;
    }

    fn drown_low(&mut self) {
        let water = self.water_y;
        for worm in &mut self.worms {
            if worm.body.alive && worm.body.pos.y < water {
                worm.body.alive = false;
                worm.body.hp = 0.0;
                worm.body.drowning = true;
            }
        }
    }

    fn next_living(&mut self, team: u8) -> Option<usize> {
        let cursor = self.cursor[team as usize];
        for k in 0..4 {
            let slot = (cursor + k) % 4;
            let index = team as usize * 4 + slot;
            if self.worms[index].body.alive {
                self.cursor[team as usize] = (slot + 1) % 4;
                return Some(index);
            }
        }
        None
    }

    fn finish_if_wiped(&mut self) -> bool {
        if matches!(self.phase, Phase::Over { .. }) {
            return true;
        }
        let a = self.team_alive(0);
        let b = self.team_alive(1);
        if a && b {
            return false;
        }
        if !self.projs.is_empty() && !matches!(self.phase, Phase::Settle) {
            self.phase = Phase::Settle;
            self.timer = SETTLE_CAP;
            return false;
        }
        if !self.resting() && !self.projs.is_empty() {
            return false;
        }
        let winner = if a {
            Some(0)
        } else if b {
            Some(1)
        } else {
            None
        };
        self.phase = Phase::Over { winner };
        true
    }
}

enum Fly {
    Keep,
    Gone,
    Hit(Vec3),
    Fuse(Vec3),
}

fn step_one(proj: &mut Projectile, terrain: &Terrain, wind: f32, water_y: f32, dt: f32) -> Fly {
    proj.vel.x += wind * dt;
    proj.vel.y -= 140.0 * proj.gravity * dt;
    let bounce = matches!(proj.kind, Kind::Fuse | Kind::Cluster);
    // Skip dirt for a short birth window so the muzzle can leave the worm/hill.
    let armed = proj.age >= 0.06;
    let steps = ((proj.vel.length() * dt).max(1.0) as i32).clamp(1, 12);
    let sub = dt / steps as f32;
    for _ in 0..steps {
        let prev = proj.pos;
        proj.pos.x += proj.vel.x * sub;
        proj.pos.y += proj.vel.y * sub;
        if proj.pos.y < water_y
            || proj.pos.x < -8.0
            || proj.pos.x > MAP_W as f32 + 8.0
            || proj.pos.y > MAP_H as f32 + 40.0
        {
            return Fly::Gone;
        }
        if bounce {
            proj.fuse -= sub;
            if proj.fuse <= 0.0 {
                return Fly::Fuse(proj.pos);
            }
        }
        if !armed {
            continue;
        }
        let (c, r) = Terrain::world_to_cell(proj.pos.x, proj.pos.y);
        if !terrain.solid_at(c, r) {
            continue;
        }
        if proj.kind == Kind::Pellet {
            return Fly::Gone;
        }
        if !bounce {
            return Fly::Hit(proj.pos);
        }
        // Fuse / cluster: push out of dirt and dampen (roll / bounce).
        proj.pos = prev;
        let (cx, cy) = Terrain::world_to_cell(proj.pos.x, proj.pos.y);
        if terrain.solid_at(cx, cy) {
            proj.pos.y += 1.0;
        }
        if proj.vel.y < 0.0 {
            proj.vel.y = -proj.vel.y * 0.35;
        }
        proj.vel.x *= 0.75;
        if proj.vel.length() < 8.0 {
            proj.vel.x *= 0.5;
            proj.vel.y = 0.0;
        }
    }
    if proj.kind == Kind::Pellet {
        proj.ttl -= dt;
        if proj.ttl <= 0.0 {
            return Fly::Gone;
        }
    }
    Fly::Keep
}

/// Pellet vs worm is resolved by the match after the move, so the helper stays pure.
fn pellet_touch(proj: &Projectile, worms: &[Worm]) -> Option<usize> {
    if proj.kind != Kind::Pellet || proj.age < 0.04 {
        return None;
    }
    for (i, worm) in worms.iter().enumerate() {
        if !worm.body.alive || i == proj.owner as usize {
            continue;
        }
        let dx = worm.body.pos.x - proj.pos.x;
        let dy = worm.body.pos.y - proj.pos.y;
        if dx * dx + dy * dy < 6.0 * 6.0 {
            return Some(i);
        }
    }
    None
}

/// Fly a shell until it hits the floor. Returns the impact x.
#[cfg(test)]
pub fn impact_x(terrain: &Terrain, wind: f32) -> f32 {
    let mut proj = Projectile {
        pos: Vec3::new(80.0, 160.0, 0.0),
        vel: Vec3::new(40.0, 10.0, 0.0),
        kind: Kind::Shell,
        fuse: 99.0,
        ttl: 8.0,
        age: 0.0,
        owner: 0,
        radius: 8.0,
        damage: 10.0,
        gravity: 1.0,
        weapon: 0,
    };
    for _ in 0..600 {
        match step_one(&mut proj, terrain, wind, 0.0, DT) {
            Fly::Hit(pos) => return pos.x,
            Fly::Gone | Fly::Fuse(_) => return proj.pos.x,
            Fly::Keep => {}
        }
    }
    proj.pos.x
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::weapons::{Behavior, Weapon};
    use beton::{Blend, ShaderId};

    fn bazooka() -> Weapon {
        Weapon {
            name: "bazooka".into(),
            blast_radius: 14.0,
            damage: 50.0,
            speed: 180.0,
            gravity: 1.0,
            trail: 10.0,
            fragments: 4,
            fragment_speed: 40.0,
            fragment_life: 0.3,
            effect: "ring".into(),
            blend: Blend::Add,
            shader: ShaderId::SPRITE,
            behavior: Behavior::Contact,
            ammo: 2,
            fuse: 0.0,
            pellets: 1,
            spread: 0.2,
            children: 0,
        }
    }

    fn grenade() -> Weapon {
        let mut w = bazooka();
        w.name = "grenade".into();
        w.behavior = Behavior::Fuse;
        w.fuse = 0.05;
        w.ammo = 1;
        w
    }

    #[test]
    fn stamp_under_a_worm_drops_it() {
        let mut terrain = Terrain::empty();
        terrain.fill_below(40.0);
        let mut m = Match::new(&terrain, 3);
        m.worms[0].body.pos.x = 60.0;
        m.worms[0].body.pos.y = 45.0;
        m.worms[0].body.grounded = true;
        terrain.stamp_circle(60.0, 40.0, 16.0);
        let y0 = m.worms[0].body.pos.y;
        for _ in 0..40 {
            m.step(&mut terrain, &[], Control::default(), DT);
        }
        assert!(
            m.worms[0].body.pos.y < y0 - 4.0,
            "y {}",
            m.worms[0].body.pos.y
        );
    }

    #[test]
    fn team_wipe_sets_over() {
        let mut terrain = Terrain::empty();
        terrain.fill_below(40.0);
        let mut m = Match::new(&terrain, 1);
        for w in &mut m.worms {
            if w.team == 1 {
                w.body.hp = 0.0;
                w.body.alive = false;
                w.body.grounded = true;
            }
        }
        m.step(&mut terrain, &[], Control::default(), DT);
        assert_eq!(m.phase, Phase::Over { winner: Some(0) });
    }

    #[test]
    fn both_teams_dead_is_a_draw() {
        let mut terrain = Terrain::empty();
        terrain.fill_below(40.0);
        let mut m = Match::new(&terrain, 1);
        for w in &mut m.worms {
            w.body.hp = 0.0;
            w.body.alive = false;
            w.body.grounded = true;
        }
        m.step(&mut terrain, &[], Control::default(), DT);
        assert_eq!(m.phase, Phase::Over { winner: None });
    }

    #[test]
    fn fuse_pops_in_the_air_and_digs() {
        let mut terrain = Terrain::empty();
        terrain.fill_below(40.0);
        // A blob in the air, beside the fuse point, so the blast has dirt to clear.
        for col in 128..145 {
            terrain.set_solid(col, 119, true);
        }
        let before = terrain.count_solid();
        let mut m = Match::new(&terrain, 2);
        m.worms[0].body.pos = Vec3::new(40.0, 50.0, 0.0);
        m.projs.push(Projectile {
            pos: Vec3::new(120.0, 150.0, 0.0),
            vel: Vec3::ZERO,
            kind: Kind::Fuse,
            fuse: 0.01,
            ttl: 2.0,
            age: 0.0,
            owner: 0,
            radius: 18.0,
            damage: 10.0,
            gravity: 0.0,
            weapon: 1,
        });
        m.phase = Phase::Retreat;
        m.timer = 2.0;
        m.step(&mut terrain, &[], Control::default(), DT);
        assert!(terrain.count_solid() < before);
        assert!(!m.blasts.is_empty());
    }

    #[test]
    fn wind_moves_the_impact() {
        let mut terrain = Terrain::empty();
        terrain.fill_below(40.0);
        let calm = impact_x(&terrain, 0.0);
        let blown = impact_x(&terrain, 80.0);
        assert!(
            (calm - blown).abs() > 4.0,
            "calm {calm} blown {blown}"
        );
    }

    #[test]
    fn empty_ammo_does_not_fire() {
        let mut terrain = Terrain::empty();
        terrain.fill_below(40.0);
        let mut m = Match::new(&terrain, 4);
        let weapons = [bazooka()];
        m.grant_loadout(&weapons);
        m.worms[0].ammo[0] = 1;
        m.charge = CHARGE_TIME;
        m.charging = true;
        let fire = Control {
            charge_released: true,
            ..Control::default()
        };
        m.step(&mut terrain, &weapons, fire, DT);
        assert_eq!(m.worms[0].ammo[0], 0);
        assert!(!m.projs.is_empty());
        m.projs.clear();
        m.phase = Phase::Aim;
        m.timer = AIM_TIME;
        m.step(&mut terrain, &weapons, fire, DT);
        assert!(m.projs.is_empty());
        assert_eq!(m.worms[0].ammo[0], 0);
    }

    #[test]
    fn rising_water_drowns_a_worm_that_did_not_move() {
        let mut terrain = Terrain::empty();
        terrain.fill_below(40.0);
        let mut m = Match::new(&terrain, 5);
        for w in &mut m.worms {
            w.body.grounded = true;
            w.body.pos.y = 50.0;
        }
        m.turns = SUDDEN_AFTER - 1;
        m.water_y = 50.0 - WATER_RISE + 1.0;
        m.timer = 0.01;
        m.step(&mut terrain, &[], Control { skip: true, ..Control::default() }, DT);
        assert!(m.sudden);
        assert!(m.water_y > 50.0);
        assert!(m.worms.iter().any(|w| !w.body.alive));
    }

    #[test]
    fn crate_touch_adds_ammo() {
        let mut terrain = Terrain::empty();
        terrain.fill_below(40.0);
        let mut m = Match::new(&terrain, 6);
        let weapons = [bazooka()];
        m.grant_loadout(&weapons);
        let before = m.worms[0].ammo[0];
        m.crate_box = Some(Crate {
            pos: m.worms[0].body.pos,
            vel: Vec3::new(0.0, 0.0, 0.0),
        });
        m.step(&mut terrain, &weapons, Control::default(), DT);
        assert!(m.crate_box.is_none());
        assert!(m.worms[0].ammo[0] > before);
    }

    #[test]
    fn grenade_weapon_is_a_fuse() {
        let w = grenade();
        assert_eq!(w.behavior, Behavior::Fuse);
    }

    #[test]
    fn shell_clears_the_muzzle_and_flies() {
        let mut terrain = Terrain::generate(0xBEEF);
        terrain.plant_ridge_trees(&[]);
        let mut m = Match::new(&terrain, 0xBEEF);
        let weapons = [bazooka()];
        m.grant_loadout(&weapons);
        m.worms[0].ammo[0] = 3;
        m.charge = CHARGE_TIME;
        m.charging = true;
        m.worms[0].aim = 1.1;
        m.step(
            &mut terrain,
            &weapons,
            Control {
                charge_released: true,
                ..Control::default()
            },
            DT,
        );
        assert!(!m.projs.is_empty(), "shell should spawn");
        for _ in 0..12 {
            m.step(&mut terrain, &weapons, Control::default(), DT);
        }
        assert!(
            !m.projs.is_empty() || !m.blasts.is_empty(),
            "shell should still be flying or have hit after grace"
        );
        // Prefer flight: if it blasted in 12 frames the crater is fine, but it must not vanish with no blast.
        if m.projs.is_empty() {
            assert!(!m.blasts.is_empty());
        } else {
            assert!(m.projs[0].age > 0.1);
        }
    }

    #[test]
    fn grenade_rolls_on_dirt_until_fuse() {
        let mut terrain = Terrain::empty();
        terrain.fill_below(40.0);
        let mut m = Match::new(&terrain, 7);
        m.projs.push(Projectile {
            pos: Vec3::new(120.0, 55.0, 0.0),
            vel: Vec3::new(40.0, -80.0, 0.0),
            kind: Kind::Fuse,
            fuse: 0.45,
            ttl: 3.0,
            age: 0.0,
            owner: 0,
            radius: 14.0,
            damage: 20.0,
            gravity: 1.0,
            weapon: 1,
        });
        m.phase = Phase::Retreat;
        m.timer = 2.0;
        let before = terrain.count_solid();
        // Hit the floor a few times without detonating.
        for _ in 0..20 {
            m.step(&mut terrain, &[], Control::default(), DT);
            if m.projs.is_empty() {
                break;
            }
        }
        assert!(
            !m.projs.is_empty() || !m.blasts.is_empty(),
            "grenade should still exist or have fused"
        );
        if let Some(p) = m.projs.first() {
            assert!(p.vel.y > -40.0, "downward speed should damp after bounce");
            assert_eq!(m.blasts.len(), 0, "must not blast on contact");
            assert_eq!(terrain.count_solid(), before);
        }
        // Run out the fuse.
        for _ in 0..60 {
            m.step(&mut terrain, &[], Control::default(), DT);
            if m.projs.is_empty() {
                break;
            }
        }
        assert!(m.projs.is_empty());
        assert!(!m.blasts.is_empty());
        assert!(terrain.count_solid() < before);
    }
}
