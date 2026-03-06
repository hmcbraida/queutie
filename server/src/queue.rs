use std::collections::VecDeque;
use std::io::Write;
use std::sync::{Arc, Mutex};

pub trait Subscriber: Send + Sync {
    fn send(&mut self, data: &[u8]) -> bool;
}

pub struct TcpSubscriber {
    stream: Arc<Mutex<std::net::TcpStream>>,
}

impl TcpSubscriber {
    pub fn new(stream: std::net::TcpStream) -> Self {
        Self {
            stream: Arc::new(Mutex::new(stream)),
        }
    }
}

impl Subscriber for TcpSubscriber {
    fn send(&mut self, data: &[u8]) -> bool {
        if let Ok(mut stream) = self.stream.lock() {
            stream.write_all(data).is_ok()
        } else {
            false
        }
    }
}

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
        Self {
            contents: contents.into(),
        }
    }

    pub fn from_string(s: String) -> Self {
        Message::new(s.into_bytes())
    }

    pub fn to_string(self) -> Result<String, StringDecodeError> {
        String::from_utf8(self.contents.into_vec()).map_err(|_| StringDecodeError)
    }

    pub fn contents(&self) -> &[u8] {
        &self.contents
    }
}

pub struct MessageQueue<S: Subscriber = TcpSubscriber> {
    messages: VecDeque<Message>,
    subscribers: Vec<S>,
}

impl<S: Subscriber> MessageQueue<S> {
    pub fn new() -> Self {
        Self {
            messages: VecDeque::new(),
            subscribers: Vec::new(),
        }
    }

    pub fn pop_message(&mut self) -> Option<Message> {
        self.messages.pop_front()
    }

    pub fn push_message(&mut self, message: Message) {
        self.messages.push_back(message);
    }

    pub fn add_subscriber(&mut self, subscriber: S) {
        self.subscribers.push(subscriber);
    }

    pub fn push_message_to_subscribers(&mut self, message: &Message) {
        self.subscribers
            .retain_mut(|sub| sub.send(message.contents()));
    }

    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    pub fn subscriber_count(&self) -> usize {
        self.subscribers.len()
    }
}

impl<S: Subscriber> Default for MessageQueue<S> {
    fn default() -> Self {
        Self::new()
    }
}
