use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::queue::{Message, MessageQueue, Subscriber, TcpSubscriber};
use queutie_common::network::{self, NetworkError, PacketType};

type SharedQueue = Arc<Mutex<MessageQueue<TcpSubscriber>>>;
pub type SharedState = Arc<Mutex<HashMap<String, SharedQueue>>>;

pub struct Server {
    state: SharedState,
    listener: TcpListener,
}

#[derive(Debug)]
pub enum ServerError {
    Io(std::io::Error),
    Network(NetworkError),
    StatePoisoned,
    QueuePoisoned,
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "io error: {error}"),
            Self::Network(error) => write!(f, "network error: {error}"),
            Self::StatePoisoned => write!(f, "shared server state mutex poisoned"),
            Self::QueuePoisoned => write!(f, "queue mutex poisoned"),
        }
    }
}

impl Error for ServerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Network(error) => Some(error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for ServerError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<NetworkError> for ServerError {
    fn from(value: NetworkError) -> Self {
        Self::Network(value)
    }
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

        for incoming in self.listener.incoming() {
            let stream = match incoming {
                Ok(stream) => stream,
                Err(error) => {
                    eprintln!("failed to accept incoming connection: {error}");
                    continue;
                }
            };

            let state = Arc::clone(&state);

            thread::spawn(move || {
                if let Err(error) = Self::handle_connection(stream, state) {
                    eprintln!("connection handler failed: {error}");
                }
            });
        }
    }

    fn handle_connection(stream: TcpStream, state: SharedState) -> Result<(), ServerError> {
        let mut stream = stream;
        let packet = network::read_packet(&mut stream)?;

        let queue_name = packet
            .header
            .packet_target
            .trim_end_matches('\0')
            .to_string();

        match packet.header.packet_type {
            PacketType::Publish => {
                let message = Message::new(packet.body);
                let queue = Self::get_or_create_queue(&state, &queue_name)?;

                let mut subscribers = {
                    let mut queue = queue.lock().map_err(|_| ServerError::QueuePoisoned)?;
                    queue.push_message(message.clone());
                    // Move subscribers out so network sends happen without holding
                    // the queue lock; surviving subscribers are restored afterward.
                    queue.take_subscribers()
                };

                subscribers.retain_mut(|sub| sub.send(message.contents()));

                let mut queue = queue.lock().map_err(|_| ServerError::QueuePoisoned)?;
                queue.restore_subscribers(subscribers);

                println!("Published message to queue");
            }
            PacketType::Subscribe => {
                let queue = Self::get_or_create_queue(&state, &queue_name)?;
                let mut queue = queue.lock().map_err(|_| ServerError::QueuePoisoned)?;
                queue.add_subscriber(TcpSubscriber::new(stream.try_clone()?));
                println!("Subscriber added to queue");
                drop(queue);
                Self::maintain_subscription(stream);
            }
        }

        Ok(())
    }

    fn get_or_create_queue(
        state: &SharedState,
        queue_name: &str,
    ) -> Result<SharedQueue, ServerError> {
        let mut state = state.lock().map_err(|_| ServerError::StatePoisoned)?;
        Ok(Arc::clone(
            state
                .entry(queue_name.to_string())
                .or_insert_with(|| Arc::new(Mutex::new(MessageQueue::new()))),
        ))
    }

    fn maintain_subscription(_stream: TcpStream) {
        loop {
            thread::sleep(std::time::Duration::from_secs(60));
        }
    }
}
