use std::{
    error::Error,
    fmt,
    io::{BufReader, BufWriter, Read, Write},
    mem::size_of,
    net::TcpStream,
    str,
};

use rand::random;

const FRAME_HEADER_LENGTH: usize = 4;
const FRAME_BODY_LENGTH: usize = 1024;

#[derive(Debug)]
struct PacketFrame {
    // TODO: The header should be destructured in this struct, and the write /
    // read frame operation should do the dirty work of deserialising. Right
    // now it is done in read_frame and write_frame
    pub header: [u8; FRAME_HEADER_LENGTH],
    pub body: [u8; FRAME_BODY_LENGTH],
}

impl PacketFrame {
    pub fn blank() -> Self {
        PacketFrame {
            header: [0u8; FRAME_HEADER_LENGTH],
            body: [0u8; FRAME_BODY_LENGTH],
        }
    }
}

#[derive(Debug)]
pub enum NetworkError {
    Io(std::io::Error),
    InvalidFrameLength { declared: usize, max: usize },
    MalformedPacket(&'static str),
    UnknownPacketType(u8),
    InvalidPacketTarget(std::str::Utf8Error),
    PacketTargetTooLong { max_len: usize, actual_len: usize },
}

impl fmt::Display for NetworkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "io error: {error}"),
            Self::InvalidFrameLength { declared, max } => {
                write!(f, "invalid frame length {declared}, max is {max}")
            }
            Self::MalformedPacket(reason) => write!(f, "malformed packet: {reason}"),
            Self::UnknownPacketType(value) => write!(f, "unknown packet type byte: {value}"),
            Self::InvalidPacketTarget(error) => {
                write!(f, "packet target is not valid utf8: {error}")
            }
            Self::PacketTargetTooLong {
                max_len,
                actual_len,
            } => {
                write!(f, "packet target length {actual_len} exceeds max {max_len}")
            }
        }
    }
}

impl Error for NetworkError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::InvalidPacketTarget(error) => Some(error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for NetworkError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

#[derive(Debug)]
pub enum PacketType {
    Publish,
    Subscribe,
    QueueFull,
    PublishAck,
}

#[derive(Debug)]
pub struct PacketHeader {
    pub packet_type: PacketType,
    pub packet_target: String,
    pub packet_id: u64,
}

impl PacketHeader {
    pub fn with_random_id(packet_type: PacketType, packet_target: impl Into<String>) -> Self {
        Self {
            packet_type,
            packet_target: packet_target.into(),
            packet_id: random::<u64>(),
        }
    }

    pub fn with_zero_id(packet_type: PacketType, packet_target: impl Into<String>) -> Self {
        Self {
            packet_type,
            packet_target: packet_target.into(),
            packet_id: 0,
        }
    }
}

const PACKET_HEADER_SIZE: usize = 32;
const PACKET_TARGET_SIZE: usize = 16;
const PACKET_ID_OFFSET: usize = 1 + PACKET_TARGET_SIZE;
const PACKET_ID_SIZE: usize = 8;

#[derive(Debug)]
pub struct Packet {
    pub header: PacketHeader,
    pub body: Vec<u8>,
}

impl Packet {
    pub fn new(header: PacketHeader, body: Vec<u8>) -> Self {
        Packet { header, body }
    }
}

fn read_frame(buf_reader: &mut BufReader<&mut TcpStream>) -> Result<PacketFrame, NetworkError> {
    let mut frame = PacketFrame::blank();
    buf_reader.read_exact(&mut frame.header)?;
    buf_reader.read_exact(&mut frame.body)?;

    Ok(frame)
}

fn write_frame(
    buf_writer: &mut BufWriter<&mut TcpStream>,
    frame: &PacketFrame,
) -> Result<(), NetworkError> {
    buf_writer.write_all(&frame.header)?;
    buf_writer.write_all(&frame.body)?;
    buf_writer.flush()?;

    Ok(())
}

pub fn read_packet(stream: &mut TcpStream) -> Result<Packet, NetworkError> {
    let mut packet_data: Vec<u8> = Vec::new();

    let mut buf_reader = BufReader::new(stream);

    loop {
        let frame = read_frame(&mut buf_reader)?;

        let frame_is_final = frame.header[0] == 0x01;
        let frame_body_length = u16::from_be_bytes([frame.header[1], frame.header[2]]) as usize;

        if frame_body_length > FRAME_BODY_LENGTH {
            return Err(NetworkError::InvalidFrameLength {
                declared: frame_body_length,
                max: FRAME_BODY_LENGTH,
            });
        }

        packet_data.extend_from_slice(&frame.body[..frame_body_length]);

        if frame_is_final {
            break;
        }
    }

    if packet_data.len() < PACKET_HEADER_SIZE {
        return Err(NetworkError::MalformedPacket(
            "packet payload is smaller than protocol header",
        ));
    }

    let body = packet_data.split_off(PACKET_HEADER_SIZE);
    let packet_type = match packet_data[0] {
        0 => PacketType::Publish,
        1 => PacketType::Subscribe,
        2 => PacketType::QueueFull,
        3 => PacketType::PublishAck,
        value => return Err(NetworkError::UnknownPacketType(value)),
    };
    let packet_target = str::from_utf8(&packet_data[1..1 + PACKET_TARGET_SIZE])
        .map_err(NetworkError::InvalidPacketTarget)?
        .to_string();
    let packet_id = u64::from_be_bytes(
        packet_data[PACKET_ID_OFFSET..PACKET_ID_OFFSET + PACKET_ID_SIZE]
            .try_into()
            .expect("packet id bytes should have fixed length"),
    );
    let header = PacketHeader {
        packet_target,
        packet_type,
        packet_id,
    };

    Ok(Packet { header, body })
}

pub fn write_packet(stream: &mut TcpStream, packet: Packet) -> Result<(), NetworkError> {
    let Packet { header, mut body } = packet;

    let mut packet_data = Vec::from([0u8; PACKET_HEADER_SIZE]);
    let PacketHeader {
        packet_type,
        packet_target,
        packet_id,
    } = header;
    packet_data[0] = match packet_type {
        PacketType::Publish => 0,
        PacketType::Subscribe => 1,
        PacketType::QueueFull => 2,
        PacketType::PublishAck => 3,
    };
    let packet_target_bytes = packet_target.as_bytes();
    if packet_target_bytes.len() > PACKET_TARGET_SIZE {
        return Err(NetworkError::PacketTargetTooLong {
            max_len: PACKET_TARGET_SIZE,
            actual_len: packet_target_bytes.len(),
        });
    }
    packet_data[1..1 + packet_target_bytes.len()].copy_from_slice(packet_target_bytes);
    packet_data[PACKET_ID_OFFSET..PACKET_ID_OFFSET + PACKET_ID_SIZE]
        .copy_from_slice(&packet_id.to_be_bytes());
    packet_data.append(&mut body);

    let mut bytes_remaining = packet_data.len();
    let mut read_offset: usize = 0;

    let mut buf_writer = BufWriter::new(stream);

    while bytes_remaining > 0 {
        let bytes_to_write: usize = if bytes_remaining > 1024 {
            1024
        } else {
            bytes_remaining
        };
        bytes_remaining -= bytes_to_write;

        // Construct a new packet frame
        // First step is constructing the frame header.
        let mut frame_header = [0u8; FRAME_HEADER_LENGTH];
        // Byte zero of the frame is 1 if this the last frame, 0 else
        frame_header[0] = if bytes_remaining == 0 { 0x01 } else { 0x00 };
        // Bytes one to two represent the size of the packet in the frame.
        frame_header[1..=2]
            .copy_from_slice(&bytes_to_write.to_be_bytes()[size_of::<usize>() - 2..]);

        // Next we construct the frame body by copying from the remaining packet.
        let mut frame_body = [0u8; FRAME_BODY_LENGTH];
        frame_body[..bytes_to_write]
            .copy_from_slice(&packet_data[read_offset..read_offset + bytes_to_write]);
        read_offset += bytes_to_write;

        let frame = PacketFrame {
            header: frame_header,
            body: frame_body,
        };

        write_frame(&mut buf_writer, &frame)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{NetworkError, Packet, PacketHeader, PacketType, read_packet, write_packet};
    use std::net::{TcpListener, TcpStream};
    use std::thread;

    #[test]
    fn write_packet_handles_payload_larger_than_frame_size() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let payload = vec![0xAB; 4097];

        let server_handle = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let packet = read_packet(&mut socket).expect("packet should decode");

            assert!(matches!(packet.header.packet_type, PacketType::Publish));
            assert_eq!(
                packet.header.packet_target.trim_end_matches('\0'),
                "big_queue"
            );
            assert_eq!(packet.header.packet_id, 77);
            assert_eq!(packet.body.len(), payload.len());
            assert_eq!(packet.body, payload);
        });

        let mut client = TcpStream::connect(addr).unwrap();
        let packet = Packet::new(
            PacketHeader {
                packet_type: PacketType::Publish,
                packet_target: String::from("big_queue"),
                packet_id: 77,
            },
            vec![0xAB; 4097],
        );

        write_packet(&mut client, packet).expect("packet should encode and send");

        server_handle.join().unwrap();
    }

    #[test]
    fn write_packet_rejects_target_longer_than_protocol_limit() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let _server_handle = thread::spawn(move || {
            let _ = listener.accept();
        });

        let mut client = TcpStream::connect(addr).unwrap();
        let packet = Packet::new(
            PacketHeader {
                packet_type: PacketType::Publish,
                packet_target: String::from("queue_name_that_is_too_long"),
                packet_id: 5,
            },
            b"message".to_vec(),
        );

        let error = write_packet(&mut client, packet).unwrap_err();

        assert!(matches!(error, NetworkError::PacketTargetTooLong { .. }));
    }

    #[test]
    fn queue_full_packet_roundtrips() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let server_handle = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let packet = read_packet(&mut socket).expect("packet should decode");

            assert!(matches!(packet.header.packet_type, PacketType::QueueFull));
            assert_eq!(
                packet.header.packet_target.trim_end_matches('\0'),
                "test_queue"
            );
            assert_eq!(packet.header.packet_id, 1234);
            assert_eq!(packet.body, b"queue is full");
        });

        let mut client = TcpStream::connect(addr).unwrap();
        let packet = Packet::new(
            PacketHeader {
                packet_type: PacketType::QueueFull,
                packet_target: String::from("test_queue"),
                packet_id: 1234,
            },
            b"queue is full".to_vec(),
        );

        write_packet(&mut client, packet).expect("packet should encode and send");

        server_handle.join().unwrap();
    }

    #[test]
    fn publish_ack_packet_roundtrips_packet_id() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let server_handle = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let packet = read_packet(&mut socket).expect("packet should decode");

            assert!(matches!(packet.header.packet_type, PacketType::PublishAck));
            assert_eq!(
                packet.header.packet_target.trim_end_matches('\0'),
                "test_queue"
            );
            assert_eq!(packet.header.packet_id, 9999);
            assert_eq!(packet.body, b"accepted");
        });

        let mut client = TcpStream::connect(addr).unwrap();
        let packet = Packet::new(
            PacketHeader {
                packet_type: PacketType::PublishAck,
                packet_target: String::from("test_queue"),
                packet_id: 9999,
            },
            b"accepted".to_vec(),
        );

        write_packet(&mut client, packet).expect("packet should encode and send");

        server_handle.join().unwrap();
    }
}
