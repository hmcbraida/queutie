use std::io::Read;
use std::net::TcpStream;

use queutie_common::network::{self, PacketHeader, PacketType};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let packet = network::Packet::new(
        PacketHeader::with_zero_id(PacketType::Subscribe, "test_queue"),
        Vec::new(),
    );

    let mut stream = TcpStream::connect("127.0.0.1:3001")?;
    network::write_packet(&mut stream, packet)?;

    let mut buf = [0u8; 1024];
    loop {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                let message = String::from_utf8_lossy(&buf[..n]);
                println!("{message}");
            }
            Err(error) => {
                eprintln!("failed to read from server: {error}");
                break;
            }
        }
    }

    Ok(())
}
