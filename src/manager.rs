//! Gestione del test: ramp a scaglioni, conteggi, breaking point, broadcast
//! verso la dashboard via canale.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use tokio::sync::{broadcast, Mutex};
use tokio::task::JoinSet;

use crate::bot::{run_bot, BotConfig};
use crate::ids;

/// Richiesta di avvio dalla dashboard (POST /api/start).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartReq {
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub protocol: Option<i32>,
    #[serde(default = "default_prefix")]
    pub prefix: String,
    #[serde(default = "default_target")]
    pub target: usize,
    #[serde(default = "default_step")]
    pub step: usize,
    #[serde(default = "default_interval")]
    pub interval_ms: u64,
    #[serde(default = "default_view")]
    pub view_distance: i8,
    #[serde(default = "default_spread")]
    pub spread_seconds: f64,
    #[serde(default = "default_mode")]
    pub mode: String,
}

fn default_port() -> u16 { 25565 }
fn default_prefix() -> String { "Bot".to_string() }
fn default_target() -> usize { 20 }
fn default_step() -> usize { 5 }
fn default_interval() -> u64 { 3000 }
fn default_view() -> i8 { 8 }
fn default_spread() -> f64 { 6.0 }
fn default_mode() -> String { "lite".to_string() }

struct BreakState {
    base_latency: Option<u64>,
    breaking_point: Option<usize>,
    prev_err: usize,
}

pub struct Manager {
    pub running: AtomicBool,
    pub alive: AtomicUsize,
    pub moving: AtomicUsize,
    pub errors: AtomicUsize,
    spawned: AtomicUsize,
    target: AtomicUsize,
    last_cfg: Mutex<Option<(String, u16, i32)>>, // host, port, protocol (per la SLP)
    brk: Mutex<BreakState>,
    tasks: Mutex<JoinSet<()>>,
    tx: broadcast::Sender<String>,
}

impl Manager {
    pub fn new() -> Arc<Self> {
        let (tx, _) = broadcast::channel(256);
        Arc::new(Manager {
            running: AtomicBool::new(false),
            alive: AtomicUsize::new(0),
            moving: AtomicUsize::new(0),
            errors: AtomicUsize::new(0),
            spawned: AtomicUsize::new(0),
            target: AtomicUsize::new(0),
            last_cfg: Mutex::new(None),
            brk: Mutex::new(BreakState { base_latency: None, breaking_point: None, prev_err: 0 }),
            tasks: Mutex::new(JoinSet::new()),
            tx,
        })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
    }

    pub fn broadcast(&self, msg: String) {
        let _ = self.tx.send(msg);
    }

    pub fn log(&self, line: String) {
        let ts = now_hms();
        let _ = self.tx.send(
            serde_json::json!({ "type": "log", "line": format!("[{ts}] {line}") }).to_string(),
        );
    }

    pub async fn start(self: &Arc<Self>, req: StartReq) -> Result<(), String> {
        if self.running.swap(true, Ordering::SeqCst) {
            return Err("Test gia' in corso. Premi Stop prima di riavviare.".into());
        }
        let protocol = req.protocol.unwrap_or(ids::PROTOCOL_VERSION);
        let view_distance = req.view_distance.clamp(2, 16);
        let target = req.target.max(1);
        let step = req.step.max(1);
        let interval_ms = req.interval_ms.max(250);
        let prefix: String = req.prefix.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_').collect();

        let cfg = BotConfig {
            host: req.host.clone(),
            port: req.port,
            protocol,
            view_distance,
            spread_secs: req.spread_seconds.max(0.0),
            active: req.mode == "active",
        };

        self.alive.store(0, Ordering::Relaxed);
        self.moving.store(0, Ordering::Relaxed);
        self.errors.store(0, Ordering::Relaxed);
        self.spawned.store(0, Ordering::Relaxed);
        self.target.store(target, Ordering::Relaxed);
        *self.last_cfg.lock().await = Some((req.host.clone(), req.port, protocol));
        *self.brk.lock().await = BreakState { base_latency: None, breaking_point: None, prev_err: 0 };

        self.log(format!(
            "Avvio: {target} bot su {}:{} · protocollo {} ({}) · scaglioni {step}/{interval_ms}ms · view {view_distance} chunk · dispersione {}s · {}",
            cfg.host, cfg.port, protocol, ids::VERSION_NAME, cfg.spread_secs, if cfg.active { "active" } else { "lite" }
        ));

        let mgr = self.clone();
        let prefix = if prefix.is_empty() { "Bot".to_string() } else { prefix };
        tokio::spawn(async move {
            while mgr.running.load(Ordering::SeqCst) {
                let spawned = mgr.spawned.load(Ordering::Relaxed);
                if spawned >= target {
                    mgr.log(format!("Ramp completato: {target} bot avviati."));
                    break;
                }
                let n = step.min(target - spawned);
                let mut tasks = mgr.tasks.lock().await;
                for _ in 0..n {
                    let id = mgr.spawned.fetch_add(1, Ordering::Relaxed) + 1;
                    let username = format!("{prefix}_{id}");
                    let c = cfg.clone();
                    let m = mgr.clone();
                    tasks.spawn(run_bot(c, username, m));
                }
                drop(tasks);
                tokio::time::sleep(Duration::from_millis(interval_ms)).await;
            }
        });
        Ok(())
    }

    pub async fn stop(&self) {
        if !self.running.swap(false, Ordering::SeqCst) {
            return;
        }
        let mut tasks = self.tasks.lock().await;
        tasks.abort_all();
        while tasks.try_join_next().is_some() {}
        drop(tasks);
        let n = self.alive.swap(0, Ordering::Relaxed);
        self.moving.store(0, Ordering::Relaxed);
        self.spawned.store(0, Ordering::Relaxed);
        self.target.store(0, Ordering::Relaxed);
        self.log(format!("Stop: disconnessi ~{n} bot."));
    }

    /// Rileva il breaking point dai proxy misurabili: latenza del server che
    /// raddoppia la baseline, o inizio dei kick.
    pub async fn observe(&self, latency: Option<u64>) {
        if !self.running.load(Ordering::SeqCst) {
            return;
        }
        let alive = self.alive.load(Ordering::Relaxed);
        let errors = self.errors.load(Ordering::Relaxed);
        let mut brk = self.brk.lock().await;
        if brk.breaking_point.is_none() && alive > 5 {
            if let Some(lat) = latency {
                match brk.base_latency {
                    None => brk.base_latency = Some(lat),
                    Some(base) if lat > base * 2 + 60 => {
                        brk.breaking_point = Some(alive);
                        self.log(format!("Breaking point ~{alive} bot: latenza server {lat}ms (baseline {base}ms)."));
                    }
                    _ => {}
                }
            }
            if brk.breaking_point.is_none() && errors > brk.prev_err && brk.prev_err == 0 {
                brk.breaking_point = Some(alive);
                self.log(format!("Breaking point ~{alive} bot: il server inizia a espellere."));
            }
        }
        brk.prev_err = errors;
    }

    pub async fn stats_json(&self, latency: Option<u64>, online: Option<i64>, max: Option<i64>) -> String {
        let brk = self.brk.lock().await;
        serde_json::json!({
            "type": "stats",
            "stats": {
                "alive": self.alive.load(Ordering::Relaxed),
                "target": self.target.load(Ordering::Relaxed),
                "moving": self.moving.load(Ordering::Relaxed),
                "errors": self.errors.load(Ordering::Relaxed),
                "breakingPoint": brk.breaking_point,
                "running": self.running.load(Ordering::SeqCst),
                "server": latency.map(|l| serde_json::json!({ "latency": l, "online": online, "max": max })),
            }
        })
        .to_string()
    }

    pub async fn last_cfg(&self) -> Option<(String, u16, i32)> {
        self.last_cfg.lock().await.clone()
    }
}

fn now_hms() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let h = (secs / 3600) % 24;
    let m = (secs / 60) % 60;
    let s = secs % 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}
