use std::net::TcpStream;
use std::time::Duration;

use queutie_common::network::{self, PacketHeader, PacketType};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let header = PacketHeader::with_random_id(PacketType::Publish, "test_queue");
    let packet_id = header.packet_id;
    let buf = Vec::from("hello world".as_bytes());
    let packet = network::Packet::new(header, buf);

    let mut stream = TcpStream::connect("127.0.0.1:3001")?;

    network::write_packet(&mut stream, packet)?;

    stream.set_read_timeout(Some(Duration::from_millis(200)))?;
    match network::read_packet(&mut stream) {
        Ok(response)
            if matches!(response.header.packet_type, PacketType::PublishAck)
                && response.header.packet_id == packet_id =>
        {
            println!(
                "publish accepted for queue '{}' with packet_id {}",
                response.header.packet_target, response.header.packet_id
            );
        }
        Ok(response) if matches!(response.header.packet_type, PacketType::QueueFull) => {
            println!(
                "publish rejected for queue '{}' (packet_id {}): {}",
                response.header.packet_target,
                response.header.packet_id,
                String::from_utf8_lossy(&response.body)
            );
        }
        Ok(_) => {}
        Err(network::NetworkError::Io(error))
            if matches!(
                error.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
            ) => {}
        Err(error) => return Err(Box::new(error)),
    }

    Ok(())
}
