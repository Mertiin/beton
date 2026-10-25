//! Polled host / client message channel.
//!
//! Games own the payload bytes. The engine does not replicate objects or encrypt
//! traffic — this is a LAN sample pipe. Events last one frame (same rule as
//! [`Input`](crate::Input)): read [`Net::events`] every `update` or they are gone.

use crate::core::platform::NetDriver;

/// Cap on one binary WebSocket payload (bytes). Larger sends fail.
pub const MAX_PAYLOAD: usize = 16 * 1024;

/// Maximum remote peers a host keeps. Further accepts are closed.
pub const MAX_PEERS: usize = 8;

/// Soft cap on queued inbound events before oldest messages are dropped.
pub const MAX_INBOUND: usize = 256;

/// Stable id for a remote peer on this socket.
///
/// The host is always [`PeerId::HOST`]. The host assigns `1, 2, …` as clients join.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PeerId(pub u32);

impl PeerId {
    /// The host peer (always `0`).
    pub const HOST: PeerId = PeerId(0);
}

/// Whether this process is offline, hosting, or connected as a client.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetRole {
    /// No socket open.
    Offline,
    /// Listening / hosting.
    Hosting,
    /// Connected to a host.
    Client,
}

/// One network notification visible during a single `update`.
#[derive(Clone, Debug)]
pub enum NetEvent {
    /// Host bound successfully. `port` is the local listen port (useful when `0` was requested).
    Listening {
        /// Bound TCP port.
        port: u16,
    },
    /// Client finished the WebSocket handshake.
    Connected,
    /// A remote peer is ready (host only for new clients).
    PeerJoined(PeerId),
    /// A remote peer closed or was dropped.
    PeerLeft(PeerId),
    /// Binary payload from `from`.
    Message {
        /// Sender peer id.
        from: PeerId,
        /// Opaque game bytes (at most [`MAX_PAYLOAD`]).
        bytes: Vec<u8>,
    },
    /// The local socket is gone (client lost host, or host stopped).
    Disconnected,
    /// Inbound queue overflowed; at least one `Message` was dropped.
    Overflow,
}

/// Frame-scoped network handle. Start offline; call [`host`](Net::host) or
/// [`connect`](Net::connect) from `boot` / `update`.
pub struct Net {
    driver: Option<NetDriver>,
    role: NetRole,
    frame_events: Vec<NetEvent>,
}

impl Default for Net {
    fn default() -> Self {
        Self::new()
    }
}

impl Net {
    /// Offline net with no socket.
    pub fn new() -> Self {
        Self {
            driver: None,
            role: NetRole::Offline,
            frame_events: Vec::new(),
        }
    }

    /// Listen on `0.0.0.0:port` (desktop). Fails on the web and if already open.
    ///
    /// Pass `0` to let the OS pick a free port; read [`NetEvent::Listening`] for the real port.
    pub fn host(&mut self, port: u16) -> Result<(), String> {
        if self.driver.is_some() {
            return Err("net already open; call close first".into());
        }
        let driver = NetDriver::host(port)?;
        self.driver = Some(driver);
        self.role = NetRole::Hosting;
        Ok(())
    }

    /// Dial a `ws://host:port` URL. Fails if already open.
    pub fn connect(&mut self, url: &str) -> Result<(), String> {
        if self.driver.is_some() {
            return Err("net already open; call close first".into());
        }
        let driver = NetDriver::connect(url)?;
        self.driver = Some(driver);
        self.role = NetRole::Client;
        Ok(())
    }

    /// Shut down the socket and go offline.
    pub fn close(&mut self) {
        if let Some(driver) = self.driver.take() {
            driver.shutdown();
        }
        self.role = NetRole::Offline;
        self.frame_events.clear();
    }

    /// Send opaque bytes to one peer. Clients may only send to [`PeerId::HOST`].
    pub fn send(&mut self, to: PeerId, bytes: &[u8]) -> Result<(), String> {
        if bytes.len() > MAX_PAYLOAD {
            return Err(format!(
                "payload {} bytes exceeds MAX_PAYLOAD ({MAX_PAYLOAD})",
                bytes.len()
            ));
        }
        match self.role {
            NetRole::Offline => return Err("net is offline".into()),
            NetRole::Client if to != PeerId::HOST => {
                return Err("client may only send to PeerId::HOST".into());
            }
            _ => {}
        }
        let Some(driver) = self.driver.as_mut() else {
            return Err("net is offline".into());
        };
        driver.send(to, bytes.to_vec())
    }

    /// Send the same bytes to every remote peer. Host only.
    pub fn broadcast(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.len() > MAX_PAYLOAD {
            return Err(format!(
                "payload {} bytes exceeds MAX_PAYLOAD ({MAX_PAYLOAD})",
                bytes.len()
            ));
        }
        match self.role {
            NetRole::Hosting => {}
            NetRole::Client => return Err("client cannot broadcast".into()),
            NetRole::Offline => return Err("net is offline".into()),
        }
        let Some(driver) = self.driver.as_mut() else {
            return Err("net is offline".into());
        };
        driver.broadcast(bytes.to_vec())
    }

    /// Events for this frame only. Empty after the frame ends if unread.
    pub fn events(&self) -> &[NetEvent] {
        &self.frame_events
    }

    /// Currently connected remote peer ids.
    pub fn peers(&self) -> Vec<PeerId> {
        match &self.driver {
            Some(driver) => driver.peers(),
            None => Vec::new(),
        }
    }

    /// Offline, hosting, or client.
    pub fn role(&self) -> NetRole {
        self.role
    }

    /// Pull driver events into this frame's slice.
    ///
    /// The engine calls this before `update`. Tests that drive [`Net`] without
    /// [`run`](crate::run) should call it the same way.
    pub fn begin_frame(&mut self) {
        self.frame_events.clear();
        let Some(driver) = self.driver.as_mut() else {
            return;
        };
        let incoming = driver.poll();
        let disconnected = incoming
            .iter()
            .any(|event| matches!(event, NetEvent::Disconnected));
        self.frame_events = incoming;
        if disconnected {
            self.role = NetRole::Offline;
            if let Some(driver) = self.driver.take() {
                driver.shutdown();
            }
        }
    }

    /// Drop unread events.
    ///
    /// The engine calls this after `update`. Unread events are discarded.
    pub fn end_frame(&mut self) {
        self.frame_events.clear();
    }
}

impl Drop for Net {
    fn drop(&mut self) {
        self.close();
    }
}
