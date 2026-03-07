//! Wire-format encode/decode helpers shared by server and clients.
//!
//! Protocol data is sent as fixed-size frames (`FRAME_HEADER_LENGTH` +
//! `FRAME_BODY_LENGTH`) and reassembled into a logical [`Packet`].
//! Packet headers are fixed width for predictable parsing.

use std::{
    error::Error,
    fmt,
    io::{BufReader, BufWriter, Read, Write},
    net::TcpStream,
    str,
};

use rand::random;

const FRAME_HEADER_LENGTH: usize = 4;
const FRAME_BODY_LENGTH: usize = 1024;
const FRAME_FINAL_FLAG_OFFSET: usize = 0;
const FRAME_BODY_LEN_OFFSET: usize = 1;
const FRAME_BODY_LEN_LENGTH: usize = 2;
const FINAL_FRAME_FLAG: u8 = 0x01;
const NON_FINAL_FRAME_FLAG: u8 = 0x00;

/// One on-the-wire frame containing framing metadata and up to 1024 payload bytes.
#[derive(Debug)]
struct PacketFrame {
    pub header: [u8; FRAME_HEADER_LENGTH],
    pub body: [u8; FRAME_BODY_LENGTH],
}

impl PacketFrame {
    /// Returns an empty frame with all header/body bytes zeroed.
    pub fn blank() -> Self {
        PacketFrame {
            header: [0u8; FRAME_HEADER_LENGTH],
            body: [0u8; FRAME_BODY_LENGTH],
        }
    }

    /// Reads exactly one full frame from the reader.
    fn read_from(reader: &mut impl Read) -> Result<Self, NetworkError> {
        let mut frame = Self::blank();
        reader.read_exact(&mut frame.header)?;
        reader.read_exact(&mut frame.body)?;

        Ok(frame)
    }

    /// Writes exactly one full frame to the writer.
    fn write_to(&self, writer: &mut impl Write) -> Result<(), NetworkError> {
        writer.write_all(&self.header)?;
        writer.write_all(&self.body)?;

        Ok(())
    }

    /// Builds a frame from a packet-data chunk and final-frame marker.
    fn from_chunk(chunk: &[u8], is_final: bool) -> Result<Self, NetworkError> {
        if chunk.len() > FRAME_BODY_LENGTH {
            return Err(NetworkError::InvalidFrameLength {
                declared: chunk.len(),
                max: FRAME_BODY_LENGTH,
            });
        }

        let mut frame = Self::blank();
        frame.header[FRAME_FINAL_FLAG_OFFSET] = if is_final {
            FINAL_FRAME_FLAG
        } else {
            NON_FINAL_FRAME_FLAG
        };

        // Header stores payload length as a big-endian u16 in bytes 1..=2.
        let chunk_len =
            u16::try_from(chunk.len()).map_err(|_| NetworkError::InvalidFrameLength {
                declared: chunk.len(),
                max: FRAME_BODY_LENGTH,
            })?;
        frame.header[FRAME_BODY_LEN_OFFSET..FRAME_BODY_LEN_OFFSET + FRAME_BODY_LEN_LENGTH]
            .copy_from_slice(&chunk_len.to_be_bytes());
        frame.body[..chunk.len()].copy_from_slice(chunk);

        Ok(frame)
    }

    /// Returns true when this frame is marked as the final frame in the packet.
    fn is_final(&self) -> bool {
        self.header[FRAME_FINAL_FLAG_OFFSET] == FINAL_FRAME_FLAG
    }

    /// Returns the declared body length from frame header bytes 1..=2.
    fn body_length(&self) -> Result<usize, NetworkError> {
        let body_length = u16::from_be_bytes([
            self.header[FRAME_BODY_LEN_OFFSET],
            self.header[FRAME_BODY_LEN_OFFSET + 1],
        ]) as usize;

        if body_length > FRAME_BODY_LENGTH {
            return Err(NetworkError::InvalidFrameLength {
                declared: body_length,
                max: FRAME_BODY_LENGTH,
            });
        }

        Ok(body_length)
    }
}

/// Errors produced while reading/writing protocol packets.
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

/// Packet operation discriminator used in protocol headers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketType {
    Publish,
    Subscribe,
    QueueFull,
    PublishAck,
}

impl TryFrom<u8> for PacketType {
    type Error = NetworkError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Publish),
            1 => Ok(Self::Subscribe),
            2 => Ok(Self::QueueFull),
            3 => Ok(Self::PublishAck),
            value => Err(NetworkError::UnknownPacketType(value)),
        }
    }
}

impl From<PacketType> for u8 {
    fn from(value: PacketType) -> Self {
        match value {
            PacketType::Publish => 0,
            PacketType::Subscribe => 1,
            PacketType::QueueFull => 2,
            PacketType::PublishAck => 3,
        }
    }
}

/// Header metadata for a logical packet.
#[derive(Debug)]
pub struct PacketHeader {
    /// Operation represented by this packet.
    pub packet_type: PacketType,
    /// Queue name target, encoded in a fixed-width protocol field.
    pub packet_target: String,
    /// Correlation id carried round-trip across request/response packets.
    pub packet_id: u64,
}

impl PacketHeader {
    /// Creates a header with a random `packet_id`.
    pub fn with_random_id(packet_type: PacketType, packet_target: impl Into<String>) -> Self {
        Self {
            packet_type,
            packet_target: packet_target.into(),
            packet_id: random::<u64>(),
        }
    }

    /// Creates a header with `packet_id = 0`.
    pub fn with_zero_id(packet_type: PacketType, packet_target: impl Into<String>) -> Self {
        Self {
            packet_type,
            packet_target: packet_target.into(),
            packet_id: 0,
        }
    }
}

const PACKET_HEADER_SIZE: usize = 32;
const PACKET_TYPE_OFFSET: usize = 0;
const PACKET_TARGET_OFFSET: usize = 1;
const PACKET_TARGET_SIZE: usize = 16;
const PACKET_ID_OFFSET: usize = PACKET_TARGET_OFFSET + PACKET_TARGET_SIZE;
const PACKET_ID_SIZE: usize = 8;
const PACKET_TARGET_END: usize = PACKET_TARGET_OFFSET + PACKET_TARGET_SIZE;
const PACKET_ID_END: usize = PACKET_ID_OFFSET + PACKET_ID_SIZE;

/// A full decoded protocol packet.
#[derive(Debug)]
pub struct Packet {
    /// Fixed-width protocol header fields.
    pub header: PacketHeader,
    /// Opaque payload bytes.
    pub body: Vec<u8>,
}

impl Packet {
    /// Constructs a packet from header and payload bytes.
    pub fn new(header: PacketHeader, body: Vec<u8>) -> Self {
        Packet { header, body }
    }
}

/// Decodes a fixed-width packet header from raw bytes.
fn decode_packet_header(header_data: &[u8]) -> Result<PacketHeader, NetworkError> {
    if header_data.len() < PACKET_HEADER_SIZE {
        return Err(NetworkError::MalformedPacket(
            "packet payload is smaller than protocol header",
        ));
    }

    let packet_type = PacketType::try_from(header_data[PACKET_TYPE_OFFSET])?;
    let packet_target = str::from_utf8(&header_data[PACKET_TARGET_OFFSET..PACKET_TARGET_END])
        .map_err(NetworkError::InvalidPacketTarget)?
        // Target field is null-padded on write due to fixed-width encoding.
        .trim_end_matches('\0')
        .to_string();
    let packet_id_bytes: [u8; PACKET_ID_SIZE] = header_data[PACKET_ID_OFFSET..PACKET_ID_END]
        .try_into()
        .map_err(|_| NetworkError::MalformedPacket("packet id field has invalid length"))?;
    let packet_id = u64::from_be_bytes(packet_id_bytes);

    Ok(PacketHeader {
        packet_type,
        packet_target,
        packet_id,
    })
}

/// Encodes a packet header into the fixed-width protocol representation.
fn encode_packet_header(header: &PacketHeader) -> Result<[u8; PACKET_HEADER_SIZE], NetworkError> {
    let mut header_data = [0u8; PACKET_HEADER_SIZE];
    header_data[PACKET_TYPE_OFFSET] = u8::from(header.packet_type);

    // No need to pad the packet_target with null bytes
    // as we initialized the header_data to be blank above.
    let packet_target_bytes = header.packet_target.as_bytes();
    if packet_target_bytes.len() > PACKET_TARGET_SIZE {
        return Err(NetworkError::PacketTargetTooLong {
            max_len: PACKET_TARGET_SIZE,
            actual_len: packet_target_bytes.len(),
        });
    }

    header_data[PACKET_TARGET_OFFSET..PACKET_TARGET_OFFSET + packet_target_bytes.len()]
        .copy_from_slice(packet_target_bytes);
    header_data[PACKET_ID_OFFSET..PACKET_ID_END].copy_from_slice(&header.packet_id.to_be_bytes());

    Ok(header_data)
}

/// Reads and decodes one logical packet from a TCP stream.
pub fn read_packet(stream: &mut TcpStream) -> Result<Packet, NetworkError> {
    let mut packet_data: Vec<u8> = Vec::new();

    let mut buf_reader = BufReader::new(stream);

    loop {
        let frame = PacketFrame::read_from(&mut buf_reader)?;
        let frame_is_final = frame.is_final();
        let frame_body_length = frame.body_length()?;

        packet_data.extend_from_slice(&frame.body[..frame_body_length]);

        if frame_is_final {
            break;
        }
    }

    let header = decode_packet_header(&packet_data)?;
    let body = packet_data.split_off(PACKET_HEADER_SIZE);

    Ok(Packet { header, body })
}

/// Encodes and writes one logical packet to a TCP stream.
pub fn write_packet(stream: &mut TcpStream, packet: Packet) -> Result<(), NetworkError> {
    let Packet { header, mut body } = packet;

    let mut packet_data = encode_packet_header(&header)?.to_vec();
    packet_data.append(&mut body);

    let mut buf_writer = BufWriter::new(stream);

    let mut packet_chunks = packet_data.chunks(FRAME_BODY_LENGTH).peekable();
    while let Some(chunk) = packet_chunks.next() {
        let is_final = packet_chunks.peek().is_none();
        let frame = PacketFrame::from_chunk(chunk, is_final)?;
        frame.write_to(&mut buf_writer)?;
    }

    buf_writer.flush()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        NetworkError, Packet, PacketFrame, PacketHeader, PacketType, read_packet, write_packet,
    };
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
            assert_eq!(packet.header.packet_target, "big_queue");
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
    fn packet_type_rejects_unknown_discriminant() {
        let error = PacketType::try_from(99).unwrap_err();

        assert!(matches!(error, NetworkError::UnknownPacketType(99)));
    }

    #[test]
    fn read_packet_rejects_payload_smaller_than_protocol_header() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let server_handle = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let error = read_packet(&mut socket).unwrap_err();

            assert!(matches!(
                error,
                NetworkError::MalformedPacket("packet payload is smaller than protocol header")
            ));
        });

        let mut client = TcpStream::connect(addr).unwrap();
        let frame = PacketFrame::from_chunk(&[0xAA; 8], true).expect("frame should encode");
        frame.write_to(&mut client).expect("frame should send");

        server_handle.join().unwrap();
    }

    #[test]
    fn queue_full_packet_roundtrips() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let server_handle = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let packet = read_packet(&mut socket).expect("packet should decode");

            assert!(matches!(packet.header.packet_type, PacketType::QueueFull));
            assert_eq!(packet.header.packet_target, "test_queue");
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
            assert_eq!(packet.header.packet_target, "test_queue");
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
