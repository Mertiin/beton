//! Native loopback: host, connect, exchange one binary frame, then clean shutdown.

#![cfg(not(target_arch = "wasm32"))]

use std::thread;
use std::time::{Duration, Instant};

use beton::{Net, NetEvent, PeerId};

fn wait_for(net: &mut Net, deadline: Instant, pred: impl Fn(&[NetEvent]) -> bool) -> Vec<NetEvent> {
    let mut collected = Vec::new();
    while Instant::now() < deadline {
        net.begin_frame();
        collected.extend_from_slice(net.events());
        let hit = pred(&collected);
        net.end_frame();
        if hit {
            return collected;
        }
        thread::sleep(Duration::from_millis(10));
    }
    collected
}

#[test]
fn host_client_exchange_and_drop() {
    let mut host = Net::new();
    host.host(0).expect("host");

    let listening = wait_for(&mut host, Instant::now() + Duration::from_secs(2), |ev| {
        ev.iter()
            .any(|e| matches!(e, NetEvent::Listening { .. }))
    });
    let port = listening
        .iter()
        .find_map(|e| match e {
            NetEvent::Listening { port } => Some(*port),
            _ => None,
        })
        .expect("Listening event");
    assert!(port > 0);

    let mut client = Net::new();
    client
        .connect(&format!("ws://127.0.0.1:{port}"))
        .expect("connect");

    let _ = wait_for(&mut client, Instant::now() + Duration::from_secs(2), |ev| {
        ev.iter().any(|e| matches!(e, NetEvent::Connected))
    });
    let host_events = wait_for(&mut host, Instant::now() + Duration::from_secs(2), |ev| {
        ev.iter().any(|e| matches!(e, NetEvent::PeerJoined(_)))
    });
    let peer = host_events
        .iter()
        .find_map(|e| match e {
            NetEvent::PeerJoined(id) => Some(*id),
            _ => None,
        })
        .expect("PeerJoined");
    assert_ne!(peer, PeerId::HOST);

    let payload = b"hello-beton".to_vec();
    client
        .send(PeerId::HOST, &payload)
        .expect("client send");

    let host_msg = wait_for(&mut host, Instant::now() + Duration::from_secs(2), |ev| {
        ev.iter().any(|e| matches!(e, NetEvent::Message { .. }))
    });
    let got = host_msg
        .iter()
        .find_map(|e| match e {
            NetEvent::Message { from, bytes } => Some((*from, bytes.clone())),
            _ => None,
        })
        .expect("host Message");
    assert_eq!(got.0, peer);
    assert_eq!(got.1, payload);

    host.send(peer, b"pong").expect("host send");
    let client_msg = wait_for(&mut client, Instant::now() + Duration::from_secs(2), |ev| {
        ev.iter().any(|e| matches!(e, NetEvent::Message { .. }))
    });
    assert!(client_msg.iter().any(|e| matches!(
        e,
        NetEvent::Message {
            from: PeerId::HOST,
            bytes
        } if bytes == b"pong"
    )));

    // Drop must join every thread without hanging.
    drop(client);
    drop(host);
}
