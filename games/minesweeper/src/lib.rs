//! Shared minesweeper board, BR match sim, net protocol, and lobby helpers.

pub mod board;
pub mod br_match;
pub mod lobby;
pub mod net;

pub use board::{Action, Board, CellView};
pub use br_match::{
    BrMatch, DamageReason, MatchEvent, Phase, PlayerSlot, BR_H, BR_MINES, BR_W, DT, HIDE_SECS,
    MAX_SEATS, MOVE_DEADLINE_SECS, START_HP,
};
pub use lobby::{
    create_room, list_rooms, lobby_base_url, public_server, CreateRoom, RoomInfo, GAME_ID,
};
pub use net::*;
