//! Browser WebSocket client (no listen socket).

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{BinaryType, ErrorEvent, MessageEvent, WebSocket};

use crate::core::net::{NetEvent, PeerId, MAX_INBOUND, MAX_PAYLOAD};

struct Shared {
    events: Vec<NetEvent>,
    open: bool,
    closed: bool,
}

/// Browser WebSocket driver owned by [`crate::core::net::Net`].
pub struct NetDriver {
    shared: Rc<RefCell<Shared>>,
    socket: Option<WebSocket>,
    // Keep closures alive for the socket lifetime.
    _on_open: Option<Closure<dyn FnMut(JsValue)>>,
    _on_message: Option<Closure<dyn FnMut(MessageEvent)>>,
    _on_close: Option<Closure<dyn FnMut(JsValue)>>,
    _on_error: Option<Closure<dyn FnMut(ErrorEvent)>>,
}

impl NetDriver {
    pub fn host(_port: u16) -> Result<Self, String> {
        Err("hosting is not available in the browser; run a desktop host".into())
    }

    pub fn connect(url: &str) -> Result<Self, String> {
        if !url.starts_with("ws://") && !url.starts_with("wss://") {
            return Err("URL must start with ws:// or wss://".into());
        }
        // From an https:// page the browser only allows ws:// to localhost, or wss://
        // anywhere. The desktop host still speaks plain ws:// — terminate TLS in front
        // (Caddy / nginx) when dialing a public host from GitHub Pages.

        let socket =
            WebSocket::new(url).map_err(|err| format!("net connect: {err:?}"))?;
        socket.set_binary_type(BinaryType::Arraybuffer);

        let shared = Rc::new(RefCell::new(Shared {
            events: Vec::new(),
            open: false,
            closed: false,
        }));

        let open_shared = Rc::clone(&shared);
        let on_open = Closure::wrap(Box::new(move |_ev: JsValue| {
            if let Ok(mut guard) = open_shared.try_borrow_mut() {
                guard.open = true;
                guard.events.push(NetEvent::Connected);
            }
        }) as Box<dyn FnMut(JsValue)>);
        socket.set_onopen(Some(on_open.as_ref().unchecked_ref()));

        let msg_shared = Rc::clone(&shared);
        let on_message = Closure::wrap(Box::new(move |ev: MessageEvent| {
            let Ok(mut guard) = msg_shared.try_borrow_mut() else {
                return;
            };
            let data = ev.data();
            let bytes = if let Ok(buf) = data.dyn_into::<js_sys::ArrayBuffer>() {
                js_sys::Uint8Array::new(&buf).to_vec()
            } else {
                return;
            };
            if bytes.len() > MAX_PAYLOAD {
                return;
            }
            push_message(&mut guard.events, PeerId::HOST, bytes);
        }) as Box<dyn FnMut(MessageEvent)>);
        socket.set_onmessage(Some(on_message.as_ref().unchecked_ref()));

        let close_shared = Rc::clone(&shared);
        let on_close = Closure::wrap(Box::new(move |_ev: JsValue| {
            if let Ok(mut guard) = close_shared.try_borrow_mut() {
                if !guard.closed {
                    guard.closed = true;
                    guard.open = false;
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
                    guard.open = false;
                    guard.events.push(NetEvent::Disconnected);
                }
            }
        }) as Box<dyn FnMut(ErrorEvent)>);
        socket.set_onerror(Some(on_error.as_ref().unchecked_ref()));

        Ok(Self {
            shared,
            socket: Some(socket),
            _on_open: Some(on_open),
            _on_message: Some(on_message),
            _on_close: Some(on_close),
            _on_error: Some(on_error),
        })
    }

    pub fn poll(&mut self) -> Vec<NetEvent> {
        match self.shared.try_borrow_mut() {
            Ok(mut guard) => std::mem::take(&mut guard.events),
            Err(_) => Vec::new(),
        }
    }

    pub fn send(&mut self, to: PeerId, bytes: Vec<u8>) -> Result<(), String> {
        if to != PeerId::HOST {
            return Err("client may only send to PeerId::HOST".into());
        }
        let Some(socket) = self.socket.as_ref() else {
            return Err("net is offline".into());
        };
        let open = self
            .shared
            .try_borrow()
            .map(|g| g.open)
            .unwrap_or(false);
        if !open {
            return Err("socket not open yet".into());
        }
        socket
            .send_with_u8_array(&bytes)
            .map_err(|err| format!("net send: {err:?}"))
    }

    pub fn broadcast(&mut self, _bytes: Vec<u8>) -> Result<(), String> {
        Err("client cannot broadcast".into())
    }

    pub fn peers(&self) -> Vec<PeerId> {
        let open = self
            .shared
            .try_borrow()
            .map(|g| g.open && !g.closed)
            .unwrap_or(false);
        if open {
            vec![PeerId::HOST]
        } else {
            Vec::new()
        }
    }

    pub fn shutdown(mut self) {
        if let Some(socket) = self.socket.take() {
            socket.set_onopen(None);
            socket.set_onmessage(None);
            socket.set_onclose(None);
            socket.set_onerror(None);
            let _ = socket.close();
        }
        self._on_open = None;
        self._on_message = None;
        self._on_close = None;
        self._on_error = None;
    }
}

fn push_message(events: &mut Vec<NetEvent>, from: PeerId, bytes: Vec<u8>) {
    let mut dropped = false;
    while events
        .iter()
        .filter(|e| matches!(e, NetEvent::Message { .. }))
        .count()
        >= MAX_INBOUND
    {
        if let Some(pos) = events
            .iter()
            .position(|e| matches!(e, NetEvent::Message { .. }))
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
    events.push(NetEvent::Message { from, bytes });
}
