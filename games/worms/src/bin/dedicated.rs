//! Headless authoritative worms host for one lobby room.
//!
//! ```text
//! worms-dedicated --port 7801 --room-id abc --leader-token tok --lobby http://127.0.0.1:8080
//! ```

use std::env;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use beton::{Net, NetEvent, PeerId};
use worms::net::{
    decode_hello, decode_input, encode_blast, encode_go, encode_mask_chunks,
    encode_snap, encode_welcome, InputMsg, Snap, MAX_SEATS, TAG_HELLO, TAG_INPUT, TAG_NEED,
    TAG_START,
};
use worms::sim::{Control, Match, DT};
use worms::terrain::Terrain;
use worms::weapons::{load_weapons_dir, Weapon};

fn main() {
    if let Err(err) = run() {
        eprintln!("worms-dedicated: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    let port = flag_u16(&args, "--port").unwrap_or(7800);
    let room_id = flag_str(&args, "--room-id").unwrap_or_else(|| "local".into());
    let leader_token = flag_str(&args, "--leader-token").unwrap_or_default();
    let lobby = flag_str(&args, "--lobby").unwrap_or_else(|| "http://127.0.0.1:8080".into());
    let assets = flag_str(&args, "--assets").map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets")
    });

    let weapons = load_weapons_dir(&assets.join("weapons"))?;
    let mut net = Net::new();
    net.host(port)?;
    eprintln!("dedicated listening :{port} room={room_id}");

    let mut seats: Vec<(PeerId, u8)> = Vec::new();
    let mut leader: Option<PeerId> = None;
    let mut pending_hello: Vec<(PeerId, bool)> = Vec::new();
    let mut remote: [Option<InputMsg>; 2] = [None, None];
    let mut match_state: Option<Match> = None;
    let mut terrain: Option<Terrain> = None;
    let mut seed: u64 = 0xA11C_E;
    let mut blast_sent = 0usize;
    let mut snap_acc = 0.0f32;
    let mut acc = 0.0f32;
    let mut last_hb = Instant::now() - Duration::from_secs(10);
    let mut empty_since: Option<Instant> = None;
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
                    // Defer welcome until HELLO (or short timeout as non-leader).
                    pending_hello.push((id, false));
                    empty_since = None;
                }
                NetEvent::PeerLeft(id) => {
                    seats.retain(|s| s.0 != id);
                    pending_hello.retain(|p| p.0 != id);
                    if leader == Some(id) {
                        leader = seats.first().map(|s| s.0);
                    }
                    if seats.is_empty() && match_state.is_none() {
                        empty_since = Some(Instant::now());
                    }
                }
                NetEvent::Message { from, bytes } => {
                    let Some(&tag) = bytes.first() else {
                        continue;
                    };
                    match tag {
                        TAG_HELLO => {
                            let ok = decode_hello(&bytes)
                                .map(|t| !leader_token.is_empty() && t == leader_token)
                                .unwrap_or(false);
                            if let Some(slot) = pending_hello.iter_mut().find(|p| p.0 == from) {
                                slot.1 = true;
                            } else if seats.iter().all(|s| s.0 != from) {
                                pending_hello.push((from, true));
                            }
                            if ok && leader.is_none() {
                                leader = Some(from);
                            }
                            welcome_peer(
                                &mut net,
                                &mut seats,
                                &mut pending_hello,
                                from,
                                leader,
                                leader_token.is_empty(),
                                match_state.as_ref(),
                                terrain.as_ref(),
                                seed,
                            );
                        }
                        TAG_START => {
                            if (leader == Some(from)
                                || (leader_token.is_empty() && seats.first().map(|s| s.0) == Some(from)))
                                && match_state.is_none()
                            {
                                begin_match(
                                    &mut net,
                                    &weapons,
                                    &mut match_state,
                                    &mut terrain,
                                    &mut seed,
                                    &mut blast_sent,
                                    &mut snap_acc,
                                    &mut acc,
                                );
                            }
                        }
                        TAG_INPUT => {
                            if let Some(msg) = decode_input(&bytes) {
                                if let Some(team) =
                                    seats.iter().find(|s| s.0 == from).map(|s| s.1)
                                {
                                    remote[team as usize] = Some(msg);
                                }
                            }
                        }
                        TAG_NEED => {
                            if let Some(t) = &terrain {
                                for chunk in encode_mask_chunks(&t.pack_solid()) {
                                    let _ = net.broadcast(&chunk);
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

        // Without a leader token (LAN), welcome immediately. With a token, wait for HELLO.
        if leader_token.is_empty() {
            let pending = pending_hello.clone();
            for (id, seen) in pending {
                if seen || seats.iter().any(|s| s.0 == id) {
                    continue;
                }
                welcome_peer(
                    &mut net,
                    &mut seats,
                    &mut pending_hello,
                    id,
                    leader,
                    true,
                    match_state.as_ref(),
                    terrain.as_ref(),
                    seed,
                );
            }
        }

        if let (Some(state), Some(terrain)) = (match_state.as_mut(), terrain.as_mut()) {
            acc += DT;
            let mut n = 0;
            while acc >= DT && n < 4 {
                acc -= DT;
                let team = state.worms[state.active].team as usize;
                let control = control_from_remote(&mut remote, team, n == 0);
                state.step(terrain, &weapons, control, DT);
                n += 1;
            }
            while blast_sent < state.blasts.len() {
                let _ = net.broadcast(&encode_blast(state.blasts[blast_sent]));
                blast_sent += 1;
            }
            snap_acc += DT;
            if snap_acc >= 1.0 / 20.0 {
                snap_acc = 0.0;
                let snap = Snap::from_match(state);
                let _ = net.broadcast_unreliable(&encode_snap(&snap));
            }
        }

        if last_hb.elapsed() >= Duration::from_secs(2) {
            last_hb = Instant::now();
            let status = if match_state.is_some() {
                "match"
            } else if seats.len() >= MAX_SEATS {
                "full"
            } else {
                "lobby"
            };
            let _ = heartbeat(&lobby, &room_id, seats.len() as u32, status);
        }

        if let Some(since) = empty_since {
            if since.elapsed() > Duration::from_secs(300) {
                eprintln!("empty lobby timeout — exiting");
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
    seats: &mut Vec<(PeerId, u8)>,
    pending: &mut Vec<(PeerId, bool)>,
    id: PeerId,
    leader: Option<PeerId>,
    lan_mode: bool,
    match_state: Option<&Match>,
    terrain: Option<&Terrain>,
    seed: u64,
) {
    if seats.iter().any(|s| s.0 == id) || seats.len() >= MAX_SEATS {
        pending.retain(|p| p.0 != id);
        return;
    }
    // Dedicated seats include the leader — assign teams 0,1,… (LAN host keeps team 0 itself).
    let team = if lan_mode {
        if seats.len() % 2 == 0 {
            1
        } else {
            0
        }
    } else {
        (seats.len() % MAX_SEATS) as u8
    };
    seats.push((id, team));
    pending.retain(|p| p.0 != id);
    let is_leader = leader == Some(id) || (lan_mode && seats.len() == 1);
    let _ = net.send(id, &encode_welcome(id, team, is_leader));
    if let Some(_m) = match_state {
        let _ = net.send(id, &encode_go(seed));
        if let Some(t) = terrain {
            for chunk in encode_mask_chunks(&t.pack_solid()) {
                let _ = net.send(id, &chunk);
            }
        }
    }
}

fn begin_match(
    net: &mut Net,
    weapons: &[Weapon],
    match_state: &mut Option<Match>,
    terrain: &mut Option<Terrain>,
    seed: &mut u64,
    blast_sent: &mut usize,
    snap_acc: &mut f32,
    acc: &mut f32,
) {
    *seed = seed.wrapping_add(0x9E37_79B9);
    let mut t = Terrain::generate(*seed);
    let mut m = Match::new(&t, *seed);
    let spawns: Vec<f32> = m.worms.iter().map(|w| w.body.pos.x).collect();
    t.plant_ridge_trees(&spawns);
    m.grant_loadout(weapons);
    *blast_sent = 0;
    *snap_acc = 0.0;
    *acc = 0.0;
    let _ = net.broadcast(&encode_go(*seed));
    for chunk in encode_mask_chunks(&t.pack_solid()) {
        let _ = net.broadcast(&chunk);
    }
    *terrain = Some(t);
    *match_state = Some(m);
    eprintln!("match started seed={seed}");
}

fn control_from_remote(
    remote: &mut [Option<InputMsg>; 2],
    team: usize,
    first: bool,
) -> Control {
    let Some(msg) = remote.get(team).copied().flatten() else {
        return Control::default();
    };
    let control = Control {
        dir: msg.dir as f32,
        jump: first && msg.jump == 1,
        aim: msg.aim,
        aim_set: msg.aim_set == 1,
        charge_held: msg.charge == 1,
        charge_released: first && msg.fire == 1,
        weapon: if first && msg.weapon != 255 {
            Some(msg.weapon as usize)
        } else {
            None
        },
        skip: first && msg.skip == 1,
    };
    if first {
        if let Some(msg) = remote[team].as_mut() {
            msg.jump = 0;
            msg.fire = 0;
            msg.skip = 0;
            msg.weapon = 255;
        }
    }
    control
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
