//! Protocollo Minecraft (Java), parte stabile: framing VarInt, lettura/scrittura
//! pacchetti con compressione opzionale (zlib). Gli ID dei pacchetti, che
//! cambiano per versione, stanno in `ids.rs`.

use std::io::{self, Read, Write};
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

// ---- VarInt / helpers su buffer in memoria ----

pub fn w_varint(buf: &mut Vec<u8>, v: i32) {
    let mut val = v as u32;
    loop {
        let mut b = (val & 0x7f) as u8;
        val >>= 7;
        if val != 0 {
            b |= 0x80;
        }
        buf.push(b);
        if val == 0 {
            break;
        }
    }
}

pub fn w_string(buf: &mut Vec<u8>, s: &str) {
    w_varint(buf, s.len() as i32);
    buf.extend_from_slice(s.as_bytes());
}

pub fn w_u16(buf: &mut Vec<u8>, v: u16) {
    buf.extend_from_slice(&v.to_be_bytes());
}
pub fn w_i64(buf: &mut Vec<u8>, v: i64) {
    buf.extend_from_slice(&v.to_be_bytes());
}
pub fn w_f64(buf: &mut Vec<u8>, v: f64) {
    buf.extend_from_slice(&v.to_be_bytes());
}
pub fn w_bool(buf: &mut Vec<u8>, v: bool) {
    buf.push(if v { 1 } else { 0 });
}

/// Cursore di sola lettura su un payload gia' decompresso.
pub struct Reader<'a> {
    buf: &'a [u8],
    pub pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Reader { buf, pos: 0 }
    }
    pub fn varint(&mut self) -> io::Result<i32> {
        let mut num = 0u32;
        let mut shift = 0;
        loop {
            let b = *self
                .buf
                .get(self.pos)
                .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "varint eof"))?;
            self.pos += 1;
            num |= ((b & 0x7f) as u32) << shift;
            if b & 0x80 == 0 {
                break;
            }
            shift += 7;
            if shift >= 35 {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "varint too long"));
            }
        }
        Ok(num as i32)
    }
    pub fn f64(&mut self) -> io::Result<f64> {
        let end = self.pos + 8;
        let b = self
            .buf
            .get(self.pos..end)
            .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "f64 eof"))?;
        self.pos = end;
        Ok(f64::from_be_bytes(b.try_into().unwrap()))
    }
    pub fn take(&mut self, n: usize) -> io::Result<&'a [u8]> {
        let end = self.pos + n;
        let b = self
            .buf
            .get(self.pos..end)
            .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "take eof"))?;
        self.pos = end;
        Ok(b)
    }
}

// ---- I/O asincrona dei pacchetti ----

async fn read_varint_async<R: AsyncRead + Unpin>(r: &mut R) -> io::Result<i32> {
    let mut num = 0u32;
    let mut shift = 0;
    loop {
        let b = r.read_u8().await?;
        num |= ((b & 0x7f) as u32) << shift;
        if b & 0x80 == 0 {
            break;
        }
        shift += 7;
        if shift >= 35 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "varint too long"));
        }
    }
    Ok(num as i32)
}

/// Legge un pacchetto completo. `threshold < 0` = nessuna compressione.
/// Ritorna (packet_id, body) dove body e' cio' che segue l'id.
pub async fn read_packet<R: AsyncRead + Unpin>(
    r: &mut R,
    threshold: i32,
) -> io::Result<(i32, Vec<u8>)> {
    let len = read_varint_async(r).await?;
    if len < 0 || len > 8 * 1024 * 1024 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "bad packet len"));
    }
    let mut frame = vec![0u8; len as usize];
    r.read_exact(&mut frame).await?;

    let data = if threshold >= 0 {
        let mut rd = Reader::new(&frame);
        let data_len = rd.varint()?;
        let rest = &frame[rd.pos..];
        if data_len == 0 {
            rest.to_vec()
        } else {
            let mut out = Vec::with_capacity(data_len as usize);
            ZlibDecoder::new(rest).read_to_end(&mut out)?;
            out
        }
    } else {
        frame
    };

    let mut rd = Reader::new(&data);
    let id = rd.varint()?;
    Ok((id, data[rd.pos..].to_vec()))
}

/// Scrive un pacchetto. `threshold < 0` = nessuna compressione.
pub async fn write_packet<W: AsyncWrite + Unpin>(
    w: &mut W,
    threshold: i32,
    id: i32,
    body: &[u8],
) -> io::Result<()> {
    let mut payload = Vec::with_capacity(body.len() + 2);
    w_varint(&mut payload, id);
    payload.extend_from_slice(body);

    let mut frame = Vec::new();
    if threshold >= 0 {
        if payload.len() as i32 >= threshold {
            w_varint(&mut frame, payload.len() as i32);
            let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
            enc.write_all(&payload)?;
            frame.extend_from_slice(&enc.finish()?);
        } else {
            w_varint(&mut frame, 0);
            frame.extend_from_slice(&payload);
        }
    } else {
        frame = payload;
    }

    let mut out = Vec::with_capacity(frame.len() + 5);
    w_varint(&mut out, frame.len() as i32);
    out.extend_from_slice(&frame);
    w.write_all(&out).await?;
    w.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varint_roundtrip() {
        for v in [0i32, 1, 127, 128, 255, 2097151, 2147483647, -1] {
            let mut buf = Vec::new();
            w_varint(&mut buf, v);
            let mut rd = Reader::new(&buf);
            assert_eq!(rd.varint().unwrap(), v, "value {v}");
        }
    }

    #[test]
    fn string_and_f64() {
        let mut buf = Vec::new();
        w_string(&mut buf, "en_us");
        w_f64(&mut buf, 123.5);
        let mut rd = Reader::new(&buf);
        let len = rd.varint().unwrap() as usize;
        assert_eq!(rd.take(len).unwrap(), b"en_us");
        assert_eq!(rd.f64().unwrap(), 123.5);
    }
}
