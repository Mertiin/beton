//! Game bytes on the beton net pipe. The host simulates. Clients paint snapshots.

use crate::sim::{Blast, Kind, Match, Phase, WORM_COUNT};
use beton::PeerId;

pub const TAG_INPUT: u8 = 1;
pub const TAG_WELCOME: u8 = 2;
pub const TAG_GO: u8 = 3;
pub const TAG_BLAST: u8 = 4;
pub const TAG_SNAP: u8 = 5;
pub const TAG_MASK: u8 = 6;
pub const TAG_NEED: u8 = 7;

pub const DEFAULT_PORT: u16 = 7778;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InputMsg {
    pub dir: i8,
    pub jump: u8,
    pub aim: f32,
    pub aim_set: u8,
    pub charge: u8,
    pub fire: u8,
    pub weapon: u8,
    pub skip: u8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Welcome {
    pub id: u32,
    pub team: u8,
}

#[derive(Clone, Copy, Debug)]
pub struct SnapWorm {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub aim: f32,
    pub facing: f32,
    pub hp: u8,
    pub flags: u8,
    pub weapon: u8,
    pub ammo: [u8; 4],
}

#[derive(Clone, Debug)]
pub struct SnapProj {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub kind: u8,
    pub weapon: u8,
}

#[derive(Clone, Debug)]
pub struct Snap {
    pub active: u8,
    pub phase: u8,
    pub wind: f32,
    pub timer: f32,
    pub water_y: f32,
    pub charge: f32,
    pub sudden: u8,
    pub winner: u8,
    pub crate_on: u8,
    pub crate_x: f32,
    pub crate_y: f32,
    pub worms: [SnapWorm; WORM_COUNT],
    pub projs: Vec<SnapProj>,
}

pub fn phase_byte(phase: Phase) -> u8 {
    match phase {
        Phase::Aim => 0,
        Phase::Retreat => 1,
        Phase::Settle => 2,
        Phase::Over { .. } => 3,
    }
}

pub fn winner_byte(phase: Phase) -> u8 {
    match phase {
        Phase::Over { winner: Some(t) } => t,
        Phase::Over { winner: None } => 254,
        _ => 255,
    }
}

pub fn kind_byte(kind: Kind) -> u8 {
    match kind {
        Kind::Shell => 0,
        Kind::Fuse => 1,
        Kind::Pellet => 2,
        Kind::Cluster => 3,
    }
}

impl Snap {
    pub fn from_match(m: &Match) -> Self {
        let worms = std::array::from_fn(|i| {
            let w = &m.worms[i];
            let mut flags = 0u8;
            if w.body.alive {
                flags |= 1;
            }
            if w.body.grounded {
                flags |= 2;
            }
            if w.body.drowning {
                flags |= 4;
            }
            SnapWorm {
                x: w.body.pos.x,
                y: w.body.pos.y,
                vx: w.body.vel.x,
                vy: w.body.vel.y,
                aim: w.aim,
                facing: w.body.facing,
                hp: w.body.hp.round().clamp(0.0, 255.0) as u8,
                flags,
                weapon: w.weapon.min(255) as u8,
                ammo: [w.ammo[0], w.ammo[1], w.ammo[2], w.ammo[3]],
            }
        });
        let (crate_on, crate_x, crate_y) = if let Some(c) = &m.crate_box {
            (1u8, c.pos.x, c.pos.y)
        } else {
            (0, 0.0, 0.0)
        };
        let projs = m
            .projs
            .iter()
            .take(16)
            .map(|p| SnapProj {
                x: p.pos.x,
                y: p.pos.y,
                vx: p.vel.x,
                vy: p.vel.y,
                kind: kind_byte(p.kind),
                weapon: p.weapon.min(255) as u8,
            })
            .collect();
        Self {
            active: m.active as u8,
            phase: phase_byte(m.phase),
            wind: m.wind,
            timer: m.timer,
            water_y: m.water_y,
            charge: m.charge,
            sudden: u8::from(m.sudden),
            winner: winner_byte(m.phase),
            crate_on,
            crate_x,
            crate_y,
            worms,
            projs,
        }
    }
}

pub fn encode_input(msg: &InputMsg) -> Vec<u8> {
    let mut o = Vec::with_capacity(12);
    o.push(TAG_INPUT);
    o.push(msg.dir as u8);
    o.push(msg.jump);
    o.extend_from_slice(&msg.aim.to_le_bytes());
    o.push(msg.aim_set);
    o.push(msg.charge);
    o.push(msg.fire);
    o.push(msg.weapon);
    o.push(msg.skip);
    o
}

pub fn decode_input(bytes: &[u8]) -> Option<InputMsg> {
    if bytes.len() < 12 || bytes[0] != TAG_INPUT {
        return None;
    }
    Some(InputMsg {
        dir: bytes[1] as i8,
        jump: bytes[2],
        aim: f32::from_le_bytes(bytes[3..7].try_into().ok()?),
        aim_set: bytes[7],
        charge: bytes[8],
        fire: bytes[9],
        weapon: bytes[10],
        skip: bytes[11],
    })
}

pub fn encode_welcome(id: PeerId, team: u8) -> Vec<u8> {
    let mut o = Vec::with_capacity(6);
    o.push(TAG_WELCOME);
    o.extend_from_slice(&id.0.to_le_bytes());
    o.push(team);
    o
}

pub fn decode_welcome(bytes: &[u8]) -> Option<Welcome> {
    if bytes.len() < 6 || bytes[0] != TAG_WELCOME {
        return None;
    }
    Some(Welcome {
        id: u32::from_le_bytes(bytes[1..5].try_into().ok()?),
        team: bytes[5],
    })
}

pub fn encode_go(seed: u64) -> Vec<u8> {
    let mut o = Vec::with_capacity(9);
    o.push(TAG_GO);
    o.extend_from_slice(&seed.to_le_bytes());
    o
}

pub fn decode_go(bytes: &[u8]) -> Option<u64> {
    if bytes.len() < 9 || bytes[0] != TAG_GO {
        return None;
    }
    Some(u64::from_le_bytes(bytes[1..9].try_into().ok()?))
}

pub fn encode_blast(b: Blast) -> Vec<u8> {
    let mut o = Vec::with_capacity(18);
    o.push(TAG_BLAST);
    o.extend_from_slice(&b.x.to_le_bytes());
    o.extend_from_slice(&b.y.to_le_bytes());
    o.extend_from_slice(&b.radius.to_le_bytes());
    o.extend_from_slice(&b.damage.to_le_bytes());
    o.push(b.weapon);
    o
}

pub fn decode_blast(bytes: &[u8]) -> Option<Blast> {
    if bytes.len() < 18 || bytes[0] != TAG_BLAST {
        return None;
    }
    Some(Blast {
        x: f32::from_le_bytes(bytes[1..5].try_into().ok()?),
        y: f32::from_le_bytes(bytes[5..9].try_into().ok()?),
        radius: f32::from_le_bytes(bytes[9..13].try_into().ok()?),
        damage: f32::from_le_bytes(bytes[13..17].try_into().ok()?),
        weapon: bytes[17],
    })
}

pub fn encode_need() -> Vec<u8> {
    vec![TAG_NEED]
}

pub fn encode_mask(solid: &[u8]) -> Vec<u8> {
    let mut o = Vec::with_capacity(1 + solid.len());
    o.push(TAG_MASK);
    o.extend_from_slice(solid);
    o
}

pub fn decode_mask(bytes: &[u8]) -> Option<&[u8]> {
    if bytes.first() != Some(&TAG_MASK) || bytes.len() < 2 {
        return None;
    }
    Some(&bytes[1..])
}

pub fn encode_snap(s: &Snap) -> Vec<u8> {
    let mut o = Vec::with_capacity(400);
    o.push(TAG_SNAP);
    o.push(s.active);
    o.push(s.phase);
    push_f32(&mut o, s.wind);
    push_f32(&mut o, s.timer);
    push_f32(&mut o, s.water_y);
    push_f32(&mut o, s.charge);
    o.push(s.sudden);
    o.push(s.winner);
    o.push(s.crate_on);
    push_f32(&mut o, s.crate_x);
    push_f32(&mut o, s.crate_y);
    for w in &s.worms {
        push_f32(&mut o, w.x);
        push_f32(&mut o, w.y);
        push_f32(&mut o, w.vx);
        push_f32(&mut o, w.vy);
        push_f32(&mut o, w.aim);
        push_f32(&mut o, w.facing);
        o.push(w.hp);
        o.push(w.flags);
        o.push(w.weapon);
        o.extend_from_slice(&w.ammo);
    }
    o.push(s.projs.len().min(16) as u8);
    for p in s.projs.iter().take(16) {
        push_f32(&mut o, p.x);
        push_f32(&mut o, p.y);
        push_f32(&mut o, p.vx);
        push_f32(&mut o, p.vy);
        o.push(p.kind);
        o.push(p.weapon);
    }
    o
}

pub fn decode_snap(bytes: &[u8]) -> Option<Snap> {
    if bytes.first() != Some(&TAG_SNAP) {
        return None;
    }
    let mut i = 1;
    let active = take_u8(bytes, &mut i)?;
    let phase = take_u8(bytes, &mut i)?;
    let wind = take_f32(bytes, &mut i)?;
    let timer = take_f32(bytes, &mut i)?;
    let water_y = take_f32(bytes, &mut i)?;
    let charge = take_f32(bytes, &mut i)?;
    let sudden = take_u8(bytes, &mut i)?;
    let winner = take_u8(bytes, &mut i)?;
    let crate_on = take_u8(bytes, &mut i)?;
    let crate_x = take_f32(bytes, &mut i)?;
    let crate_y = take_f32(bytes, &mut i)?;
    let mut worms = [SnapWorm {
        x: 0.0,
        y: 0.0,
        vx: 0.0,
        vy: 0.0,
        aim: 0.0,
        facing: 1.0,
        hp: 0,
        flags: 0,
        weapon: 0,
        ammo: [0; 4],
    }; WORM_COUNT];
    for worm in &mut worms {
        worm.x = take_f32(bytes, &mut i)?;
        worm.y = take_f32(bytes, &mut i)?;
        worm.vx = take_f32(bytes, &mut i)?;
        worm.vy = take_f32(bytes, &mut i)?;
        worm.aim = take_f32(bytes, &mut i)?;
        worm.facing = take_f32(bytes, &mut i)?;
        worm.hp = take_u8(bytes, &mut i)?;
        worm.flags = take_u8(bytes, &mut i)?;
        worm.weapon = take_u8(bytes, &mut i)?;
        for slot in &mut worm.ammo {
            *slot = take_u8(bytes, &mut i)?;
        }
    }
    let n = take_u8(bytes, &mut i)? as usize;
    let mut projs = Vec::with_capacity(n);
    for _ in 0..n {
        projs.push(SnapProj {
            x: take_f32(bytes, &mut i)?,
            y: take_f32(bytes, &mut i)?,
            vx: take_f32(bytes, &mut i)?,
            vy: take_f32(bytes, &mut i)?,
            kind: take_u8(bytes, &mut i)?,
            weapon: take_u8(bytes, &mut i)?,
        });
    }
    Some(Snap {
        active,
        phase,
        wind,
        timer,
        water_y,
        charge,
        sudden,
        winner,
        crate_on,
        crate_x,
        crate_y,
        worms,
        projs,
    })
}

fn push_f32(o: &mut Vec<u8>, v: f32) {
    o.extend_from_slice(&v.to_le_bytes());
}

fn take_u8(bytes: &[u8], i: &mut usize) -> Option<u8> {
    let v = *bytes.get(*i)?;
    *i += 1;
    Some(v)
}

fn take_f32(bytes: &[u8], i: &mut usize) -> Option<f32> {
    let end = i.checked_add(4)?;
    let v = f32::from_le_bytes(bytes.get(*i..end)?.try_into().ok()?);
    *i = end;
    Some(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::Terrain;

    #[test]
    fn input_welcome_and_snap_roundtrip() {
        let msg = InputMsg {
            dir: -1,
            jump: 1,
            aim: 1.25,
            aim_set: 1,
            charge: 1,
            fire: 0,
            weapon: 2,
            skip: 0,
        };
        let back = decode_input(&encode_input(&msg)).expect("input");
        assert_eq!(back, msg);

        let welcome = decode_welcome(&encode_welcome(PeerId(3), 1)).expect("welcome");
        assert_eq!(welcome, Welcome { id: 3, team: 1 });
        assert_eq!(decode_go(&encode_go(0xABC)).unwrap(), 0xABC);

        let terrain = Terrain::generate(9);
        let m = Match::new(&terrain, 9);
        let snap = Snap::from_match(&m);
        let decoded = decode_snap(&encode_snap(&snap)).expect("snap");
        assert_eq!(decoded.active, snap.active);
        assert_eq!(decoded.worms[3].x, snap.worms[3].x);
        assert_eq!(decoded.worms[3].hp, snap.worms[3].hp);
        assert_eq!(decoded.water_y, snap.water_y);
    }
}
