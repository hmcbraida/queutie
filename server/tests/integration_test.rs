use std::io::Read;
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use queutie_common::network::{self, PacketHeader, PacketType};
use server::queue::{Message, MessageQueue, TcpSubscriber};

type TestState = Arc<Mutex<std::collections::HashMap<String, MessageQueue<TcpSubscriber>>>>;

fn start_server(addr: String) {
    thread::spawn(move || {
        let listener = TcpListener::bind(&addr).unwrap();
        let state: TestState = Arc::new(Mutex::new(std::collections::HashMap::new()));

        for stream in listener.incoming().map(|x| x.unwrap()) {
            let state = Arc::clone(&state);
            thread::spawn(move || {
                let mut stream = stream;
                let packet = network::read_packet(&mut stream);

                let queue_name = packet
                    .header
                    .packet_target
                    .trim_end_matches('\0')
                    .to_string();

                match packet.header.packet_type {
                    PacketType::Publish => {
                        let state_for_notify = Arc::clone(&state);
                        let message = Message::new(packet.body);
                        {
                            let mut state = state.lock().unwrap();
                            let queue = state
                                .entry(queue_name.clone())
                                .or_insert_with(MessageQueue::new);
                            queue.push_message(message.clone());
                        }
                        {
                            let mut state = state_for_notify.lock().unwrap();
                            if let Some(queue) = state.get_mut(&queue_name) {
                                queue.push_message_to_subscribers(&message);
                            }
                        }
                    }
                    PacketType::Subscribe => {
                        let stream = stream.try_clone().unwrap();
                        {
                            let mut state = state.lock().unwrap();
                            let queue = state
                                .entry(queue_name.clone())
                                .or_insert_with(MessageQueue::new);
                            queue.add_subscriber(TcpSubscriber::new(stream));
                        }
                        drop(state);
                        loop {
                            thread::sleep(Duration::from_secs(60));
                        }
                    }
                }
            });
        }
    });
    thread::sleep(Duration::from_millis(100));
}

#[test]
fn test_subscriber_receives_published_message() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let addr_str = format!("127.0.0.1:{}", addr.port());
    drop(listener);

    start_server(addr_str.clone());

    let mut subscriber = TcpStream::connect(&addr_str).unwrap();
    let subscribe_packet = network::Packet::new(
        PacketHeader {
            packet_target: String::from("test_queue"),
            packet_type: PacketType::Subscribe,
        },
        Vec::new(),
    );
    network::write_packet(&mut subscriber, subscribe_packet);

    thread::sleep(Duration::from_millis(100));

    let mut publisher = TcpStream::connect(&addr_str).unwrap();
    let publish_packet = network::Packet::new(
        PacketHeader {
            packet_target: String::from("test_queue"),
            packet_type: PacketType::Publish,
        },
        Vec::from("hello world"),
    );
    network::write_packet(&mut publisher, publish_packet);

    thread::sleep(Duration::from_millis(100));

    let mut buffer = [0u8; 1024];
    let n = subscriber.read(&mut buffer).unwrap();
    let received = String::from_utf8_lossy(&buffer[..n]);

    assert_eq!(received, "hello world");
}
