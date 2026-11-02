//! Multi-room lobby: list/create dedicated worms matches.
//!
//! Env:
//! - `LOBBY_BIND` (default `0.0.0.0:8080`)
//! - `PUBLIC_WS_BASE` (default `ws://127.0.0.1`) — join URLs use `{base}:{port}`
//!   or path form `{base}/{port}` when base ends with `/r` (e.g. `wss://host/r`)
//! - `DEDICATED_BIN` (default `worms-dedicated`)
//! - `PORT_MIN` / `PORT_MAX` (default 7800–7899)
//! - `ASSETS_DIR` optional path passed to dedicated
//!
//! HTTPS clients (GitHub Pages) need `PUBLIC_WS_BASE=wss://worms.itknegaren.com/r`.
//! Traefik terminates TLS on 443; this process proxies `/r/{port}` → `ws://127.0.0.1:{port}`.

use std::collections::HashMap;
use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::ws::{Message as AxumMessage, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::Message as TungsteniteMessage;
use tower_http::cors::CorsLayer;
use uuid::Uuid;

#[derive(Clone)]
struct App {
    inner: Arc<Mutex<Inner>>,
}

struct Inner {
    rooms: HashMap<String, Room>,
    public_ws_base: String,
    dedicated_bin: PathBuf,
    mines_dedicated_bin: PathBuf,
    assets_dir: Option<PathBuf>,
    port_min: u16,
    port_max: u16,
}

struct Room {
    id: String,
    name: String,
    port: u16,
    #[allow(dead_code)]
    leader_token: String,
    players: u32,
    max_players: u32,
    status: String,
    game: String,
    /// Always-on public server (not reaped for empty/stale; respawned if child dies).
    permanent: bool,
    child: Child,
    last_hb: Instant,
    created: Instant,
}

#[derive(Serialize)]
struct RoomInfo {
    id: String,
    name: String,
    players: u32,
    max_players: u32,
    status: String,
    join_url: String,
    game: String,
}

#[derive(Serialize)]
struct CreateResp {
    id: String,
    name: String,
    join_url: String,
    leader_token: String,
}

#[derive(Deserialize)]
struct CreateReq {
    #[serde(default = "default_name")]
    name: String,
    #[serde(default = "default_game")]
    game: String,
}

fn default_name() -> String {
    "worms".into()
}

fn default_game() -> String {
    "worms".into()
}

#[derive(Deserialize)]
struct HeartbeatReq {
    players: u32,
    status: String,
}

#[tokio::main]
async fn main() {
    let bind = env::var("LOBBY_BIND").unwrap_or_else(|_| "0.0.0.0:8080".into());
    let public_ws_base = env::var("PUBLIC_WS_BASE").unwrap_or_else(|_| "ws://127.0.0.1".into());
    let dedicated_bin = PathBuf::from(
        env::var("DEDICATED_BIN").unwrap_or_else(|_| "worms-dedicated".into()),
    );
    let mines_dedicated_bin = PathBuf::from(
        env::var("MINES_DEDICATED_BIN").unwrap_or_else(|_| "minesweeper-dedicated".into()),
    );
    let assets_dir = env::var("ASSETS_DIR").ok().map(PathBuf::from);
    let port_min: u16 = env::var("PORT_MIN")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(7800);
    let port_max: u16 = env::var("PORT_MAX")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(7999);

    let app = App {
        inner: Arc::new(Mutex::new(Inner {
            rooms: HashMap::new(),
            public_ws_base: public_ws_base.trim_end_matches('/').to_string(),
            dedicated_bin,
            mines_dedicated_bin,
            assets_dir,
            port_min,
            port_max,
        })),
    };

    let reaper = app.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            reap(&reaper).await;
        }
    });

    {
        let boot = app.clone();
        tokio::spawn(async move {
            // Give the HTTP server a moment, then ensure the public mines server.
            tokio::time::sleep(Duration::from_millis(200)).await;
            if let Err(e) = ensure_public_minesweeper(&boot).await {
                eprintln!("public minesweeper: {e}");
            }
        });
    }

    let router = Router::new()
        .route("/api/rooms", get(list_rooms).post(create_room))
        .route("/api/rooms/{id}", get(get_room))
        .route("/api/rooms/{id}/heartbeat", post(heartbeat))
        .route("/api/minesweeper", get(public_minesweeper))
        .route("/r/{port}", get(room_ws))
        .layer(CorsLayer::permissive())
        .with_state(app);

    let addr: SocketAddr = bind.parse().expect("LOBBY_BIND");
    eprintln!("worms-lobby listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind");
    axum::serve(listener, router).await.expect("serve");
}

const PUBLIC_MINES_ID: &str = "mines-public";

fn mines_public_port() -> u16 {
    env::var("MINES_PUBLIC_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(7900)
}

async fn ensure_public_minesweeper(app: &App) -> Result<(), String> {
    let mut inner = app.inner.lock().await;
    if inner.rooms.contains_key(PUBLIC_MINES_ID) {
        return Ok(());
    }
    let port = mines_public_port();
    if !(inner.port_min..=inner.port_max).contains(&port) {
        return Err(format!(
            "MINES_PUBLIC_PORT {port} outside {}-{}",
            inner.port_min, inner.port_max
        ));
    }
    if inner.rooms.values().any(|r| r.port == port) {
        return Err(format!("port {port} already in use"));
    }
    spawn_room_locked(
        &mut inner,
        PUBLIC_MINES_ID.to_string(),
        "Minesweeper BR".into(),
        "minesweeper".into(),
        port,
        true,
        50,
    )?;
    eprintln!("public minesweeper on :{port}");
    Ok(())
}

async fn public_minesweeper(State(app): State<App>) -> Result<Json<RoomInfo>, StatusCode> {
    {
        let inner = app.inner.lock().await;
        if let Some(r) = inner.rooms.get(PUBLIC_MINES_ID) {
            return Ok(Json(RoomInfo {
                id: r.id.clone(),
                name: r.name.clone(),
                players: r.players,
                max_players: r.max_players,
                status: r.status.clone(),
                join_url: join_url(&inner.public_ws_base, r.port),
                game: r.game.clone(),
            }));
        }
    }
    ensure_public_minesweeper(&app)
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    let inner = app.inner.lock().await;
    let r = inner
        .rooms
        .get(PUBLIC_MINES_ID)
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    Ok(Json(RoomInfo {
        id: r.id.clone(),
        name: r.name.clone(),
        players: r.players,
        max_players: r.max_players,
        status: r.status.clone(),
        join_url: join_url(&inner.public_ws_base, r.port),
        game: r.game.clone(),
    }))
}

fn spawn_room_locked(
    inner: &mut Inner,
    id: String,
    name: String,
    game: String,
    port: u16,
    permanent: bool,
    max_players: u32,
) -> Result<(String, String), String> {
    let lobby_url = env::var("LOBBY_PUBLIC_HTTP")
        .unwrap_or_else(|_| format!("http://127.0.0.1:{}", lobby_http_port()));
    let leader_token = if permanent {
        String::new()
    } else {
        Uuid::new_v4().to_string()
    };

    let bin = if game == "minesweeper" {
        inner.mines_dedicated_bin.clone()
    } else {
        inner.dedicated_bin.clone()
    };

    let mut cmd = Command::new(&bin);
    cmd.arg("--port")
        .arg(port.to_string())
        .arg("--room-id")
        .arg(&id)
        .arg("--leader-token")
        .arg(&leader_token)
        .arg("--lobby")
        .arg(&lobby_url)
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);
    if game == "minesweeper" {
        cmd.arg("--persistent");
    } else if let Some(assets) = &inner.assets_dir {
        cmd.arg("--assets").arg(assets);
    }

    let child = cmd
        .spawn()
        .map_err(|e| format!("spawn dedicated: {e}"))?;

    let join = join_url(&inner.public_ws_base, port);
    inner.rooms.insert(
        id.clone(),
        Room {
            id: id.clone(),
            name,
            port,
            leader_token: leader_token.clone(),
            players: 0,
            max_players,
            status: "lobby".into(),
            game,
            permanent,
            child,
            last_hb: Instant::now(),
            created: Instant::now(),
        },
    );
    Ok((join, leader_token))
}

fn join_url(base: &str, port: u16) -> String {
    // PUBLIC_WS_BASE is either `ws://host` / `wss://host` or path prefix ending in `/r`.
    let base = base.trim_end_matches('/');
    if base.ends_with("/r") {
        format!("{base}/{port}")
    } else {
        format!("{base}:{port}")
    }
}

async fn room_ws(
    State(app): State<App>,
    Path(port): Path<u16>,
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse, StatusCode> {
    {
        let inner = app.inner.lock().await;
        if port < inner.port_min || port > inner.port_max {
            return Err(StatusCode::BAD_REQUEST);
        }
        if !inner.rooms.values().any(|r| r.port == port) {
            return Err(StatusCode::NOT_FOUND);
        }
    }
    Ok(ws.on_upgrade(move |socket| proxy_room_ws(socket, port)))
}

async fn proxy_room_ws(client: WebSocket, port: u16) {
    let url = format!("ws://127.0.0.1:{port}");
    let Ok((server, _)) = tokio_tungstenite::connect_async(&url).await else {
        return;
    };
    let (mut client_tx, mut client_rx) = client.split();
    let (mut server_tx, mut server_rx) = server.split();

    let client_to_server = async {
        while let Some(Ok(msg)) = client_rx.next().await {
            let mapped = match msg {
                AxumMessage::Binary(b) => TungsteniteMessage::Binary(b),
                AxumMessage::Text(t) => TungsteniteMessage::Text(t.to_string().into()),
                AxumMessage::Ping(p) => TungsteniteMessage::Ping(p),
                AxumMessage::Pong(p) => TungsteniteMessage::Pong(p),
                AxumMessage::Close(_) => break,
            };
            if server_tx.send(mapped).await.is_err() {
                break;
            }
        }
    };
    let server_to_client = async {
        while let Some(Ok(msg)) = server_rx.next().await {
            let mapped = match msg {
                TungsteniteMessage::Binary(b) => AxumMessage::Binary(b),
                TungsteniteMessage::Text(t) => AxumMessage::Text(t.to_string().into()),
                TungsteniteMessage::Ping(p) => AxumMessage::Ping(p),
                TungsteniteMessage::Pong(p) => AxumMessage::Pong(p),
                TungsteniteMessage::Close(_) | TungsteniteMessage::Frame(_) => break,
            };
            if client_tx.send(mapped).await.is_err() {
                break;
            }
        }
    };
    tokio::select! {
        _ = client_to_server => {}
        _ = server_to_client => {}
    }
}

#[derive(Deserialize, Default)]
struct ListQuery {
    game: Option<String>,
}

async fn list_rooms(
    State(app): State<App>,
    axum::extract::Query(query): axum::extract::Query<ListQuery>,
) -> Json<Vec<RoomInfo>> {
    let inner = app.inner.lock().await;
    let mut out: Vec<_> = inner
        .rooms
        .values()
        .filter(|r| {
            query
                .game
                .as_ref()
                .map(|g| &r.game == g)
                .unwrap_or(true)
        })
        .map(|r| RoomInfo {
            id: r.id.clone(),
            name: r.name.clone(),
            players: r.players,
            max_players: r.max_players,
            status: r.status.clone(),
            join_url: join_url(&inner.public_ws_base, r.port),
            game: r.game.clone(),
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Json(out)
}

async fn get_room(
    State(app): State<App>,
    Path(id): Path<String>,
) -> Result<Json<RoomInfo>, StatusCode> {
    let inner = app.inner.lock().await;
    let r = inner.rooms.get(&id).ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(RoomInfo {
        id: r.id.clone(),
        name: r.name.clone(),
        players: r.players,
        max_players: r.max_players,
        status: r.status.clone(),
        join_url: join_url(&inner.public_ws_base, r.port),
        game: r.game.clone(),
    }))
}

async fn create_room(
    State(app): State<App>,
    Json(req): Json<CreateReq>,
) -> Result<Json<CreateResp>, (StatusCode, String)> {
    let mut inner = app.inner.lock().await;
    let game = if req.game.trim().is_empty() {
        "worms".to_string()
    } else {
        req.game.trim().to_string()
    };
    // Minesweeper uses the always-on public server — don't spawn player-hosted rooms.
    if game == "minesweeper" {
        drop(inner);
        let info = public_minesweeper(State(app))
            .await
            .map_err(|s| (s, "public minesweeper unavailable".into()))?;
        let info = info.0;
        return Ok(Json(CreateResp {
            id: info.id,
            name: info.name,
            join_url: info.join_url,
            leader_token: String::new(),
        }));
    }
    let port = next_port(&inner).ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "no free room ports".into(),
    ))?;
    let id = Uuid::new_v4().to_string();
    let name = if req.name.trim().is_empty() {
        format!("room-{}", &id[..8])
    } else {
        req.name.trim().to_string()
    };
    let max_players = 3u32;
    let (join, leader_token) = spawn_room_locked(
        &mut inner,
        id.clone(),
        name.clone(),
        game.clone(),
        port,
        false,
        max_players,
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    eprintln!("spawned {game} room {id} on :{port}");
    Ok(Json(CreateResp {
        id,
        name,
        join_url: join,
        leader_token,
    }))
}

async fn heartbeat(
    State(app): State<App>,
    Path(id): Path<String>,
    Json(body): Json<HeartbeatReq>,
) -> StatusCode {
    let mut inner = app.inner.lock().await;
    if let Some(room) = inner.rooms.get_mut(&id) {
        room.players = body.players;
        room.status = body.status;
        room.last_hb = Instant::now();
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}

fn next_port(inner: &Inner) -> Option<u16> {
    let used: std::collections::HashSet<u16> = inner.rooms.values().map(|r| r.port).collect();
    (inner.port_min..=inner.port_max).find(|p| !used.contains(p))
}

fn lobby_http_port() -> u16 {
    env::var("LOBBY_BIND")
        .ok()
        .and_then(|b| b.rsplit(':').next()?.parse().ok())
        .unwrap_or(8080)
}

async fn reap(app: &App) {
    let mut to_respawn: Vec<(String, String, String, u16, u32)> = Vec::new();
    let mut dead = Vec::new();
    {
        let mut inner = app.inner.lock().await;
        for (id, room) in inner.rooms.iter_mut() {
            let exited = room.child.try_wait().ok().flatten().is_some();
            if room.permanent {
                if exited {
                    eprintln!("public room {id} exited — respawning");
                    to_respawn.push((
                        room.id.clone(),
                        room.name.clone(),
                        room.game.clone(),
                        room.port,
                        room.max_players,
                    ));
                    dead.push(id.clone());
                }
                continue;
            }
            let stale = room.last_hb.elapsed() > Duration::from_secs(15)
                && room.created.elapsed() > Duration::from_secs(20);
            let empty = room.players == 0 && room.created.elapsed() > Duration::from_secs(30);
            if stale || empty || exited {
                let _ = room.child.start_kill();
                dead.push(id.clone());
            }
        }
        for id in &dead {
            eprintln!("reaping room {id}");
            inner.rooms.remove(id);
        }
    }
    for (id, name, game, port, max_players) in to_respawn {
        let mut inner = app.inner.lock().await;
        if let Err(e) = spawn_room_locked(&mut inner, id, name, game, port, true, max_players) {
            eprintln!("respawn failed: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::join_url;

    #[test]
    fn join_url_host_port() {
        assert_eq!(join_url("ws://127.0.0.1", 7801), "ws://127.0.0.1:7801");
    }

    #[test]
    fn join_url_path_prefix() {
        assert_eq!(
            join_url("wss://worms.example.com/r/", 7801),
            "wss://worms.example.com/r/7801"
        );
        assert_eq!(
            join_url("wss://worms.example.com/r", 7801),
            "wss://worms.example.com/r/7801"
        );
    }
}
