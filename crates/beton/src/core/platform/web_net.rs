//! Browser WebRTC client (WebSocket signaling + RTCPeerConnection).
//!
//! A tab cannot host. Signaling uses binary tags on the WebSocket; game bytes
//! use the `"rel"` and `"dg"` data channels.

use std::cell::RefCell;
use std::rc::Rc;

use js_sys::Uint8Array;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{
    BinaryType, ErrorEvent, MessageEvent, RtcConfiguration, RtcDataChannel, RtcDataChannelEvent,
    RtcIceCandidateInit, RtcPeerConnection, RtcPeerConnectionIceEvent, RtcSdpType,
    RtcSessionDescriptionInit, WebSocket,
};

use crate::core::net::{NetEvent, PeerId, MAX_INBOUND, MAX_PAYLOAD};

const SIGNAL_OFFER: u8 = 1;
const SIGNAL_ANSWER: u8 = 2;
const SIGNAL_CANDIDATE: u8 = 3;

const LABEL_REL: &str = "rel";
const LABEL_DG: &str = "dg";

struct Shared {
    events: Vec<NetEvent>,
    /// Both data channels open.
    ready: bool,
    closed: bool,
    rel: Option<RtcDataChannel>,
    dg: Option<RtcDataChannel>,
    rel_open: bool,
    dg_open: bool,
    /// Host trickle candidates arrive before `setRemoteDescription` finishes.
    remote_ready: bool,
    pending_candidates: Vec<String>,
    /// `a=mid:` from the remote offer — Chrome rejects a mismatched `sdpMid`.
    sdp_mid: Option<String>,
}

/// Closures that must outlive the peer connection / channels.
struct ClosureBag {
    channel_open: Vec<Closure<dyn FnMut(JsValue)>>,
    channel_msg: Vec<Closure<dyn FnMut(MessageEvent)>>,
}

/// Browser WebRTC driver owned by [`crate::core::net::Net`].
pub struct NetDriver {
    shared: Rc<RefCell<Shared>>,
    bags: Rc<RefCell<ClosureBag>>,
    socket: Option<WebSocket>,
    pc: Option<RtcPeerConnection>,
    _on_ws_open: Option<Closure<dyn FnMut(JsValue)>>,
    _on_ws_message: Option<Closure<dyn FnMut(MessageEvent)>>,
    _on_ws_close: Option<Closure<dyn FnMut(JsValue)>>,
    _on_ws_error: Option<Closure<dyn FnMut(ErrorEvent)>>,
    _on_ice: Option<Closure<dyn FnMut(RtcPeerConnectionIceEvent)>>,
    _on_datachannel: Option<Closure<dyn FnMut(RtcDataChannelEvent)>>,
}

impl NetDriver {
    pub fn host(_port: u16) -> Result<Self, String> {
        Err("hosting is not available in the browser; run a desktop host".into())
    }

    pub fn connect(url: &str) -> Result<Self, String> {
        if !url.starts_with("ws://") && !url.starts_with("wss://") {
            return Err("URL must start with ws:// or wss://".into());
        }

        let socket =
            WebSocket::new(url).map_err(|err| format!("net connect: {err:?}"))?;
        socket.set_binary_type(BinaryType::Arraybuffer);

        let shared = Rc::new(RefCell::new(Shared {
            events: Vec::new(),
            ready: false,
            closed: false,
            rel: None,
            dg: None,
            rel_open: false,
            dg_open: false,
            remote_ready: false,
            pending_candidates: Vec::new(),
            sdp_mid: None,
        }));
        let bags = Rc::new(RefCell::new(ClosureBag {
            channel_open: Vec::new(),
            channel_msg: Vec::new(),
        }));

        let config = RtcConfiguration::new();
        // Public STUN so browser clients can reach a Hetzner / internet host.
        let ice_servers = js_sys::Array::new();
        let stun = web_sys::RtcIceServer::new();
        let urls = js_sys::Array::of1(&JsValue::from_str("stun:stun.l.google.com:19302"));
        stun.set_urls(&urls);
        ice_servers.push(&stun);
        config.set_ice_servers(&ice_servers);
        let pc = RtcPeerConnection::new_with_configuration(&config)
            .map_err(|err| format!("RTCPeerConnection: {err:?}"))?;

        let ice_socket = socket.clone();
        let on_ice = Closure::wrap(Box::new(move |ev: RtcPeerConnectionIceEvent| {
            let Some(cand) = ev.candidate() else {
                return;
            };
            let cand_str = cand.candidate();
            if cand_str.is_empty() {
                return;
            }
            let frame = encode_signal(SIGNAL_CANDIDATE, cand_str.as_bytes());
            let _ = ice_socket.send_with_u8_array(&frame);
        }) as Box<dyn FnMut(RtcPeerConnectionIceEvent)>);
        pc.set_onicecandidate(Some(on_ice.as_ref().unchecked_ref()));

        let dc_shared = Rc::clone(&shared);
        let dc_bags = Rc::clone(&bags);
        let on_dc = Closure::wrap(Box::new(move |ev: RtcDataChannelEvent| {
            let channel = ev.channel();
            let label = channel.label();

            let open_shared = Rc::clone(&dc_shared);
            let label_open = label.clone();
            let on_open = Closure::wrap(Box::new(move |_ev: JsValue| {
                if let Ok(mut guard) = open_shared.try_borrow_mut() {
                    match label_open.as_str() {
                        LABEL_REL => guard.rel_open = true,
                        LABEL_DG => guard.dg_open = true,
                        _ => {}
                    }
                    if guard.rel_open && guard.dg_open && !guard.ready {
                        guard.ready = true;
                        guard.events.push(NetEvent::Connected);
                    }
                }
            }) as Box<dyn FnMut(JsValue)>);
            channel.set_onopen(Some(on_open.as_ref().unchecked_ref()));

            let msg_shared = Rc::clone(&dc_shared);
            let label_msg = label.clone();
            let on_msg = Closure::wrap(Box::new(move |ev: MessageEvent| {
                let Ok(mut guard) = msg_shared.try_borrow_mut() else {
                    return;
                };
                let data = ev.data();
                let bytes = if let Ok(buf) = data.dyn_into::<js_sys::ArrayBuffer>() {
                    Uint8Array::new(&buf).to_vec()
                } else {
                    return;
                };
                if bytes.len() > MAX_PAYLOAD {
                    return;
                }
                match label_msg.as_str() {
                    LABEL_REL => push_payload(&mut guard.events, false, bytes),
                    LABEL_DG => push_payload(&mut guard.events, true, bytes),
                    _ => {}
                }
            }) as Box<dyn FnMut(MessageEvent)>);
            channel.set_onmessage(Some(on_msg.as_ref().unchecked_ref()));

            if let Ok(mut guard) = dc_shared.try_borrow_mut() {
                match label.as_str() {
                    LABEL_REL => guard.rel = Some(channel),
                    LABEL_DG => guard.dg = Some(channel),
                    _ => {}
                }
            }
            if let Ok(mut bag) = dc_bags.try_borrow_mut() {
                bag.channel_open.push(on_open);
                bag.channel_msg.push(on_msg);
            }
        }) as Box<dyn FnMut(RtcDataChannelEvent)>);
        pc.set_ondatachannel(Some(on_dc.as_ref().unchecked_ref()));

        let on_open = Closure::wrap(Box::new(move |_ev: JsValue| {}) as Box<dyn FnMut(JsValue)>);
        socket.set_onopen(Some(on_open.as_ref().unchecked_ref()));

        let msg_pc = pc.clone();
        let msg_socket = socket.clone();
        let msg_shared = Rc::clone(&shared);
        let on_message = Closure::wrap(Box::new(move |ev: MessageEvent| {
            let data = ev.data();
            let bytes = if let Ok(buf) = data.dyn_into::<js_sys::ArrayBuffer>() {
                Uint8Array::new(&buf).to_vec()
            } else {
                return;
            };
            let Some((tag, payload)) = decode_signal(&bytes) else {
                return;
            };
            match tag {
                SIGNAL_OFFER => {
                    let Ok(sdp) = std::str::from_utf8(payload) else {
                        return;
                    };
                    let mid = extract_sdp_mid(sdp);
                    if let Ok(mut guard) = msg_shared.try_borrow_mut() {
                        guard.sdp_mid = mid;
                        guard.remote_ready = false;
                    }
                    let desc = RtcSessionDescriptionInit::new(RtcSdpType::Offer);
                    desc.set_sdp(sdp);
                    let set_remote = msg_pc.set_remote_description(&desc);
                    let pc2 = msg_pc.clone();
                    let sock2 = msg_socket.clone();
                    let shared2 = Rc::clone(&msg_shared);
                    wasm_bindgen_futures::spawn_local(async move {
                        if wasm_bindgen_futures::JsFuture::from(set_remote).await.is_err() {
                            return;
                        }
                        let pending = if let Ok(mut guard) = shared2.try_borrow_mut() {
                            guard.remote_ready = true;
                            std::mem::take(&mut guard.pending_candidates)
                        } else {
                            Vec::new()
                        };
                        let mid = shared2
                            .try_borrow()
                            .ok()
                            .and_then(|g| g.sdp_mid.clone());
                        for cand in pending {
                            add_remote_candidate(&pc2, &cand, mid.as_deref()).await;
                        }
                        let answer_promise = pc2.create_answer();
                        let Ok(answer_js) =
                            wasm_bindgen_futures::JsFuture::from(answer_promise).await
                        else {
                            return;
                        };
                        // Chrome resolves createAnswer to a plain SDP dict, not
                        // RTCSessionDescription — dyn_into would silently abort.
                        let Some(sdp) = sdp_from_description_js(&answer_js) else {
                            return;
                        };
                        let local = RtcSessionDescriptionInit::new(RtcSdpType::Answer);
                        local.set_sdp(&sdp);
                        let set_local = pc2.set_local_description(&local);
                        let _ = wasm_bindgen_futures::JsFuture::from(set_local).await;
                        let frame = encode_signal(SIGNAL_ANSWER, sdp.as_bytes());
                        let _ = sock2.send_with_u8_array(&frame);
                    });
                }
                SIGNAL_CANDIDATE => {
                    let Ok(cand_str) = std::str::from_utf8(payload) else {
                        return;
                    };
                    let cand = normalize_candidate(cand_str);
                    if cand.is_empty() {
                        return;
                    }
                    let (ready, mid) = match msg_shared.try_borrow_mut() {
                        Ok(mut guard) if !guard.remote_ready => {
                            guard.pending_candidates.push(cand);
                            return;
                        }
                        Ok(guard) => (guard.remote_ready, guard.sdp_mid.clone()),
                        Err(_) => return,
                    };
                    if !ready {
                        return;
                    }
                    let pc2 = msg_pc.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        add_remote_candidate(&pc2, &cand, mid.as_deref()).await;
                    });
                }
                _ => {}
            }
        }) as Box<dyn FnMut(MessageEvent)>);
        socket.set_onmessage(Some(on_message.as_ref().unchecked_ref()));

        let close_shared = Rc::clone(&shared);
        let on_close = Closure::wrap(Box::new(move |_ev: JsValue| {
            if let Ok(mut guard) = close_shared.try_borrow_mut() {
                if !guard.closed {
                    guard.closed = true;
                    guard.ready = false;
                    guard.events.push(NetEvent::Disconnected);
                }
            }
        }) as Box<dyn FnMut(JsValue)>);
        socket.set_onclose(Some(on_close.as_ref().unchecked_ref()));

        let err_shared = Rc::clone(&shared);
        let on_error = Closure::wrap(Box::new(move |_ev: ErrorEvent| {
            if let Ok(mut guard) = err_shared.try_borrow_mut() {
                if !guard.closed {
                    guard.closed = true;
                    guard.ready = false;
                    guard.events.push(NetEvent::Disconnected);
                }
            }
        }) as Box<dyn FnMut(ErrorEvent)>);
        socket.set_onerror(Some(on_error.as_ref().unchecked_ref()));

        Ok(Self {
            shared,
            bags,
            socket: Some(socket),
            pc: Some(pc),
            _on_ws_open: Some(on_open),
            _on_ws_message: Some(on_message),
            _on_ws_close: Some(on_close),
            _on_ws_error: Some(on_error),
            _on_ice: Some(on_ice),
            _on_datachannel: Some(on_dc),
        })
    }

    pub fn poll(&mut self) -> Vec<NetEvent> {
        match self.shared.try_borrow_mut() {
            Ok(mut guard) => std::mem::take(&mut guard.events),
            Err(_) => Vec::new(),
        }
    }

    pub fn send(&mut self, to: PeerId, bytes: Vec<u8>) -> Result<(), String> {
        self.send_on(to, bytes, false)
    }

    pub fn send_unreliable(&mut self, to: PeerId, bytes: Vec<u8>) -> Result<(), String> {
        self.send_on(to, bytes, true)
    }

    fn send_on(&mut self, to: PeerId, bytes: Vec<u8>, unreliable: bool) -> Result<(), String> {
        if to != PeerId::HOST {
            return Err("client may only send to PeerId::HOST".into());
        }
        let guard = self
            .shared
            .try_borrow()
            .map_err(|_| "net shared borrow".to_string())?;
        if !guard.ready {
            return Err("socket not open yet".into());
        }
        let channel = if unreliable {
            guard.dg.as_ref()
        } else {
            guard.rel.as_ref()
        }
        .ok_or_else(|| "data channel missing".to_string())?;
        channel
            .send_with_u8_array(&bytes)
            .map_err(|err| format!("net send: {err:?}"))
    }

    pub fn broadcast(&mut self, _bytes: Vec<u8>) -> Result<(), String> {
        Err("client cannot broadcast".into())
    }

    pub fn broadcast_unreliable(&mut self, _bytes: Vec<u8>) -> Result<(), String> {
        Err("client cannot broadcast".into())
    }

    pub fn peers(&self) -> Vec<PeerId> {
        let ready = self
            .shared
            .try_borrow()
            .map(|g| g.ready && !g.closed)
            .unwrap_or(false);
        if ready {
            vec![PeerId::HOST]
        } else {
            Vec::new()
        }
    }

    pub fn shutdown(mut self) {
        if let Ok(mut guard) = self.shared.try_borrow_mut() {
            if let Some(ch) = guard.rel.take() {
                ch.set_onopen(None);
                ch.set_onmessage(None);
                let _ = ch.close();
            }
            if let Some(ch) = guard.dg.take() {
                ch.set_onopen(None);
                ch.set_onmessage(None);
                let _ = ch.close();
            }
        }
        if let Some(pc) = self.pc.take() {
            pc.set_onicecandidate(None);
            pc.set_ondatachannel(None);
            let _ = pc.close();
        }
        if let Some(socket) = self.socket.take() {
            socket.set_onopen(None);
            socket.set_onmessage(None);
            socket.set_onclose(None);
            socket.set_onerror(None);
            let _ = socket.close();
        }
        self._on_ws_open = None;
        self._on_ws_message = None;
        self._on_ws_close = None;
        self._on_ws_error = None;
        self._on_ice = None;
        self._on_datachannel = None;
        if let Ok(mut bag) = self.bags.try_borrow_mut() {
            bag.channel_open.clear();
            bag.channel_msg.clear();
        }
    }
}

fn sdp_from_description_js(value: &JsValue) -> Option<String> {
    if let Ok(desc) = value.clone().dyn_into::<web_sys::RtcSessionDescription>() {
        let sdp = desc.sdp();
        if !sdp.is_empty() {
            return Some(sdp);
        }
    }
    let sdp = js_sys::Reflect::get(value, &JsValue::from_str("sdp")).ok()?;
    let sdp = sdp.as_string()?;
    (!sdp.is_empty()).then_some(sdp)
}

fn extract_sdp_mid(sdp: &str) -> Option<String> {
    sdp.lines().find_map(|line| {
        let line = line.trim();
        line.strip_prefix("a=mid:")
            .map(|m| m.trim().to_string())
            .filter(|m| !m.is_empty())
    })
}

fn normalize_candidate(raw: &str) -> String {
    raw.trim()
        .trim_start_matches("a=")
        .trim()
        .to_string()
}

async fn add_remote_candidate(pc: &RtcPeerConnection, cand: &str, mid: Option<&str>) {
    let init = RtcIceCandidateInit::new(cand);
    if let Some(mid) = mid {
        init.set_sdp_mid(Some(mid));
    } else {
        // Fallback when the offer has no mid — first media line.
        init.set_sdp_m_line_index(Some(0));
    }
    let promise = pc.add_ice_candidate_with_opt_rtc_ice_candidate_init(Some(&init));
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

fn encode_signal(tag: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(5 + payload.len());
    out.push(tag);
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(payload);
    out
}

fn decode_signal(bytes: &[u8]) -> Option<(u8, &[u8])> {
    if bytes.len() < 5 {
        return None;
    }
    let tag = bytes[0];
    let len = u32::from_le_bytes(bytes[1..5].try_into().ok()?) as usize;
    if bytes.len() != 5 + len {
        return None;
    }
    Some((tag, &bytes[5..]))
}

fn push_payload(events: &mut Vec<NetEvent>, unreliable: bool, bytes: Vec<u8>) {
    let mut dropped = false;
    while events
        .iter()
        .filter(|e| matches!(e, NetEvent::Message { .. } | NetEvent::Datagram { .. }))
        .count()
        >= MAX_INBOUND
    {
        if let Some(pos) = events
            .iter()
            .position(|e| matches!(e, NetEvent::Message { .. } | NetEvent::Datagram { .. }))
        {
            events.remove(pos);
            dropped = true;
        } else {
            break;
        }
    }
    if dropped && !events.iter().any(|e| matches!(e, NetEvent::Overflow)) {
        events.push(NetEvent::Overflow);
    }
    if unreliable {
        events.push(NetEvent::Datagram {
            from: PeerId::HOST,
            bytes,
        });
    } else {
        events.push(NetEvent::Message {
            from: PeerId::HOST,
            bytes,
        });
    }
}
