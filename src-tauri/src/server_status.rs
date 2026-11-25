//! Мінімальний Server List Ping (той самий запит, що робить меню "Мережева гра"):
//! показує на головному екрані, чи сервер онлайн і скільки там гравців.

use std::time::Duration;

use serde::Serialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

use crate::error::{fail, Result};

const TIMEOUT: Duration = Duration::from_secs(4);
const MAX_RESPONSE: usize = 512 * 1024;

#[derive(Serialize, Clone, Debug, Default)]
pub struct ServerStatus {
    pub online: bool,
    pub players: u32,
    pub max_players: u32,
    pub version: String,
    pub latency_ms: u64,
}

fn write_varint(out: &mut Vec<u8>, value: i32) {
    let mut value = value as u32;
    loop {
        if value & !0x7f == 0 {
            out.push(value as u8);
            return;
        }
        out.push(((value & 0x7f) | 0x80) as u8);
        value >>= 7;
    }
}

async fn read_varint(stream: &mut TcpStream) -> Result<i32> {
    let mut result: u32 = 0;
    for shift in 0..5 {
        let byte = stream.read_u8().await?;
        result |= ((byte & 0x7f) as u32) << (7 * shift);
        if byte & 0x80 == 0 {
            return Ok(result as i32);
        }
    }
    fail("некоректна відповідь сервера (varint)")
}

fn split_address(address: &str) -> Option<(String, u16)> {
    let address = address.trim();
    if address.is_empty() {
        return None;
    }
    match address.rsplit_once(':') {
        Some((host, port)) if !host.is_empty() => Some((host.to_string(), port.parse().ok()?)),
        _ => Some((address.to_string(), 25565)),
    }
}

async fn query(host: &str, port: u16) -> Result<ServerStatus> {
    let started = std::time::Instant::now();
    let mut stream = TcpStream::connect((host, port)).await?;
    stream.set_nodelay(true)?;

    let mut handshake = Vec::new();
    write_varint(&mut handshake, 0x00);
    write_varint(&mut handshake, -1);
    write_varint(&mut handshake, host.len() as i32);
    handshake.extend_from_slice(host.as_bytes());
    handshake.extend_from_slice(&port.to_be_bytes());
    write_varint(&mut handshake, 1);

    let mut packet = Vec::new();
    write_varint(&mut packet, handshake.len() as i32);
    packet.extend_from_slice(&handshake);
    packet.extend_from_slice(&[0x01, 0x00]);
    stream.write_all(&packet).await?;
    stream.flush().await?;

    let _length = read_varint(&mut stream).await?;
    let packet_id = read_varint(&mut stream).await?;
    if packet_id != 0 {
        return fail("неочікувана відповідь сервера");
    }
    let json_length = read_varint(&mut stream).await?;
    if json_length <= 0 || json_length as usize > MAX_RESPONSE {
        return fail("некоректна довжина відповіді сервера");
    }
    let mut buffer = vec![0u8; json_length as usize];
    stream.read_exact(&mut buffer).await?;
    let latency_ms = started.elapsed().as_millis() as u64;

    let value: serde_json::Value = serde_json::from_slice(&buffer)?;
    Ok(ServerStatus {
        online: true,
        players: value
            .pointer("/players/online")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
        max_players: value
            .pointer("/players/max")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
        version: value
            .pointer("/version/name")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .chars()
            .take(40)
            .collect(),
        latency_ms,
    })
}

/// Ніколи не повертає помилку для "сервер недоступний" — тоді просто online = false.
pub async fn ping(address: &str) -> ServerStatus {
    let Some((host, port)) = split_address(address) else {
        return ServerStatus::default();
    };
    match timeout(TIMEOUT, query(&host, port)).await {
        Ok(Ok(status)) => status,
        _ => ServerStatus::default(),
    }
}
