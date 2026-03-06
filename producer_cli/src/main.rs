use std::net::TcpStream;

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

    Ok(())
}
