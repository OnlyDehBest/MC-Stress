//! Un singolo bot: client raw, offline-mode. Fa handshake -> login (con
//! compressione) -> configuration -> play, poi tiene viva la connessione
//! (keep-alive), conferma i teleport, invia la view distance e si muove per
//! disperdersi. In "lite" dopo la dispersione sta fermo (zero costo), in
//! "active" continua a muoversi leggermente. Multi-protocollo: gli ID e i
//! formati arrivano dallo `Spec` della versione.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use rand::Rng;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, Mutex};
use tokio::time::{interval, Instant};

use crate::ids::{self, Spec};
use crate::manager::Manager;
use crate::protocol::{self as proto, Reader};

#[derive(Clone)]
pub struct BotConfig {
    pub host: String,
    pub port: u16,
    pub spec: &'static Spec,
    pub view_distance: i8,
    pub spread_secs: f64,
    pub active: bool, // false = lite
}

const WALK_SPEED: f64 = 4.3; // blocchi/s di un player che cammina

/// Decrementa `alive`/`moving` anche se il task viene abortito (Drop su stack).
struct Guard {
    mgr: Arc<Manager>,
    counter: Counter,
}
#[derive(Clone, Copy)]
enum Counter {
    Alive,
    Moving,
}
impl Guard {
    fn new(mgr: Arc<Manager>, counter: Counter) -> Self {
        match counter {
            Counter::Alive => mgr.alive.fetch_add(1, Ordering::Relaxed),
            Counter::Moving => mgr.moving.fetch_add(1, Ordering::Relaxed),
        };
        Guard { mgr, counter }
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        match self.counter {
            Counter::Alive => self.mgr.alive.fetch_sub(1, Ordering::Relaxed),
            Counter::Moving => self.mgr.moving.fetch_sub(1, Ordering::Relaxed),
        };
    }
}

pub async fn run_bot(cfg: BotConfig, username: String, mgr: Arc<Manager>) {
    if let Err(e) = connect_and_play(&cfg, &username, &mgr).await {
        mgr.errors.fetch_add(1, Ordering::Relaxed);
        if mgr.errors.load(Ordering::Relaxed) <= 8 {
            mgr.log(format!("{username}: {e}"));
        }
    }
}

async fn connect_and_play(
    cfg: &BotConfig,
    username: &str,
    mgr: &Arc<Manager>,
) -> std::io::Result<()> {
    let spec = cfg.spec;
    let stream = tokio::time::timeout(
        Duration::from_secs(10),
        TcpStream::connect((cfg.host.as_str(), cfg.port)),
    )
    .await
    .map_err(|_| err("connect timeout"))??;
    stream.set_nodelay(true).ok();

    let (mut rd, mut wr) = stream.into_split();
    let mut threshold: i32 = -1;

    // Handshake (next state = 2: login)
    let mut b = Vec::new();
    proto::w_varint(&mut b, spec.protocol);
    proto::w_string(&mut b, &cfg.host);
    proto::w_u16(&mut b, cfg.port);
    proto::w_varint(&mut b, 2);
    proto::write_packet(&mut wr, threshold, ids::SB_HANDSHAKE, &b).await?;

    // Login Start (name + UUID casuale: in offline il server lo rigenera)
    let mut b = Vec::new();
    proto::w_string(&mut b, username);
    let uuid: [u8; 16] = rand::thread_rng().gen();
    b.extend_from_slice(&uuid);
    proto::write_packet(&mut wr, threshold, ids::SB_LOGIN_START, &b).await?;

    // Login loop
    loop {
        let (id, data) = proto::read_packet(&mut rd, threshold).await?;
        if id == ids::CB_LOGIN_SET_COMPRESSION {
            let mut r = Reader::new(&data);
            threshold = r.varint()?;
        } else if id == ids::CB_LOGIN_SUCCESS {
            proto::write_packet(&mut wr, threshold, ids::SB_LOGIN_ACK, &[]).await?;
            break;
        } else if id == ids::CB_LOGIN_DISCONNECT {
            return Err(err("login disconnect"));
        } else if id == ids::CB_LOGIN_ENCRYPTION_REQUEST {
            return Err(err("server in online-mode: non supportato (serve offline-mode)"));
        }
    }

    // Configuration
    send_client_information(&mut wr, threshold, cfg).await?;
    loop {
        let (id, data) = proto::read_packet(&mut rd, threshold).await?;
        if id == spec.cb_cfg_keepalive {
            proto::write_packet(&mut wr, threshold, spec.sb_cfg_keepalive, &data).await?;
        } else if id == spec.cb_cfg_ping {
            proto::write_packet(&mut wr, threshold, spec.sb_cfg_pong, &data).await?;
        } else if id == spec.cb_cfg_known_packs {
            let mut b = Vec::new();
            proto::w_varint(&mut b, 0); // 0 pacchetti noti
            proto::write_packet(&mut wr, threshold, spec.sb_cfg_known_packs, &b).await?;
        } else if id == spec.cb_cfg_finish {
            proto::write_packet(&mut wr, threshold, spec.sb_cfg_finish, &[]).await?;
            break;
        } else if id == spec.cb_cfg_disconnect {
            return Err(err("config disconnect"));
        }
    }

    // Play
    let _alive = Guard::new(mgr.clone(), Counter::Alive);

    let (tx, mut rx) = mpsc::channel::<(i32, Vec<u8>)>(64);
    let writer = tokio::spawn(async move {
        while let Some((id, body)) = rx.recv().await {
            if proto::write_packet(&mut wr, threshold, id, &body).await.is_err() {
                break;
            }
        }
    });

    let pos: Arc<Mutex<Option<(f64, f64, f64)>>> = Arc::new(Mutex::new(None));
    let mover = tokio::spawn(movement(cfg.clone(), mgr.clone(), tx.clone(), pos.clone()));
    let result = play_loop(&mut rd, threshold, spec, &tx, &pos).await;

    mover.abort();
    drop(tx);
    writer.abort();
    result
}

async fn play_loop(
    rd: &mut (impl tokio::io::AsyncRead + Unpin),
    threshold: i32,
    spec: &'static Spec,
    tx: &mpsc::Sender<(i32, Vec<u8>)>,
    pos: &Arc<Mutex<Option<(f64, f64, f64)>>>,
) -> std::io::Result<()> {
    loop {
        let (id, data) = proto::read_packet(rd, threshold).await?;
        if id == spec.cb_play_keepalive {
            let _ = tx.send((spec.sb_play_keepalive, data)).await;
        } else if id == spec.cb_play_sync_pos {
            let mut r = Reader::new(&data);
            let (x, y, z, teleport_id) = if spec.pos_v2 {
                // 1.21.2+: teleport_id, x,y,z, vx,vy,vz, yaw,pitch, flags(i32)
                let tid = r.varint()?;
                let x = r.f64()?;
                let y = r.f64()?;
                let z = r.f64()?;
                (x, y, z, tid)
            } else {
                // <=1.21.1: x,y,z, yaw,pitch, flags(u8), teleport_id
                let x = r.f64()?;
                let y = r.f64()?;
                let z = r.f64()?;
                r.take(4)?; // yaw
                r.take(4)?; // pitch
                r.take(1)?; // flags
                let tid = r.varint()?;
                (x, y, z, tid)
            };

            let mut tb = Vec::new();
            proto::w_varint(&mut tb, teleport_id);
            let _ = tx.send((spec.sb_play_confirm_teleport, tb)).await;

            *pos.lock().await = Some((x, y, z));
            let _ = tx.send((spec.sb_play_set_pos, set_position_body(x, y, z, spec.pos_v2))).await;
        } else if id == spec.cb_play_disconnect {
            return Ok(()); // fine normale
        }
    }
}

async fn movement(
    cfg: BotConfig,
    mgr: Arc<Manager>,
    tx: mpsc::Sender<(i32, Vec<u8>)>,
    pos: Arc<Mutex<Option<(f64, f64, f64)>>>,
) {
    let spec = cfg.spec;
    loop {
        if pos.lock().await.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    if !(cfg.spread_secs > 0.0 || cfg.active) {
        return; // lite senza dispersione: resta fermo
    }

    let _moving = Guard::new(mgr.clone(), Counter::Moving);
    let yaw: f64 = rand::thread_rng().gen_range(0.0..std::f64::consts::TAU);
    let vx = -yaw.sin() * WALK_SPEED / 10.0;
    let vz = yaw.cos() * WALK_SPEED / 10.0;
    let jitter = rand::thread_rng().gen_range(0.6..1.4);
    let deadline = Instant::now() + Duration::from_secs_f64(cfg.spread_secs.max(0.5) * jitter);

    let mut tick = interval(Duration::from_millis(100));
    loop {
        tick.tick().await;
        if Instant::now() >= deadline {
            break;
        }
        let mut p = pos.lock().await;
        if let Some((x, y, z)) = p.as_mut() {
            *x += vx;
            *z += vz;
            if tx.send((spec.sb_play_set_pos, set_position_body(*x, *y, *z, spec.pos_v2))).await.is_err() {
                return;
            }
        }
    }
    drop(_moving);

    if cfg.active {
        let _moving = Guard::new(mgr.clone(), Counter::Moving);
        let mut tick = interval(Duration::from_secs(1));
        loop {
            tick.tick().await;
            let a: f64 = rand::thread_rng().gen_range(0.0..std::f64::consts::TAU);
            let mut p = pos.lock().await;
            if let Some((x, y, z)) = p.as_mut() {
                *x += a.cos() * 0.6;
                *z += a.sin() * 0.6;
                if tx.send((spec.sb_play_set_pos, set_position_body(*x, *y, *z, spec.pos_v2))).await.is_err() {
                    return;
                }
            }
        }
    }
}

fn set_position_body(x: f64, y: f64, z: f64, pos_v2: bool) -> Vec<u8> {
    let mut b = Vec::with_capacity(25);
    proto::w_f64(&mut b, x);
    proto::w_f64(&mut b, y);
    proto::w_f64(&mut b, z);
    if pos_v2 {
        b.push(0x01); // flags: bit0 = on_ground
    } else {
        proto::w_bool(&mut b, true); // on_ground
    }
    b
}

async fn send_client_information(
    wr: &mut (impl tokio::io::AsyncWrite + Unpin),
    threshold: i32,
    cfg: &BotConfig,
) -> std::io::Result<()> {
    let mut b = Vec::new();
    proto::w_string(&mut b, "en_us");
    b.push(cfg.view_distance as u8); // view distance -> carica i chunk lato server
    proto::w_varint(&mut b, 0); // chat mode: enabled
    proto::w_bool(&mut b, true); // chat colors
    b.push(0x7f); // skin parts
    proto::w_varint(&mut b, 1); // main hand: right
    proto::w_bool(&mut b, false); // text filtering
    proto::w_bool(&mut b, true); // allow server listings
    if cfg.spec.client_info_particle {
        proto::w_varint(&mut b, 0); // particle status: all (1.21.2+)
    }
    proto::write_packet(wr, threshold, cfg.spec.sb_cfg_client_info, &b).await
}

fn err(msg: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Other, msg.to_string())
}
