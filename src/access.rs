use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Redirect, Response};
use rand::distributions::Alphanumeric;
use rand::Rng;

const COOKIE: &str = "mcs_key";
const KEY_LENGTH: usize = 24;

#[derive(Clone)]
pub struct AccessKey(Arc<str>);

impl AccessKey {
    pub fn load() -> AccessKey {
        if let Ok(value) = std::env::var("MC_STRESS_KEY") {
            let trimmed = value.trim();
            if trimmed.len() >= 8 && trimmed.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
                return AccessKey(Arc::from(trimmed));
            }
            println!("\n  MC_STRESS_KEY ignored: use at least 8 letters or digits. A random key was generated.");
        }
        let generated: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(KEY_LENGTH)
            .map(char::from)
            .collect();
        AccessKey(Arc::from(generated))
    }

    pub fn value(&self) -> &str {
        &self.0
    }

    fn matches(&self, candidate: &str) -> bool {
        let expected = self.0.as_bytes();
        let given = candidate.as_bytes();
        if expected.len() != given.len() {
            return false;
        }
        let mut difference = 0u8;
        for index in 0..expected.len() {
            difference |= expected[index] ^ given[index];
        }
        difference == 0
    }
}

pub async fn guard(State(key): State<AccessKey>, request: Request, next: Next) -> Response {
    if let Some(candidate) = query_key(request.uri().query()) {
        if key.matches(candidate) {
            let mut response = Redirect::to(request.uri().path()).into_response();
            let cookie = format!("{COOKIE}={}; Path=/; Max-Age=31536000; HttpOnly; SameSite=Strict", key.value());
            if let Ok(value) = HeaderValue::from_str(&cookie) {
                response.headers_mut().insert(header::SET_COOKIE, value);
            }
            return response;
        }
        return denied();
    }
    if let Some(candidate) = cookie_key(request.headers()) {
        if key.matches(candidate) {
            return next.run(request).await;
        }
    }
    denied()
}

fn query_key(query: Option<&str>) -> Option<&str> {
    for pair in query?.split('&') {
        if let Some(value) = pair.strip_prefix("key=") {
            return Some(value);
        }
    }
    None
}

fn cookie_key(headers: &HeaderMap) -> Option<&str> {
    for header_value in headers.get_all(header::COOKIE) {
        let Ok(text) = header_value.to_str() else { continue };
        for part in text.split(';') {
            if let Some(value) = part.trim().strip_prefix("mcs_key=") {
                return Some(value);
            }
        }
    }
    None
}

fn denied() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        DENIED_PAGE,
    )
        .into_response()
}

const DENIED_PAGE: &str = r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>MC-Stress · Access denied</title><style>body{margin:0;min-height:100vh;display:grid;place-items:center;background:#0c0a0b;color:#d7cccd;font:15px/1.6 ui-sans-serif,system-ui,-apple-system,'Segoe UI',Roboto,sans-serif;padding:20px}main{max-width:420px;text-align:center}h1{margin:0 0 8px;font-size:22px;color:#fafafa}b{color:#f0434a}</style></head><body><main><h1>Access denied</h1><p>Open MC-Stress with the <b>private link</b> printed in the terminal where it is running.</p></main></body></html>"#;
