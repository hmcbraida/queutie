use std::collections::HashMap;
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::queue::{Message, MessageQueue, Subscriber, TcpSubscriber};

type SharedQueue = Arc<Mutex<MessageQueue<TcpSubscriber>>>;
pub type SharedState = Arc<Mutex<HashMap<String, SharedQueue>>>;

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
                let queue = Self::get_or_create_queue(&state, &queue_name);

                let mut subscribers = {
                    let mut queue = queue.lock().unwrap();
                    queue.push_message(message.clone());
                    // Move subscribers out so network sends happen without holding
                    // the queue lock; surviving subscribers are restored afterward.
                    queue.take_subscribers()
                };

                subscribers.retain_mut(|sub| sub.send(message.contents()));

                let mut queue = queue.lock().unwrap();
                queue.restore_subscribers(subscribers);

                println!("Published message to queue");
            }
            PacketType::Subscribe => {
                let queue = Self::get_or_create_queue(&state, &queue_name);
                let mut queue = queue.lock().unwrap();
                queue.add_subscriber(TcpSubscriber::new(stream.try_clone().unwrap()));
                println!("Subscriber added to queue");
                drop(queue);
                Self::maintain_subscription(stream);
            }
        }
    }

    fn get_or_create_queue(state: &SharedState, queue_name: &str) -> SharedQueue {
        let mut state = state.lock().unwrap();
        Arc::clone(
            state
                .entry(queue_name.to_string())
                .or_insert_with(|| Arc::new(Mutex::new(MessageQueue::new()))),
        )
    }

    fn maintain_subscription(_stream: TcpStream) {
        loop {
            thread::sleep(std::time::Duration::from_secs(60));
        }
    }
}
