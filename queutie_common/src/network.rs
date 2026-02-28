use std::{
    io::{BufReader, BufWriter, Read, Write},
    net::TcpStream,
};

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
        return PacketFrame {
            header: [0u8; FRAME_HEADER_LENGTH],
            body: [0u8; FRAME_BODY_LENGTH],
        };
    }
}

fn read_frame(buf_reader: &mut BufReader<&mut TcpStream>) -> PacketFrame {
    let mut frame = PacketFrame::blank();
    buf_reader.read_exact(&mut frame.header).unwrap();
    buf_reader.read_exact(&mut frame.body).unwrap();

    frame
}

fn write_frame(buf_writer: &mut BufWriter<&mut TcpStream>, frame: &PacketFrame) {
    buf_writer.write(&frame.header).unwrap();
    buf_writer.write(&frame.body).unwrap();

    buf_writer.flush().unwrap();
}

#[derive(Debug)]
pub enum PacketType {
    Publish,
    Subscribe,
}

#[derive(Debug)]
pub struct PacketHeader {
    pub packet_type: PacketType,
    pub packet_target: String,
}

const PACKET_HEADER_SIZE: usize = 32;
const PACKET_TARGET_SIZE: usize = 16;

#[derive(Debug)]
pub struct Packet {
    pub header: PacketHeader,
    pub body: Vec<u8>,
}

impl Packet {
    pub fn new(header: PacketHeader, body: Vec<u8>) -> Self {
        return Packet { header, body };
    }
}

pub fn read_packet(stream: &mut TcpStream) -> Packet {
    let mut packet_data: Vec<u8> = Vec::new();

    let mut buf_reader = BufReader::new(stream);

    loop {
        let frame = read_frame(&mut buf_reader);

        let frame_is_final = frame.header[0] == 0x01;
        let frame_body_length =
            u16::from_be_bytes(frame.header[1..=2].try_into().unwrap()) as usize;

        packet_data.extend_from_slice(&frame.body[..frame_body_length]);

        if frame_is_final {
            break;
        }
    }

    let body = packet_data.split_off(PACKET_HEADER_SIZE);
    let packet_type = match packet_data[0] {
        0 => PacketType::Publish,
        1 => PacketType::Subscribe,
        _ => panic!("unknown package type"),
    };
    let packet_target = String::from(str::from_utf8(&packet_data[1..PACKET_TARGET_SIZE]).unwrap());
    let header = PacketHeader {
        packet_target,
        packet_type,
    };

    Packet { header, body }
}

pub fn write_packet(stream: &mut TcpStream, packet: Packet) {
    let Packet { header, mut body } = packet;

    let mut packet_data = Vec::from([0u8; PACKET_HEADER_SIZE]);
    let PacketHeader {
        packet_type,
        packet_target,
    } = header;
    packet_data[0] = match packet_type {
        PacketType::Publish => 0,
        PacketType::Subscribe => 1,
    };
    let packet_target_bytes = packet_target.as_bytes();
    if packet_target_bytes.len() > PACKET_TARGET_SIZE {
        panic!("Packet target too big!")
    }
    packet_data[1..1 + packet_target_bytes.len()].copy_from_slice(packet_target_bytes);
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
        frame_body[..bytes_to_write].copy_from_slice(&packet_data[read_offset..]);
        read_offset += bytes_to_write;

        let frame = PacketFrame {
            header: frame_header,
            body: frame_body,
        };

        write_frame(&mut buf_writer, &frame);
    }
}
