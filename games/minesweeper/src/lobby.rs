//! HTTP client for the multi-room lobby API (same contract as worms).

use serde::{Deserialize, Serialize};

/// Default lobby when `MINES_LOBBY` is unset (shared worms Coolify host).
pub const DEFAULT_LOBBY: &str = "https://worms.itknegaren.com";

/// Game id sent on create so the lobby spawns `minesweeper-dedicated`.
pub const GAME_ID: &str = "minesweeper";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoomInfo {
    pub id: String,
    pub name: String,
    pub players: u32,
    pub max_players: u32,
    pub status: String,
    pub join_url: String,
    #[serde(default)]
    pub game: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateRoom {
    pub id: String,
    pub name: String,
    pub join_url: String,
    pub leader_token: String,
}

#[derive(Serialize)]
struct CreateBody<'a> {
    name: &'a str,
    game: &'a str,
}

/// Resolve lobby base URL: `MINES_LOBBY` env, else [`DEFAULT_LOBBY`].
pub fn lobby_base_url() -> String {
    std::env::var("MINES_LOBBY").unwrap_or_else(|_| DEFAULT_LOBBY.to_string())
}

fn trim_base(base: &str) -> &str {
    base.trim_end_matches('/')
}

/// `GET /api/rooms?game=minesweeper`
pub fn list_rooms(base: &str) -> Result<Vec<RoomInfo>, String> {
    let url = format!("{}/api/rooms?game={}", trim_base(base), GAME_ID);
    let rooms: Vec<RoomInfo> = http_get_json(&url)?;
    Ok(rooms
        .into_iter()
        .filter(|r| r.game.as_deref() == Some(GAME_ID))
        .collect())
}

/// `POST /api/rooms` with `game=minesweeper` (legacy; prefer [`public_server`]).
#[allow(dead_code)]
pub fn create_room(base: &str, name: &str) -> Result<CreateRoom, String> {
    let url = format!("{}/api/rooms", trim_base(base));
    let body = CreateBody {
        name,
        game: GAME_ID,
    };
    http_post_json(&url, &body)
}

/// Always-on public BR server: `GET /api/minesweeper`.
pub fn public_server(base: &str) -> Result<RoomInfo, String> {
    let url = format!("{}/api/minesweeper", trim_base(base));
    http_get_json(&url)
}

#[cfg(not(target_arch = "wasm32"))]
fn http_get_json<T: for<'de> Deserialize<'de>>(url: &str) -> Result<T, String> {
    let resp = ureq::get(url)
        .timeout(std::time::Duration::from_secs(5))
        .call()
        .map_err(|e| format!("lobby GET {url}: {e}"))?;
    resp.into_json()
        .map_err(|e| format!("lobby decode: {e}"))
}

#[cfg(not(target_arch = "wasm32"))]
fn http_post_json<B: Serialize, T: for<'de> Deserialize<'de>>(
    url: &str,
    body: &B,
) -> Result<T, String> {
    let resp = ureq::post(url)
        .timeout(std::time::Duration::from_secs(8))
        .send_json(body)
        .map_err(|e| format!("lobby POST {url}: {e}"))?;
    resp.into_json()
        .map_err(|e| format!("lobby decode: {e}"))
}

#[cfg(target_arch = "wasm32")]
fn http_get_json<T: for<'de> Deserialize<'de>>(url: &str) -> Result<T, String> {
    let text = xhr("GET", url, None)?;
    serde_json::from_str(&text).map_err(|e| format!("lobby decode: {e}"))
}

#[cfg(target_arch = "wasm32")]
fn http_post_json<B: Serialize, T: for<'de> Deserialize<'de>>(
    url: &str,
    body: &B,
) -> Result<T, String> {
    let payload = serde_json::to_string(body).map_err(|e| e.to_string())?;
    let text = xhr("POST", url, Some(&payload))?;
    serde_json::from_str(&text).map_err(|e| format!("lobby decode: {e}"))
}

#[cfg(target_arch = "wasm32")]
fn xhr(method: &str, url: &str, body: Option<&str>) -> Result<String, String> {
    use wasm_bindgen::JsCast;
    let xhr = web_sys::XmlHttpRequest::new().map_err(|e| format!("{e:?}"))?;
    xhr.open_with_async(method, url, false)
        .map_err(|e| format!("xhr open: {e:?}"))?;
    if body.is_some() {
        xhr.set_request_header("Content-Type", "application/json")
            .map_err(|e| format!("xhr header: {e:?}"))?;
    }
    match body {
        Some(b) => xhr
            .send_with_opt_str(Some(b))
            .map_err(|e| format!("xhr send: {e:?}"))?,
        None => xhr.send().map_err(|e| format!("xhr send: {e:?}"))?,
    }
    let status = xhr.status().unwrap_or(0);
    if !(200..300).contains(&status) {
        return Err(format!("lobby HTTP {status}"));
    }
    xhr.response_text()
        .map_err(|e| format!("{e:?}"))?
        .ok_or_else(|| "empty lobby response".into())
}
