use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

use queutie_common::network::{self, PacketHeader, PacketType};
use server::{Server, SharedState};

pub fn reserve_ephemeral_addr() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);

    format!("127.0.0.1:{}", addr.port())
}

pub fn start_server(addr: &str) -> SharedState {
    let server = Server::new(addr, 4).unwrap();
    let state = server.state();

    thread::spawn(move || {
        server.run();
    });

    thread::sleep(Duration::from_millis(100));
    state
}

pub fn subscribe(stream: &mut TcpStream, queue: &str) {
    let subscribe_packet = network::Packet::new(
        PacketHeader {
            packet_target: String::from(queue),
            packet_type: PacketType::Subscribe,
        },
        Vec::new(),
    );
    network::write_packet(stream, subscribe_packet).expect("subscribe packet should be sent");
}

pub fn publish(stream: &mut TcpStream, queue: &str, body: Vec<u8>) {
    let publish_packet = network::Packet::new(
        PacketHeader {
            packet_target: String::from(queue),
            packet_type: PacketType::Publish,
        },
        body,
    );
    network::write_packet(stream, publish_packet).expect("publish packet should be sent");
}

#[allow(dead_code)]
pub fn wait_for_message_count(
    state: &SharedState,
    queue_name: &str,
    expected_count: usize,
    timeout: Duration,
) -> bool {
    let start = Instant::now();

    while start.elapsed() < timeout {
        let queue = match state.try_lock() {
            Ok(state) => state.get(queue_name).cloned(),
            Err(_) => {
                thread::sleep(Duration::from_millis(10));
                continue;
            }
        };

        if let Some(queue) = queue {
            let has_count = match queue.try_lock() {
                Ok(queue) => queue.message_count() >= expected_count,
                Err(_) => {
                    thread::sleep(Duration::from_millis(10));
                    continue;
                }
            };

            if has_count {
                return true;
            }
        }

        thread::sleep(Duration::from_millis(10));
    }

    false
}
