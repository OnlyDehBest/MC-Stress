use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rand::Rng;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinSet;
use tokio::time::{interval, Instant};

use crate::model::{BotModel, WALK_BLOCKS_PER_SECOND};
use crate::swarm::BotSwarm;
use crate::protocol::{self, PayloadReader};
use crate::versions::{self, ProtocolSpec};
use crate::world::World;

type WorldHandle = Arc<Mutex<Option<World>>>;

#[derive(Clone)]
pub struct BotConfig {
    pub host: String,
    pub port: u16,
    pub spec: &'static ProtocolSpec,
    pub view_distance: i8,
    pub spread_seconds: f64,
    pub model: BotModel,
    pub chat_message: Option<String>,
    pub command: Option<String>,
    pub swing: bool,
    pub action_interval: Duration,
}

enum Tracked {
    Connected,
    Moving,
}

struct CountScope {
    swarm: Arc<BotSwarm>,
    tracked: Tracked,
}

impl CountScope {
    fn enter(swarm: Arc<BotSwarm>, tracked: Tracked) -> Self {
        match tracked {
            Tracked::Connected => swarm.connected.fetch_add(1, Ordering::Relaxed),
            Tracked::Moving => swarm.moving.fetch_add(1, Ordering::Relaxed),
        };
        CountScope { swarm, tracked }
    }
}

impl Drop for CountScope {
    fn drop(&mut self) {
        match self.tracked {
            Tracked::Connected => self.swarm.connected.fetch_sub(1, Ordering::Relaxed),
            Tracked::Moving => self.swarm.moving.fetch_sub(1, Ordering::Relaxed),
        };
    }
}

pub async fn run_bot(config: BotConfig, username: String, swarm: Arc<BotSwarm>) {
    if let Err(reason) = join_and_hold(&config, &username, &swarm).await {
        swarm.errors.fetch_add(1, Ordering::Relaxed);
        if swarm.errors.load(Ordering::Relaxed) <= 8 {
            swarm.log(format!("{username}: {reason}"));
        }
    }
}

async fn join_and_hold(
    config: &BotConfig,
    username: &str,
    swarm: &Arc<BotSwarm>,
) -> std::io::Result<()> {
    let spec = config.spec;
    let stream = tokio::time::timeout(
        Duration::from_secs(10),
        TcpStream::connect((config.host.as_str(), config.port)),
    )
    .await
    .map_err(|_| failure("connect timeout"))??;
    stream.set_nodelay(true).ok();

    let (mut reader, mut writer) = stream.into_split();
    let mut compression_threshold: i32 = -1;

    let mut handshake = Vec::new();
    protocol::write_varint(&mut handshake, spec.protocol);
    protocol::write_string(&mut handshake, &config.host);
    protocol::write_u16(&mut handshake, config.port);
    protocol::write_varint(&mut handshake, 2);
    protocol::write_packet(&mut writer, compression_threshold, versions::SB_HANDSHAKE, &handshake).await?;

    let mut login_start = Vec::new();
    protocol::write_string(&mut login_start, username);
    let random_uuid: [u8; 16] = rand::thread_rng().gen();
    login_start.extend_from_slice(&random_uuid);
    protocol::write_packet(&mut writer, compression_threshold, versions::SB_LOGIN_START, &login_start).await?;

    loop {
        let (packet_id, body) = protocol::read_packet(&mut reader, compression_threshold).await?;
        if packet_id == versions::CB_LOGIN_SET_COMPRESSION {
            compression_threshold = PayloadReader::new(&body).read_varint()?;
        } else if packet_id == versions::CB_LOGIN_SUCCESS {
            protocol::write_packet(&mut writer, compression_threshold, versions::SB_LOGIN_ACKNOWLEDGED, &[]).await?;
            break;
        } else if packet_id == versions::CB_LOGIN_DISCONNECT {
            return Err(failure("login disconnect"));
        } else if packet_id == versions::CB_LOGIN_ENCRYPTION_REQUEST {
            return Err(failure("server is in online-mode: offline-mode is required"));
        }
    }

    send_client_information(&mut writer, compression_threshold, config).await?;
    loop {
        let (packet_id, body) = protocol::read_packet(&mut reader, compression_threshold).await?;
        if packet_id == spec.config_clientbound_keep_alive {
            protocol::write_packet(&mut writer, compression_threshold, spec.config_serverbound_keep_alive, &body).await?;
        } else if packet_id == spec.config_clientbound_ping {
            protocol::write_packet(&mut writer, compression_threshold, spec.config_serverbound_pong, &body).await?;
        } else if packet_id == spec.config_clientbound_known_packs {
            let mut empty = Vec::new();
            protocol::write_varint(&mut empty, 0);
            protocol::write_packet(&mut writer, compression_threshold, spec.config_serverbound_known_packs, &empty).await?;
        } else if packet_id == spec.config_clientbound_finish {
            protocol::write_packet(&mut writer, compression_threshold, spec.config_serverbound_finish, &[]).await?;
            break;
        } else if packet_id == spec.config_clientbound_disconnect {
            return Err(failure("config disconnect"));
        }
    }

    let _connected = CountScope::enter(swarm.clone(), Tracked::Connected);

    // JoinSet aborts these on drop, so an aborted bot closes its socket at once instead of lingering.
    let mut tasks = JoinSet::new();
    let (outgoing, mut outgoing_rx) = mpsc::channel::<(i32, Vec<u8>)>(64);
    tasks.spawn(async move {
        while let Some((packet_id, body)) = outgoing_rx.recv().await {
            if protocol::write_packet(&mut writer, compression_threshold, packet_id, &body).await.is_err() {
                break;
            }
        }
    });

    let position: Arc<Mutex<Option<(f64, f64, f64)>>> = Arc::new(Mutex::new(None));
    let world: WorldHandle = Arc::new(Mutex::new(if spec.world_supported {
        Some(World::new(spec.protocol))
    } else {
        None
    }));
    tasks.spawn(follow_model(
        config.clone(),
        swarm.clone(),
        outgoing.clone(),
        position.clone(),
        world.clone(),
    ));

    let has_activity = config.chat_message.is_some() || config.command.is_some() || config.swing;
    if has_activity {
        tasks.spawn(run_activity(config.clone(), outgoing.clone()));
    }

    hold_in_play(&mut reader, compression_threshold, spec, &outgoing, &position, &world).await
}

async fn run_activity(config: BotConfig, outgoing: mpsc::Sender<(i32, Vec<u8>)>) {
    let spec = config.spec;
    tokio::time::sleep(Duration::from_secs(2)).await;
    let mut tick = interval(config.action_interval.max(Duration::from_millis(500)));
    loop {
        tick.tick().await;
        if config.swing {
            let mut body = Vec::new();
            protocol::write_varint(&mut body, 0);
            if outgoing.send((spec.play_serverbound_swing_arm, body)).await.is_err() {
                return;
            }
        }
        if let Some(command) = &config.command {
            let mut body = Vec::new();
            protocol::write_string(&mut body, command);
            if outgoing.send((spec.play_serverbound_chat_command, body)).await.is_err() {
                return;
            }
        }
        if let Some(message) = &config.chat_message {
            if outgoing.send((spec.play_serverbound_chat_message, chat_message_body(message, spec.chat_message_has_checksum))).await.is_err() {
                return;
            }
        }
    }
}

fn chat_message_body(message: &str, has_checksum: bool) -> Vec<u8> {
    let mut body = Vec::new();
    protocol::write_string(&mut body, message);
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as i64;
    protocol::write_i64(&mut body, now);
    protocol::write_i64(&mut body, 0);
    protocol::write_bool(&mut body, false);
    protocol::write_varint(&mut body, 0);
    body.extend_from_slice(&[0, 0, 0]);
    if has_checksum {
        body.push(0x00);
    }
    body
}

async fn hold_in_play(
    reader: &mut (impl tokio::io::AsyncRead + Unpin),
    compression_threshold: i32,
    spec: &'static ProtocolSpec,
    outgoing: &mpsc::Sender<(i32, Vec<u8>)>,
    position: &Arc<Mutex<Option<(f64, f64, f64)>>>,
    world: &WorldHandle,
) -> std::io::Result<()> {
    loop {
        let (packet_id, body) = protocol::read_packet(reader, compression_threshold).await?;
        if spec.world_supported && apply_world_packet(spec, packet_id, &body, world).await {
            continue;
        }
        if packet_id == spec.play_clientbound_keep_alive {
            let _ = outgoing.send((spec.play_serverbound_keep_alive, body)).await;
        } else if packet_id == spec.play_clientbound_synchronize_position {
            let mut reader = PayloadReader::new(&body);
            let (x, y, z, teleport_id) = if spec.teleport_id_leads_position {
                let teleport_id = reader.read_varint()?;
                let x = reader.read_f64()?;
                let y = reader.read_f64()?;
                let z = reader.read_f64()?;
                (x, y, z, teleport_id)
            } else {
                let x = reader.read_f64()?;
                let y = reader.read_f64()?;
                let z = reader.read_f64()?;
                reader.skip(9)?;
                let teleport_id = reader.read_varint()?;
                (x, y, z, teleport_id)
            };

            let mut confirm = Vec::new();
            protocol::write_varint(&mut confirm, teleport_id);
            let _ = outgoing.send((spec.play_serverbound_confirm_teleport, confirm)).await;

            *position.lock().await = Some((x, y, z));
            let _ = outgoing
                .send((spec.play_serverbound_set_position, set_position_body(x, y, z, spec.teleport_id_leads_position)))
                .await;
        } else if packet_id == spec.play_clientbound_disconnect {
            return Ok(());
        }
    }
}

async fn apply_world_packet(
    spec: &'static ProtocolSpec,
    packet_id: i32,
    body: &[u8],
    world: &WorldHandle,
) -> bool {
    if packet_id == spec.play_clientbound_chunk_data {
        if let Some(w) = world.lock().await.as_mut() {
            w.load_chunk(body);
        }
        true
    } else if packet_id == spec.play_clientbound_unload_chunk {
        let mut reader = PayloadReader::new(body);
        if let (Ok(cz), Ok(cx)) = (reader.read_i32(), reader.read_i32()) {
            if let Some(w) = world.lock().await.as_mut() {
                w.unload(cx, cz);
            }
        }
        true
    } else if packet_id == spec.play_clientbound_block_change {
        let mut reader = PayloadReader::new(body);
        if let (Ok(location), Ok(state)) = (reader.read_i64(), reader.read_varint()) {
            let (x, y, z) = decode_position(location);
            if let Some(w) = world.lock().await.as_mut() {
                w.set_block(x, y, z, state as u32);
            }
        }
        true
    } else if packet_id == spec.play_clientbound_multi_block_change {
        apply_multi_block(body, world).await;
        true
    } else {
        false
    }
}

async fn apply_multi_block(body: &[u8], world: &WorldHandle) {
    let mut reader = PayloadReader::new(body);
    let Ok(packed) = reader.read_i64() else { return };
    let section_x = (packed >> 42) as i32;
    let section_z = ((packed << 22) >> 42) as i32;
    let section_y = ((packed << 44) >> 44) as i32;
    let Ok(count) = reader.read_varint() else { return };
    let mut guard = world.lock().await;
    let Some(w) = guard.as_mut() else { return };
    for _ in 0..count {
        let Ok(record) = reader.read_varint() else { return };
        let record = record as u32;
        let state = record >> 12;
        let local_x = ((record >> 8) & 0xF) as i32;
        let local_z = ((record >> 4) & 0xF) as i32;
        let local_y = (record & 0xF) as i32;
        w.set_block(
            section_x * 16 + local_x,
            section_y * 16 + local_y,
            section_z * 16 + local_z,
            state,
        );
    }
}

fn decode_position(value: i64) -> (i32, i32, i32) {
    let x = (value >> 38) as i32;
    let y = ((value << 52) >> 52) as i32;
    let z = ((value << 26) >> 38) as i32;
    (x, y, z)
}

async fn follow_model(
    config: BotConfig,
    swarm: Arc<BotSwarm>,
    outgoing: mpsc::Sender<(i32, Vec<u8>)>,
    position: Arc<Mutex<Option<(f64, f64, f64)>>>,
    world: WorldHandle,
) {
    let spec = config.spec;
    loop {
        if position.lock().await.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let profile = config.model.profile();
    let heading: f64 = rand::thread_rng().gen_range(0.0..std::f64::consts::TAU);

    if profile.disperse && config.spread_seconds > 0.0 {
        let _moving = CountScope::enter(swarm.clone(), Tracked::Moving);
        let mut disperse_heading = heading;
        let jitter = rand::thread_rng().gen_range(0.6..1.4);
        let seconds = config.spread_seconds.max(0.5) * jitter * profile.disperse_scale;
        let disperse_until = Instant::now() + Duration::from_secs_f64(seconds);
        let mut tick = interval(Duration::from_millis(100));
        loop {
            tick.tick().await;
            if Instant::now() >= disperse_until {
                break;
            }
            let step_x = -disperse_heading.sin() * WALK_BLOCKS_PER_SECOND / 10.0;
            let step_z = disperse_heading.cos() * WALK_BLOCKS_PER_SECOND / 10.0;
            match advance(&position, &outgoing, spec, &world, step_x, step_z).await {
                Err(()) => return,
                Ok(Step::Blocked) => {
                    // Turn away from the obstacle instead of pushing into it.
                    disperse_heading = rand::thread_rng().gen_range(0.0..std::f64::consts::TAU);
                }
                Ok(Step::Moved) => {}
            }
        }
    }

    let continuous = match profile.continuous {
        Some(continuous) => continuous,
        None => {
            settle(&position, &outgoing, spec, &world).await;
            return;
        }
    };

    let _moving = CountScope::enter(swarm.clone(), Tracked::Moving);
    let mut travel_heading = heading;
    let mut swing = 1.0;
    let mut tick = interval(continuous.tick);
    loop {
        tick.tick().await;
        if !continuous.keep_heading {
            travel_heading = rand::thread_rng().gen_range(0.0..std::f64::consts::TAU);
        }
        let direction = if continuous.oscillate_in_place {
            swing = -swing;
            swing
        } else {
            1.0
        };
        let step_x = -travel_heading.sin() * continuous.blocks_per_tick * direction;
        let step_z = travel_heading.cos() * continuous.blocks_per_tick * direction;
        match advance(&position, &outgoing, spec, &world, step_x, step_z).await {
            Err(()) => return,
            Ok(Step::Blocked) => {
                // Obstacle ahead: pick a new heading and route around it.
                travel_heading = rand::thread_rng().gen_range(0.0..std::f64::consts::TAU);
            }
            Ok(Step::Moved) => {}
        }
    }
}

/// Keep applying gravity in place so still models (idle, sentry, spread) rest on
/// the real ground instead of hovering at their spawn height.
async fn settle(
    position: &Arc<Mutex<Option<(f64, f64, f64)>>>,
    outgoing: &mpsc::Sender<(i32, Vec<u8>)>,
    spec: &'static ProtocolSpec,
    world: &WorldHandle,
) {
    let mut tick = interval(Duration::from_millis(250));
    loop {
        tick.tick().await;
        if advance(position, outgoing, spec, world, 0.0, 0.0).await.is_err() {
            return;
        }
    }
}

async fn advance(
    position: &Arc<Mutex<Option<(f64, f64, f64)>>>,
    outgoing: &mpsc::Sender<(i32, Vec<u8>)>,
    spec: &'static ProtocolSpec,
    world: &WorldHandle,
    step_x: f64,
    step_z: f64,
) -> Result<Step, ()> {
    const FALL_PER_TICK: f64 = 4.0;

    let mut current = position.lock().await;
    let Some((x, y, z)) = current.as_mut() else {
        return Ok(Step::Moved);
    };
    let next_x = *x + step_x;
    let next_z = *z + step_z;
    let mut next_y = *y;
    let mut blocked = false;

    // World-aware step: follow the real ground, step up a single block, fall into
    // drops under gravity, and refuse to walk into a wall or a ceiling. Falls
    // back to a flat slide while the destination chunk is not loaded yet.
    {
        let guard = world.lock().await;
        if let Some(w) = guard.as_ref() {
            let bx = next_x.floor() as i32;
            let bz = next_z.floor() as i32;
            let feet = y.floor() as i32;
            if w.is_loaded(bx, bz) {
                match w.surface_y(bx, feet + 1, bz) {
                    Some(surface) if surface - feet <= 1 && w.passable_column(bx, surface, bz) => {
                        let target = surface as f64;
                        next_y = if target < *y {
                            (*y - FALL_PER_TICK).max(target) // fall gradually
                        } else {
                            target // flat or single step up
                        };
                    }
                    _ => blocked = true, // wall, low ceiling or void
                }
            }
        }
    }

    if blocked {
        // Hold the horizontal position, but keep applying gravity in place so the
        // bot still settles onto the ground instead of hovering at a wall.
        let feet_here = y.floor() as i32;
        let guard = world.lock().await;
        if let Some(w) = guard.as_ref() {
            if let Some(surface) = w.surface_y(x.floor() as i32, feet_here + 1, z.floor() as i32) {
                let target = surface as f64;
                if target < *y {
                    *y = (*y - FALL_PER_TICK).max(target);
                    let body = set_position_body(*x, *y, *z, spec.teleport_id_leads_position);
                    let _ = outgoing.send((spec.play_serverbound_set_position, body)).await;
                }
            }
        }
        return Ok(Step::Blocked);
    }

    *x = next_x;
    *z = next_z;
    *y = next_y;
    let body = set_position_body(*x, *y, *z, spec.teleport_id_leads_position);
    if outgoing.send((spec.play_serverbound_set_position, body)).await.is_err() {
        return Err(());
    }
    Ok(Step::Moved)
}

enum Step {
    Moved,
    Blocked,
}

pub(crate) fn set_position_body(x: f64, y: f64, z: f64, modern_flags: bool) -> Vec<u8> {
    let mut body = Vec::with_capacity(25);
    protocol::write_f64(&mut body, x);
    protocol::write_f64(&mut body, y);
    protocol::write_f64(&mut body, z);
    if modern_flags {
        body.push(0x01);
    } else {
        protocol::write_bool(&mut body, true);
    }
    body
}

async fn send_client_information(
    writer: &mut (impl tokio::io::AsyncWrite + Unpin),
    compression_threshold: i32,
    config: &BotConfig,
) -> std::io::Result<()> {
    let mut body = Vec::new();
    protocol::write_string(&mut body, "en_us");
    body.push(config.view_distance as u8);
    protocol::write_varint(&mut body, 0);
    protocol::write_bool(&mut body, true);
    body.push(0x7f);
    protocol::write_varint(&mut body, 1);
    protocol::write_bool(&mut body, false);
    protocol::write_bool(&mut body, true);
    if config.spec.client_information_has_particle_status {
        protocol::write_varint(&mut body, 0);
    }
    protocol::write_packet(writer, compression_threshold, config.spec.config_serverbound_client_information, &body).await
}

fn failure(message: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Other, message.to_string())
}
