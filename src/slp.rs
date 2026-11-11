//! Server List Ping: misura la latenza del server (e legge online/max player).
//! Serve come segnale per il breaking point e per la dashboard.

use std::time::Duration;

use tokio::net::TcpStream;
use tokio::time::Instant;

use crate::ids;
use crate::protocol::{self as proto, Reader};

pub struct Status {
    pub latency_ms: u64,
    pub online: Option<i64>,
    pub max: Option<i64>,
    pub protocol: Option<i32>,
    pub version_name: Option<String>,
}

pub async fn ping(host: &str, port: u16, protocol: i32) -> std::io::Result<Status> {
    let stream = tokio::time::timeout(Duration::from_secs(5), TcpStream::connect((host, port)))
        .await
        .map_err(|_| err("slp timeout"))??;
    stream.set_nodelay(true).ok();
    let (mut rd, mut wr) = stream.into_split();
    let threshold = -1; // lo status non usa compressione

    // Handshake (next state = 1: status)
    let mut b = Vec::new();
    proto::w_varint(&mut b, protocol);
    proto::w_string(&mut b, host);
    proto::w_u16(&mut b, port);
    proto::w_varint(&mut b, 1);
    proto::write_packet(&mut wr, threshold, ids::SB_HANDSHAKE, &b).await?;

    // Status Request
    proto::write_packet(&mut wr, threshold, ids::SB_STATUS_REQUEST, &[]).await?;
    let (_id, data) = proto::read_packet(&mut rd, threshold).await?;
    let mut r = Reader::new(&data);
    let len = r.varint()? as usize;
    let json = String::from_utf8_lossy(r.take(len)?).to_string();
    let (online, max, protocol, version_name) = parse_status(&json);

    // Ping -> Pong per misurare la latenza
    let now = Instant::now();
    let mut b = Vec::new();
    proto::w_i64(&mut b, 1234);
    proto::write_packet(&mut wr, threshold, ids::SB_STATUS_PING, &b).await?;
    let _ = proto::read_packet(&mut rd, threshold).await?;
    let latency_ms = now.elapsed().as_millis() as u64;

    Ok(Status { latency_ms, online, max, protocol, version_name })
}

fn parse_status(json: &str) -> (Option<i64>, Option<i64>, Option<i32>, Option<String>) {
    let v: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return (None, None, None, None),
    };
    let online = v.pointer("/players/online").and_then(|x| x.as_i64());
    let max = v.pointer("/players/max").and_then(|x| x.as_i64());
    let protocol = v.pointer("/version/protocol").and_then(|x| x.as_i64()).map(|p| p as i32);
    let version_name = v.pointer("/version/name").and_then(|x| x.as_str()).map(|s| s.to_string());
    (online, max, protocol, version_name)
}

fn err(msg: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Other, msg.to_string())
}
