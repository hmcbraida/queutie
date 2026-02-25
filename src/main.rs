mod consumer;
mod filesystem;
mod producer;

fn main() {
    let message_producer = producer::BasicMessageProducer::new("runtime/", "test_queue");
    let message_consumer = consumer::BasicMessageConsumer::new("runtime/", "test_queue");

    message_producer.push_to_queue("hello world");

    let msg_bytes = message_consumer.consume_one_message().unwrap();
    let msg = str::from_utf8(&msg_bytes).unwrap();

    println!("{}", &msg);
}
