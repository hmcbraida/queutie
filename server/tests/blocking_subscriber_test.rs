use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use queutie_common::network::{self, PacketHeader, PacketType};
use server::{Server, SharedState};

fn start_server(addr: String) -> SharedState {
    let server = Server::new(&addr).unwrap();
    let state = server.state();

    thread::spawn(move || {
        server.run();
    });

    thread::sleep(Duration::from_millis(100));
    state
}

fn subscribe(stream: &mut TcpStream, queue: &str) {
    let subscribe_packet = network::Packet::new(
        PacketHeader {
            packet_target: String::from(queue),
            packet_type: PacketType::Subscribe,
        },
        Vec::new(),
    );
    network::write_packet(stream, subscribe_packet);
}

fn publish(stream: &mut TcpStream, queue: &str, body: Vec<u8>) {
    let publish_packet = network::Packet::new(
        PacketHeader {
            packet_target: String::from(queue),
            packet_type: PacketType::Publish,
        },
        body,
    );
    network::write_packet(stream, publish_packet);
}

fn wait_for_message_count(
    state: &SharedState,
    queue_name: &str,
    expected_count: usize,
    timeout: Duration,
) -> bool {
    let start = Instant::now();

    while start.elapsed() < timeout {
        let has_count = match state.try_lock() {
            Ok(state) => state
                .get(queue_name)
                .map(|queue| queue.message_count() >= expected_count)
                .unwrap_or(false),
            Err(_) => {
                thread::sleep(Duration::from_millis(10));
                continue;
            }
        };

        if has_count {
            return true;
        }

        thread::sleep(Duration::from_millis(10));
    }

    false
}

#[test]
fn test_publish_not_blocked_by_slow_subscriber() {
    let queue_name = "test_queue";
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let addr_str = format!("127.0.0.1:{}", addr.port());
    drop(listener);

    let state = start_server(addr_str.clone());

    let mut blocking_subscribers = Vec::new();
    for _ in 0..3 {
        let mut blocking_subscriber = TcpStream::connect(&addr_str).unwrap();
        subscribe(&mut blocking_subscriber, queue_name);
        blocking_subscribers.push(blocking_subscriber);
    }

    thread::sleep(Duration::from_millis(100));

    let first_publish = thread::spawn({
        let addr_str = addr_str.clone();
        move || {
            let mut publisher = TcpStream::connect(&addr_str).unwrap();
            publisher
                .set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            publish(&mut publisher, queue_name, vec![b'x'; 8 * 1024 * 1024]);
        }
    });

    assert!(
        wait_for_message_count(&state, queue_name, 1, Duration::from_secs(1)),
        "First publish did not reach queue in time"
    );

    let (tx, rx) = mpsc::channel();
    thread::spawn({
        let addr_str = addr_str.clone();
        move || {
            let result = (|| {
                let mut publisher2 = TcpStream::connect(&addr_str)?;
                publisher2.set_write_timeout(Some(Duration::from_millis(500)))?;
                publish(&mut publisher2, queue_name, Vec::from("second message"));
                Ok::<(), std::io::Error>(())
            })();

            let _ = tx.send(result);
        }
    });

    match rx.recv_timeout(Duration::from_secs(1)) {
        Ok(Ok(())) => {}
        Ok(Err(err)) => panic!("Second publish failed unexpectedly: {err}"),
        Err(_) => panic!("Second publish call timed out, likely blocked by lock contention"),
    }

    assert!(
        wait_for_message_count(&state, queue_name, 2, Duration::from_millis(400)),
        "Second publish was not enqueued while slow subscribers were stalled"
    );

    let _ = blocking_subscribers;
    let _ = first_publish;
}
