//! HTTP client for the multi-room lobby API.

use serde::{Deserialize, Serialize};

/// Default lobby when `WORMS_LOBBY` is unset (Hetzner / Coolify).
pub const DEFAULT_LOBBY: &str = "https://worms.itknegaren.com";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoomInfo {
    pub id: String,
    pub name: String,
    pub players: u32,
    pub max_players: u32,
    pub status: String,
    pub join_url: String,
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
}

/// Resolve lobby base URL: `WORMS_LOBBY` env, else [`DEFAULT_LOBBY`].
pub fn lobby_base_url() -> String {
    std::env::var("WORMS_LOBBY").unwrap_or_else(|_| DEFAULT_LOBBY.to_string())
}

fn trim_base(base: &str) -> &str {
    base.trim_end_matches('/')
}

/// `GET /api/rooms`
pub fn list_rooms(base: &str) -> Result<Vec<RoomInfo>, String> {
    let url = format!("{}/api/rooms", trim_base(base));
    http_get_json(&url)
}

/// `POST /api/rooms`
pub fn create_room(base: &str, name: &str) -> Result<CreateRoom, String> {
    let url = format!("{}/api/rooms", trim_base(base));
    let body = CreateBody { name };
    http_post_json(&url, &body)
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
