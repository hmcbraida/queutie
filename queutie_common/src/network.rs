use std::{
    io::{BufReader, BufWriter, Read, Write},
    net::TcpStream,
};

const FRAME_HEADER_LENGTH: usize = 4;
const FRAME_BODY_LENGTH: usize = 1024;

#[derive(Debug)]
struct MessageFrame {
    pub header: [u8; FRAME_HEADER_LENGTH],
    pub body: [u8; FRAME_BODY_LENGTH],
}

impl MessageFrame {
    pub fn blank() -> Self {
        return MessageFrame {
            header: [0u8; FRAME_HEADER_LENGTH],
            body: [0u8; FRAME_BODY_LENGTH],
        };
    }
}

fn read_frame(buf_reader: &mut BufReader<&mut TcpStream>) -> MessageFrame {
    let mut frame = MessageFrame::blank();
    buf_reader.read_exact(&mut frame.header).unwrap();
    buf_reader.read_exact(&mut frame.body).unwrap();

    frame
}

fn write_frame(buf_writer: &mut BufWriter<&mut TcpStream>, frame: &MessageFrame) {
    buf_writer.write(&frame.header).unwrap();
    buf_writer.write(&frame.body).unwrap();

    buf_writer.flush().unwrap();
}

#[derive(Debug)]
pub struct Message {
    body: Vec<u8>,
}

impl Message {
    pub fn new(body: Vec<u8>) -> Self {
        return Message { body };
    }
}

pub fn read_message(stream: &mut TcpStream) -> Message {
    let mut message_body: Vec<u8> = Vec::new();

    let mut buf_reader = BufReader::new(stream);

    loop {
        let frame = read_frame(&mut buf_reader);

        let frame_is_final = frame.header[0] == 0x01;
        let frame_body_length =
            u16::from_be_bytes(frame.header[1..=2].try_into().unwrap()) as usize;

        message_body.extend_from_slice(&frame.body[..frame_body_length]);

        if frame_is_final {
            break;
        }
    }

    Message { body: message_body }
}

pub fn write_message(stream: &mut TcpStream, message: &Message) {
    let mut bytes_remaining = message.body.len();
    let mut read_offset: usize = 0;

    let mut buf_writer = BufWriter::new(stream);

    while bytes_remaining > 0 {
        let bytes_to_write = if bytes_remaining > 1024 {
            1024
        } else {
            bytes_remaining
        };
        bytes_remaining -= bytes_to_write;

        // Construct a new message frame
        // First step is constructing the frame header.
        let mut frame_header = [0u8; FRAME_HEADER_LENGTH];
        // Byte zero of the frame is 1 if this the last frame, 0 else
        frame_header[0] = if bytes_remaining == 0 { 0x01 } else { 0x00 };
        // Bytes one to two represent the size of the message in the frame.
        frame_header[1..=2].copy_from_slice(&bytes_to_write.to_be_bytes()[6..]);

        // Next we construct the frame body by copying from the remaining message.
        let mut frame_body = [0u8; FRAME_BODY_LENGTH];
        frame_body[..bytes_to_write].copy_from_slice(&message.body[read_offset..]);
        read_offset += bytes_to_write;

        let frame = MessageFrame {
            header: frame_header,
            body: frame_body,
        };

        write_frame(&mut buf_writer, &frame);
    }
}
