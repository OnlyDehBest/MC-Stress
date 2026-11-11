//! ID dei pacchetti, DIPENDENTI DALLA VERSIONE.
//!
//! Valori per protocollo **767 (Minecraft 1.21 / 1.21.1)**. Gli ID di Play e
//! Configuration cambiano tra versioni (1.21.2+ ha spostato parecchi pacchetti):
//! se il tuo server e' su un'altra versione, verifica e aggiorna qui i valori.
//! Riferimento: https://minecraft.wiki/w/Java_Edition_protocol
//!
//! Handshake e Login sono stabili da molte versioni, quindi quelli raramente
//! cambiano.

pub const PROTOCOL_VERSION: i32 = 767; // 1.21 / 1.21.1
pub const VERSION_NAME: &str = "1.21.1";

// --- Handshake (serverbound) ---
pub const SB_HANDSHAKE: i32 = 0x00;

// --- Login ---
pub const SB_LOGIN_START: i32 = 0x00;
pub const SB_LOGIN_ACK: i32 = 0x03;
pub const CB_LOGIN_DISCONNECT: i32 = 0x00;
pub const CB_LOGIN_ENCRYPTION_REQUEST: i32 = 0x01;
pub const CB_LOGIN_SUCCESS: i32 = 0x02;
pub const CB_LOGIN_SET_COMPRESSION: i32 = 0x03;

// --- Configuration ---
pub const SB_CONFIG_CLIENT_INFORMATION: i32 = 0x00;
pub const SB_CONFIG_FINISH: i32 = 0x03;
pub const SB_CONFIG_KEEP_ALIVE: i32 = 0x04;
pub const SB_CONFIG_PONG: i32 = 0x05;
pub const SB_CONFIG_KNOWN_PACKS: i32 = 0x07;

pub const CB_CONFIG_DISCONNECT: i32 = 0x02;
pub const CB_CONFIG_FINISH: i32 = 0x03;
pub const CB_CONFIG_KEEP_ALIVE: i32 = 0x04;
pub const CB_CONFIG_PING: i32 = 0x05;
pub const CB_CONFIG_KNOWN_PACKS: i32 = 0x0E;

// --- Play ---
pub const SB_PLAY_CONFIRM_TELEPORT: i32 = 0x00;
pub const SB_PLAY_KEEP_ALIVE: i32 = 0x18;
pub const SB_PLAY_SET_POSITION: i32 = 0x1A;

pub const CB_PLAY_DISCONNECT: i32 = 0x1D;
pub const CB_PLAY_KEEP_ALIVE: i32 = 0x27;
pub const CB_PLAY_SYNC_POSITION: i32 = 0x42;

// --- Status (per la SLP / latenza) ---
pub const SB_STATUS_REQUEST: i32 = 0x00;
pub const SB_STATUS_PING: i32 = 0x01;
#[allow(dead_code)]
pub const CB_STATUS_RESPONSE: i32 = 0x00;
#[allow(dead_code)]
pub const CB_STATUS_PONG: i32 = 0x01;
