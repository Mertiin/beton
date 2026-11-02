//! Headless authoritative minesweeper BR host for one lobby room.
//!
//! Open join: anyone who connects is seated. When 2+ players are waiting, a
//! countdown starts and the match begins automatically.
//!
//! ```text
//! minesweeper-dedicated --port 7901 --room-id abc --leader-token tok --lobby http://127.0.0.1:8080
//! ```

use std::env;
use std::thread;
use std::time::{Duration, Instant};

use beton::{Net, NetEvent, PeerId};
use minesweeper::{
    decode_hello, decode_input, default_welcome, encode_board_from_player, encode_event, encode_go,
    encode_snap, encode_welcome, BrMatch, Phase, Snap, DT, MAX_SEATS, TAG_HELLO, TAG_INPUT,
};

/// Seconds after the 2nd player joins before the match starts.
const AUTO_START_SECS: f32 = 10.0;

fn main() {
    if let Err(err) = run() {
        eprintln!("minesweeper-dedicated: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    let port = flag_u16(&args, "--port").unwrap_or(7900);
    let room_id = flag_str(&args, "--room-id").unwrap_or_else(|| "local".into());
    let _leader_token = flag_str(&args, "--leader-token").unwrap_or_default();
    let lobby = flag_str(&args, "--lobby").unwrap_or_else(|| "http://127.0.0.1:8080".into());

    let mut net = Net::new();
    net.host(port)?;
    eprintln!("dedicated listening :{port} room={room_id} (open join, auto-start {AUTO_START_SECS}s)");

    let mut seats: Vec<PeerId> = Vec::new();
    let mut pending_hello: Vec<PeerId> = Vec::new();
    let mut match_state: Option<BrMatch> = None;
    let mut seed: u64 = 0x41_4e_53_57;
    let mut snap_acc = 0.0f32;
    let mut lobby_countdown: Option<f32> = None;
    let mut last_hb = Instant::now() - Duration::from_secs(10);
    let mut empty_since: Option<Instant> = Some(Instant::now());
    let tick = Duration::from_secs_f32(DT);

    loop {
        let frame_start = Instant::now();
        net.begin_frame();

        for event in net.events().to_vec() {
            match event {
                NetEvent::Listening { port } => {
                    eprintln!("bound {port}");
                }
                NetEvent::PeerJoined(id) => {
                    if seats.len() >= MAX_SEATS {
                        continue;
                    }
                    empty_since = None;
                    // Open join: seat immediately (HELLO optional).
                    welcome_peer(
                        &mut net,
                        &mut seats,
                        &mut pending_hello,
                        &mut match_state,
                        id,
                        seed,
                    );
                }
                NetEvent::PeerLeft(id) => {
                    seats.retain(|&s| s != id);
                    pending_hello.retain(|&p| p != id);
                    if let Some(m) = match_state.as_mut() {
                        for ev in m.remove_peer(id) {
                            let _ = net.broadcast(&encode_event(&ev));
                        }
                    }
                    if seats.len() < 2 {
                        lobby_countdown = None;
                    }
                    if seats.is_empty() {
                        empty_since = Some(Instant::now());
                        // Reset lobby match shell so the next pair starts fresh.
                        if match_state
                            .as_ref()
                            .map(|m| m.phase != Phase::Playing)
                            .unwrap_or(true)
                        {
                            match_state = None;
                        }
                    }
                }
                NetEvent::Message { from, bytes } => {
                    let Some(&tag) = bytes.first() else {
                        continue;
                    };
                    match tag {
                        TAG_HELLO => {
                            let _ = decode_hello(&bytes);
                            // Already seated on PeerJoined; HELLO is a no-op ack.
                            if seats.iter().all(|&s| s != from) {
                                welcome_peer(
                                    &mut net,
                                    &mut seats,
                                    &mut pending_hello,
                                    &mut match_state,
                                    from,
                                    seed,
                                );
                            }
                        }
                        TAG_INPUT => {
                            if let (Some(msg), Some(m)) =
                                (decode_input(&bytes), match_state.as_mut())
                            {
                                let events =
                                    m.apply_input(from, msg.x as i32, msg.y as i32, msg.action);
                                if let Some(p) = m.player_by_peer_mut(from) {
                                    if p.board_dirty {
                                        let payload = encode_board_from_player(p);
                                        p.board_dirty = false;
                                        let _ = net.send(from, &payload);
                                    }
                                }
                                for ev in events {
                                    let _ = net.broadcast(&encode_event(&ev));
                                }
                            }
                        }
                        _ => {}
                    }
                }
                NetEvent::Disconnected => {
                    return Err("net disconnected".into());
                }
                _ => {}
            }
        }

        // Auto-start countdown when 2+ players wait in lobby.
        let in_lobby = match_state
            .as_ref()
            .map(|m| m.phase == Phase::Lobby)
            .unwrap_or(true);
        if in_lobby && seats.len() >= 2 {
            let rem = lobby_countdown.get_or_insert(AUTO_START_SECS);
            *rem -= DT;
            if *rem <= 0.0 {
                lobby_countdown = None;
                begin_match(
                    &mut net,
                    &mut match_state,
                    &seats,
                    &mut seed,
                    &mut snap_acc,
                );
            }
        } else if in_lobby {
            lobby_countdown = None;
        }

        let countdown_ms = lobby_countdown
            .map(|t| (t.max(0.0) * 1000.0) as u16)
            .unwrap_or(0);

        if let Some(state) = match_state.as_mut() {
            if state.phase == Phase::Playing {
                let events = state.tick(DT);
                for ev in events {
                    let _ = net.broadcast(&encode_event(&ev));
                }
                for p in state.players.iter_mut() {
                    if p.board_dirty {
                        let payload = encode_board_from_player(p);
                        p.board_dirty = false;
                        let _ = net.send(p.peer, &payload);
                    }
                }
            }

            snap_acc += DT;
            if snap_acc >= 1.0 / 12.0 {
                snap_acc = 0.0;
                let snap = Snap::from_match_with_countdown(state, countdown_ms);
                let _ = net.broadcast_unreliable(&encode_snap(&snap));
            }
        } else if seats.len() >= 1 {
            // Thin lobby snap before BrMatch exists: create shell for roster.
            let m = match_state.get_or_insert_with(|| BrMatch::new(seed));
            for &id in &seats {
                let _ = m.add_player(id);
            }
            snap_acc += DT;
            if snap_acc >= 1.0 / 12.0 {
                snap_acc = 0.0;
                let snap = Snap::from_match_with_countdown(m, countdown_ms);
                let _ = net.broadcast_unreliable(&encode_snap(&snap));
            }
        }

        if last_hb.elapsed() >= Duration::from_secs(2) {
            last_hb = Instant::now();
            let status = match match_state.as_ref().map(|m| m.phase) {
                Some(Phase::Playing) => "match",
                Some(Phase::Over) => "over",
                _ if seats.len() >= MAX_SEATS => "full",
                _ if lobby_countdown.is_some() => "starting",
                _ => "lobby",
            };
            let _ = heartbeat(&lobby, &room_id, seats.len() as u32, status);
        }

        if let Some(since) = empty_since {
            if since.elapsed() > Duration::from_secs(30) {
                eprintln!("empty room timeout — exiting");
                break;
            }
        }

        net.end_frame();
        let spent = frame_start.elapsed();
        if spent < tick {
            thread::sleep(tick - spent);
        }
    }
    Ok(())
}

fn welcome_peer(
    net: &mut Net,
    seats: &mut Vec<PeerId>,
    pending: &mut Vec<PeerId>,
    match_state: &mut Option<BrMatch>,
    id: PeerId,
    seed: u64,
) {
    if seats.iter().any(|&s| s == id) || seats.len() >= MAX_SEATS {
        pending.retain(|&p| p != id);
        return;
    }
    // Late join during playing/over: reject (wait for next room).
    if let Some(m) = match_state.as_ref() {
        if m.phase != Phase::Lobby {
            pending.retain(|&p| p != id);
            return;
        }
    }

    seats.push(id);
    pending.retain(|&p| p != id);

    let m = match_state.get_or_insert_with(|| BrMatch::new(seed));
    let Some(seat) = m.add_player(id) else {
        seats.retain(|&s| s != id);
        return;
    };

    // No leader gate — everyone is a normal seat.
    let welcome = default_welcome(id, seat, false);
    let _ = net.send(id, &encode_welcome(&welcome));
    if let Some(p) = m.player_by_seat(seat) {
        let _ = net.send(id, &encode_board_from_player(p));
    }
    eprintln!("seated peer={} seat={} players={}", id.0, seat, seats.len());
}

fn begin_match(
    net: &mut Net,
    match_state: &mut Option<BrMatch>,
    seats: &[PeerId],
    seed: &mut u64,
    snap_acc: &mut f32,
) {
    *seed = seed.wrapping_add(0x9E37_79B9);
    let m = match_state.get_or_insert_with(|| BrMatch::new(*seed));
    for &id in seats {
        let _ = m.add_player(id);
    }
    if !m.start() {
        return;
    }
    *snap_acc = 0.0;
    let _ = net.broadcast(&encode_go(*seed));
    for p in &mut m.players {
        let payload = encode_board_from_player(p);
        p.board_dirty = false;
        let _ = net.send(p.peer, &payload);
    }
    let snap = Snap::from_match_with_countdown(m, 0);
    let _ = net.broadcast_unreliable(&encode_snap(&snap));
    eprintln!("match auto-started seed={seed} players={}", m.players.len());
}

fn heartbeat(lobby: &str, room_id: &str, players: u32, status: &str) -> Result<(), String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let url = format!(
            "{}/api/rooms/{room_id}/heartbeat",
            lobby.trim_end_matches('/')
        );
        let body = serde_json::json!({ "players": players, "status": status });
        let _ = ureq::post(&url)
            .timeout(Duration::from_secs(2))
            .send_json(body);
        Ok(())
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (lobby, room_id, players, status);
        Ok(())
    }
}

fn flag_str(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|w| w[0] == name)
        .map(|w| w[1].clone())
}

fn flag_u16(args: &[String], name: &str) -> Option<u16> {
    flag_str(args, name)?.parse().ok()
}
