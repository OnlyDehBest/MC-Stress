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
    pub teleport_id_leads_position: bool,
    pub client_information_has_particle_status: bool,
    pub play_serverbound_chat_message: i32,
    pub play_serverbound_chat_command: i32,
    pub play_serverbound_swing: i32,
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
    teleport_id_leads_position: bool,
    client_information_has_particle_status: bool,
) -> ProtocolSpec {
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
        teleport_id_leads_position,
        client_information_has_particle_status,
        play_serverbound_chat_message: 0x06,
        play_serverbound_chat_command: 0x04,
        play_serverbound_swing: 0x36,
    }
}

pub static SPECS: &[ProtocolSpec] = &[
    describe("1.20.2", 764, CONFIGURATION_MODERN, 0x1B, 0x24, 0x3E, 0x00, 0x14, 0x17, false, false),
    describe("1.20.4", 765, CONFIGURATION_MODERN, 0x1B, 0x24, 0x3E, 0x00, 0x14, 0x17, false, false),
    describe("1.20.6", 766, CONFIGURATION_MODERN, 0x1D, 0x26, 0x40, 0x00, 0x18, 0x1A, false, false),
    describe("1.21.1", 767, CONFIGURATION_MODERN, 0x1D, 0x27, 0x42, 0x00, 0x18, 0x1A, false, false),
    describe("1.21.3", 768, CONFIGURATION_MODERN, 0x1C, 0x26, 0x41, 0x00, 0x1A, 0x1D, true, true),
    describe("1.21.4", 769, CONFIGURATION_MODERN, 0x1C, 0x26, 0x41, 0x00, 0x1A, 0x1D, true, true),
    describe("1.21.5", 770, CONFIGURATION_MODERN, 0x1C, 0x26, 0x41, 0x00, 0x1A, 0x1D, true, true),
    describe("1.21.6", 771, CONFIGURATION_MODERN, 0x1C, 0x26, 0x41, 0x00, 0x1A, 0x1D, true, true),
    describe("1.21.8", 772, CONFIGURATION_MODERN, 0x1C, 0x26, 0x41, 0x00, 0x1A, 0x1D, true, true),
    describe("26.1", 773, CONFIGURATION_MODERN, 0x1C, 0x26, 0x41, 0x00, 0x1A, 0x1D, true, true),
    describe("26.2", 774, CONFIGURATION_MODERN, 0x1C, 0x26, 0x41, 0x00, 0x1A, 0x1D, true, true),
    describe("26.3", 775, CONFIGURATION_MODERN, 0x1C, 0x26, 0x41, 0x00, 0x1A, 0x1D, true, true),
];

pub fn spec_for(protocol: i32) -> Option<&'static ProtocolSpec> {
    SPECS.iter().find(|spec| spec.protocol == protocol)
}

pub fn supported() -> Vec<(i32, &'static str)> {
    SPECS.iter().map(|spec| (spec.protocol, spec.name)).collect()
}
