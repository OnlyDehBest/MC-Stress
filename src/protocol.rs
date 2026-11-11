use std::io::{self, Read, Write};

use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub fn write_varint(buffer: &mut Vec<u8>, value: i32) {
    let mut remaining = value as u32;
    loop {
        let mut byte = (remaining & 0x7f) as u8;
        remaining >>= 7;
        if remaining != 0 {
            byte |= 0x80;
        }
        buffer.push(byte);
        if remaining == 0 {
            break;
        }
    }
}

pub fn write_string(buffer: &mut Vec<u8>, value: &str) {
    write_varint(buffer, value.len() as i32);
    buffer.extend_from_slice(value.as_bytes());
}

pub fn write_u16(buffer: &mut Vec<u8>, value: u16) {
    buffer.extend_from_slice(&value.to_be_bytes());
}

pub fn write_i64(buffer: &mut Vec<u8>, value: i64) {
    buffer.extend_from_slice(&value.to_be_bytes());
}

pub fn write_f64(buffer: &mut Vec<u8>, value: f64) {
    buffer.extend_from_slice(&value.to_be_bytes());
}

pub fn write_bool(buffer: &mut Vec<u8>, value: bool) {
    buffer.push(if value { 1 } else { 0 });
}

pub struct PayloadReader<'a> {
    bytes: &'a [u8],
    pub position: usize,
}

impl<'a> PayloadReader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        PayloadReader { bytes, position: 0 }
    }

    pub fn read_varint(&mut self) -> io::Result<i32> {
        let mut accumulated = 0u32;
        let mut shift = 0;
        loop {
            let byte = *self
                .bytes
                .get(self.position)
                .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "varint eof"))?;
            self.position += 1;
            accumulated |= ((byte & 0x7f) as u32) << shift;
            if byte & 0x80 == 0 {
                break;
            }
            shift += 7;
            if shift >= 35 {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "varint too long"));
            }
        }
        Ok(accumulated as i32)
    }

    pub fn read_f64(&mut self) -> io::Result<f64> {
        let end = self.position + 8;
        let slice = self
            .bytes
            .get(self.position..end)
            .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "f64 eof"))?;
        self.position = end;
        Ok(f64::from_be_bytes(slice.try_into().unwrap()))
    }

    pub fn skip(&mut self, amount: usize) -> io::Result<()> {
        let end = self.position + amount;
        if end > self.bytes.len() {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "skip eof"));
        }
        self.position = end;
        Ok(())
    }

    pub fn read_bytes(&mut self, amount: usize) -> io::Result<&'a [u8]> {
        let end = self.position + amount;
        let slice = self
            .bytes
            .get(self.position..end)
            .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "bytes eof"))?;
        self.position = end;
        Ok(slice)
    }
}

async fn read_varint_async<R: AsyncRead + Unpin>(reader: &mut R) -> io::Result<i32> {
    let mut accumulated = 0u32;
    let mut shift = 0;
    loop {
        let byte = reader.read_u8().await?;
        accumulated |= ((byte & 0x7f) as u32) << shift;
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
        if shift >= 35 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "varint too long"));
        }
    }
    Ok(accumulated as i32)
}

pub async fn read_packet<R: AsyncRead + Unpin>(
    reader: &mut R,
    compression_threshold: i32,
) -> io::Result<(i32, Vec<u8>)> {
    let frame_length = read_varint_async(reader).await?;
    if frame_length < 0 || frame_length > 8 * 1024 * 1024 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "bad packet length"));
    }
    let mut frame = vec![0u8; frame_length as usize];
    reader.read_exact(&mut frame).await?;

    let decoded = if compression_threshold >= 0 {
        let mut header = PayloadReader::new(&frame);
        let uncompressed_length = header.read_varint()?;
        let body = &frame[header.position..];
        if uncompressed_length == 0 {
            body.to_vec()
        } else {
            let mut output = Vec::with_capacity(uncompressed_length as usize);
            ZlibDecoder::new(body).read_to_end(&mut output)?;
            output
        }
    } else {
        frame
    };

    let mut payload = PayloadReader::new(&decoded);
    let packet_id = payload.read_varint()?;
    Ok((packet_id, decoded[payload.position..].to_vec()))
}

pub async fn write_packet<W: AsyncWrite + Unpin>(
    writer: &mut W,
    compression_threshold: i32,
    packet_id: i32,
    body: &[u8],
) -> io::Result<()> {
    let mut payload = Vec::with_capacity(body.len() + 2);
    write_varint(&mut payload, packet_id);
    payload.extend_from_slice(body);

    let mut frame = Vec::new();
    if compression_threshold >= 0 {
        if payload.len() as i32 >= compression_threshold {
            write_varint(&mut frame, payload.len() as i32);
            let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(&payload)?;
            frame.extend_from_slice(&encoder.finish()?);
        } else {
            write_varint(&mut frame, 0);
            frame.extend_from_slice(&payload);
        }
    } else {
        frame = payload;
    }

    let mut out = Vec::with_capacity(frame.len() + 5);
    write_varint(&mut out, frame.len() as i32);
    out.extend_from_slice(&frame);
    writer.write_all(&out).await?;
    writer.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varint_roundtrip() {
        for value in [0i32, 1, 127, 128, 255, 2097151, 2147483647, -1] {
            let mut buffer = Vec::new();
            write_varint(&mut buffer, value);
            let mut reader = PayloadReader::new(&buffer);
            assert_eq!(reader.read_varint().unwrap(), value, "value {value}");
        }
    }

    #[test]
    fn string_then_f64() {
        let mut buffer = Vec::new();
        write_string(&mut buffer, "en_us");
        write_f64(&mut buffer, 123.5);
        let mut reader = PayloadReader::new(&buffer);
        let length = reader.read_varint().unwrap() as usize;
        assert_eq!(reader.read_bytes(length).unwrap(), b"en_us");
        assert_eq!(reader.read_f64().unwrap(), 123.5);
    }

    #[test]
    fn set_position_flags_differ_by_era() {
        let legacy = crate::bot::set_position_body(1.0, 2.0, 3.0, false);
        let modern = crate::bot::set_position_body(1.0, 2.0, 3.0, true);
        assert_eq!(legacy.len(), modern.len());
        assert_eq!(legacy[24], 1);
        assert_eq!(modern[24], 0x01);
    }
}
