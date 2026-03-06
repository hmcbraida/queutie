use std::net::TcpStream;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

mod support;

#[test]
fn test_publish_not_blocked_by_slow_subscriber() {
    let queue_name = "test_queue";
    let addr_str = support::reserve_ephemeral_addr();

    let state = support::start_server(&addr_str);

    let mut blocking_subscribers = Vec::new();
    for _ in 0..3 {
        let mut blocking_subscriber = TcpStream::connect(&addr_str).unwrap();
        support::subscribe(&mut blocking_subscriber, queue_name);
        blocking_subscribers.push(blocking_subscriber);
    }

    thread::sleep(Duration::from_millis(100));

    let first_publish = thread::spawn({
        let addr_str = addr_str.clone();
        move || {
            let mut publisher = TcpStream::connect(&addr_str).unwrap();
            publisher
                .set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            support::publish(&mut publisher, queue_name, vec![b'x'; 8 * 1024 * 1024]);
        }
    });

    assert!(
        support::wait_for_message_count(&state, queue_name, 1, Duration::from_secs(1)),
        "First publish did not reach queue in time"
    );

    let (tx, rx) = mpsc::channel();
    thread::spawn({
        let addr_str = addr_str.clone();
        move || {
            let result = (|| {
                let mut publisher2 = TcpStream::connect(&addr_str)?;
                publisher2.set_write_timeout(Some(Duration::from_millis(500)))?;
                support::publish(&mut publisher2, queue_name, Vec::from("second message"));
                Ok::<(), std::io::Error>(())
            })();

            let _ = tx.send(result);
        }
    });

    match rx.recv_timeout(Duration::from_secs(1)) {
        Ok(Ok(())) => {}
        Ok(Err(err)) => panic!("Second publish failed unexpectedly: {err}"),
        Err(_) => panic!("Second publish call timed out, likely blocked by lock contention"),
    }

    assert!(
        support::wait_for_message_count(&state, queue_name, 2, Duration::from_millis(400)),
        "Second publish was not enqueued while slow subscribers were stalled"
    );

    let _ = blocking_subscribers;
    let _ = first_publish;
}
