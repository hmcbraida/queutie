use std::collections::VecDeque;

#[derive(Debug)]
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
}

pub struct MessageQueue {
    messages: VecDeque<Message>,
}

impl MessageQueue {
    pub fn new() -> Self {
        Self {
            messages: VecDeque::new(),
        }
    }

    pub fn pop_message(&mut self) -> Option<Message> {
        return self.messages.pop_front();
    }

    pub fn push_message(&mut self, message: Message) {
        self.messages.push_back(message);
    }
}
