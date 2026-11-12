pub const SB_HANDSHAKE: i32 = 0x00;

pub const SB_LOGIN_START: i32 = 0x00;
pub const SB_LOGIN_ACKNOWLEDGED: i32 = 0x03;
pub const CB_LOGIN_DISCONNECT: i32 = 0x00;
pub const CB_LOGIN_ENCRYPTION_REQUEST: i32 = 0x01;
pub const CB_LOGIN_SUCCESS: i32 = 0x02;
pub const CB_LOGIN_SET_COMPRESSION: i32 = 0x03;

pub const SB_STATUS_REQUEST: i32 = 0x00;
pub const SB_STATUS_PING: i32 = 0x01;

pub const DEFAULT_PROTOCOL: i32 = 767;

#[derive(Clone, Copy)]
#[allow(dead_code)]
pub struct ProtocolSpec {
    pub name: &'static str,
    pub protocol: i32,
    pub config_clientbound_disconnect: i32,
    pub config_clientbound_finish: i32,
    pub config_clientbound_keep_alive: i32,
    pub config_clientbound_ping: i32,
    pub config_clientbound_known_packs: i32,
    pub config_serverbound_client_information: i32,
    pub config_serverbound_finish: i32,
    pub config_serverbound_keep_alive: i32,
    pub config_serverbound_pong: i32,
    pub config_serverbound_known_packs: i32,
    pub play_clientbound_disconnect: i32,
    pub play_clientbound_keep_alive: i32,
    pub play_clientbound_synchronize_position: i32,
    pub play_serverbound_confirm_teleport: i32,
    pub play_serverbound_keep_alive: i32,
    pub play_serverbound_set_position: i32,
    pub play_serverbound_chat_message: i32,
    pub play_serverbound_chat_command: i32,
    pub play_serverbound_swing_arm: i32,
    pub play_serverbound_player_action: i32,
    pub teleport_id_leads_position: bool,
    pub client_information_has_particle_status: bool,
    pub chat_message_has_checksum: bool,
    pub play_clientbound_chunk_data: i32,
    pub play_clientbound_unload_chunk: i32,
    pub play_clientbound_block_change: i32,
    pub play_clientbound_multi_block_change: i32,
    pub world_supported: bool,
    pub play_serverbound_use_entity: i32,
    pub play_serverbound_use_item: i32,
    pub play_clientbound_spawn_entity: i32,
    pub play_clientbound_entity_move: i32,
    pub play_clientbound_entity_move_look: i32,
    pub play_clientbound_entity_teleport: i32,
    pub play_clientbound_entity_destroy: i32,
}

struct ConfigurationIds {
    clientbound_disconnect: i32,
    clientbound_finish: i32,
    clientbound_keep_alive: i32,
    clientbound_ping: i32,
    clientbound_known_packs: i32,
    serverbound_client_information: i32,
    serverbound_finish: i32,
    serverbound_keep_alive: i32,
    serverbound_pong: i32,
    serverbound_known_packs: i32,
}

struct ActivityIds {
    chat_message: i32,
    chat_command: i32,
    swing_arm: i32,
    player_action: i32,
}

const CONFIGURATION_MODERN: ConfigurationIds = ConfigurationIds {
    clientbound_disconnect: 0x02,
    clientbound_finish: 0x03,
    clientbound_keep_alive: 0x04,
    clientbound_ping: 0x05,
    clientbound_known_packs: 0x0E,
    serverbound_client_information: 0x00,
    serverbound_finish: 0x03,
    serverbound_keep_alive: 0x04,
    serverbound_pong: 0x05,
    serverbound_known_packs: 0x07,
};

const CONFIGURATION_LEGACY: ConfigurationIds = ConfigurationIds {
    clientbound_disconnect: 0x01,
    clientbound_finish: 0x02,
    clientbound_keep_alive: 0x03,
    clientbound_ping: 0x04,
    clientbound_known_packs: 0xFF,
    serverbound_client_information: 0x00,
    serverbound_finish: 0x02,
    serverbound_keep_alive: 0x03,
    serverbound_pong: 0x04,
    serverbound_known_packs: 0xFF,
};

const fn describe(
    name: &'static str,
    protocol: i32,
    configuration: ConfigurationIds,
    play_clientbound_disconnect: i32,
    play_clientbound_keep_alive: i32,
    play_clientbound_synchronize_position: i32,
    play_serverbound_confirm_teleport: i32,
    play_serverbound_keep_alive: i32,
    play_serverbound_set_position: i32,
    activity: ActivityIds,
    teleport_id_leads_position: bool,
    client_information_has_particle_status: bool,
) -> ProtocolSpec {
    // Clientbound chunk/block packet IDs (world model). Only the protocols with
    // the array-heightmap chunk format (>= 770) are mapped; others disable it.
    let (chunk_data, unload_chunk, block_change, multi_block_change) = match protocol {
        770 | 771 | 772 => (0x27, 0x21, 0x08, 0x4D),
        773 | 774 => (0x2C, 0x25, 0x08, 0x52),
        775 => (0x2D, 0x25, 0x08, 0x54),
        _ => (-1, -1, -1, -1),
    };
    // Serverbound use_entity/use_item + clientbound entity spawn/move/teleport/destroy.
    let (use_entity, use_item, spawn_entity, entity_move, entity_move_look, entity_teleport, entity_destroy) = match protocol {
        770 => (0x18, 0x3F, 0x01, 0x2E, 0x2F, 0x76, 0x46),
        771 | 772 => (0x19, 0x40, 0x01, 0x2E, 0x2F, 0x76, 0x46),
        773 | 774 => (0x19, 0x40, 0x01, 0x33, 0x34, 0x7B, 0x4B),
        775 => (0x1A, 0x43, 0x01, 0x35, 0x36, 0x7D, 0x4D),
        _ => (-1, -1, -1, -1, -1, -1, -1),
    };
    ProtocolSpec {
        name,
        protocol,
        config_clientbound_disconnect: configuration.clientbound_disconnect,
        config_clientbound_finish: configuration.clientbound_finish,
        config_clientbound_keep_alive: configuration.clientbound_keep_alive,
        config_clientbound_ping: configuration.clientbound_ping,
        config_clientbound_known_packs: configuration.clientbound_known_packs,
        config_serverbound_client_information: configuration.serverbound_client_information,
        config_serverbound_finish: configuration.serverbound_finish,
        config_serverbound_keep_alive: configuration.serverbound_keep_alive,
        config_serverbound_pong: configuration.serverbound_pong,
        config_serverbound_known_packs: configuration.serverbound_known_packs,
        play_clientbound_disconnect,
        play_clientbound_keep_alive,
        play_clientbound_synchronize_position,
        play_serverbound_confirm_teleport,
        play_serverbound_keep_alive,
        play_serverbound_set_position,
        play_serverbound_chat_message: activity.chat_message,
        play_serverbound_chat_command: activity.chat_command,
        play_serverbound_swing_arm: activity.swing_arm,
        play_serverbound_player_action: activity.player_action,
        teleport_id_leads_position,
        client_information_has_particle_status,
        chat_message_has_checksum: protocol >= 770,
        play_clientbound_chunk_data: chunk_data,
        play_clientbound_unload_chunk: unload_chunk,
        play_clientbound_block_change: block_change,
        play_clientbound_multi_block_change: multi_block_change,
        world_supported: protocol >= 770,
        play_serverbound_use_entity: use_entity,
        play_serverbound_use_item: use_item,
        play_clientbound_spawn_entity: spawn_entity,
        play_clientbound_entity_move: entity_move,
        play_clientbound_entity_move_look: entity_move_look,
        play_clientbound_entity_teleport: entity_teleport,
        play_clientbound_entity_destroy: entity_destroy,
    }
}

const fn activity(chat_message: i32, chat_command: i32, swing_arm: i32, player_action: i32) -> ActivityIds {
    ActivityIds { chat_message, chat_command, swing_arm, player_action }
}

// Packet IDs below are generated from PrismarineJS/minecraft-data (canonical).
// Each row carries that exact version's configuration + play packet IDs.
pub static SPECS: &[ProtocolSpec] = &[
    describe("1.20.2", 764, CONFIGURATION_LEGACY, 0x1B, 0x24, 0x3E, 0x00, 0x14, 0x16, activity(0x05, 0x04, 0x32, 0x20), false, false),
    describe("1.20.4", 765, CONFIGURATION_LEGACY, 0x1B, 0x24, 0x3E, 0x00, 0x15, 0x17, activity(0x05, 0x04, 0x33, 0x21), false, false),
    describe("1.20.6", 766, CONFIGURATION_MODERN, 0x1D, 0x26, 0x40, 0x00, 0x18, 0x1A, activity(0x06, 0x04, 0x36, 0x24), false, false),
    describe("1.21.1", 767, CONFIGURATION_MODERN, 0x1D, 0x26, 0x40, 0x00, 0x18, 0x1A, activity(0x06, 0x04, 0x36, 0x24), false, false),
    describe("1.21.3", 768, CONFIGURATION_MODERN, 0x1D, 0x27, 0x42, 0x00, 0x1A, 0x1C, activity(0x07, 0x05, 0x38, 0x26), true, true),
    describe("1.21.4", 769, CONFIGURATION_MODERN, 0x1D, 0x27, 0x42, 0x00, 0x1A, 0x1C, activity(0x07, 0x05, 0x3A, 0x27), true, true),
    describe("1.21.5", 770, CONFIGURATION_MODERN, 0x1C, 0x26, 0x41, 0x00, 0x1A, 0x1C, activity(0x07, 0x05, 0x3B, 0x27), true, true),
    describe("1.21.6", 771, CONFIGURATION_MODERN, 0x1C, 0x26, 0x41, 0x00, 0x1B, 0x1D, activity(0x08, 0x06, 0x3C, 0x28), true, true),
    describe("1.21.8", 772, CONFIGURATION_MODERN, 0x1C, 0x26, 0x41, 0x00, 0x1B, 0x1D, activity(0x08, 0x06, 0x3C, 0x28), true, true),
    describe("1.21.9", 773, CONFIGURATION_MODERN, 0x20, 0x2B, 0x46, 0x00, 0x1B, 0x1D, activity(0x08, 0x06, 0x3C, 0x28), true, true),
    describe("1.21.11", 774, CONFIGURATION_MODERN, 0x20, 0x2B, 0x46, 0x00, 0x1B, 0x1D, activity(0x08, 0x06, 0x3C, 0x28), true, true),
    describe("26.1", 775, CONFIGURATION_MODERN, 0x20, 0x2C, 0x48, 0x00, 0x1C, 0x1E, activity(0x09, 0x07, 0x3F, 0x29), true, true),
];

pub fn spec_for(protocol: i32) -> Option<&'static ProtocolSpec> {
    SPECS.iter().find(|spec| spec.protocol == protocol)
}

/// Newest spec we know about (highest protocol).
pub fn newest() -> &'static ProtocolSpec {
    SPECS.iter().max_by_key(|spec| spec.protocol).unwrap()
}

/// Best spec for an unknown protocol number: the exact match, else the highest
/// known protocol not above it, else the newest. Modern servers are the norm,
/// so an unrecognised (usually newer) protocol lands on the newest spec rather
/// than the oldest.
pub fn closest_spec(protocol: i32) -> &'static ProtocolSpec {
    if let Some(spec) = spec_for(protocol) {
        return spec;
    }
    SPECS
        .iter()
        .filter(|spec| spec.protocol <= protocol)
        .max_by_key(|spec| spec.protocol)
        .unwrap_or_else(|| newest())
}

/// Match a server's reported version name (e.g. "Paper 1.21.11") against our
/// known version names, preferring the longest name that appears in it so
/// "1.21.11" wins over the "1.21.1" substring.
pub fn spec_for_name(name: &str) -> Option<&'static ProtocolSpec> {
    SPECS
        .iter()
        .filter(|spec| name.contains(spec.name))
        .max_by_key(|spec| spec.name.len())
}

pub fn supported() -> Vec<(i32, &'static str)> {
    SPECS.iter().map(|spec| (spec.protocol, spec.name)).collect()
}
