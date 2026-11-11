//! Registro multi-protocollo.
//!
//! Handshake, Login, compressione e Status sono stabili tra le versioni, quindi
//! sono costanti. Gli ID di Configuration e Play cambiano per versione: ognuna ha
//! il suo `Spec`. La 1.21.2 (768) ha anche cambiato il FORMATO dei pacchetti di
//! posizione, gestito dal flag `pos_v2`.
//!
//! La versione giusta viene rilevata via Server List Ping (vedi `slp.rs`); si puo'
//! comunque forzare un protocollo dalla dashboard.
//!
//! ⚠️ Gli ID per-versione vanno verificati sulla tua versione esatta:
//!    https://minecraft.wiki/w/Java_Edition_protocol
//!    (767 / 1.21.1 e' quello testato come flusso; gli altri sono best-effort.)

// --- Stabili: Handshake / Login / Status ---
pub const SB_HANDSHAKE: i32 = 0x00;

pub const SB_LOGIN_START: i32 = 0x00;
pub const SB_LOGIN_ACK: i32 = 0x03;
pub const CB_LOGIN_DISCONNECT: i32 = 0x00;
pub const CB_LOGIN_ENCRYPTION_REQUEST: i32 = 0x01;
pub const CB_LOGIN_SUCCESS: i32 = 0x02;
pub const CB_LOGIN_SET_COMPRESSION: i32 = 0x03;

pub const SB_STATUS_REQUEST: i32 = 0x00;
pub const SB_STATUS_PING: i32 = 0x01;

/// ID dei pacchetti dipendenti dalla versione.
#[derive(Clone, Copy)]
pub struct Spec {
    pub name: &'static str,
    pub protocol: i32,
    // Configuration
    pub cb_cfg_disconnect: i32,
    pub cb_cfg_finish: i32,
    pub cb_cfg_keepalive: i32,
    pub cb_cfg_ping: i32,
    pub cb_cfg_known_packs: i32,
    pub sb_cfg_client_info: i32,
    pub sb_cfg_finish: i32,
    pub sb_cfg_keepalive: i32,
    pub sb_cfg_pong: i32,
    pub sb_cfg_known_packs: i32,
    // Play
    pub cb_play_disconnect: i32,
    pub cb_play_keepalive: i32,
    pub cb_play_sync_pos: i32,
    pub sb_play_confirm_teleport: i32,
    pub sb_play_keepalive: i32,
    pub sb_play_set_pos: i32,
    // Differenze di FORMATO (non solo di ID)
    pub pos_v2: bool,             // 1.21.2+: teleport id in testa, velocity, flags i32; set-position con flags byte
    pub client_info_particle: bool, // 1.21.2+: Client Information ha un campo "particle status" finale
}

// Config state: IDs stabili per 1.20.2–1.21.1; known_packs/ping presenti da 1.20.2.
// (Per 1.20.5+ alcuni ID di config potrebbero differire per i pacchetti Cookie/Transfer: verifica.)
const CFG_1_21: ConfigIds = ConfigIds {
    cb_disconnect: 0x02, cb_finish: 0x03, cb_keepalive: 0x04, cb_ping: 0x05, cb_known_packs: 0x0E,
    sb_client_info: 0x00, sb_finish: 0x03, sb_keepalive: 0x04, sb_pong: 0x05, sb_known_packs: 0x07,
};

struct ConfigIds {
    cb_disconnect: i32, cb_finish: i32, cb_keepalive: i32, cb_ping: i32, cb_known_packs: i32,
    sb_client_info: i32, sb_finish: i32, sb_keepalive: i32, sb_pong: i32, sb_known_packs: i32,
}

const fn spec(
    name: &'static str, protocol: i32, c: ConfigIds,
    cb_play_disconnect: i32, cb_play_keepalive: i32, cb_play_sync_pos: i32,
    sb_play_confirm_teleport: i32, sb_play_keepalive: i32, sb_play_set_pos: i32,
    pos_v2: bool, client_info_particle: bool,
) -> Spec {
    Spec {
        name, protocol,
        cb_cfg_disconnect: c.cb_disconnect, cb_cfg_finish: c.cb_finish, cb_cfg_keepalive: c.cb_keepalive,
        cb_cfg_ping: c.cb_ping, cb_cfg_known_packs: c.cb_known_packs,
        sb_cfg_client_info: c.sb_client_info, sb_cfg_finish: c.sb_finish, sb_cfg_keepalive: c.sb_keepalive,
        sb_cfg_pong: c.sb_pong, sb_cfg_known_packs: c.sb_known_packs,
        cb_play_disconnect, cb_play_keepalive, cb_play_sync_pos,
        sb_play_confirm_teleport, sb_play_keepalive, sb_play_set_pos,
        pos_v2, client_info_particle,
    }
}

// Tabella delle versioni supportate (dalla 1.20.2, prima versione con lo stato
// Configuration). ⚠️ Gli ID di Play per le versioni diverse dalla 1.21.1 sono
// best-effort: verificali e correggili qui.
pub static SPECS: &[Spec] = &[
    // 1.20.2
    spec("1.20.2", 764, CFG_1_21, 0x1B, 0x24, 0x3E, 0x00, 0x14, 0x17, false, false),
    // 1.20.4
    spec("1.20.4", 765, CFG_1_21, 0x1B, 0x24, 0x3E, 0x00, 0x14, 0x17, false, false),
    // 1.20.6
    spec("1.20.6", 766, CFG_1_21, 0x1D, 0x26, 0x40, 0x00, 0x18, 0x1A, false, false),
    // 1.21 / 1.21.1  (flusso testato)
    spec("1.21.1", 767, CFG_1_21, 0x1D, 0x27, 0x42, 0x00, 0x18, 0x1A, false, false),
    // 1.21.2 / 1.21.3  (nuovo formato posizione)
    spec("1.21.3", 768, CFG_1_21, 0x1C, 0x26, 0x41, 0x00, 0x1A, 0x1D, true, true),
    // 1.21.4
    spec("1.21.4", 769, CFG_1_21, 0x1C, 0x26, 0x41, 0x00, 0x1A, 0x1D, true, true),
];

/// Spec di default se non si riesce a rilevare o a riconoscere la versione.
pub const DEFAULT_PROTOCOL: i32 = 767;

pub fn spec_for(protocol: i32) -> Option<&'static Spec> {
    SPECS.iter().find(|s| s.protocol == protocol)
}

pub fn supported() -> Vec<(i32, &'static str)> {
    SPECS.iter().map(|s| (s.protocol, s.name)).collect()
}
