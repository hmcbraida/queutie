use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::io;
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;

use crate::queue::{Message, MessageQueue, Subscriber, TcpSubscriber};
use queutie_common::network::{self, NetworkError, PacketType};

type SharedQueue = Arc<Mutex<MessageQueue<TcpSubscriber>>>;
pub type SharedState = Arc<Mutex<HashMap<String, SharedQueue>>>;

pub struct Server {
    connection_pool_size: usize,
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
    pub fn new(addr: &str, connection_pool_size: usize) -> io::Result<Self> {
        if connection_pool_size == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "connection_pool_size must be greater than 0",
            ));
        }

        let listener = TcpListener::bind(addr)?;
        let state: SharedState = Arc::new(Mutex::new(HashMap::new()));
        Ok(Self {
            connection_pool_size,
            state,
            listener,
        })
    }

    pub fn state(&self) -> SharedState {
        Arc::clone(&self.state)
    }

    pub fn run(self) {
        let (sender, receiver) = mpsc::channel::<TcpStream>();
        let receiver = Arc::new(Mutex::new(receiver));
        let state = self.state;

        // Spawn a fixed set of workers so connection handling threads are bounded.
        // All workers share one `mpsc::Receiver<TcpStream>` via `Arc<Mutex<_>>`:
        // each worker briefly locks the receiver, calls `recv()`, and gets the next
        // available socket. That makes incoming connections fan out across workers
        // without creating a new thread per client.
        for _ in 0..self.connection_pool_size {
            let receiver = Arc::clone(&receiver);
            let state = Arc::clone(&state);

            thread::spawn(move || {
                loop {
                    let stream = {
                        let receiver = match receiver.lock() {
                            Ok(receiver) => receiver,
                            Err(_) => {
                                eprintln!("connection receiver mutex poisoned");
                                return;
                            }
                        };
                        receiver.recv()
                    };

                    let stream = match stream {
                        Ok(stream) => stream,
                        Err(_) => return,
                    };

                    if let Err(error) = Self::handle_connection(stream, Arc::clone(&state)) {
                        eprintln!("connection handler failed: {error}");
                    }
                }
            });
        }

        for incoming in self.listener.incoming() {
            let stream = match incoming {
                Ok(stream) => stream,
                Err(error) => {
                    eprintln!("failed to accept incoming connection: {error}");
                    continue;
                }
            };

            if sender.send(stream).is_err() {
                eprintln!("all connection workers have stopped");
                break;
            }
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

                // retain_mut means we will eliminate any subscribers for whom
                // the message send failed
                subscribers.retain_mut(|sub| sub.send(message.contents()));

                let mut queue = queue.lock().map_err(|_| ServerError::QueuePoisoned)?;
                queue.restore_subscribers(subscribers);

                println!("Published message to queue");
            }
            PacketType::Subscribe => {
                let queue = Self::get_or_create_queue(&state, &queue_name)?;
                let mut queue = queue.lock().map_err(|_| ServerError::QueuePoisoned)?;
                queue.add_subscriber(TcpSubscriber::new(stream));
                println!("Subscriber added to queue");
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
}
