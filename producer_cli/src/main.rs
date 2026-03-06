use std::net::TcpStream;
use std::time::Duration;

use queutie_common::network::{self, PacketHeader, PacketType};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let buf = Vec::from("hello world".as_bytes());
    let packet = network::Packet::new(
        PacketHeader {
            packet_target: String::from("test_queue"),
            packet_type: PacketType::Publish,
        },
        buf,
    );

    let mut stream = TcpStream::connect("127.0.0.1:3001")?;

    network::write_packet(&mut stream, packet)?;

    stream.set_read_timeout(Some(Duration::from_millis(200)))?;
    match network::read_packet(&mut stream) {
        Ok(response) if matches!(response.header.packet_type, PacketType::QueueFull) => {
            println!(
                "publish rejected for queue '{}' : {}",
                response.header.packet_target.trim_end_matches('\0'),
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
