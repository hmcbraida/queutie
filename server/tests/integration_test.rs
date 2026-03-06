use std::collections::{HashMap, VecDeque};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use queutie_common::network::{self, PacketHeader, PacketType};

#[derive(Clone, Debug)]
struct Message {
    contents: Box<[u8]>,
}

impl Message {
    fn new<T: Into<Box<[u8]>>>(contents: T) -> Self {
        Self {
            contents: contents.into(),
        }
    }

    fn contents(&self) -> &[u8] {
        &self.contents
    }
}

struct MessageQueue {
    messages: VecDeque<Message>,
    subscribers: Vec<Arc<Mutex<TcpStream>>>,
}

impl MessageQueue {
    fn new() -> Self {
        Self {
            messages: VecDeque::new(),
            subscribers: Vec::new(),
        }
    }

    fn push_message(&mut self, message: Message) {
        self.messages.push_back(message);
    }

    fn add_subscriber(&mut self, subscriber: Arc<Mutex<TcpStream>>) {
        self.subscribers.push(subscriber);
    }

    fn push_message_to_subscribers(&mut self, message: &Message) {
        let mut disconnected = Vec::new();

        for (i, subscriber) in self.subscribers.iter().enumerate() {
            if let Ok(mut stream) = subscriber.lock() {
                if stream.write_all(message.contents()).is_err() {
                    disconnected.push(i);
                }
            }
        }

        for i in disconnected.iter().rev() {
            self.subscribers.remove(*i);
        }
    }
}

fn start_server(addr: String) {
    thread::spawn(move || {
        let listener = TcpListener::bind(&addr).unwrap();
        let state: Arc<Mutex<HashMap<String, MessageQueue>>> = Arc::new(Mutex::new(HashMap::new()));

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
                let mut state = state.lock().unwrap();

                match packet.header.packet_type {
                    PacketType::Publish => {
                        let message = Message::new(packet.body);
                        let queue = state.entry(queue_name).or_insert_with(MessageQueue::new);
                        queue.push_message(message.clone());
                        queue.push_message_to_subscribers(&message);
                    }
                    PacketType::Subscribe => {
                        let queue = state.entry(queue_name).or_insert_with(MessageQueue::new);
                        queue.add_subscriber(Arc::new(Mutex::new(stream)));

                        drop(state);
                        loop {
                            thread::sleep(Duration::from_secs(1));
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
