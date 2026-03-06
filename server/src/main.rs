use std::collections::HashMap;
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;

mod queue;

use crate::queue::{Message, MessageQueue};
use queutie_common::network::{self, PacketType};

type SharedState = Arc<Mutex<HashMap<String, MessageQueue>>>;

fn handle_connection(stream: std::net::TcpStream, state: SharedState) {
    let packet = network::read_packet(&mut stream.try_clone().unwrap());

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
            println!("Published message to queue");
        }
        PacketType::Subscribe => {
            let queue = state.entry(queue_name).or_insert_with(MessageQueue::new);
            queue.add_subscriber(Arc::new(Mutex::new(stream.try_clone().unwrap())));
            println!("Subscriber added to queue");

            drop(state);

            loop {
                thread::sleep(std::time::Duration::from_secs(1));
            }
        }
    }
}

fn main() {
    let listener = TcpListener::bind("127.0.0.1:3001").unwrap();
    let state: SharedState = Arc::new(Mutex::new(HashMap::new()));

    for stream in listener.incoming().map(|x| x.unwrap()) {
        let state = Arc::clone(&state);
        thread::spawn(move || {
            handle_connection(stream, state);
        });
    }
}
