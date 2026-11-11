use std::time::Duration;

use tokio::net::TcpStream;
use tokio::time::Instant;

use crate::protocol::{self, PayloadReader};
use crate::versions;

pub struct ServerStatus {
    pub latency_ms: u64,
    pub online: Option<i64>,
    pub max: Option<i64>,
    pub protocol: Option<i32>,
    pub version_name: Option<String>,
}

pub async fn ping(host: &str, port: u16, handshake_protocol: i32) -> std::io::Result<ServerStatus> {
    let stream = tokio::time::timeout(Duration::from_secs(5), TcpStream::connect((host, port)))
        .await
        .map_err(|_| failure("slp timeout"))??;
    stream.set_nodelay(true).ok();
    let (mut reader, mut writer) = stream.into_split();
    let compression_threshold = -1;

    let mut handshake = Vec::new();
    protocol::write_varint(&mut handshake, handshake_protocol);
    protocol::write_string(&mut handshake, host);
    protocol::write_u16(&mut handshake, port);
    protocol::write_varint(&mut handshake, 1);
    protocol::write_packet(&mut writer, compression_threshold, versions::SB_HANDSHAKE, &handshake).await?;

    protocol::write_packet(&mut writer, compression_threshold, versions::SB_STATUS_REQUEST, &[]).await?;
    let (_packet_id, body) = protocol::read_packet(&mut reader, compression_threshold).await?;
    let mut payload = PayloadReader::new(&body);
    let length = payload.read_varint()? as usize;
    let json = String::from_utf8_lossy(payload.read_bytes(length)?).to_string();
    let parsed = parse_status(&json);

    let before = Instant::now();
    let mut ping_body = Vec::new();
    protocol::write_i64(&mut ping_body, 1234);
    protocol::write_packet(&mut writer, compression_threshold, versions::SB_STATUS_PING, &ping_body).await?;
    let _ = protocol::read_packet(&mut reader, compression_threshold).await?;
    let latency_ms = before.elapsed().as_millis() as u64;

    Ok(ServerStatus {
        latency_ms,
        online: parsed.0,
        max: parsed.1,
        protocol: parsed.2,
        version_name: parsed.3,
    })
}

fn parse_status(json: &str) -> (Option<i64>, Option<i64>, Option<i32>, Option<String>) {
    let value: serde_json::Value = match serde_json::from_str(json) {
        Ok(value) => value,
        Err(_) => return (None, None, None, None),
    };
    let online = value.pointer("/players/online").and_then(|x| x.as_i64());
    let max = value.pointer("/players/max").and_then(|x| x.as_i64());
    let protocol = value.pointer("/version/protocol").and_then(|x| x.as_i64()).map(|x| x as i32);
    let version_name = value.pointer("/version/name").and_then(|x| x.as_str()).map(|x| x.to_string());
    (online, max, protocol, version_name)
}

fn failure(message: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Other, message.to_string())
}
