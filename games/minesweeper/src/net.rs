//! Game bytes on the beton net pipe. Dedicated host simulates; clients paint.

use beton::PeerId;

use crate::board::{Action, CellView};
use crate::br_match::{
    decode_cell_view, BrMatch, DamageReason, MatchEvent, Phase, PlayerSlot, BR_H, BR_MINES, BR_W,
    HIDE_SECS, MAX_SEATS, MOVE_DEADLINE_SECS, START_HP,
};

pub const TAG_INPUT: u8 = 1;
pub const TAG_WELCOME: u8 = 2;
pub const TAG_GO: u8 = 3;
pub const TAG_SNAP: u8 = 5;
pub const TAG_BOARD: u8 = 6;
pub const TAG_EVENT: u8 = 7;
pub const TAG_HELLO: u8 = 8;
pub const TAG_START: u8 = 9;

pub const DEFAULT_PORT: u16 = 7900;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputMsg {
    pub x: u8,
    pub y: u8,
    pub action: Action,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Welcome {
    pub id: u32,
    pub seat: u8,
    pub leader: u8,
    pub width: u8,
    pub height: u8,
    pub mines: u8,
    pub deadline_secs: u8,
    pub hide_secs: u8,
    pub start_hp: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeatSnap {
    pub seat: u8,
    pub hp: u8,
    pub alive: u8,
    pub deadline_ms: u16,
    pub hide_ms: u16,
    pub clears: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snap {
    pub phase: u8,
    pub winner: u8,
    /// Lobby auto-start countdown (ms). 0 = not counting / match running.
    pub lobby_countdown_ms: u16,
    pub seats: Vec<SeatSnap>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoardMsg {
    pub seat: u8,
    pub width: u8,
    pub height: u8,
    pub cells: Vec<u8>,
}

impl Snap {
    pub fn from_match(m: &BrMatch) -> Self {
        Self::from_match_with_countdown(m, 0)
    }

    pub fn from_match_with_countdown(m: &BrMatch, lobby_countdown_ms: u16) -> Self {
        let phase = match m.phase {
            Phase::Lobby => 0,
            Phase::Playing => 1,
            Phase::Over => 2,
        };
        let winner = m.winner.unwrap_or(255);
        let seats = m
            .players
            .iter()
            .map(|p| SeatSnap {
                seat: p.seat,
                hp: p.hp,
                alive: u8::from(p.alive),
                deadline_ms: (p.deadline.max(0.0) * 1000.0) as u16,
                hide_ms: (p.hide_rem.max(0.0) * 1000.0) as u16,
                clears: p.clears.min(u16::MAX as u32) as u16,
            })
            .collect();
        Self {
            phase,
            winner,
            lobby_countdown_ms,
            seats,
        }
    }
}

pub fn encode_input(msg: &InputMsg) -> Vec<u8> {
    let act = match msg.action {
        Action::Sweep => 0u8,
        Action::Flag => 1u8,
    };
    vec![TAG_INPUT, msg.x, msg.y, act]
}

pub fn decode_input(bytes: &[u8]) -> Option<InputMsg> {
    if bytes.len() < 4 || bytes[0] != TAG_INPUT {
        return None;
    }
    let action = match bytes[3] {
        0 => Action::Sweep,
        1 => Action::Flag,
        _ => return None,
    };
    Some(InputMsg {
        x: bytes[1],
        y: bytes[2],
        action,
    })
}

pub fn encode_welcome(w: &Welcome) -> Vec<u8> {
    let mut o = Vec::with_capacity(14);
    o.push(TAG_WELCOME);
    o.extend_from_slice(&w.id.to_le_bytes());
    o.push(w.seat);
    o.push(w.leader);
    o.push(w.width);
    o.push(w.height);
    o.push(w.mines);
    o.push(w.deadline_secs);
    o.push(w.hide_secs);
    o.push(w.start_hp);
    o
}

pub fn decode_welcome(bytes: &[u8]) -> Option<Welcome> {
    if bytes.len() < 13 || bytes[0] != TAG_WELCOME {
        return None;
    }
    Some(Welcome {
        id: u32::from_le_bytes(bytes[1..5].try_into().ok()?),
        seat: bytes[5],
        leader: bytes[6],
        width: bytes[7],
        height: bytes[8],
        mines: bytes[9],
        deadline_secs: bytes[10],
        hide_secs: bytes[11],
        start_hp: bytes.get(12).copied().unwrap_or(START_HP),
    })
}

pub fn default_welcome(id: PeerId, seat: u8, leader: bool) -> Welcome {
    Welcome {
        id: id.0,
        seat,
        leader: u8::from(leader),
        width: BR_W as u8,
        height: BR_H as u8,
        mines: BR_MINES as u8,
        deadline_secs: MOVE_DEADLINE_SECS as u8,
        hide_secs: HIDE_SECS as u8,
        start_hp: START_HP,
    }
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

const SEAT_SNAP_BYTES: usize = 9;

pub fn encode_snap(s: &Snap) -> Vec<u8> {
    let n = s.seats.len().min(MAX_SEATS) as u8;
    let mut o = Vec::with_capacity(6 + n as usize * SEAT_SNAP_BYTES);
    o.push(TAG_SNAP);
    o.push(s.phase);
    o.push(s.winner);
    o.extend_from_slice(&s.lobby_countdown_ms.to_le_bytes());
    o.push(n);
    for seat in s.seats.iter().take(n as usize) {
        o.push(seat.seat);
        o.push(seat.hp);
        o.push(seat.alive);
        o.extend_from_slice(&seat.deadline_ms.to_le_bytes());
        o.extend_from_slice(&seat.hide_ms.to_le_bytes());
        o.extend_from_slice(&seat.clears.to_le_bytes());
    }
    o
}

pub fn decode_snap(bytes: &[u8]) -> Option<Snap> {
    if bytes.len() < 6 || bytes[0] != TAG_SNAP {
        return None;
    }
    let phase = bytes[1];
    let winner = bytes[2];
    let lobby_countdown_ms = u16::from_le_bytes(bytes[3..5].try_into().ok()?);
    let n = bytes[5] as usize;
    if bytes.len() < 6 + n * SEAT_SNAP_BYTES {
        return None;
    }
    let mut seats = Vec::with_capacity(n);
    let mut i = 6;
    for _ in 0..n {
        seats.push(SeatSnap {
            seat: bytes[i],
            hp: bytes[i + 1],
            alive: bytes[i + 2],
            deadline_ms: u16::from_le_bytes(bytes[i + 3..i + 5].try_into().ok()?),
            hide_ms: u16::from_le_bytes(bytes[i + 5..i + 7].try_into().ok()?),
            clears: u16::from_le_bytes(bytes[i + 7..i + 9].try_into().ok()?),
        });
        i += SEAT_SNAP_BYTES;
    }
    Some(Snap {
        phase,
        winner,
        lobby_countdown_ms,
        seats,
    })
}

pub fn encode_board(seat: u8, width: u8, height: u8, cells: &[u8]) -> Vec<u8> {
    let mut o = Vec::with_capacity(4 + cells.len());
    o.push(TAG_BOARD);
    o.push(seat);
    o.push(width);
    o.push(height);
    o.extend_from_slice(cells);
    o
}

pub fn encode_board_from_player(p: &PlayerSlot) -> Vec<u8> {
    let cells = BrMatch::encode_board_cells(&p.board);
    encode_board(p.seat, p.board.width() as u8, p.board.height() as u8, &cells)
}

pub fn decode_board(bytes: &[u8]) -> Option<BoardMsg> {
    if bytes.len() < 4 || bytes[0] != TAG_BOARD {
        return None;
    }
    let seat = bytes[1];
    let width = bytes[2];
    let height = bytes[3];
    let need = width as usize * height as usize;
    if bytes.len() < 4 + need {
        return None;
    }
    Some(BoardMsg {
        seat,
        width,
        height,
        cells: bytes[4..4 + need].to_vec(),
    })
}

pub fn cells_to_views(msg: &BoardMsg) -> Vec<CellView> {
    msg.cells.iter().copied().map(decode_cell_view).collect()
}

pub fn encode_event(ev: &MatchEvent) -> Vec<u8> {
    match ev {
        MatchEvent::Cleared { seat, hide_target } => {
            vec![TAG_EVENT, 0, *seat, *hide_target]
        }
        MatchEvent::Damaged { seat, reason, hp } => {
            let r = match reason {
                DamageReason::Mine => 0u8,
                DamageReason::Deadline => 1u8,
            };
            vec![TAG_EVENT, 1, *seat, r, *hp]
        }
        MatchEvent::Eliminated { seat } => vec![TAG_EVENT, 2, *seat],
        MatchEvent::Winner { seat } => vec![TAG_EVENT, 3, seat.unwrap_or(255)],
    }
}

pub fn decode_event(bytes: &[u8]) -> Option<MatchEvent> {
    if bytes.first() != Some(&TAG_EVENT) || bytes.len() < 2 {
        return None;
    }
    match bytes[1] {
        0 if bytes.len() >= 4 => Some(MatchEvent::Cleared {
            seat: bytes[2],
            hide_target: bytes[3],
        }),
        1 if bytes.len() >= 5 => Some(MatchEvent::Damaged {
            seat: bytes[2],
            reason: if bytes[3] == 1 {
                DamageReason::Deadline
            } else {
                DamageReason::Mine
            },
            hp: bytes[4],
        }),
        2 if bytes.len() >= 3 => Some(MatchEvent::Eliminated { seat: bytes[2] }),
        3 if bytes.len() >= 3 => Some(MatchEvent::Winner {
            seat: if bytes[2] == 255 {
                None
            } else {
                Some(bytes[2])
            },
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_input_snap_board_event() {
        let input = InputMsg {
            x: 3,
            y: 4,
            action: Action::Flag,
        };
        assert_eq!(decode_input(&encode_input(&input)), Some(input));

        let snap = Snap {
            phase: 1,
            winner: 255,
            lobby_countdown_ms: 0,
            seats: vec![SeatSnap {
                seat: 0,
                hp: 3,
                alive: 1,
                deadline_ms: 8000,
                hide_ms: 0,
                clears: 2,
            }],
        };
        assert_eq!(decode_snap(&encode_snap(&snap)), Some(snap));

        let cells = vec![0u8, 1, 16, 64];
        let board = encode_board(0, 2, 2, &cells);
        let msg = decode_board(&board).unwrap();
        assert_eq!(msg.cells, cells);

        let ev = MatchEvent::Cleared {
            seat: 0,
            hide_target: 1,
        };
        assert_eq!(decode_event(&encode_event(&ev)), Some(ev));
        assert_eq!(
            decode_hello(&encode_hello("tok")),
            Some("tok")
        );
        assert!(decode_start(&encode_start()));
    }

    #[test]
    fn cell_view_roundtrip() {
        use crate::br_match::encode_cell_view;
        for v in [
            CellView::Unvisited,
            CellView::Number(3),
            CellView::Empty,
            CellView::Flag,
            CellView::Bomb,
            CellView::FlaggedBomb,
        ] {
            assert_eq!(decode_cell_view(encode_cell_view(v)), v);
        }
    }
}
