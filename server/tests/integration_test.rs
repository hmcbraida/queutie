use std::io::Read;
use std::net::TcpStream;
use std::time::Duration;

mod support;

#[test]
fn test_subscriber_receives_published_message() {
    let addr = support::reserve_ephemeral_addr();
    let _state = support::start_server(&addr);

    let mut subscriber = TcpStream::connect(&addr).unwrap();
    subscriber
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    support::subscribe(&mut subscriber, "test_queue");

    std::thread::sleep(Duration::from_millis(100));

    let mut publisher = TcpStream::connect(&addr).unwrap();
    support::publish(&mut publisher, "test_queue", Vec::from("hello world"));

    let mut buffer = [0u8; 1024];
    let n = subscriber.read(&mut buffer).unwrap();
    let received = String::from_utf8_lossy(&buffer[..n]);

    assert_eq!(received, "hello world");
}
