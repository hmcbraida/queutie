use std::collections::HashMap;
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::queue::{Message, MessageQueue, Subscriber, TcpSubscriber};

pub type SharedState = Arc<Mutex<HashMap<String, MessageQueue<TcpSubscriber>>>>;

pub struct Server {
    state: SharedState,
    listener: TcpListener,
}

impl Server {
    pub fn new(addr: &str) -> std::io::Result<Self> {
        let listener = TcpListener::bind(addr)?;
        let state: SharedState = Arc::new(Mutex::new(HashMap::new()));
        Ok(Self { state, listener })
    }

    pub fn state(&self) -> SharedState {
        Arc::clone(&self.state)
    }

    pub fn run(self) {
        let state = self.state;

        for stream in self.listener.incoming().map(|x| x.unwrap()) {
            let state = Arc::clone(&state);

            thread::spawn(move || {
                Self::handle_connection(stream, state);
            });
        }
    }

    fn handle_connection(stream: TcpStream, state: SharedState) {
        use queutie_common::network::{self, PacketType};

        let mut stream = stream;
        let packet = network::read_packet(&mut stream);

        let queue_name = packet
            .header
            .packet_target
            .trim_end_matches('\0')
            .to_string();

        match packet.header.packet_type {
            PacketType::Publish => {
                let message = Message::new(packet.body);

                let subscribers = {
                    let mut state = state.lock().unwrap();
                    let queue = state
                        .entry(queue_name.clone())
                        .or_insert_with(MessageQueue::new);
                    queue.push_message(message.clone());
                    queue.subscribers()
                };

                let failed_subscribers = subscribers
                    .into_iter()
                    .filter_map(|mut sub| {
                        if sub.send(message.contents()) {
                            None
                        } else {
                            Some(sub)
                        }
                    })
                    .collect::<Vec<_>>();

                if !failed_subscribers.is_empty() {
                    let mut state = state.lock().unwrap();
                    if let Some(queue) = state.get_mut(&queue_name) {
                        queue.remove_subscribers(&failed_subscribers);
                    }
                }

                println!("Published message to queue");
            }
            PacketType::Subscribe => {
                let mut state = state.lock().unwrap();
                let queue = state
                    .entry(queue_name.clone())
                    .or_insert_with(MessageQueue::new);
                queue.add_subscriber(TcpSubscriber::new(stream.try_clone().unwrap()));
                println!("Subscriber added to queue");
                drop(state);
                Self::maintain_subscription(stream);
            }
        }
    }

    fn maintain_subscription(_stream: TcpStream) {
        loop {
            thread::sleep(std::time::Duration::from_secs(60));
        }
    }
}
