use std::collections::VecDeque;
use std::io::Write;
use std::net::TcpStream;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct Message {
    contents: Box<[u8]>,
}

#[derive(Debug)]
pub struct StringDecodeError;

impl Message {
    pub fn new<T>(contents: T) -> Self
    where
        T: Into<Box<[u8]>>,
    {
        return Self {
            contents: (contents.into()),
        };
    }

    pub fn from_string(s: String) -> Self {
        return Message::new(s.into_bytes());
    }

    pub fn to_string(self) -> Result<String, StringDecodeError> {
        String::from_utf8(self.contents.to_vec()).map_err(|_| StringDecodeError)
    }

    pub fn contents(&self) -> &[u8] {
        &self.contents
    }
}

pub struct MessageQueue {
    messages: VecDeque<Message>,
    subscribers: Vec<Arc<Mutex<TcpStream>>>,
}

impl MessageQueue {
    pub fn new() -> Self {
        Self {
            messages: VecDeque::new(),
            subscribers: Vec::new(),
        }
    }

    pub fn pop_message(&mut self) -> Option<Message> {
        return self.messages.pop_front();
    }

    pub fn push_message(&mut self, message: Message) {
        self.messages.push_back(message);
    }

    pub fn add_subscriber(&mut self, subscriber: Arc<Mutex<TcpStream>>) {
        self.subscribers.push(subscriber);
    }

    pub fn push_message_to_subscribers(&mut self, message: &Message) {
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
