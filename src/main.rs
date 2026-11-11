mod access;
mod bot;
mod browser;
mod model;
mod protocol;
mod slp;
mod swarm;
mod versions;

use std::net::SocketAddr;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::middleware;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;

use swarm::{BotSwarm, StartRequest};

#[derive(Clone)]
struct AppState {
    swarm: Arc<BotSwarm>,
}

#[tokio::main]
async fn main() {
    let swarm = BotSwarm::new();
    spawn_metrics_loop(swarm.clone());

    let key = access::AccessKey::load();
    let state = AppState { swarm };
    let app = Router::new()
        .route("/", get(index))
        .route("/style.css", get(style))
        .route("/app.js", get(app_js))
        .route("/api/start", post(start))
        .route("/api/stop", post(stop))
        .route("/ws", get(websocket))
        .with_state(state)
        .layer(middleware::from_fn_with_state(key.clone(), access::guard));

    let port: u16 = std::env::var("PORT").ok().and_then(|value| value.parse().ok()).unwrap_or(3000);
    let address = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(address).await.expect("bind");
    browser::announce(port, key.value());
    axum::serve(listener, app).await.expect("serve");
}

fn spawn_metrics_loop(swarm: Arc<BotSwarm>) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(1));
        loop {
            tick.tick().await;
            let mut latency = None;
            let mut online = None;
            let mut max = None;
            if let Some((host, port, protocol)) = swarm.endpoint().await {
                if swarm.running.load(Ordering::SeqCst) {
                    if let Ok(status) = slp::ping(&host, port, protocol).await {
                        latency = Some(status.latency_ms);
                        online = status.online;
                        max = status.max;
                    }
                }
            }
            swarm.observe(latency).await;
            let payload = swarm.stats_json(latency, online, max).await;
            swarm.broadcast(payload);
        }
    });
}

async fn index() -> Response {
    serve_html(include_str!("../public/index.html"))
}

async fn style() -> Response {
    ([(header::CONTENT_TYPE, "text/css")], include_str!("../public/style.css")).into_response()
}

async fn app_js() -> Response {
    ([(header::CONTENT_TYPE, "application/javascript")], include_str!("../public/app.js")).into_response()
}

fn serve_html(body: &'static str) -> Response {
    ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], body).into_response()
}

async fn start(State(state): State<AppState>, Json(request): Json<StartRequest>) -> Response {
    match state.swarm.start(request).await {
        Ok(()) => Json(serde_json::json!({ "ok": true })).into_response(),
        Err(message) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "ok": false, "error": message })),
        )
            .into_response(),
    }
}

async fn stop(State(state): State<AppState>) -> Response {
    state.swarm.stop().await;
    Json(serde_json::json!({ "ok": true })).into_response()
}

async fn websocket(upgrade: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    upgrade.on_upgrade(move |socket| forward_updates(socket, state.swarm.clone()))
}

async fn forward_updates(socket: WebSocket, swarm: Arc<BotSwarm>) {
    let (mut sender, mut receiver) = socket.split();
    let mut updates = swarm.subscribe();

    let send_task = tokio::spawn(async move {
        while let Ok(message) = updates.recv().await {
            if sender.send(Message::Text(message)).await.is_err() {
                break;
            }
        }
    });

    while let Some(Ok(_)) = receiver.next().await {}
    send_task.abort();
}
