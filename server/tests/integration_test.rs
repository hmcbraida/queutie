use std::io::Read;
use std::net::TcpStream;
use std::thread;
use std::time::Duration;
use std::time::Instant;

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

#[test]
fn test_disconnected_subscriber_is_pruned_after_failed_publish() {
    let queue_name = "test_queue";
    let addr = support::reserve_ephemeral_addr();
    let state = support::start_server(&addr);

    let mut subscriber = TcpStream::connect(&addr).unwrap();
    support::subscribe(&mut subscriber, queue_name);

    assert!(
        support::wait_for_subscriber_count(&state, queue_name, 1, Duration::from_secs(1)),
        "Subscriber was not registered in time"
    );

    drop(subscriber);

    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(2) {
        let mut publisher = TcpStream::connect(&addr).unwrap();
        support::publish(&mut publisher, queue_name, Vec::from("probe"));

        if support::wait_for_subscriber_count(&state, queue_name, 0, Duration::from_millis(100)) {
            return;
        }

        thread::sleep(Duration::from_millis(50));
    }

    panic!("Disconnected subscriber was not pruned after publish retries");
}
