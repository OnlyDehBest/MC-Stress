use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use tokio::sync::{broadcast, Mutex};
use tokio::task::JoinSet;

use crate::bot::{run_bot, BotConfig};
use crate::{slp, versions};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartRequest {
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

struct BreakingPointState {
    baseline_latency: Option<u64>,
    at_bots: Option<usize>,
    previous_errors: usize,
}

pub struct BotSwarm {
    pub running: AtomicBool,
    pub connected: AtomicUsize,
    pub moving: AtomicUsize,
    pub errors: AtomicUsize,
    dispatched: AtomicUsize,
    target: AtomicUsize,
    endpoint: Mutex<Option<(String, u16, i32)>>,
    detected_version: Mutex<Option<String>>,
    breaking_point: Mutex<BreakingPointState>,
    bots: Mutex<JoinSet<()>>,
    broadcaster: broadcast::Sender<String>,
}

impl BotSwarm {
    pub fn new() -> Arc<Self> {
        let (broadcaster, _) = broadcast::channel(256);
        Arc::new(BotSwarm {
            running: AtomicBool::new(false),
            connected: AtomicUsize::new(0),
            moving: AtomicUsize::new(0),
            errors: AtomicUsize::new(0),
            dispatched: AtomicUsize::new(0),
            target: AtomicUsize::new(0),
            endpoint: Mutex::new(None),
            detected_version: Mutex::new(None),
            breaking_point: Mutex::new(BreakingPointState {
                baseline_latency: None,
                at_bots: None,
                previous_errors: 0,
            }),
            bots: Mutex::new(JoinSet::new()),
            broadcaster,
        })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.broadcaster.subscribe()
    }

    pub fn broadcast(&self, message: String) {
        let _ = self.broadcaster.send(message);
    }

    pub fn log(&self, line: String) {
        let stamp = clock();
        let _ = self.broadcaster.send(
            serde_json::json!({ "type": "log", "line": format!("[{stamp}] {line}") }).to_string(),
        );
    }

    pub async fn start(self: &Arc<Self>, request: StartRequest) -> Result<(), String> {
        if self.running.swap(true, Ordering::SeqCst) {
            return Err("Test gia' in corso. Premi Stop prima di riavviare.".into());
        }

        let view_distance = request.view_distance.clamp(2, 16);
        let target = request.target.max(1);
        let step = request.step.max(1);
        let interval_ms = request.interval_ms.max(250);
        let prefix: String = request
            .prefix
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        let prefix = if prefix.is_empty() { "Bot".to_string() } else { prefix };

        let spec = match self.resolve_spec(&request).await {
            Ok(spec) => spec,
            Err(message) => {
                self.running.store(false, Ordering::SeqCst);
                return Err(message);
            }
        };

        let config = BotConfig {
            host: request.host.clone(),
            port: request.port,
            spec,
            view_distance,
            spread_seconds: request.spread_seconds.max(0.0),
            active: request.mode == "active",
        };

        self.connected.store(0, Ordering::Relaxed);
        self.moving.store(0, Ordering::Relaxed);
        self.errors.store(0, Ordering::Relaxed);
        self.dispatched.store(0, Ordering::Relaxed);
        self.target.store(target, Ordering::Relaxed);
        *self.endpoint.lock().await = Some((request.host.clone(), request.port, spec.protocol));
        *self.detected_version.lock().await = Some(format!("{} ({})", spec.name, spec.protocol));
        *self.breaking_point.lock().await = BreakingPointState {
            baseline_latency: None,
            at_bots: None,
            previous_errors: 0,
        };

        self.log(format!(
            "Avvio: {target} bot su {}:{} · {} (proto {}) · scaglioni {step}/{interval_ms}ms · view {view_distance} chunk · dispersione {}s · {}",
            config.host,
            config.port,
            spec.name,
            spec.protocol,
            config.spread_seconds,
            if config.active { "active" } else { "lite" }
        ));

        let swarm = self.clone();
        tokio::spawn(async move {
            while swarm.running.load(Ordering::SeqCst) {
                let dispatched = swarm.dispatched.load(Ordering::Relaxed);
                if dispatched >= target {
                    swarm.log(format!("Ramp completato: {target} bot avviati."));
                    break;
                }
                let batch = step.min(target - dispatched);
                let mut bots = swarm.bots.lock().await;
                for _ in 0..batch {
                    let sequence = swarm.dispatched.fetch_add(1, Ordering::Relaxed) + 1;
                    let username = format!("{prefix}_{sequence}");
                    bots.spawn(run_bot(config.clone(), username, swarm.clone()));
                }
                drop(bots);
                tokio::time::sleep(Duration::from_millis(interval_ms)).await;
            }
        });
        Ok(())
    }

    async fn resolve_spec(&self, request: &StartRequest) -> Result<&'static versions::ProtocolSpec, String> {
        if let Some(protocol) = request.protocol {
            return versions::spec_for(protocol)
                .ok_or_else(|| format!("Protocollo {protocol} non supportato. Supportati: {}", supported_list()));
        }
        match slp::ping(&request.host, request.port, versions::DEFAULT_PROTOCOL).await {
            Ok(status) => match status.protocol.and_then(versions::spec_for) {
                Some(spec) => {
                    self.log(format!(
                        "Versione rilevata: {} (protocollo {}).",
                        status.version_name.as_deref().unwrap_or("?"),
                        spec.protocol
                    ));
                    Ok(spec)
                }
                None => {
                    let spec = versions::spec_for(versions::DEFAULT_PROTOCOL).unwrap();
                    self.log(format!(
                        "Versione non riconosciuta ({:?} / proto {:?}): uso {} di default. Forza il protocollo o aggiorna src/versions.rs.",
                        status.version_name, status.protocol, spec.name
                    ));
                    Ok(spec)
                }
            },
            Err(reason) => {
                let spec = versions::spec_for(versions::DEFAULT_PROTOCOL).unwrap();
                self.log(format!("SLP non riuscita ({reason}): uso {} di default.", spec.name));
                Ok(spec)
            }
        }
    }

    pub async fn stop(&self) {
        if !self.running.swap(false, Ordering::SeqCst) {
            return;
        }
        let mut bots = self.bots.lock().await;
        bots.abort_all();
        while bots.try_join_next().is_some() {}
        drop(bots);
        let released = self.connected.swap(0, Ordering::Relaxed);
        self.moving.store(0, Ordering::Relaxed);
        self.dispatched.store(0, Ordering::Relaxed);
        self.target.store(0, Ordering::Relaxed);
        self.log(format!("Stop: disconnessi ~{released} bot."));
    }

    pub async fn observe(&self, latency: Option<u64>) {
        if !self.running.load(Ordering::SeqCst) {
            return;
        }
        let connected = self.connected.load(Ordering::Relaxed);
        let errors = self.errors.load(Ordering::Relaxed);
        let mut state = self.breaking_point.lock().await;
        if state.at_bots.is_none() && connected > 5 {
            if let Some(latency) = latency {
                match state.baseline_latency {
                    None => state.baseline_latency = Some(latency),
                    Some(baseline) if latency > baseline * 2 + 60 => {
                        state.at_bots = Some(connected);
                        self.log(format!("Breaking point ~{connected} bot: latenza server {latency}ms (baseline {baseline}ms)."));
                    }
                    _ => {}
                }
            }
            if state.at_bots.is_none() && errors > state.previous_errors && state.previous_errors == 0 {
                state.at_bots = Some(connected);
                self.log(format!("Breaking point ~{connected} bot: il server inizia a espellere."));
            }
        }
        state.previous_errors = errors;
    }

    pub async fn stats_json(&self, latency: Option<u64>, online: Option<i64>, max: Option<i64>) -> String {
        let breaking_point = self.breaking_point.lock().await.at_bots;
        let version = self.detected_version.lock().await.clone();
        serde_json::json!({
            "type": "stats",
            "stats": {
                "alive": self.connected.load(Ordering::Relaxed),
                "target": self.target.load(Ordering::Relaxed),
                "moving": self.moving.load(Ordering::Relaxed),
                "errors": self.errors.load(Ordering::Relaxed),
                "breakingPoint": breaking_point,
                "running": self.running.load(Ordering::SeqCst),
                "version": version,
                "server": latency.map(|value| serde_json::json!({ "latency": value, "online": online, "max": max })),
            }
        })
        .to_string()
    }

    pub async fn endpoint(&self) -> Option<(String, u16, i32)> {
        self.endpoint.lock().await.clone()
    }
}

fn supported_list() -> String {
    let mut parts = Vec::new();
    for (protocol, name) in versions::supported() {
        parts.push(format!("{name} ({protocol})"));
    }
    parts.join(", ")
}

fn clock() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seconds = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let hours = (seconds / 3600) % 24;
    let minutes = (seconds / 60) % 60;
    let secs = seconds % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, secs)
}
