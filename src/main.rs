use crate::queue::{Message, MessageQueue};

mod queue;

fn main() {
    let mut queue = MessageQueue::new();

    queue.push_message(Message::new(vec![1, 2]));
    queue.push_message(Message::from_string(String::from("hello world")));

    if let Some(msg) = queue.pop_message() {
        println!("{:?}", msg);
    }
    if let Some(msg) = queue.pop_message() {
        let msg_contents = msg.to_string().unwrap();
        println!("{}", msg_contents);
    }
}
