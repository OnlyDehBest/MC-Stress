//! MC-Stress: load/stress test per server Minecraft.
//! Client raw su tokio (offline-mode) + dashboard web axum con breaking point.

mod bot;
mod ids;
mod manager;
mod protocol;
mod slp;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::SinkExt;
use tokio::net::TcpListener;

use manager::{Manager, StartReq};

#[derive(Clone)]
struct AppState {
    mgr: Arc<Manager>,
}

#[tokio::main]
async fn main() {
    let mgr = Manager::new();

    // Loop metriche: ogni secondo SLP (latenza/online), breaking point, broadcast.
    {
        let mgr = mgr.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(1));
            loop {
                tick.tick().await;
                let mut latency = None;
                let mut online = None;
                let mut max = None;
                if let Some((host, port, protocol)) = mgr.last_cfg().await {
                    if mgr.running.load(std::sync::atomic::Ordering::SeqCst) {
                        if let Ok(st) = slp::ping(&host, port, protocol).await {
                            latency = Some(st.latency_ms);
                            online = st.online;
                            max = st.max;
                        }
                    }
                }
                mgr.observe(latency).await;
                let json = mgr.stats_json(latency, online, max).await;
                mgr.broadcast(json);
            }
        });
    }

    let state = AppState { mgr };
    let app = Router::new()
        .route("/", get(index))
        .route("/style.css", get(style))
        .route("/app.js", get(appjs))
        .route("/api/start", post(api_start))
        .route("/api/stop", post(api_stop))
        .route("/ws", get(ws_handler))
        .with_state(state);

    let port: u16 = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(3000);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await.expect("bind");
    println!("\n  MC-Stress dashboard attiva su http://localhost:{port}\n");
    axum::serve(listener, app).await.expect("serve");
}

// --- Handlers statici ---

async fn index() -> Response {
    html(include_str!("../public/index.html"))
}
async fn style() -> Response {
    ([(header::CONTENT_TYPE, "text/css")], include_str!("../public/style.css")).into_response()
}
async fn appjs() -> Response {
    ([(header::CONTENT_TYPE, "application/javascript")], include_str!("../public/app.js")).into_response()
}
fn html(body: &'static str) -> Response {
    ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], body).into_response()
}

// --- API ---

async fn api_start(State(st): State<AppState>, Json(req): Json<StartReq>) -> Response {
    match st.mgr.start(req).await {
        Ok(()) => Json(serde_json::json!({ "ok": true })).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "ok": false, "error": e }))).into_response(),
    }
}

async fn api_stop(State(st): State<AppState>) -> Response {
    st.mgr.stop().await;
    Json(serde_json::json!({ "ok": true })).into_response()
}

// --- WebSocket ---

async fn ws_handler(ws: WebSocketUpgrade, State(st): State<AppState>) -> Response {
    ws.on_upgrade(move |socket| ws_loop(socket, st.mgr.clone()))
}

async fn ws_loop(socket: WebSocket, mgr: Arc<Manager>) {
    use futures_util::StreamExt;
    let (mut sender, mut receiver) = socket.split();
    let mut rx = mgr.subscribe();

    // Task di invio: inoltra ogni messaggio broadcast al client.
    let send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if sender.send(Message::Text(msg)).await.is_err() {
                break;
            }
        }
    });

    // Scarta i messaggi in ingresso finche' il socket vive.
    while let Some(Ok(_)) = receiver.next().await {}
    send_task.abort();
}
