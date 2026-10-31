//! Desktop WebRTC data channels with WebSocket signaling (blocking threads).
//!
//! Signaling frames are binary tags on the WebSocket. Game bytes ride two SCTP
//! channels over UDP: `"rel"` (reliable ordered) and `"dg"` (unreliable unordered).

use std::collections::HashMap;
use std::io::ErrorKind;
use std::net::{IpAddr, Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use str0m::change::{SdpAnswer, SdpOffer, SdpPendingOffer};
use str0m::channel::{ChannelConfig, ChannelId, Reliability};
use str0m::net::{Protocol, Receive};
use str0m::{Candidate, Event, IceConnectionState, Input, Output, Rtc};
use tungstenite::{accept, client, Message, WebSocket};

use crate::core::net::{NetEvent, PeerId, MAX_INBOUND, MAX_PAYLOAD, MAX_PEERS};

const SIGNAL_OFFER: u8 = 1;
const SIGNAL_ANSWER: u8 = 2;
const SIGNAL_CANDIDATE: u8 = 3;

const LABEL_REL: &str = "rel";
const LABEL_DG: &str = "dg";

enum Outbound {
    Reliable(Vec<u8>),
    Unreliable(Vec<u8>),
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

    fn send_to(&self, to: PeerId, msg: Outbound) -> Result<(), String> {
        let guard = self
            .writers
            .lock()
            .map_err(|_| "net peer lock poisoned".to_string())?;
        let tx = guard
            .get(&to)
            .ok_or_else(|| format!("unknown peer {to:?}"))?;
        tx.send(msg)
            .map_err(|_| format!("peer {to:?} write channel closed"))
    }

    fn broadcast(&self, unreliable: bool, bytes: Vec<u8>) -> Result<(), String> {
        let guard = self
            .writers
            .lock()
            .map_err(|_| "net peer lock poisoned".to_string())?;
        for (_, tx) in guard.iter() {
            let msg = if unreliable {
                Outbound::Unreliable(bytes.clone())
            } else {
                Outbound::Reliable(bytes.clone())
            };
            let _ = tx.send(msg);
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

fn is_payload_event(event: &NetEvent) -> bool {
    matches!(event, NetEvent::Message { .. } | NetEvent::Datagram { .. })
}

fn push_event(events: &Mutex<Vec<NetEvent>>, event: NetEvent) {
    let Ok(mut queue) = events.lock() else {
        return;
    };
    if is_payload_event(&event) {
        let mut dropped = false;
        while queue.iter().filter(|e| is_payload_event(e)).count() >= MAX_INBOUND {
            if let Some(pos) = queue.iter().position(|e| is_payload_event(e)) {
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

fn local_ipv4s() -> Vec<Ipv4Addr> {
    let mut ips = vec![Ipv4Addr::LOCALHOST];
    if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
        if socket.connect("8.8.8.8:80").is_ok() {
            if let Ok(SocketAddr::V4(v4)) = socket.local_addr() {
                let ip = *v4.ip();
                if !ip.is_unspecified() && !ips.contains(&ip) {
                    ips.push(ip);
                }
            }
        }
    }
    ips
}

fn add_host_candidates(rtc: &mut Rtc, port: u16) -> Vec<String> {
    let mut signaled = Vec::new();
    for ip in local_ipv4s() {
        let addr = SocketAddr::new(IpAddr::V4(ip), port);
        if let Ok(candidate) = Candidate::host(addr, "udp") {
            if let Some(accepted) = rtc.add_local_candidate(candidate) {
                signaled.push(accepted.to_sdp_string());
            }
        }
    }
    signaled
}

fn receive_destination(local_port: u16, source: SocketAddr) -> SocketAddr {
    // Packets arrive on a 0.0.0.0 bind; str0m matches against host-candidate IPs.
    if source.ip().is_loopback() {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), local_port)
    } else if let IpAddr::V4(src) = source.ip() {
        // Prefer the interface IP we advertised, else the packet's source-side view.
        let ips = local_ipv4s();
        let ip = ips
            .iter()
            .copied()
            .find(|ip| !ip.is_loopback())
            .unwrap_or(src);
        SocketAddr::new(IpAddr::V4(ip), local_port)
    } else {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), local_port)
    }
}

fn resolve_mdns_host(host: &str) -> Option<IpAddr> {
    if !host.ends_with(".local") && !host.contains(".local:") {
        return None;
    }
    let name = host.split(':').next().unwrap_or(host);
    std::net::ToSocketAddrs::to_socket_addrs(&(name, 0))
        .ok()?
        .next()
        .map(|a| a.ip())
}

fn parse_remote_candidate(raw: &str) -> Option<Candidate> {
    let trimmed = raw.trim().trim_start_matches("a=");
    if let Ok(c) = Candidate::from_sdp_string(trimmed) {
        return Some(c);
    }
    // Chrome may hide LAN IPs behind *.local — rewrite the host token and retry.
    let mut parts: Vec<String> = trimmed
        .split_whitespace()
        .map(str::to_string)
        .collect();
    // candidate:<foundation> <component> <proto> <prio> <ip> <port> typ ...
    if parts.len() < 6 {
        return None;
    }
    let ip = resolve_mdns_host(&parts[4])?;
    parts[4] = ip.to_string();
    Candidate::from_sdp_string(&parts.join(" ")).ok()
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
        .set_read_timeout(Some(Duration::from_millis(20)));
    slots.register_stream(&stream);
    let (tx, rx) = mpsc::channel::<Outbound>();
    if let Ok(mut guard) = slots.writers.lock() {
        guard.insert(peer, tx);
    }

    thread::spawn(move || {
        peer_io_loop(peer, ws, rx, events, slots, stop, is_host);
    })
}

struct ChannelMap {
    rel: Option<ChannelId>,
    dg: Option<ChannelId>,
    ready_announced: bool,
}

impl ChannelMap {
    fn new() -> Self {
        Self {
            rel: None,
            dg: None,
            ready_announced: false,
        }
    }

    fn both_open(&self) -> bool {
        self.rel.is_some() && self.dg.is_some()
    }

    fn on_open(&mut self, id: ChannelId, label: &str) {
        match label {
            LABEL_REL => self.rel = Some(id),
            LABEL_DG => self.dg = Some(id),
            _ => {}
        }
    }
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
    let from = if is_host { peer } else { PeerId::HOST };
    let result = run_webrtc_peer(&mut ws, &rx, &events, stop.as_ref(), is_host, from, peer);

    slots.remove_writer(peer);
    let _ = ws.close(None);
    match result {
        Ok(()) | Err(_) => {
            if is_host {
                push_event(&events, NetEvent::PeerLeft(peer));
            } else {
                push_event(&events, NetEvent::Disconnected);
                stop.store(true, Ordering::SeqCst);
            }
        }
    }
}

fn run_webrtc_peer(
    ws: &mut WebSocket<TcpStream>,
    rx: &Receiver<Outbound>,
    events: &Mutex<Vec<NetEvent>>,
    stop: &AtomicBool,
    is_host: bool,
    from: PeerId,
    peer: PeerId,
) -> Result<(), String> {
    let udp = UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("udp bind: {e}"))?;
    udp.set_nonblocking(true)
        .map_err(|e| format!("udp nonblocking: {e}"))?;
    let local_port = udp
        .local_addr()
        .map_err(|e| format!("udp local_addr: {e}"))?
        .port();

    let mut rtc = Rtc::new(Instant::now());
    let local_candidates = add_host_candidates(&mut rtc, local_port);

    let mut pending: Option<SdpPendingOffer> = None;
    let mut channels = ChannelMap::new();
    let mut closing = false;
    let mut buf = vec![0u8; 2000];

    if is_host {
        let mut change = rtc.sdp_api();
        change.add_channel(LABEL_REL.to_string());
        change.add_channel_with_config(ChannelConfig {
            label: LABEL_DG.to_string(),
            ordered: false,
            reliability: Reliability::MaxRetransmits { retransmits: 0 },
            ..Default::default()
        });
        let (offer, pend) = change
            .apply()
            .ok_or_else(|| "expected SDP offer after adding channels".to_string())?;
        pending = Some(pend);
        let sdp = offer.to_sdp_string();
        ws.send(Message::Binary(encode_signal(SIGNAL_OFFER, sdp.as_bytes()).into()))
            .map_err(|e| format!("send offer: {e}"))?;
        for cand in &local_candidates {
            let _ = ws.send(Message::Binary(
                encode_signal(SIGNAL_CANDIDATE, cand.as_bytes()).into(),
            ));
        }
    }

    while !stop.load(Ordering::SeqCst) && !closing {
        // Outbound game bytes (buffered until channels open).
        loop {
            match rx.try_recv() {
                Ok(Outbound::Reliable(bytes)) => {
                    if let Some(id) = channels.rel {
                        if let Some(mut ch) = rtc.channel(id) {
                            let _ = ch.write(true, &bytes);
                        }
                    }
                }
                Ok(Outbound::Unreliable(bytes)) => {
                    if let Some(id) = channels.dg {
                        if let Some(mut ch) = rtc.channel(id) {
                            let _ = ch.write(true, &bytes);
                        }
                    }
                }
                Ok(Outbound::Close) => {
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

        // Drain str0m output.
        let mut timeout = Duration::from_millis(20);
        loop {
            match rtc.poll_output() {
                Ok(Output::Timeout(t)) => {
                    let now = Instant::now();
                    timeout = t.saturating_duration_since(now).min(Duration::from_millis(20));
                    break;
                }
                Ok(Output::Transmit(t)) => {
                    let _ = udp.send_to(&t.contents, t.destination);
                }
                Ok(Output::Event(ev)) => {
                    handle_rtc_event(
                        ev,
                        &mut channels,
                        events,
                        is_host,
                        from,
                        peer,
                        &mut closing,
                    );
                }
                Err(_) => {
                    closing = true;
                    break;
                }
            }
        }
        if closing {
            break;
        }

        // UDP input.
        loop {
            match udp.recv_from(&mut buf) {
                Ok((n, source)) => {
                    let destination = receive_destination(local_port, source);
                    let Ok(contents) = buf[..n].try_into() else {
                        continue;
                    };
                    let recv = Receive {
                        proto: Protocol::Udp,
                        source,
                        destination,
                        contents,
                    };
                    let _ = rtc.handle_input(Input::Receive(Instant::now(), recv));
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::TimedOut => {
                    break;
                }
                Err(_) => {
                    closing = true;
                    break;
                }
            }
        }
        if closing {
            break;
        }

        // Signaling WebSocket.
        match ws.read() {
            Ok(Message::Binary(data)) => {
                let bytes: Vec<u8> = data.into();
                if let Some((tag, payload)) = decode_signal(&bytes) {
                    match tag {
                        SIGNAL_OFFER if !is_host => {
                            let sdp = std::str::from_utf8(payload)
                                .map_err(|e| format!("offer utf8: {e}"))?;
                            let offer = SdpOffer::from_sdp_string(sdp)
                                .map_err(|e| format!("parse offer: {e}"))?;
                            let answer = rtc
                                .sdp_api()
                                .accept_offer(offer)
                                .map_err(|e| format!("accept offer: {e}"))?;
                            let ans = answer.to_sdp_string();
                            ws.send(Message::Binary(
                                encode_signal(SIGNAL_ANSWER, ans.as_bytes()).into(),
                            ))
                            .map_err(|e| format!("send answer: {e}"))?;
                            for cand in &local_candidates {
                                let _ = ws.send(Message::Binary(
                                    encode_signal(SIGNAL_CANDIDATE, cand.as_bytes()).into(),
                                ));
                            }
                        }
                        SIGNAL_ANSWER if is_host => {
                            let sdp = std::str::from_utf8(payload)
                                .map_err(|e| format!("answer utf8: {e}"))?;
                            let answer = SdpAnswer::from_sdp_string(sdp)
                                .map_err(|e| format!("parse answer: {e}"))?;
                            let pend = pending
                                .take()
                                .ok_or_else(|| "answer without pending offer".to_string())?;
                            rtc.sdp_api()
                                .accept_answer(pend, answer)
                                .map_err(|e| format!("accept answer: {e}"))?;
                        }
                        SIGNAL_CANDIDATE => {
                            let text = std::str::from_utf8(payload).unwrap_or("");
                            if let Some(c) = parse_remote_candidate(text) {
                                rtc.add_remote_candidate(c);
                            }
                        }
                        _ => {}
                    }
                }
            }
            Ok(Message::Ping(data)) => {
                let _ = ws.send(Message::Pong(data));
            }
            Ok(Message::Pong(_)) | Ok(Message::Frame(_)) | Ok(Message::Text(_)) => {}
            Ok(Message::Close(_)) => {
                closing = true;
            }
            Err(tungstenite::Error::Io(err))
                if err.kind() == ErrorKind::WouldBlock || err.kind() == ErrorKind::TimedOut => {}
            Err(_) => {
                // Signaling closed before channels opened → handshake failed.
                if !channels.ready_announced {
                    return Err("signaling closed before WebRTC ready".into());
                }
                closing = true;
            }
        }

        let _ = rtc.handle_input(Input::Timeout(Instant::now()));
        if timeout > Duration::ZERO {
            thread::sleep(timeout.min(Duration::from_millis(5)));
        }
    }

    Ok(())
}

fn handle_rtc_event(
    ev: Event,
    channels: &mut ChannelMap,
    events: &Mutex<Vec<NetEvent>>,
    is_host: bool,
    from: PeerId,
    peer: PeerId,
    closing: &mut bool,
) {
    match ev {
        Event::ChannelOpen(id, label) => {
            channels.on_open(id, &label);
            if channels.both_open() && !channels.ready_announced {
                channels.ready_announced = true;
                if is_host {
                    push_event(events, NetEvent::PeerJoined(peer));
                } else {
                    push_event(events, NetEvent::Connected);
                }
            }
        }
        Event::ChannelData(data) => {
            if data.data.len() > MAX_PAYLOAD {
                return;
            }
            if Some(data.id) == channels.rel {
                push_event(
                    events,
                    NetEvent::Message {
                        from,
                        bytes: data.data,
                    },
                );
            } else if Some(data.id) == channels.dg {
                push_event(
                    events,
                    NetEvent::Datagram {
                        from,
                        bytes: data.data,
                    },
                );
            }
        }
        Event::ChannelClose(id) => {
            if Some(id) == channels.rel {
                *closing = true;
            }
        }
        Event::IceConnectionStateChange(IceConnectionState::Disconnected) | Event::Closed => {
            *closing = true;
        }
        _ => {}
    }
}

/// Native WebRTC + signaling driver owned by [`crate::core::net::Net`].
pub struct NetDriver {
    events: Arc<Mutex<Vec<NetEvent>>>,
    slots: Arc<PeerSlots>,
    stop: Arc<AtomicBool>,
    listener: Option<TcpListener>,
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

        let events = Arc::new(Mutex::new(Vec::new()));
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
        self.slots.send_to(to, Outbound::Reliable(bytes))
    }

    pub fn send_unreliable(&mut self, to: PeerId, bytes: Vec<u8>) -> Result<(), String> {
        self.slots.send_to(to, Outbound::Unreliable(bytes))
    }

    pub fn broadcast(&mut self, bytes: Vec<u8>) -> Result<(), String> {
        if !self.is_host {
            return Err("client cannot broadcast".into());
        }
        self.slots.broadcast(false, bytes)
    }

    pub fn broadcast_unreliable(&mut self, bytes: Vec<u8>) -> Result<(), String> {
        if !self.is_host {
            return Err("client cannot broadcast".into());
        }
        self.slots.broadcast(true, bytes)
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
        let handle = spawn_peer_io(
            peer,
            stream,
            ws,
            Arc::clone(&events),
            Arc::clone(&slots),
            Arc::clone(&stop),
            true,
        );
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
