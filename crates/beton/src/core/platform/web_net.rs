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
        }));
        let bags = Rc::new(RefCell::new(ClosureBag {
            channel_open: Vec::new(),
            channel_msg: Vec::new(),
        }));

        let config = RtcConfiguration::new();
        // Empty iceServers: LAN / localhost only (host advertises real candidates).
        config.set_ice_servers(&js_sys::Array::new());
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
                    let desc = RtcSessionDescriptionInit::new(RtcSdpType::Offer);
                    desc.set_sdp(sdp);
                    let set_remote = msg_pc.set_remote_description(&desc);
                    let pc2 = msg_pc.clone();
                    let sock2 = msg_socket.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        if wasm_bindgen_futures::JsFuture::from(set_remote).await.is_err() {
                            return;
                        }
                        let answer_promise = pc2.create_answer();
                        let Ok(answer_js) =
                            wasm_bindgen_futures::JsFuture::from(answer_promise).await
                        else {
                            return;
                        };
                        let Ok(answer) = answer_js.dyn_into::<web_sys::RtcSessionDescription>()
                        else {
                            return;
                        };
                        let sdp = answer.sdp();
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
                    let init = RtcIceCandidateInit::new(cand_str);
                    init.set_sdp_mid(Some("0"));
                    let _ = msg_pc.add_ice_candidate_with_opt_rtc_ice_candidate_init(Some(&init));
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
