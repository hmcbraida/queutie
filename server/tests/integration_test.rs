use std::io::Read;
use std::net::TcpStream;
use std::thread;
use std::time::Duration;
use std::time::Instant;

use queutie_common::network::{self, PacketHeader, PacketType};
use server::Server;

mod support;

#[test]
fn test_subscriber_receives_published_message() {
    let addr = support::reserve_ephemeral_addr();
    let _state = support::start_server(&addr);

    let mut subscriber = TcpStream::connect(&addr).unwrap();
    subscriber
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    support::subscribe(&mut subscriber, "test_queue");

    std::thread::sleep(Duration::from_millis(100));

    let mut publisher = TcpStream::connect(&addr).unwrap();
    support::publish(&mut publisher, "test_queue", Vec::from("hello world"));

    let mut buffer = [0u8; 1024];
    let n = subscriber.read(&mut buffer).unwrap();
    let received = String::from_utf8_lossy(&buffer[..n]);

    assert_eq!(received, "hello world");
}

#[test]
fn test_disconnected_subscriber_is_pruned_after_failed_publish() {
    let queue_name = "test_queue";
    let addr = support::reserve_ephemeral_addr();
    let state = support::start_server(&addr);

    let mut subscriber = TcpStream::connect(&addr).unwrap();
    support::subscribe(&mut subscriber, queue_name);

    assert!(
        support::wait_for_subscriber_count(&state, queue_name, 1, Duration::from_secs(1)),
        "Subscriber was not registered in time"
    );

    drop(subscriber);

    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(2) {
        let mut publisher = TcpStream::connect(&addr).unwrap();
        support::publish(&mut publisher, queue_name, Vec::from("probe"));

        if support::wait_for_subscriber_count(&state, queue_name, 0, Duration::from_millis(100)) {
            return;
        }

        thread::sleep(Duration::from_millis(50));
    }

    panic!("Disconnected subscriber was not pruned after publish retries");
}

#[test]
fn test_publish_dropped_when_queue_is_full() {
    let queue_name = "full_queue";
    let rejected_packet_id = 202;
    let addr = support::reserve_ephemeral_addr();
    let server = Server::new(&addr, 4, 1).unwrap();
    let state = server.state();

    thread::spawn(move || {
        server.run();
    });

    thread::sleep(Duration::from_millis(100));

    let mut publisher1 = TcpStream::connect(&addr).unwrap();
    support::publish(&mut publisher1, queue_name, b"first".to_vec());

    assert!(
        support::wait_for_message_count(&state, queue_name, 1, Duration::from_secs(1)),
        "First publish did not reach queue in time"
    );

    let mut publisher2 = TcpStream::connect(&addr).unwrap();
    let publish_packet = network::Packet::new(
        PacketHeader {
            packet_target: String::from(queue_name),
            packet_type: PacketType::Publish,
            packet_id: rejected_packet_id,
        },
        b"second".to_vec(),
    );
    network::write_packet(&mut publisher2, publish_packet).unwrap();

    let response = network::read_packet(&mut publisher2).unwrap();
    assert!(matches!(response.header.packet_type, PacketType::QueueFull));
    assert_eq!(
        response.header.packet_target.trim_end_matches('\0'),
        queue_name
    );
    assert_eq!(response.header.packet_id, rejected_packet_id);
    assert_eq!(response.body, b"queue is full");

    let queue = {
        let state_guard = state.lock().unwrap();
        state_guard.get(queue_name).cloned().unwrap()
    };
    let queue_guard = queue.lock().unwrap();
    assert_eq!(queue_guard.message_count(), 1);
}

#[test]
fn test_publish_ack_echoes_packet_id_for_accepted_message() {
    let queue_name = "ack_queue";
    let accepted_packet_id = 101;
    let addr = support::reserve_ephemeral_addr();
    let server = Server::new(&addr, 4, 10).unwrap();
    let state = server.state();

    thread::spawn(move || {
        server.run();
    });

    thread::sleep(Duration::from_millis(100));

    let mut publisher = TcpStream::connect(&addr).unwrap();
    let publish_packet = network::Packet::new(
        PacketHeader {
            packet_target: String::from(queue_name),
            packet_type: PacketType::Publish,
            packet_id: accepted_packet_id,
        },
        b"hello ack".to_vec(),
    );
    network::write_packet(&mut publisher, publish_packet).unwrap();

    let response = network::read_packet(&mut publisher).unwrap();
    assert!(matches!(
        response.header.packet_type,
        PacketType::PublishAck
    ));
    assert_eq!(
        response.header.packet_target.trim_end_matches('\0'),
        queue_name
    );
    assert_eq!(response.header.packet_id, accepted_packet_id);

    let queue = {
        let state_guard = state.lock().unwrap();
        state_guard.get(queue_name).cloned().unwrap()
    };
    let queue_guard = queue.lock().unwrap();
    assert_eq!(queue_guard.message_count(), 1);
}
