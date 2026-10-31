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
pub const TAG_HELLO: u8 = 8;
pub const TAG_START: u8 = 9;

pub const DEFAULT_PORT: u16 = 7778;
/// Max players seated on a dedicated / hosted match (not counting the sim host).
pub const MAX_SEATS: usize = 3;

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
    /// 1 if this peer may press Start on a dedicated room.
    pub leader: u8,
}

#[derive(Clone, Copy, Debug)]
pub struct SnapWorm {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub aim: f32,
    pub facing: f32,
    pub travel: f32,
    pub hp: u8,
    pub flags: u8,
    pub weapon: u8,
    pub ammo: [u8; 8],
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
        Kind::Banana => 4,
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
                travel: w.body.travel,
                hp: w.body.hp.round().clamp(0.0, 255.0) as u8,
                flags,
                weapon: w.weapon.min(255) as u8,
                ammo: w.ammo,
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

pub fn encode_welcome(id: PeerId, team: u8, leader: bool) -> Vec<u8> {
    let mut o = Vec::with_capacity(7);
    o.push(TAG_WELCOME);
    o.extend_from_slice(&id.0.to_le_bytes());
    o.push(team);
    o.push(u8::from(leader));
    o
}

pub fn decode_welcome(bytes: &[u8]) -> Option<Welcome> {
    if bytes.len() < 6 || bytes[0] != TAG_WELCOME {
        return None;
    }
    Some(Welcome {
        id: u32::from_le_bytes(bytes[1..5].try_into().ok()?),
        team: bytes[5],
        leader: bytes.get(6).copied().unwrap_or(0),
    })
}

pub fn encode_hello(token: &str) -> Vec<u8> {
    let raw = token.as_bytes();
    let mut o = Vec::with_capacity(2 + raw.len());
    o.push(TAG_HELLO);
    o.push(raw.len().min(255) as u8);
    o.extend_from_slice(&raw[..raw.len().min(255)]);
    o
}

pub fn decode_hello(bytes: &[u8]) -> Option<&str> {
    if bytes.first() != Some(&TAG_HELLO) || bytes.len() < 2 {
        return None;
    }
    let n = bytes[1] as usize;
    let end = 2 + n;
    if bytes.len() < end {
        return None;
    }
    std::str::from_utf8(&bytes[2..end]).ok()
}

pub fn encode_start() -> Vec<u8> {
    vec![TAG_START]
}

pub fn decode_start(bytes: &[u8]) -> bool {
    bytes.first() == Some(&TAG_START)
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

/// Max solid bytes per MASK packet so the full frame stays under 16 KiB.
pub const MASK_CHUNK: usize = 15 * 1024;

/// Split a packed solid mask into reliable MASK packets: tag, total, index, payload.
pub fn encode_mask_chunks(solid: &[u8]) -> Vec<Vec<u8>> {
    let total = solid.len().div_ceil(MASK_CHUNK).max(1) as u8;
    let mut out = Vec::with_capacity(total as usize);
    for (index, chunk) in solid.chunks(MASK_CHUNK).enumerate() {
        let mut o = Vec::with_capacity(3 + chunk.len());
        o.push(TAG_MASK);
        o.push(total);
        o.push(index as u8);
        o.extend_from_slice(chunk);
        out.push(o);
    }
    if out.is_empty() {
        out.push(vec![TAG_MASK, 1, 0]);
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MaskChunk<'a> {
    pub total: u8,
    pub index: u8,
    pub data: &'a [u8],
}

pub fn decode_mask_chunk(bytes: &[u8]) -> Option<MaskChunk<'_>> {
    if bytes.first() != Some(&TAG_MASK) || bytes.len() < 3 {
        return None;
    }
    Some(MaskChunk {
        total: bytes[1],
        index: bytes[2],
        data: &bytes[3..],
    })
}

/// Collect MASK chunks until the full solid buffer is ready.
#[derive(Default)]
pub struct MaskAssembler {
    total: u8,
    parts: Vec<Option<Vec<u8>>>,
    got: u8,
}

impl MaskAssembler {
    pub fn push(&mut self, chunk: MaskChunk<'_>) -> Option<Vec<u8>> {
        if chunk.total == 0 || chunk.index >= chunk.total {
            return None;
        }
        if self.total != chunk.total || self.parts.len() != chunk.total as usize {
            self.total = chunk.total;
            self.parts = vec![None; chunk.total as usize];
            self.got = 0;
        }
        let slot = chunk.index as usize;
        if self.parts[slot].is_none() {
            self.got += 1;
        }
        self.parts[slot] = Some(chunk.data.to_vec());
        if self.got < self.total {
            return None;
        }
        let mut solid = Vec::new();
        for part in self.parts.drain(..) {
            solid.extend(part.unwrap_or_default());
        }
        self.total = 0;
        self.got = 0;
        Some(solid)
    }
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
        push_f32(&mut o, w.travel);
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
        travel: 0.0,
        hp: 0,
        flags: 0,
        weapon: 0,
        ammo: [0; 8],
    }; WORM_COUNT];
    for worm in &mut worms {
        worm.x = take_f32(bytes, &mut i)?;
        worm.y = take_f32(bytes, &mut i)?;
        worm.vx = take_f32(bytes, &mut i)?;
        worm.vy = take_f32(bytes, &mut i)?;
        worm.aim = take_f32(bytes, &mut i)?;
        worm.facing = take_f32(bytes, &mut i)?;
        worm.travel = take_f32(bytes, &mut i)?;
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

        let welcome = decode_welcome(&encode_welcome(PeerId(3), 1, true)).expect("welcome");
        assert_eq!(
            welcome,
            Welcome {
                id: 3,
                team: 1,
                leader: 1
            }
        );
        assert_eq!(decode_go(&encode_go(0xABC)).unwrap(), 0xABC);
        assert_eq!(decode_hello(&encode_hello("tok-abc")), Some("tok-abc"));
        assert!(decode_start(&encode_start()));

        let terrain = Terrain::generate(9);
        let m = Match::new(&terrain, 9);
        let snap = Snap::from_match(&m);
        let decoded = decode_snap(&encode_snap(&snap)).expect("snap");
        assert_eq!(decoded.active, snap.active);
        assert_eq!(decoded.worms[3].x, snap.worms[3].x);
        assert_eq!(decoded.worms[3].hp, snap.worms[3].hp);
        assert_eq!(decoded.water_y, snap.water_y);
    }

    #[test]
    fn mask_chunks_roundtrip_under_payload() {
        let terrain = Terrain::generate(42);
        let packed = terrain.pack_solid();
        assert!(packed.len() > MASK_CHUNK, "map should need multiple chunks");
        let chunks = encode_mask_chunks(&packed);
        assert!(chunks.len() > 1);
        for chunk in &chunks {
            assert!(
                chunk.len() <= 3 + MASK_CHUNK,
                "chunk {} too big",
                chunk.len()
            );
            assert!(chunk.len() < 16 * 1024);
        }
        let mut asm = MaskAssembler::default();
        let mut solid = None;
        for chunk in &chunks {
            let decoded = decode_mask_chunk(chunk).expect("chunk");
            if let Some(done) = asm.push(decoded) {
                solid = Some(done);
            }
        }
        let solid = solid.expect("assembled");
        assert_eq!(solid, packed);
        let mut other = Terrain::empty();
        assert!(other.unpack_solid(&solid));
        assert_eq!(other.checksum(), terrain.checksum());
    }
}
