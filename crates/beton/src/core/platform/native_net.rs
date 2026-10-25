//! Desktop WebSocket host / client (blocking threads + tungstenite).
//!
//! One I/O thread per peer multiplexes reads (with a short timeout) and writes
//! so the WebSocket is never locked by two threads at once.

use std::collections::HashMap;
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use tungstenite::{accept, client, Message, WebSocket};

use crate::core::net::{NetEvent, PeerId, MAX_INBOUND, MAX_PAYLOAD, MAX_PEERS};

enum Outbound {
    Bytes(Vec<u8>),
    Close,
}

struct PeerSlots {
    next_id: AtomicU32,
    writers: Mutex<HashMap<PeerId, Sender<Outbound>>>,
    streams: Mutex<Vec<TcpStream>>,
}

impl PeerSlots {
    fn new() -> Self {
        Self {
            next_id: AtomicU32::new(1),
            writers: Mutex::new(HashMap::new()),
            streams: Mutex::new(Vec::new()),
        }
    }

    fn peer_list(&self) -> Vec<PeerId> {
        self.writers
            .lock()
            .map(|map| {
                let mut ids: Vec<_> = map.keys().copied().collect();
                ids.sort_by_key(|id| id.0);
                ids
            })
            .unwrap_or_default()
    }

    fn register_stream(&self, stream: &TcpStream) {
        if let Ok(clone) = stream.try_clone() {
            if let Ok(mut guard) = self.streams.lock() {
                guard.push(clone);
            }
        }
    }

    fn send_to(&self, to: PeerId, bytes: Vec<u8>) -> Result<(), String> {
        let guard = self
            .writers
            .lock()
            .map_err(|_| "net peer lock poisoned".to_string())?;
        let tx = guard
            .get(&to)
            .ok_or_else(|| format!("unknown peer {to:?}"))?;
        tx.send(Outbound::Bytes(bytes))
            .map_err(|_| format!("peer {to:?} write channel closed"))
    }

    fn broadcast(&self, bytes: Vec<u8>) -> Result<(), String> {
        let guard = self
            .writers
            .lock()
            .map_err(|_| "net peer lock poisoned".to_string())?;
        for (_, tx) in guard.iter() {
            let _ = tx.send(Outbound::Bytes(bytes.clone()));
        }
        Ok(())
    }

    fn remove_writer(&self, id: PeerId) {
        if let Ok(mut guard) = self.writers.lock() {
            guard.remove(&id);
        }
    }

    fn close_all_writers(&self) {
        if let Ok(mut guard) = self.writers.lock() {
            for (_, tx) in guard.drain() {
                let _ = tx.send(Outbound::Close);
            }
        }
    }

    fn shutdown_streams(&self) {
        if let Ok(mut guard) = self.streams.lock() {
            for stream in guard.drain(..) {
                let _ = stream.shutdown(Shutdown::Both);
            }
        }
    }
}

fn push_event(events: &Mutex<Vec<NetEvent>>, event: NetEvent) {
    let Ok(mut queue) = events.lock() else {
        return;
    };
    if matches!(event, NetEvent::Message { .. }) {
        let mut dropped = false;
        while queue
            .iter()
            .filter(|e| matches!(e, NetEvent::Message { .. }))
            .count()
            >= MAX_INBOUND
        {
            if let Some(pos) = queue
                .iter()
                .position(|e| matches!(e, NetEvent::Message { .. }))
            {
                queue.remove(pos);
                dropped = true;
            } else {
                break;
            }
        }
        if dropped && !queue.iter().any(|e| matches!(e, NetEvent::Overflow)) {
            queue.push(NetEvent::Overflow);
        }
    }
    queue.push(event);
}

fn spawn_peer_io(
    peer: PeerId,
    stream: TcpStream,
    ws: WebSocket<TcpStream>,
    events: Arc<Mutex<Vec<NetEvent>>>,
    slots: Arc<PeerSlots>,
    stop: Arc<AtomicBool>,
    is_host: bool,
) -> JoinHandle<()> {
    let _ = ws
        .get_ref()
        .set_read_timeout(Some(Duration::from_millis(50)));
    slots.register_stream(&stream);
    let (tx, rx) = mpsc::channel::<Outbound>();
    if let Ok(mut guard) = slots.writers.lock() {
        guard.insert(peer, tx);
    }

    thread::spawn(move || {
        peer_io_loop(peer, ws, rx, events, slots, stop, is_host);
    })
}

fn peer_io_loop(
    peer: PeerId,
    mut ws: WebSocket<TcpStream>,
    rx: Receiver<Outbound>,
    events: Arc<Mutex<Vec<NetEvent>>>,
    slots: Arc<PeerSlots>,
    stop: Arc<AtomicBool>,
    is_host: bool,
) {
    let mut closing = false;
    while !stop.load(Ordering::SeqCst) && !closing {
        // Drain outbound first so sends stay responsive.
        loop {
            match rx.try_recv() {
                Ok(Outbound::Bytes(bytes)) => {
                    if ws.send(Message::Binary(bytes.into())).is_err() {
                        closing = true;
                        break;
                    }
                }
                Ok(Outbound::Close) => {
                    let _ = ws.close(None);
                    closing = true;
                    break;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    closing = true;
                    break;
                }
            }
        }
        if closing {
            break;
        }

        match ws.read() {
            Ok(Message::Binary(data)) => {
                let bytes: Vec<u8> = data.into();
                if bytes.len() > MAX_PAYLOAD {
                    continue;
                }
                let from = if is_host { peer } else { PeerId::HOST };
                push_event(&events, NetEvent::Message { from, bytes });
            }
            Ok(Message::Ping(data)) => {
                let _ = ws.send(Message::Pong(data));
            }
            Ok(Message::Pong(_)) | Ok(Message::Frame(_)) | Ok(Message::Text(_)) => {}
            Ok(Message::Close(_)) => {
                closing = true;
            }
            Err(tungstenite::Error::Io(err))
                if err.kind() == std::io::ErrorKind::WouldBlock
                    || err.kind() == std::io::ErrorKind::TimedOut => {}
            Err(_) => {
                closing = true;
            }
        }
    }

    slots.remove_writer(peer);
    let _ = ws.close(None);
    if is_host {
        push_event(&events, NetEvent::PeerLeft(peer));
    } else {
        push_event(&events, NetEvent::Disconnected);
        stop.store(true, Ordering::SeqCst);
    }
}

/// Native WebSocket driver owned by [`crate::core::net::Net`].
pub struct NetDriver {
    events: Arc<Mutex<Vec<NetEvent>>>,
    slots: Arc<PeerSlots>,
    stop: Arc<AtomicBool>,
    listener: Option<TcpListener>,
    /// Accept thread (host) or peer I/O threads (client). Peer threads spawned
    /// by the accept loop are joined there on shutdown.
    threads: Vec<JoinHandle<()>>,
    is_host: bool,
}

impl NetDriver {
    pub fn host(port: u16) -> Result<Self, String> {
        let listener = TcpListener::bind(("0.0.0.0", port))
            .map_err(|err| format!("net host bind: {err}"))?;
        let bound = listener
            .local_addr()
            .map_err(|err| format!("net host local_addr: {err}"))?
            .port();

        let events = Arc::new(Mutex::new(vec![NetEvent::Listening { port: bound }]));
        let slots = Arc::new(PeerSlots::new());
        let stop = Arc::new(AtomicBool::new(false));

        let accept_listener = listener
            .try_clone()
            .map_err(|err| format!("net host clone listener: {err}"))?;
        let accept_events = Arc::clone(&events);
        let accept_slots = Arc::clone(&slots);
        let accept_stop = Arc::clone(&stop);
        let accept_handle = thread::spawn(move || {
            accept_loop(accept_listener, accept_events, accept_slots, accept_stop);
        });

        Ok(Self {
            events,
            slots,
            stop,
            listener: Some(listener),
            threads: vec![accept_handle],
            is_host: true,
        })
    }

    pub fn connect(url: &str) -> Result<Self, String> {
        let (host, port, path) = parse_ws_url(url)?;
        let stream = TcpStream::connect((host.as_str(), port))
            .map_err(|err| format!("net connect tcp: {err}"))?;
        stream
            .set_nodelay(true)
            .map_err(|err| format!("net connect nodelay: {err}"))?;

        let stream_for_ws = stream
            .try_clone()
            .map_err(|err| format!("net connect clone: {err}"))?;
        let (ws, _response) =
            client::client(format!("ws://{host}:{port}{path}"), stream_for_ws)
                .map_err(|err| format!("net connect handshake: {err}"))?;

        let events = Arc::new(Mutex::new(vec![NetEvent::Connected]));
        let slots = Arc::new(PeerSlots::new());
        let stop = Arc::new(AtomicBool::new(false));
        let handle = spawn_peer_io(
            PeerId::HOST,
            stream,
            ws,
            Arc::clone(&events),
            Arc::clone(&slots),
            Arc::clone(&stop),
            false,
        );

        Ok(Self {
            events,
            slots,
            stop,
            listener: None,
            threads: vec![handle],
            is_host: false,
        })
    }

    pub fn poll(&mut self) -> Vec<NetEvent> {
        match self.events.lock() {
            Ok(mut guard) => std::mem::take(&mut *guard),
            Err(_) => Vec::new(),
        }
    }

    pub fn send(&mut self, to: PeerId, bytes: Vec<u8>) -> Result<(), String> {
        self.slots.send_to(to, bytes)
    }

    pub fn broadcast(&mut self, bytes: Vec<u8>) -> Result<(), String> {
        if !self.is_host {
            return Err("client cannot broadcast".into());
        }
        self.slots.broadcast(bytes)
    }

    pub fn peers(&self) -> Vec<PeerId> {
        if self.is_host {
            self.slots.peer_list()
        } else if self.slots.peer_list().is_empty() {
            Vec::new()
        } else {
            vec![PeerId::HOST]
        }
    }

    pub fn shutdown(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.slots.close_all_writers();
        self.slots.shutdown_streams();
        if let Some(listener) = self.listener.take() {
            // Wake the blocking accept. Use loopback — connecting to 0.0.0.0 fails
            // or hangs on Windows even when the listener bound that address.
            if let Ok(addr) = listener.local_addr() {
                let _ = TcpStream::connect(("127.0.0.1", addr.port()));
            }
            drop(listener);
        }
        for handle in self.threads.drain(..) {
            let _ = handle.join();
        }
    }
}

fn accept_loop(
    listener: TcpListener,
    events: Arc<Mutex<Vec<NetEvent>>>,
    slots: Arc<PeerSlots>,
    stop: Arc<AtomicBool>,
) {
    let mut peer_threads: Vec<JoinHandle<()>> = Vec::new();
    while !stop.load(Ordering::SeqCst) {
        let stream = match listener.accept() {
            Ok((stream, _)) => stream,
            Err(_) => break,
        };
        if stop.load(Ordering::SeqCst) {
            let _ = stream.shutdown(Shutdown::Both);
            break;
        }
        let peer_count = slots.writers.lock().map(|g| g.len()).unwrap_or(MAX_PEERS);
        if peer_count >= MAX_PEERS {
            let _ = stream.shutdown(Shutdown::Both);
            continue;
        }
        let stream_for_ws = match stream.try_clone() {
            Ok(s) => s,
            Err(_) => continue,
        };
        let ws = match accept(stream_for_ws) {
            Ok(ws) => ws,
            Err(_) => {
                let _ = stream.shutdown(Shutdown::Both);
                continue;
            }
        };
        let id = slots.next_id.fetch_add(1, Ordering::SeqCst);
        let peer = PeerId(id);
        // Register the write channel before PeerJoined so a same-frame send works.
        let handle = spawn_peer_io(
            peer,
            stream,
            ws,
            Arc::clone(&events),
            Arc::clone(&slots),
            Arc::clone(&stop),
            true,
        );
        push_event(&events, NetEvent::PeerJoined(peer));
        peer_threads.push(handle);
    }
    for handle in peer_threads {
        let _ = handle.join();
    }
}

fn parse_ws_url(url: &str) -> Result<(String, u16, String), String> {
    let rest = url
        .strip_prefix("ws://")
        .ok_or_else(|| "only ws:// URLs are supported (no TLS)".to_string())?;
    let (hostport, path) = match rest.split_once('/') {
        Some((hp, p)) => (hp, format!("/{p}")),
        None => (rest, "/".to_string()),
    };
    let (host, port) = if let Some((h, p)) = hostport.split_once(':') {
        let port: u16 = p
            .parse()
            .map_err(|_| format!("invalid port in URL: {url}"))?;
        (h.to_string(), port)
    } else {
        (hostport.to_string(), 80)
    };
    if host.is_empty() {
        return Err(format!("invalid host in URL: {url}"));
    }
    Ok((host, port, path))
}
